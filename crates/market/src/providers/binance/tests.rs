use crate::args::{CryptoMarket, CryptoStreamKind, FuturesPeriod};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::*;

#[test]
fn normalize_symbol_accepts_common_pair_spellings() {
    assert_eq!(normalize_symbol("btc/usdt").unwrap(), "BTCUSDT");
    assert_eq!(normalize_symbol("BTC-USDT").unwrap(), "BTCUSDT");
    assert_eq!(normalize_symbol("btc_usdt").unwrap(), "BTCUSDT");
}

#[test]
fn normalize_symbol_rejects_empty_or_weird_symbols() {
    assert!(normalize_symbol(" / ").is_err());
    assert!(normalize_symbol("BTC.USDT").is_err());
}

#[test]
fn registry_has_live_case_for_each_symbol_endpoint() {
    assert!(!BINANCE_ENDPOINTS.is_empty());
    for endpoint in BINANCE_ENDPOINTS {
        assert!(
            !endpoint.official_endpoint.is_empty(),
            "{} should document its official Binance endpoint",
            endpoint.route
        );
        assert!(
            endpoint.implementation.starts_with("binance::"),
            "{} should document the internal implementation entrypoint",
            endpoint.route
        );
        assert!(
            endpoint.output_model.ends_with("Report"),
            "{} should document the normalized output model",
            endpoint.route
        );
        if endpoint.route.contains("exchange-info") && endpoint.market == "usds-futures" {
            continue;
        }
        assert!(
            endpoint.live_symbol.is_some(),
            "{} should declare a live smoke symbol",
            endpoint.route
        );
    }
}

#[test]
fn stream_names_follow_official_binance_stream_paths() {
    assert_eq!(
        websocket::stream_path(CryptoMarket::Spot, CryptoStreamKind::Trade, "BTCUSDT", "1m")
            .unwrap(),
        "btcusdt@trade"
    );
    assert_eq!(
        websocket::stream_path(
            CryptoMarket::UsdsFutures,
            CryptoStreamKind::MarkPrice,
            "BTCUSDT",
            "1m"
        )
        .unwrap(),
        "market/ws/btcusdt@markPrice@1s"
    );
    assert_eq!(
        websocket::stream_path(
            CryptoMarket::UsdsFutures,
            CryptoStreamKind::Depth,
            "BTCUSDT",
            "1m"
        )
        .unwrap(),
        "public/ws/btcusdt@depth@100ms"
    );
}

#[tokio::test]
async fn spot_ticker_uses_official_symbol_query_path() {
    let base_url = one_shot_json_server(
        "GET /api/v3/ticker/price?symbol=BTCUSDT ",
        r#"{"price":"1.23"}"#,
    )
    .await;
    let config = test_config(base_url, "http://127.0.0.1:1".to_string());

    let report = spot_ticker(&config, "btc/usdt").await.unwrap();

    assert_eq!(report.symbol.as_deref(), Some("BTCUSDT"));
    assert_eq!(report.payload["price"], "1.23");
}

#[tokio::test]
async fn futures_basis_uses_pair_not_symbol_query_path() {
    let base_url = one_shot_json_server(
        "GET /futures/data/basis?pair=BTCUSDT&contractType=PERPETUAL&period=5m&limit=2 ",
        r#"[{"basis":"0.1"}]"#,
    )
    .await;
    let config = test_config("http://127.0.0.1:1".to_string(), base_url);

    let report = futures_basis(&config, "BTC-USDT", FuturesPeriod::FiveMin, 2)
        .await
        .unwrap();

    assert_eq!(report.symbol.as_deref(), Some("BTCUSDT"));
    assert_eq!(report.payload.as_array().unwrap().len(), 1);
}

#[test]
fn aggregate_reports_reject_partial_errors() {
    let snapshot = CryptoSnapshotReport {
        symbol: "BTCUSDT".to_string(),
        provider: "binance".to_string(),
        fetched_at_utc: "2026-01-01T00:00:00Z".to_string(),
        spot: Default::default(),
        futures: Default::default(),
        errors: [("spot.ticker".to_string(), "invalid symbol".to_string())].into(),
    };
    let sentiment = CryptoSentimentReport {
        symbol: "BTCUSDT".to_string(),
        provider: "binance".to_string(),
        fetched_at_utc: "2026-01-01T00:00:00Z".to_string(),
        futures: Default::default(),
        errors: [("funding".to_string(), "invalid symbol".to_string())].into(),
    };

    assert!(snapshot.ensure_complete().is_err());
    assert!(sentiment.ensure_complete().is_err());
}

#[tokio::test]
#[ignore = "requires AGENT_FINANCE_LIVE_BINANCE=1 and live Binance network access"]
async fn live_binance_public_market_endpoints_are_usable() {
    if std::env::var("AGENT_FINANCE_LIVE_BINANCE").ok().as_deref() != Some("1") {
        eprintln!("skipping live Binance test; set AGENT_FINANCE_LIVE_BINANCE=1");
        return;
    }
    let config = BinanceConfig::from_env(15, None, false);
    let symbol = "BTCUSDT";
    assert!(
        spot_ticker(&config, symbol)
            .await
            .unwrap()
            .payload
            .is_object()
    );
    assert!(
        !spot_trades(&config, symbol, 2, true)
            .await
            .unwrap()
            .payload
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !spot_klines(&config, symbol, "1m", 2)
            .await
            .unwrap()
            .payload
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        futures_ticker(&config, symbol)
            .await
            .unwrap()
            .payload
            .is_object()
    );
    assert!(
        futures_mark(&config, symbol)
            .await
            .unwrap()
            .payload
            .is_object()
    );
    assert!(
        futures_open_interest(&config, symbol)
            .await
            .unwrap()
            .payload
            .is_object()
    );
    assert!(
        !futures_funding(&config, symbol, 2)
            .await
            .unwrap()
            .payload
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !futures_ratios(&config, symbol, FuturesPeriod::FiveMin, 2)
            .await
            .unwrap()
            .payload
            .as_object()
            .unwrap()
            .is_empty()
    );
    assert!(
        !futures_flow(&config, symbol, FuturesPeriod::FiveMin, 2)
            .await
            .unwrap()
            .payload
            .as_array()
            .unwrap()
            .is_empty()
    );
}

fn test_config(spot_base_url: String, futures_base_url: String) -> BinanceConfig {
    BinanceConfig {
        timeout_seconds: 5,
        proxy: None,
        no_proxy: true,
        spot_base_url,
        spot_base_url_overridden: true,
        futures_base_url,
        spot_ws_url: "ws://127.0.0.1:1/ws".to_string(),
        futures_ws_url: "ws://127.0.0.1:1".to_string(),
        api_key: None,
    }
}

async fn one_shot_json_server(expected_request_prefix: &'static str, body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buffer = vec![0; 4096];
        let read = stream.read(&mut buffer).await.unwrap();
        let request = String::from_utf8_lossy(&buffer[..read]);
        assert!(
            request.starts_with(expected_request_prefix),
            "request was {request:?}"
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await.unwrap();
    });
    format!("http://{address}")
}
