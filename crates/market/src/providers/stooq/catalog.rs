use std::path::PathBuf;

use anyhow::Result;

use crate::args::{StooqAsset, StooqFrequency, StooqMarket};
use crate::cache;
use crate::http::utc_now;
use crate::model::{StooqCatalog, StooqCatalogEntry};

pub fn catalog() -> StooqCatalog {
    StooqCatalog {
        fetched_at_utc: utc_now(),
        source_url: "https://stooq.com/db/h/".to_string(),
        entries: catalog_entries()
            .into_iter()
            .map(|mut entry| {
                entry.cached_zip_path = cached_zip_path(&entry.cache_key)
                    .ok()
                    .filter(|path| path.exists())
                    .map(|path| path.display().to_string());
                entry
            })
            .collect(),
    }
}

pub(super) fn catalog_entries() -> Vec<StooqCatalogEntry> {
    PACKAGES
        .iter()
        .map(|package| StooqCatalogEntry {
            frequency: package.frequency.label().to_string(),
            market: package.market.label().to_string(),
            asset: package.asset.label().to_string(),
            label: package.label.to_string(),
            approx_size_mb: package.approx_size_mb,
            listing_url: "https://stooq.com/db/h/".to_string(),
            direct_download_requires_captcha: true,
            cache_key: package.cache_key(),
            cached_zip_path: None,
        })
        .collect()
}

#[derive(Clone, Copy)]
pub(super) struct StooqPackage {
    pub(super) frequency: StooqFrequency,
    pub(super) market: StooqMarket,
    pub(super) asset: StooqAsset,
    pub(super) label: &'static str,
    pub(super) approx_size_mb: Option<f64>,
}

impl StooqPackage {
    pub(super) fn cache_key(self) -> String {
        stooq_cache_key(self.frequency, self.market, self.asset)
    }
}

const PACKAGES: &[StooqPackage] = &[
    StooqPackage {
        frequency: StooqFrequency::Daily,
        market: StooqMarket::Us,
        asset: StooqAsset::Stocks,
        label: "U.S. stocks daily",
        approx_size_mb: Some(509.0),
    },
    StooqPackage {
        frequency: StooqFrequency::Daily,
        market: StooqMarket::Us,
        asset: StooqAsset::Etfs,
        label: "U.S. ETFs daily",
        approx_size_mb: Some(509.0),
    },
    StooqPackage {
        frequency: StooqFrequency::Hourly,
        market: StooqMarket::Us,
        asset: StooqAsset::Stocks,
        label: "U.S. stocks hourly",
        approx_size_mb: Some(426.0),
    },
    StooqPackage {
        frequency: StooqFrequency::Hourly,
        market: StooqMarket::Us,
        asset: StooqAsset::Etfs,
        label: "U.S. ETFs hourly",
        approx_size_mb: Some(426.0),
    },
    StooqPackage {
        frequency: StooqFrequency::FiveMin,
        market: StooqMarket::Us,
        asset: StooqAsset::Stocks,
        label: "U.S. stocks 5 minute",
        approx_size_mb: Some(597.0),
    },
    StooqPackage {
        frequency: StooqFrequency::FiveMin,
        market: StooqMarket::Us,
        asset: StooqAsset::Etfs,
        label: "U.S. ETFs 5 minute",
        approx_size_mb: Some(597.0),
    },
    StooqPackage {
        frequency: StooqFrequency::Daily,
        market: StooqMarket::World,
        asset: StooqAsset::Currencies,
        label: "World currencies daily",
        approx_size_mb: Some(182.0),
    },
    StooqPackage {
        frequency: StooqFrequency::Daily,
        market: StooqMarket::World,
        asset: StooqAsset::Crypto,
        label: "World crypto daily",
        approx_size_mb: Some(182.0),
    },
    StooqPackage {
        frequency: StooqFrequency::Hourly,
        market: StooqMarket::World,
        asset: StooqAsset::Currencies,
        label: "World currencies hourly",
        approx_size_mb: Some(249.0),
    },
    StooqPackage {
        frequency: StooqFrequency::FiveMin,
        market: StooqMarket::World,
        asset: StooqAsset::Currencies,
        label: "World currencies 5 minute",
        approx_size_mb: Some(467.0),
    },
    StooqPackage {
        frequency: StooqFrequency::Daily,
        market: StooqMarket::Macro,
        asset: StooqAsset::Macro,
        label: "Macro daily",
        approx_size_mb: Some(0.9),
    },
];

pub(super) fn catalog_package(
    frequency: StooqFrequency,
    market: StooqMarket,
    asset: StooqAsset,
) -> Option<StooqPackage> {
    PACKAGES.iter().copied().find(|package| {
        package.frequency == frequency && package.market == market && package.asset == asset
    })
}

fn stooq_cache_key(frequency: StooqFrequency, market: StooqMarket, asset: StooqAsset) -> String {
    format!("{}_{}_{}", frequency.label(), market.label(), asset.label())
}

pub(super) fn cached_zip_path(cache_key: &str) -> Result<PathBuf> {
    Ok(cache_root()?.join(format!("{cache_key}.zip")))
}

fn cache_root() -> Result<PathBuf> {
    Ok(cache::agent_finance_cache_root()?.join("stooq-bulk"))
}
