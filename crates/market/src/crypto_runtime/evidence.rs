use std::future::Future;

use anyhow::Result;
use futures_util::future::{BoxFuture, FutureExt, join_all};
use serde::Serialize;

use crate::args::{CryptoInstrument, CryptoProvider};
use crate::crypto_capability::{CryptoCapability, provider_supports, selected_providers};

#[derive(Clone, Copy, Debug)]
pub struct EvidenceRequest {
    pub provider: CryptoProvider,
    pub instrument: CryptoInstrument,
    pub capability: CryptoCapability,
}

impl EvidenceRequest {
    pub const fn new(
        provider: CryptoProvider,
        instrument: CryptoInstrument,
        capability: CryptoCapability,
    ) -> Self {
        Self {
            provider,
            instrument,
            capability,
        }
    }
}

pub struct EvidenceEngine;

impl EvidenceEngine {
    pub async fn collect<F, Fut>(request: EvidenceRequest, fetch: F) -> Vec<ProviderEvidence>
    where
        F: Fn(CryptoProvider) -> Fut,
        Fut: Future<Output = ProviderEvidence>,
    {
        let futures = selected_providers(request.provider, request.instrument, request.capability)
            .into_iter()
            .map(|provider| {
                let fetch = &fetch;
                async move {
                    if provider_supports(provider, request.instrument, request.capability) {
                        fetch(provider).await
                    } else {
                        unsupported_provider(
                            provider.label(),
                            request.capability,
                            request.instrument,
                        )
                    }
                }
            });
        join_all(futures).await
    }
}

pub async fn collect_endpoint_evidence(
    endpoints: Vec<BoxFuture<'static, EndpointEvidence>>,
) -> Vec<EndpointEvidence> {
    join_all(endpoints).await
}

pub fn required_endpoint<T, Fut>(
    endpoint: &'static str,
    result: Fut,
) -> BoxFuture<'static, EndpointEvidence>
where
    T: Serialize + Send + 'static,
    Fut: Future<Output = Result<T>> + Send + 'static,
{
    endpoint_future(endpoint, true, result)
}

pub fn supplemental_endpoint<T, Fut>(
    endpoint: &'static str,
    result: Fut,
) -> BoxFuture<'static, EndpointEvidence>
where
    T: Serialize + Send + 'static,
    Fut: Future<Output = Result<T>> + Send + 'static,
{
    endpoint_future(endpoint, false, result)
}

fn endpoint_future<T, Fut>(
    endpoint: &'static str,
    required: bool,
    result: Fut,
) -> BoxFuture<'static, EndpointEvidence>
where
    T: Serialize + Send + 'static,
    Fut: Future<Output = Result<T>> + Send + 'static,
{
    async move { endpoint_result(endpoint, required, result.await) }.boxed()
}

pub fn required_payload<T: Serialize>(endpoint: &str, result: Result<T>) -> EndpointEvidence {
    endpoint_result(endpoint, true, result)
}

pub fn required_value(endpoint: &str, result: Result<serde_json::Value>) -> EndpointEvidence {
    endpoint_value(endpoint, true, result)
}

fn endpoint_value(
    endpoint: &str,
    required: bool,
    result: Result<serde_json::Value>,
) -> EndpointEvidence {
    match result {
        Ok(payload) => EndpointEvidence {
            endpoint: endpoint.to_string(),
            required,
            ok: true,
            error: None,
            payload: Some(payload),
        },
        Err(error) => EndpointEvidence::error(endpoint, required, format!("{error:#}")),
    }
}

fn endpoint_result<T: Serialize>(
    endpoint: &str,
    required: bool,
    result: Result<T>,
) -> EndpointEvidence {
    match result {
        Ok(payload) => endpoint_value(
            endpoint,
            required,
            serde_json::to_value(payload).map_err(anyhow::Error::from),
        ),
        Err(error) => EndpointEvidence::error(endpoint, required, format!("{error:#}")),
    }
}

pub fn evidence_report(
    capability: CryptoCapability,
    instrument: CryptoInstrument,
    symbol: Option<&str>,
    results: Vec<ProviderEvidence>,
) -> CryptoEvidenceReport {
    CryptoEvidenceReport {
        capability: capability.label().to_string(),
        instrument: instrument.label().to_string(),
        symbol: symbol.map(ToString::to_string),
        fetched_at_utc: crate::http::utc_now(),
        results,
    }
}

pub fn unsupported_provider(
    provider: &str,
    capability: CryptoCapability,
    instrument: CryptoInstrument,
) -> ProviderEvidence {
    provider_from_endpoints(
        provider,
        vec![EndpointEvidence::error(
            capability.label(),
            true,
            format!(
                "provider does not support capability={} instrument={}",
                capability.label(),
                instrument.label()
            ),
        )],
    )
}

pub fn provider_from_endpoints(
    provider: impl Into<String>,
    endpoints: Vec<EndpointEvidence>,
) -> ProviderEvidence {
    let required_endpoints = endpoints
        .iter()
        .filter(|endpoint| endpoint.required)
        .collect::<Vec<_>>();
    let ok = if required_endpoints.is_empty() {
        endpoints.iter().any(|endpoint| endpoint.ok)
    } else {
        required_endpoints.iter().all(|endpoint| endpoint.ok)
    };
    ProviderEvidence {
        provider: provider.into(),
        ok,
        endpoints,
    }
}

#[derive(Debug, Serialize)]
pub struct CryptoEvidenceReport {
    pub capability: String,
    pub instrument: String,
    pub symbol: Option<String>,
    pub fetched_at_utc: String,
    pub results: Vec<ProviderEvidence>,
}

#[derive(Debug, Serialize)]
pub struct ProviderEvidence {
    pub provider: String,
    pub ok: bool,
    pub endpoints: Vec<EndpointEvidence>,
}

#[derive(Debug, Serialize)]
pub struct EndpointEvidence {
    pub endpoint: String,
    pub required: bool,
    pub ok: bool,
    pub error: Option<String>,
    pub payload: Option<serde_json::Value>,
}

impl EndpointEvidence {
    fn error(endpoint: &str, required: bool, error: String) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            required,
            ok: false,
            error: Some(error),
            payload: None,
        }
    }
}
