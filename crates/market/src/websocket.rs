use anyhow::{Context, Result, anyhow};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::client_async_tls_with_config;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::handshake::client::{Request, Response};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use url::Url;

pub async fn connect(
    request: Request,
    proxy: Option<&str>,
) -> Result<(WebSocketStream<MaybeTlsStream<TcpStream>>, Response)> {
    let Some(proxy) = proxy else {
        return connect_async(request)
            .await
            .context("direct websocket connect failed");
    };
    let proxy =
        Url::parse(proxy).with_context(|| format!("invalid websocket proxy URL: {proxy}"))?;
    if proxy.scheme() != "http" {
        return Err(anyhow!(
            "websocket stream currently supports http proxy only; got {}",
            proxy.scheme()
        ));
    }
    let proxy_host = proxy
        .host_str()
        .ok_or_else(|| anyhow!("websocket proxy URL missing host"))?;
    let proxy_port = proxy
        .port_or_known_default()
        .ok_or_else(|| anyhow!("websocket proxy URL missing port"))?;
    let target_host = request
        .uri()
        .host()
        .ok_or_else(|| anyhow!("websocket target URL missing host"))?
        .to_string();
    let target_port = request
        .uri()
        .port_u16()
        .or_else(|| match request.uri().scheme_str() {
            Some("wss") => Some(443),
            Some("ws") => Some(80),
            _ => None,
        })
        .ok_or_else(|| anyhow!("websocket target URL missing port"))?;
    let target = format!("{target_host}:{target_port}");
    let mut stream = TcpStream::connect((proxy_host, proxy_port))
        .await
        .with_context(|| format!("failed to connect websocket proxy {proxy_host}:{proxy_port}"))?;
    let connect = format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n");
    stream
        .write_all(connect.as_bytes())
        .await
        .context("failed to write websocket proxy CONNECT")?;
    let mut response = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let read = stream
            .read(&mut buffer)
            .await
            .context("failed to read websocket proxy CONNECT response")?;
        if read == 0 {
            return Err(anyhow!("websocket proxy closed before CONNECT completed"));
        }
        response.extend_from_slice(&buffer[..read]);
        if response.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if response.len() > 8192 {
            return Err(anyhow!("websocket proxy CONNECT response was too large"));
        }
    }
    let response_text = String::from_utf8_lossy(&response);
    if !(response_text.starts_with("HTTP/1.1 200") || response_text.starts_with("HTTP/1.0 200")) {
        return Err(anyhow!(
            "websocket proxy CONNECT failed: {}",
            response_text.lines().next().unwrap_or("<empty>")
        ));
    }
    client_async_tls_with_config(request, stream, None, None)
        .await
        .context("websocket TLS handshake through proxy failed")
}
