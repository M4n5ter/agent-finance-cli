use std::time::Duration;

use agent_finance_core::{
    CancelIntent, Environment, FuturesStateIntent, Market, OrderIdentifier, OrderIntent, OrderKind,
    OrderSide, OrderSpec, TransferDirection, TransferIntent,
};
use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use url::Url;
use wreq::Client;

use crate::exchange_rules::{ExchangeRuleCheck, check_order_exchange_rules};
use crate::futures_state;
use crate::signer::{HmacSha256Signer, Signer};

const LIVE_SPOT_BASE_URL: &str = "https://api.binance.com";
const LIVE_FUTURES_BASE_URL: &str = "https://fapi.binance.com";
const LIVE_SAPI_BASE_URL: &str = "https://api.binance.com";
const TESTNET_SPOT_BASE_URL: &str = "https://testnet.binance.vision";
const TESTNET_FUTURES_BASE_URL: &str = "https://testnet.binancefuture.com";

#[derive(Debug, Clone)]
pub struct BinanceCredentials {
    pub api_key: String,
    pub api_secret: String,
}

impl BinanceCredentials {
    pub fn from_env(api_key_env: &str, api_secret_env: &str) -> Result<Self> {
        let api_key = std::env::var(api_key_env)
            .with_context(|| format!("missing Binance API key env var {api_key_env}"))?;
        let api_secret = std::env::var(api_secret_env)
            .with_context(|| format!("missing Binance API secret env var {api_secret_env}"))?;
        Ok(Self {
            api_key,
            api_secret,
        })
    }
}

#[derive(Debug, Clone)]
pub struct BinanceEndpoints {
    pub environment: Environment,
    pub spot_base_url: String,
    pub futures_base_url: String,
    pub sapi_base_url: String,
}

impl BinanceEndpoints {
    pub fn new(
        environment: Environment,
        spot_base_url: Option<String>,
        futures_base_url: Option<String>,
        sapi_base_url: Option<String>,
    ) -> Self {
        let (default_spot, default_futures, default_sapi) = match environment {
            Environment::Live => (
                LIVE_SPOT_BASE_URL,
                LIVE_FUTURES_BASE_URL,
                LIVE_SAPI_BASE_URL,
            ),
            Environment::Testnet => (
                TESTNET_SPOT_BASE_URL,
                TESTNET_FUTURES_BASE_URL,
                LIVE_SAPI_BASE_URL,
            ),
        };
        Self {
            environment,
            spot_base_url: spot_base_url.unwrap_or_else(|| default_spot.to_string()),
            futures_base_url: futures_base_url.unwrap_or_else(|| default_futures.to_string()),
            sapi_base_url: sapi_base_url.unwrap_or_else(|| default_sapi.to_string()),
        }
    }

    fn validate_signed_hosts(&self) -> Result<()> {
        let live_spot = [
            "api.binance.com",
            "api1.binance.com",
            "api2.binance.com",
            "api3.binance.com",
            "api4.binance.com",
        ];
        let live_futures = ["fapi.binance.com"];
        let live_sapi = ["api.binance.com"];
        let testnet_spot = ["testnet.binance.vision"];
        let testnet_futures = ["testnet.binancefuture.com"];
        let (spot_hosts, futures_hosts, sapi_hosts) = match self.environment {
            Environment::Live => (
                live_spot.as_slice(),
                live_futures.as_slice(),
                live_sapi.as_slice(),
            ),
            Environment::Testnet => (
                testnet_spot.as_slice(),
                testnet_futures.as_slice(),
                live_sapi.as_slice(),
            ),
        };
        validate_signed_base_url("spot", &self.spot_base_url, spot_hosts)?;
        validate_signed_base_url("usds-futures", &self.futures_base_url, futures_hosts)?;
        validate_signed_base_url("sapi", &self.sapi_base_url, sapi_hosts)?;
        Ok(())
    }
}

fn validate_signed_base_url(role: &str, base_url: &str, allowed_hosts: &[&str]) -> Result<()> {
    let url = Url::parse(base_url)
        .with_context(|| format!("invalid Binance {role} signed base URL: {base_url}"))?;
    if url.scheme() != "https" {
        return Err(anyhow!(
            "refusing to send signed Binance {role} requests over non-HTTPS scheme {}",
            url.scheme()
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("Binance {role} signed base URL has no host: {base_url}"))?;
    if !allowed_hosts.contains(&host) {
        return Err(anyhow!(
            "refusing to send signed Binance {role} requests to non-official host {host}"
        ));
    }
    Ok(())
}

pub struct BinanceClient {
    http: Client,
    credentials: BinanceCredentials,
    endpoints: BinanceEndpoints,
    signer: Box<dyn Signer>,
    recv_window_ms: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum BinanceRequestMode {
    Test,
    Live,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignedRequest {
    pub method: String,
    pub url: String,
    pub params: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BinanceOrderSubmitResponse {
    pub exchange_rules: ExchangeRuleCheck,
    pub exchange_response: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct BinanceFuturesStateSubmitResponse {
    pub request: SignedRequest,
    pub exchange_response: Value,
}

pub struct BinancePlanner {
    endpoints: BinanceEndpoints,
}

impl BinancePlanner {
    pub fn new(endpoints: BinanceEndpoints) -> Self {
        Self { endpoints }
    }

    pub fn order_request(&self, intent: &OrderIntent, test: bool) -> Result<SignedRequest> {
        Ok(unsigned_request(
            "POST",
            order_base_url(&self.endpoints, intent.market),
            order_path(intent.market, test),
            order_params(intent)?,
        ))
    }

    pub fn cancel_request(&self, intent: &CancelIntent) -> Result<SignedRequest> {
        Ok(unsigned_request(
            "DELETE",
            order_base_url(&self.endpoints, intent.market),
            order_path(intent.market, false),
            cancel_params(intent)?,
        ))
    }

    pub fn query_order_request(
        &self,
        market: Market,
        symbol: &str,
        target: &OrderIdentifier,
    ) -> Result<SignedRequest> {
        Ok(unsigned_request(
            "GET",
            order_base_url(&self.endpoints, market),
            order_path(market, false),
            order_query_params(symbol, target)?,
        ))
    }

    pub fn exchange_info_request(&self, market: Market, symbol: &str) -> SignedRequest {
        unsigned_request(
            "GET",
            market_base_url(&self.endpoints, market),
            exchange_info_path(market),
            exchange_info_params(market, symbol),
        )
    }

    pub fn transfer_request(&self, intent: &TransferIntent) -> Result<SignedRequest> {
        Ok(unsigned_request(
            "POST",
            &self.endpoints.sapi_base_url,
            "/sapi/v1/asset/transfer",
            transfer_params(intent),
        ))
    }

    pub fn transfer_history_request(
        &self,
        direction: TransferDirection,
        current: usize,
        size: usize,
    ) -> SignedRequest {
        unsigned_request(
            "GET",
            &self.endpoints.sapi_base_url,
            "/sapi/v1/asset/transfer",
            transfer_history_params(direction, current, size),
        )
    }

    pub fn futures_state_request(&self, intent: &FuturesStateIntent) -> Result<SignedRequest> {
        Ok(unsigned_request(
            "POST",
            &self.endpoints.futures_base_url,
            futures_state_path(intent),
            futures_state_params(intent),
        ))
    }
}

impl BinanceClient {
    pub fn new(
        credentials: BinanceCredentials,
        endpoints: BinanceEndpoints,
        timeout_seconds: u64,
    ) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .context("failed to build Binance HTTP client")?;
        endpoints.validate_signed_hosts()?;
        let signer = Box::new(HmacSha256Signer::new(credentials.api_secret.clone()));
        Ok(Self {
            http,
            credentials,
            endpoints,
            signer,
            recv_window_ms: 5000,
        })
    }

    pub fn endpoints(&self) -> &BinanceEndpoints {
        &self.endpoints
    }

    pub async fn account_permissions(&self) -> Result<Value> {
        self.signed_get(
            &self.endpoints.sapi_base_url,
            "/sapi/v1/account/apiRestrictions",
            vec![],
        )
        .await
    }

    pub async fn spot_account(&self) -> Result<Value> {
        self.signed_get(&self.endpoints.spot_base_url, "/api/v3/account", vec![])
            .await
    }

    pub async fn futures_account(&self) -> Result<Value> {
        self.signed_get(&self.endpoints.futures_base_url, "/fapi/v3/account", vec![])
            .await
    }

    pub async fn open_orders(&self, market: Market, symbol: Option<&str>) -> Result<Value> {
        let mut params = Vec::new();
        if let Some(symbol) = symbol {
            params.push(("symbol".to_string(), symbol.to_ascii_uppercase()));
        }
        match market {
            Market::Spot => {
                self.signed_get(&self.endpoints.spot_base_url, "/api/v3/openOrders", params)
                    .await
            }
            Market::UsdsFutures => {
                self.signed_get(
                    &self.endpoints.futures_base_url,
                    "/fapi/v1/openOrders",
                    params,
                )
                .await
            }
        }
    }

    pub async fn query_order(
        &self,
        market: Market,
        symbol: &str,
        target: &OrderIdentifier,
    ) -> Result<Value> {
        self.signed_get(
            order_base_url(&self.endpoints, market),
            order_path(market, false),
            order_query_params(symbol, target)?,
        )
        .await
    }

    pub async fn exchange_info(&self, market: Market, symbol: &str) -> Result<Value> {
        self.public_get(
            market_base_url(&self.endpoints, market),
            exchange_info_path(market),
            exchange_info_params(market, symbol),
        )
        .await
    }

    pub async fn submit_order(
        &self,
        intent: &OrderIntent,
        mode: BinanceRequestMode,
    ) -> Result<BinanceOrderSubmitResponse> {
        let exchange_info = self.exchange_info(intent.market, &intent.symbol).await?;
        let exchange_rules = check_order_exchange_rules(intent, &exchange_info)?;
        if !exchange_rules.allowed {
            return Err(anyhow!(
                "Binance exchange rules blocked order: {}",
                serde_json::to_string(&exchange_rules)?
            ));
        }
        let exchange_response = self.submit_unchecked_order(intent, mode).await?;
        Ok(BinanceOrderSubmitResponse {
            exchange_rules,
            exchange_response,
        })
    }

    async fn submit_unchecked_order(
        &self,
        intent: &OrderIntent,
        mode: BinanceRequestMode,
    ) -> Result<Value> {
        let params = order_params(intent)?;
        match (mode, intent.market) {
            (BinanceRequestMode::Test, Market::Spot) => {
                self.signed_post(&self.endpoints.spot_base_url, "/api/v3/order/test", params)
                    .await
            }
            (BinanceRequestMode::Test, Market::UsdsFutures) => {
                self.signed_post(
                    &self.endpoints.futures_base_url,
                    "/fapi/v1/order/test",
                    params,
                )
                .await
            }
            (BinanceRequestMode::Live, market) => {
                self.signed_post(
                    order_base_url(&self.endpoints, market),
                    order_path(market, false),
                    params,
                )
                .await
            }
        }
    }

    pub async fn cancel_order(&self, intent: &CancelIntent) -> Result<Value> {
        let params = cancel_params(intent)?;
        match intent.market {
            Market::Spot => {
                self.signed_delete(&self.endpoints.spot_base_url, "/api/v3/order", params)
                    .await
            }
            Market::UsdsFutures => {
                self.signed_delete(&self.endpoints.futures_base_url, "/fapi/v1/order", params)
                    .await
            }
        }
    }

    pub async fn submit_transfer(
        &self,
        intent: &TransferIntent,
        mode: BinanceRequestMode,
    ) -> Result<Value> {
        let params = transfer_params(intent);
        match mode {
            BinanceRequestMode::Test => Ok(serde_json::json!({
                "test": true,
                "request": BinancePlanner::new(self.endpoints.clone()).transfer_request(intent)?,
                "note": "Binance universal transfer has no dedicated test endpoint; test mode does not submit.",
            })),
            BinanceRequestMode::Live => {
                self.signed_post(
                    &self.endpoints.sapi_base_url,
                    "/sapi/v1/asset/transfer",
                    params,
                )
                .await
            }
        }
    }

    pub async fn submit_futures_state(
        &self,
        intent: &FuturesStateIntent,
    ) -> Result<BinanceFuturesStateSubmitResponse> {
        let request = BinancePlanner::new(self.endpoints.clone()).futures_state_request(intent)?;
        let exchange_response = self
            .signed_post(
                &self.endpoints.futures_base_url,
                futures_state_path(intent),
                futures_state_params(intent),
            )
            .await?;
        Ok(BinanceFuturesStateSubmitResponse {
            request,
            exchange_response,
        })
    }

    pub async fn transfer_history(
        &self,
        direction: TransferDirection,
        current: usize,
        size: usize,
    ) -> Result<Value> {
        self.signed_get(
            &self.endpoints.sapi_base_url,
            "/sapi/v1/asset/transfer",
            transfer_history_params(direction, current, size),
        )
        .await
    }

    fn signed_url(
        &self,
        base_url: &str,
        path: &str,
        mut params: Vec<(String, String)>,
    ) -> Result<Url> {
        params.push(("recvWindow".to_string(), self.recv_window_ms.to_string()));
        params.push((
            "timestamp".to_string(),
            Utc::now().timestamp_millis().to_string(),
        ));
        let query = form_urlencoded(&params);
        let signature = self.signer.sign(&query)?;
        params.push(("signature".to_string(), signature));
        build_url(base_url, path, &params)
    }

    async fn signed_get(
        &self,
        base_url: &str,
        path: &str,
        params: Vec<(String, String)>,
    ) -> Result<Value> {
        let url = self.signed_url(base_url, path, params)?;
        self.send(self.http.get(url.as_str())).await
    }

    async fn signed_post(
        &self,
        base_url: &str,
        path: &str,
        params: Vec<(String, String)>,
    ) -> Result<Value> {
        let url = self.signed_url(base_url, path, params)?;
        self.send(self.http.post(url.as_str())).await
    }

    async fn signed_delete(
        &self,
        base_url: &str,
        path: &str,
        params: Vec<(String, String)>,
    ) -> Result<Value> {
        let url = self.signed_url(base_url, path, params)?;
        self.send(self.http.delete(url.as_str())).await
    }

    async fn public_get(
        &self,
        base_url: &str,
        path: &str,
        params: Vec<(String, String)>,
    ) -> Result<Value> {
        let url = build_url(base_url, path, &params)?;
        self.send_unsigned(self.http.get(url.as_str())).await
    }

    async fn send(&self, request: wreq::RequestBuilder) -> Result<Value> {
        let response = request
            .header("X-MBX-APIKEY", &self.credentials.api_key)
            .send()
            .await
            .context("Binance signed request failed")?;
        decode_response(response).await
    }

    async fn send_unsigned(&self, request: wreq::RequestBuilder) -> Result<Value> {
        let response = request
            .send()
            .await
            .context("Binance public request failed")?;
        decode_response(response).await
    }
}

async fn decode_response(response: wreq::Response) -> Result<Value> {
    let status = response.status();
    let body = response
        .text()
        .await
        .context("failed to read Binance response")?;
    if !status.is_success() {
        return Err(anyhow!(
            "Binance request failed status={} body={body}",
            status.as_u16()
        ));
    }
    if body.trim().is_empty() {
        return Ok(serde_json::json!({ "status": status.as_u16() }));
    }
    serde_json::from_str(&body).context("failed to decode Binance JSON response")
}

fn order_params(intent: &OrderIntent) -> Result<Vec<(String, String)>> {
    let mut params = vec![
        ("symbol".to_string(), intent.symbol.to_ascii_uppercase()),
        ("side".to_string(), side(intent.side).to_string()),
        (
            "type".to_string(),
            order_type(intent.market, intent.spec.kind())?.to_string(),
        ),
        ("quantity".to_string(), intent.quantity.to_string()),
        (
            "newClientOrderId".to_string(),
            intent.client_order_id.clone(),
        ),
    ];
    match &intent.spec {
        OrderSpec::Market { .. } => {}
        OrderSpec::Limit {
            price,
            time_in_force,
        } => {
            params.push(("price".to_string(), price.to_string()));
            params.push(("timeInForce".to_string(), time_in_force.to_string()));
        }
        OrderSpec::PostOnlyLimit { price } => {
            params.push(("price".to_string(), price.to_string()));
        }
        OrderSpec::StopLoss { stop_price } | OrderSpec::TakeProfit { stop_price } => {
            params.push(("stopPrice".to_string(), stop_price.to_string()));
        }
    }
    if intent.market == Market::UsdsFutures {
        if intent.reduce_only {
            params.push(("reduceOnly".to_string(), "true".to_string()));
        }
        if let Some(position_side) = intent.position_side {
            params.push(("positionSide".to_string(), position_side.to_string()));
        }
    }
    Ok(params)
}

fn cancel_params(intent: &CancelIntent) -> Result<Vec<(String, String)>> {
    order_query_params(&intent.symbol, &intent.target)
}

fn order_query_params(symbol: &str, target: &OrderIdentifier) -> Result<Vec<(String, String)>> {
    let mut params = vec![("symbol".to_string(), symbol.to_ascii_uppercase())];
    match target {
        OrderIdentifier::OrderId { order_id } => {
            params.push(("orderId".to_string(), order_id.to_string()));
        }
        OrderIdentifier::ClientOrderId { client_order_id } => {
            params.push(("origClientOrderId".to_string(), client_order_id.to_string()));
        }
    }
    Ok(params)
}

fn transfer_params(intent: &TransferIntent) -> Vec<(String, String)> {
    vec![
        (
            "type".to_string(),
            transfer_type(intent.direction).to_string(),
        ),
        ("asset".to_string(), intent.asset.to_ascii_uppercase()),
        ("amount".to_string(), intent.amount.to_string()),
        (
            "clientTranId".to_string(),
            intent.client_transfer_id.clone(),
        ),
    ]
}

fn transfer_history_params(
    direction: TransferDirection,
    current: usize,
    size: usize,
) -> Vec<(String, String)> {
    vec![
        ("type".to_string(), transfer_type(direction).to_string()),
        ("current".to_string(), current.max(1).to_string()),
        ("size".to_string(), size.clamp(1, 100).to_string()),
    ]
}

fn futures_state_path(intent: &FuturesStateIntent) -> &'static str {
    futures_state::path(intent)
}

fn futures_state_params(intent: &FuturesStateIntent) -> Vec<(String, String)> {
    futures_state::params(intent)
}

fn order_base_url(endpoints: &BinanceEndpoints, market: Market) -> &str {
    market_base_url(endpoints, market)
}

fn market_base_url(endpoints: &BinanceEndpoints, market: Market) -> &str {
    match market {
        Market::Spot => &endpoints.spot_base_url,
        Market::UsdsFutures => &endpoints.futures_base_url,
    }
}

fn order_path(market: Market, test: bool) -> &'static str {
    match (market, test) {
        (Market::Spot, true) => "/api/v3/order/test",
        (Market::Spot, false) => "/api/v3/order",
        (Market::UsdsFutures, true) => "/fapi/v1/order/test",
        (Market::UsdsFutures, false) => "/fapi/v1/order",
    }
}

fn exchange_info_path(market: Market) -> &'static str {
    match market {
        Market::Spot => "/api/v3/exchangeInfo",
        Market::UsdsFutures => "/fapi/v1/exchangeInfo",
    }
}

fn exchange_info_params(market: Market, symbol: &str) -> Vec<(String, String)> {
    match market {
        Market::Spot => vec![("symbol".to_string(), symbol.to_ascii_uppercase())],
        Market::UsdsFutures => Vec::new(),
    }
}

fn side(side: OrderSide) -> &'static str {
    match side {
        OrderSide::Buy => "BUY",
        OrderSide::Sell => "SELL",
    }
}

fn order_type(market: Market, kind: OrderKind) -> Result<&'static str> {
    match (market, kind) {
        (_, OrderKind::Market) => Ok("MARKET"),
        (_, OrderKind::Limit) => Ok("LIMIT"),
        (Market::Spot, OrderKind::PostOnlyLimit) => Ok("LIMIT_MAKER"),
        (Market::Spot, OrderKind::StopLoss) => Ok("STOP_LOSS"),
        (Market::Spot, OrderKind::TakeProfit) => Ok("TAKE_PROFIT"),
        (
            Market::UsdsFutures,
            OrderKind::PostOnlyLimit | OrderKind::StopLoss | OrderKind::TakeProfit,
        ) => Err(anyhow!("{kind} is not supported for usds-futures yet")),
    }
}

fn transfer_type(direction: TransferDirection) -> &'static str {
    match direction {
        TransferDirection::SpotToUsdsFutures => "MAIN_UMFUTURE",
        TransferDirection::UsdsFuturesToSpot => "UMFUTURE_MAIN",
    }
}

fn build_url(base_url: &str, path: &str, params: &[(String, String)]) -> Result<Url> {
    let base_url = if base_url.ends_with('/') {
        base_url.to_string()
    } else {
        format!("{base_url}/")
    };
    let mut url = Url::parse(&base_url)
        .with_context(|| format!("invalid Binance base URL: {base_url}"))?
        .join(path.trim_start_matches('/'))
        .with_context(|| format!("invalid Binance API path: {path}"))?;
    {
        let mut query = url.query_pairs_mut();
        for (key, value) in params {
            query.append_pair(key, value);
        }
    }
    Ok(url)
}

fn unsigned_request(
    method: &str,
    base_url: &str,
    path: &str,
    params: Vec<(String, String)>,
) -> SignedRequest {
    let base_url = base_url.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    SignedRequest {
        method: method.to_string(),
        url: format!("{base_url}/{path}"),
        params,
    }
}

fn form_urlencoded(params: &[(String, String)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in params {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_finance_core::{DecimalValue, Provider, TimeInForce};

    #[test]
    fn maps_spot_limit_order_params() {
        let intent = OrderIntent {
            profile: "test".to_string(),
            provider: Provider::Binance,
            environment: Environment::Testnet,
            market: Market::Spot,
            symbol: "btcusdt".to_string(),
            side: OrderSide::Buy,
            quantity: "0.01".parse::<DecimalValue>().unwrap(),
            spec: OrderSpec::Limit {
                price: "50000".parse::<DecimalValue>().unwrap(),
                time_in_force: TimeInForce::Gtc,
            },
            reduce_only: false,
            position_side: None,
            client_order_id: "af-test".to_string(),
        };

        let params = order_params(&intent).unwrap();

        assert!(params.contains(&("symbol".to_string(), "BTCUSDT".to_string())));
        assert!(params.contains(&("type".to_string(), "LIMIT".to_string())));
        assert!(params.contains(&("timeInForce".to_string(), "GTC".to_string())));
    }

    #[test]
    fn maps_universal_transfer_direction() {
        assert_eq!(
            transfer_type(TransferDirection::SpotToUsdsFutures),
            "MAIN_UMFUTURE"
        );
        assert_eq!(
            transfer_type(TransferDirection::UsdsFuturesToSpot),
            "UMFUTURE_MAIN"
        );
    }

    #[test]
    fn maps_transfer_history_request_params() {
        let request =
            BinancePlanner::new(BinanceEndpoints::new(Environment::Live, None, None, None))
                .transfer_history_request(TransferDirection::SpotToUsdsFutures, 0, 250);

        assert_eq!(request.method, "GET");
        assert!(request.url.ends_with("/sapi/v1/asset/transfer"));
        assert!(
            request
                .params
                .contains(&("type".to_string(), "MAIN_UMFUTURE".to_string()))
        );
        assert!(
            request
                .params
                .contains(&("current".to_string(), "1".to_string()))
        );
        assert!(
            request
                .params
                .contains(&("size".to_string(), "100".to_string()))
        );
    }

    #[test]
    fn maps_query_order_request_params() {
        let request =
            BinancePlanner::new(BinanceEndpoints::new(Environment::Live, None, None, None))
                .query_order_request(
                    Market::Spot,
                    "btcusdt",
                    &OrderIdentifier::ClientOrderId {
                        client_order_id: "af-test".to_string(),
                    },
                )
                .expect("query request");

        assert_eq!(request.method, "GET");
        assert!(request.url.ends_with("/api/v3/order"));
        assert!(
            request
                .params
                .contains(&("symbol".to_string(), "BTCUSDT".to_string()))
        );
        assert!(
            request
                .params
                .contains(&("origClientOrderId".to_string(), "af-test".to_string()))
        );
    }

    #[test]
    fn exchange_info_request_follows_market_api_contract() {
        let planner =
            BinancePlanner::new(BinanceEndpoints::new(Environment::Live, None, None, None));

        let spot = planner.exchange_info_request(Market::Spot, "btcusdt");
        let futures = planner.exchange_info_request(Market::UsdsFutures, "btcusdt");

        assert!(spot.url.ends_with("/api/v3/exchangeInfo"));
        assert!(
            spot.params
                .contains(&("symbol".to_string(), "BTCUSDT".to_string()))
        );
        assert!(futures.url.ends_with("/fapi/v1/exchangeInfo"));
        assert!(
            futures.params.is_empty(),
            "USD-M futures exchangeInfo has no request parameters"
        );
    }

    #[test]
    fn rejects_limit_without_price() {
        let error = OrderSpec::new(
            Market::Spot,
            OrderKind::Limit,
            None,
            None,
            None,
            Some(TimeInForce::Gtc),
        )
        .expect_err("limit without price should be rejected");

        assert!(
            format!("{error:#}").contains("requires price"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn market_order_uses_valuation_without_exchange_price() {
        let intent = OrderIntent {
            profile: "test".to_string(),
            provider: Provider::Binance,
            environment: Environment::Testnet,
            market: Market::Spot,
            symbol: "BTCUSDT".to_string(),
            side: OrderSide::Buy,
            quantity: "0.01".parse::<DecimalValue>().unwrap(),
            spec: OrderSpec::Market {
                valuation_price: "50000".parse::<DecimalValue>().unwrap(),
            },
            reduce_only: false,
            position_side: None,
            client_order_id: "af-test".to_string(),
        };

        let params = order_params(&intent).unwrap();

        assert!(params.contains(&("type".to_string(), "MARKET".to_string())));
        assert!(!params.iter().any(|(key, _)| key == "price"));
    }

    #[test]
    fn live_signed_client_rejects_non_official_hosts() {
        let credentials = BinanceCredentials {
            api_key: "key".to_string(),
            api_secret: "secret".to_string(),
        };
        let endpoints = BinanceEndpoints::new(
            Environment::Live,
            Some("https://example.com".to_string()),
            None,
            None,
        );

        let error = match BinanceClient::new(credentials, endpoints, 10) {
            Ok(_) => panic!("non-official live host should be rejected"),
            Err(error) => error,
        };

        assert!(
            format!("{error:#}").contains("non-official host"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn signed_client_rejects_non_https_hosts() {
        let credentials = BinanceCredentials {
            api_key: "key".to_string(),
            api_secret: "secret".to_string(),
        };
        let endpoints = BinanceEndpoints::new(
            Environment::Live,
            Some("http://api.binance.com".to_string()),
            None,
            None,
        );

        let error = match BinanceClient::new(credentials, endpoints, 10) {
            Ok(_) => panic!("non-HTTPS signed host should be rejected"),
            Err(error) => error,
        };

        assert!(
            format!("{error:#}").contains("non-HTTPS"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn testnet_signed_client_rejects_mainnet_spot_host() {
        let credentials = BinanceCredentials {
            api_key: "key".to_string(),
            api_secret: "secret".to_string(),
        };
        let endpoints = BinanceEndpoints::new(
            Environment::Testnet,
            Some("https://api.binance.com".to_string()),
            None,
            None,
        );

        let error = match BinanceClient::new(credentials, endpoints, 10) {
            Ok(_) => panic!("testnet spot host should not route to mainnet"),
            Err(error) => error,
        };

        assert!(
            format!("{error:#}").contains("Binance spot"),
            "unexpected error: {error:#}"
        );
    }
}
