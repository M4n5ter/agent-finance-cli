use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use wreq::{
    Client,
    header::{ACCEPT, USER_AGENT},
};

use crate::model::{ResearchHighlight, research_value_string};

const SEC_TICKERS_URL: &str = "https://www.sec.gov/files/company_tickers.json";
const SEC_SUBMISSIONS_BASE_URL: &str = "https://data.sec.gov/submissions";
const SEC_COMPANYFACTS_BASE_URL: &str = "https://data.sec.gov/api/xbrl/companyfacts";
const SEC_USER_AGENT_ENV: &str = "AGENT_FINANCE_SEC_USER_AGENT";
const DEFAULT_SEC_USER_AGENT: &str = concat!(
    "agent-finance/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/M4n5ter/agent-finance)"
);

#[derive(Debug, Clone, PartialEq, Eq)]
struct SecCompany {
    cik: u64,
    ticker: String,
    title: String,
}

pub async fn fetch_company_bundle(
    client: &Client,
    symbol: &str,
    include_companyfacts: bool,
) -> Result<Value> {
    let normalized = symbol.trim().to_uppercase();
    let tickers = fetch_json(client, SEC_TICKERS_URL, "SEC company tickers").await?;
    let company = find_company(&tickers, &normalized)?;
    let cik = format!("{:010}", company.cik);
    let submissions_url = format!("{SEC_SUBMISSIONS_BASE_URL}/CIK{cik}.json");
    let companyfacts_url =
        include_companyfacts.then(|| format!("{SEC_COMPANYFACTS_BASE_URL}/CIK{cik}.json"));
    let (submissions, companyfacts) = match companyfacts_url.as_deref() {
        Some(companyfacts_url) => {
            let (submissions, companyfacts) = tokio::try_join!(
                fetch_json(client, &submissions_url, "SEC submissions"),
                fetch_json(client, companyfacts_url, "SEC companyfacts")
            )?;
            (submissions, Some(companyfacts))
        }
        None => (
            fetch_json(client, &submissions_url, "SEC submissions").await?,
            None,
        ),
    };
    let mut payload = json!({
        "symbol": normalized,
        "cik": cik,
        "company": {
            "ticker": company.ticker,
            "title": company.title,
        },
        "submissions": submissions,
    });

    if let Some(companyfacts) = companyfacts {
        payload["companyfacts"] = companyfacts;
    }

    Ok(payload)
}

async fn fetch_json(client: &Client, url: &str, label: &str) -> Result<Value> {
    let response = client
        .get(url)
        .header(ACCEPT, "application/json")
        .header(USER_AGENT, sec_user_agent())
        .send()
        .await
        .with_context(|| format!("{label} request failed"))?;
    let status = response.status();
    if status.is_success() {
        return response
            .json::<Value>()
            .await
            .with_context(|| format!("{label} JSON parse failed"));
    }
    let body = response
        .text()
        .await
        .with_context(|| format!("{label} response text parse failed"))?;
    Err(anyhow!("{label} returned HTTP {status}: {body}"))
}

fn sec_user_agent() -> String {
    std::env::var(SEC_USER_AGENT_ENV).unwrap_or_else(|_| DEFAULT_SEC_USER_AGENT.to_string())
}

fn find_company(tickers: &Value, symbol: &str) -> Result<SecCompany> {
    let companies = tickers
        .as_object()
        .ok_or_else(|| anyhow!("SEC company tickers payload is not an object"))?;
    companies
        .values()
        .filter_map(parse_company)
        .find(|company| company.ticker.eq_ignore_ascii_case(symbol))
        .ok_or_else(|| anyhow!("SEC company tickers did not contain {symbol}"))
}

fn parse_company(value: &Value) -> Option<SecCompany> {
    let cik = value
        .get("cik_str")
        .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))?;
    let ticker = value.get("ticker")?.as_str()?.to_uppercase();
    let title = value.get("title")?.as_str()?.to_string();
    Some(SecCompany { cik, ticker, title })
}

pub fn fundamentals_highlights(payload: &Value) -> Vec<ResearchHighlight> {
    let mut rows = Vec::new();
    push_path(
        &mut rows,
        payload,
        "Company",
        "/company/title",
        "submissions",
    );
    push_path(
        &mut rows,
        payload,
        "SEC Entity",
        "/companyfacts/entityName",
        "companyfacts",
    );
    push_path(&mut rows, payload, "CIK", "/cik", "submissions");
    push_path(
        &mut rows,
        payload,
        "SIC",
        "/submissions/sicDescription",
        "submissions",
    );
    push_path(
        &mut rows,
        payload,
        "Fiscal year end",
        "/submissions/fiscalYearEnd",
        "submissions",
    );

    for (label, tags) in [
        (
            "Latest revenue",
            &[
                "RevenueFromContractWithCustomerExcludingAssessedTax",
                "Revenues",
                "SalesRevenueNet",
            ][..],
        ),
        ("Latest net income", &["NetIncomeLoss"][..]),
        ("Latest assets", &["Assets"][..]),
        ("Latest liabilities", &["Liabilities"][..]),
        (
            "Latest operating cash flow",
            &["NetCashProvidedByUsedInOperatingActivities"][..],
        ),
        (
            "Latest cash and equivalents",
            &["CashAndCashEquivalentsAtCarryingValue"][..],
        ),
        ("Latest R&D expense", &["ResearchAndDevelopmentExpense"][..]),
    ] {
        if let Some(point) = latest_usd_fact(payload, tags) {
            rows.push(ResearchHighlight::new(
                label,
                point.display_value(),
                "sec-edgar",
                "companyfacts",
            ));
        }
    }
    rows
}

pub fn events_highlights(payload: &Value) -> Vec<ResearchHighlight> {
    let mut rows = Vec::new();
    push_path(
        &mut rows,
        payload,
        "Company",
        "/company/title",
        "submissions",
    );
    push_path(&mut rows, payload, "CIK", "/cik", "submissions");

    let recent = payload.pointer("/submissions/filings/recent");
    let forms = recent
        .and_then(|value| value.pointer("/form"))
        .and_then(Value::as_array);
    let Some(forms) = forms else {
        return rows;
    };
    for (index, form) in forms.iter().take(8).enumerate() {
        let form = research_value_string(Some(form)).unwrap_or_else(|| "-".to_string());
        let filed =
            recent_array_string(recent, "filingDate", index).unwrap_or_else(|| "-".to_string());
        let accession = recent_array_string(recent, "accessionNumber", index)
            .unwrap_or_else(|| "-".to_string());
        let document = recent_array_string(recent, "primaryDocument", index)
            .unwrap_or_else(|| "-".to_string());
        rows.push(ResearchHighlight::new(
            &format!("SEC filing {}", index + 1),
            format!("{form} filed={filed} accession={accession} doc={document}"),
            "sec-edgar",
            "submissions",
        ));
    }
    rows
}

#[derive(Debug, Clone, PartialEq)]
struct SecFactPoint {
    tag: String,
    value: f64,
    form: Option<String>,
    end: Option<String>,
    filed: Option<String>,
}

impl SecFactPoint {
    fn display_value(&self) -> String {
        let form = self.form.as_deref().unwrap_or("-");
        let end = self.end.as_deref().unwrap_or("-");
        let filed = self.filed.as_deref().unwrap_or("-");
        format!(
            "{} | form={form} end={end} filed={filed} tag={}",
            compact_usd(self.value),
            self.tag
        )
    }
}

fn latest_usd_fact(payload: &Value, tags: &[&str]) -> Option<SecFactPoint> {
    let us_gaap = payload.pointer("/companyfacts/facts/us-gaap")?;
    tags.iter()
        .filter_map(|tag| us_gaap.get(*tag).map(|node| (*tag, node)))
        .flat_map(|(tag, node)| {
            node.pointer("/units/USD")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(move |entry| sec_fact_point(tag, entry))
        })
        .max_by(|left, right| {
            (
                left.filed.as_deref().unwrap_or(""),
                left.end.as_deref().unwrap_or(""),
            )
                .cmp(&(
                    right.filed.as_deref().unwrap_or(""),
                    right.end.as_deref().unwrap_or(""),
                ))
        })
}

fn sec_fact_point(tag: &str, entry: &Value) -> Option<SecFactPoint> {
    Some(SecFactPoint {
        tag: tag.to_string(),
        value: entry.get("val")?.as_f64()?,
        form: research_value_string(entry.get("form")),
        end: research_value_string(entry.get("end")),
        filed: research_value_string(entry.get("filed")),
    })
}

fn compact_usd(value: f64) -> String {
    let sign = if value < 0.0 { "-" } else { "" };
    let value = value.abs();
    let (scaled, suffix) = if value >= 1_000_000_000_000.0 {
        (value / 1_000_000_000_000.0, "T")
    } else if value >= 1_000_000_000.0 {
        (value / 1_000_000_000.0, "B")
    } else if value >= 1_000_000.0 {
        (value / 1_000_000.0, "M")
    } else if value >= 1_000.0 {
        (value / 1_000.0, "K")
    } else {
        (value, "")
    };
    format!("{sign}${scaled:.2}{suffix}")
}

fn push_path(
    rows: &mut Vec<ResearchHighlight>,
    root: &Value,
    label: &str,
    path: &str,
    module: &str,
) {
    if let Some(row) = ResearchHighlight::from_path(Some(root), label, path, "sec-edgar", module) {
        rows.push(row);
    }
}

fn recent_array_string(root: Option<&Value>, field: &str, index: usize) -> Option<String> {
    root.and_then(|value| value.get(field))
        .and_then(Value::as_array)
        .and_then(|values| values.get(index))
        .and_then(|value| research_value_string(Some(value)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finds_ticker_case_insensitively_and_formats_cik_later() {
        let tickers = json!({
            "0": {"cik_str": 320193, "ticker": "AAPL", "title": "Apple Inc."},
            "1": {"cik_str": "1730168", "ticker": "CRDO", "title": "Credo Technology Group Holding Ltd"}
        });

        let company = find_company(&tickers, "crdo").expect("CRDO should be found");

        assert_eq!(
            company,
            SecCompany {
                cik: 1_730_168,
                ticker: "CRDO".to_string(),
                title: "Credo Technology Group Holding Ltd".to_string(),
            }
        );
    }

    #[test]
    fn latest_usd_fact_prefers_latest_filing_across_accepted_tags() {
        let payload = json!({
            "companyfacts": {
                "facts": {
                    "us-gaap": {
                        "RevenueFromContractWithCustomerExcludingAssessedTax": {
                            "units": {
                                "USD": [
                                    {"val": 100.0, "form": "10-K", "end": "2025-12-31", "filed": "2026-02-01"}
                                ]
                            }
                        },
                        "Revenues": {
                            "units": {
                                "USD": [
                                    {"val": 130.0, "form": "10-Q", "end": "2026-03-31", "filed": "2026-05-05"}
                                ],
                                "EUR": [
                                    {"val": 999.0, "form": "10-Q", "end": "2026-03-31", "filed": "2026-05-06"}
                                ]
                            }
                        }
                    }
                }
            }
        });

        let point = latest_usd_fact(
            &payload,
            &[
                "RevenueFromContractWithCustomerExcludingAssessedTax",
                "Revenues",
            ],
        )
        .expect("latest USD revenue fact");

        assert_eq!(point.tag, "Revenues");
        assert_eq!(point.value, 130.0);
        assert_eq!(point.form.as_deref(), Some("10-Q"));
    }

    #[test]
    fn events_highlights_keep_filing_fields_aligned_by_index() {
        let payload = json!({
            "cik": "0001730168",
            "company": {"title": "Credo Technology Group Holding Ltd"},
            "submissions": {
                "filings": {
                    "recent": {
                        "form": ["8-K", "10-Q"],
                        "filingDate": ["2026-06-01", "2026-05-20"],
                        "accessionNumber": ["0001", "0002"],
                        "primaryDocument": ["current.htm", "quarterly.htm"]
                    }
                }
            }
        });

        let highlights = events_highlights(&payload);

        assert!(highlights.iter().any(|row| {
            row.label == "SEC filing 2"
                && row.value.contains("10-Q")
                && row.value.contains("filed=2026-05-20")
                && row.value.contains("accession=0002")
                && row.provider == "sec-edgar"
                && row.module == "submissions"
        }));
    }
}
