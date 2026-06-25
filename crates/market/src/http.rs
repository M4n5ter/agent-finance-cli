use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use chrono::{SecondsFormat, TimeZone, Utc};
use url::Url;
use wreq::{Client, Proxy, RequestBuilder, StatusCode};
use wreq_util::Emulation;

pub fn http_client(timeout_seconds: u64, proxy: Option<&str>, no_proxy: bool) -> Result<Client> {
    let mut builder = Client::builder()
        .emulation(Emulation::Chrome137)
        .timeout(Duration::from_secs(timeout_seconds))
        .cookie_store(true);

    if let Some(proxy) = selected_proxy(proxy, no_proxy) {
        builder = builder
            .proxy(Proxy::all(&proxy).with_context(|| format!("invalid proxy URL: {proxy}"))?);
    } else if no_proxy {
        builder = builder.no_proxy();
    }

    builder.build().context("failed to build HTTP client")
}

pub fn selected_proxy(proxy: Option<&str>, no_proxy: bool) -> Option<String> {
    if no_proxy {
        return None;
    }
    proxy
        .map(str::to_string)
        .or_else(|| std::env::var("AGENT_FINANCE_PROXY").ok())
        .or_else(|| std::env::var("ALL_PROXY").ok())
        .or_else(|| std::env::var("HTTPS_PROXY").ok())
        .or_else(|| std::env::var("HTTP_PROXY").ok())
}

pub async fn send_get_text(
    client: &Client,
    provider: &str,
    url: &Url,
    headers: &[(&'static str, String)],
) -> Result<(StatusCode, String)> {
    send_text_with_retries(provider, url.as_str(), || {
        let mut request = client.get(url.as_str());
        for (key, value) in headers {
            request = request.header(*key, value);
        }
        request
    })
    .await
}

pub async fn send_get_text_from_base_urls(
    client: &Client,
    provider: &str,
    source: &str,
    base_urls: &[&str],
    path: &str,
    params: &[(&'static str, String)],
    headers: &[(&'static str, String)],
) -> Result<(Url, StatusCode, String)> {
    let mut errors = Vec::new();
    for base_url in base_urls {
        let url = build_url(base_url, path, params)
            .with_context(|| format!("invalid {provider} API URL: {base_url}{path}"))?;
        match send_get_text(client, provider, &url, headers).await {
            Ok((status, body)) => return Ok((url, status, body)),
            Err(error) => errors.push(format!("{base_url}: {error:#}")),
        }
    }
    Err(anyhow!(
        "all {provider} {source} endpoints failed for {path}: {}",
        errors.join(" | ")
    ))
}

pub fn build_url(base_url: &str, path: &str, params: &[(&'static str, String)]) -> Result<Url> {
    let normalized_base_url = if base_url.ends_with('/') {
        base_url.to_string()
    } else {
        format!("{base_url}/")
    };
    let mut url = Url::parse(&normalized_base_url)
        .with_context(|| format!("invalid base URL: {base_url}"))?
        .join(path.trim_start_matches('/'))
        .with_context(|| format!("invalid API path: {path}"))?;
    {
        let mut query = url.query_pairs_mut();
        for (key, value) in params {
            query.append_pair(key, value);
        }
    }
    Ok(url)
}

pub async fn send_text_with_retries<F>(
    provider: &str,
    url: &str,
    build_request: F,
) -> Result<(StatusCode, String)>
where
    F: Fn() -> RequestBuilder,
{
    const ATTEMPTS: usize = 3;
    let mut last_error = None;

    for attempt in 1..=ATTEMPTS {
        match build_request().send().await {
            Ok(response) => {
                let status = response.status();
                match response.text().await {
                    Ok(body) => return Ok((status, body)),
                    Err(error) => {
                        last_error = Some(format!(
                            "{provider} response body read failed: {url}: {error:#}"
                        ));
                    }
                }
            }
            Err(error) => {
                let message = format!("{error:#}");
                if !is_transient_network_error(&message) || attempt == ATTEMPTS {
                    return Err(anyhow!("{provider} request failed: {url}: {message}"));
                }
                last_error = Some(format!("{provider} request failed: {url}: {message}"));
            }
        }

        if attempt < ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(250 * attempt as u64)).await;
        }
    }

    Err(anyhow!(
        "{} after {} attempts",
        last_error.unwrap_or_else(|| format!("{provider} request failed: {url}")),
        ATTEMPTS
    ))
}

fn is_transient_network_error(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    [
        "unexpected eof",
        "timed out",
        "timeout",
        "connection reset",
        "connection closed",
        "connect",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

pub fn utc_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn timestamp_ms_to_utc(timestamp: i64) -> Option<String> {
    Utc.timestamp_millis_opt(timestamp)
        .single()
        .map(|datetime| datetime.to_rfc3339_opts(SecondsFormat::Secs, true))
}

pub fn timestamp_sec_to_utc(timestamp: i64) -> Option<String> {
    Utc.timestamp_opt(timestamp, 0)
        .single()
        .map(|datetime| datetime.to_rfc3339_opts(SecondsFormat::Secs, true))
}

pub fn change_pct(price: f64, previous_close: Option<f64>) -> Option<f64> {
    let previous_close = previous_close?;
    if previous_close == 0.0 {
        None
    } else {
        Some((price - previous_close) / previous_close * 100.0)
    }
}

pub fn parse_optional_f64(value: Option<&str>) -> Option<f64> {
    let value = clean_text(value)?;
    value.parse::<f64>().ok()
}

pub fn parse_optional_u64(value: Option<&str>) -> Option<u64> {
    let value = clean_text(value)?;
    value.parse::<f64>().ok().map(|number| number as u64)
}

pub fn clean_text(value: Option<&str>) -> Option<&str> {
    match value.map(str::trim) {
        Some("") | Some("N/D") | None => None,
        Some(value) => Some(value),
    }
}
