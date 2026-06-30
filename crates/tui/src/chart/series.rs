use agent_finance_market::history_snapshot::HistoryBarSnapshot;

#[derive(Debug, Clone, PartialEq)]
pub struct CandleBucket {
    pub open_time: String,
    pub close_time: Option<String>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: Option<f64>,
    pub close_only: bool,
}

pub fn compressed_bars(bars: &[HistoryBarSnapshot], max_columns: usize) -> Vec<CandleBucket> {
    let bars = bars
        .iter()
        .filter_map(normalize_bar)
        .collect::<Vec<NormalizedBar>>();
    if bars.is_empty() || max_columns == 0 {
        return Vec::new();
    }
    if bars.len() <= max_columns {
        return bars.into_iter().map(CandleBucket::from).collect();
    }

    let chunk_size = bars.len().div_ceil(max_columns);
    bars.chunks(chunk_size)
        .filter_map(|chunk| {
            let first = chunk.first()?;
            let last = chunk.last()?;
            Some(CandleBucket {
                open_time: first.open_time.clone(),
                close_time: last
                    .close_time
                    .clone()
                    .or_else(|| Some(last.open_time.clone())),
                open: first.open,
                high: chunk
                    .iter()
                    .map(|bar| bar.high)
                    .fold(f64::NEG_INFINITY, f64::max),
                low: chunk
                    .iter()
                    .map(|bar| bar.low)
                    .fold(f64::INFINITY, f64::min),
                close: last.close,
                volume: sum_optional(chunk.iter().map(|bar| bar.volume)),
                close_only: chunk.iter().all(|bar| bar.close_only),
            })
        })
        .collect()
}

pub fn moving_average(buckets: &[CandleBucket], window: usize) -> Vec<Option<f64>> {
    let mut series = Vec::with_capacity(buckets.len());
    let mut sum = 0.0;
    for (index, bucket) in buckets.iter().enumerate() {
        sum += bucket.close;
        if index >= window {
            sum -= buckets[index - window].close;
        }
        if index + 1 >= window {
            series.push(Some(sum / window as f64));
        } else {
            series.push(None);
        }
    }
    series
}

pub fn vwap(buckets: &[CandleBucket]) -> Vec<Option<f64>> {
    let mut price_volume = 0.0;
    let mut volume = 0.0;
    buckets
        .iter()
        .map(|bucket| {
            let bucket_volume = bucket.volume?;
            let typical_price = (bucket.high + bucket.low + bucket.close) / 3.0;
            price_volume += typical_price * bucket_volume;
            volume += bucket_volume;
            (volume > 0.0).then_some(price_volume / volume)
        })
        .collect()
}

#[derive(Debug, Clone)]
struct NormalizedBar {
    open_time: String,
    close_time: Option<String>,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: Option<f64>,
    close_only: bool,
}

impl From<NormalizedBar> for CandleBucket {
    fn from(bar: NormalizedBar) -> Self {
        Self {
            open_time: bar.open_time,
            close_time: bar.close_time,
            open: bar.open,
            high: bar.high,
            low: bar.low,
            close: bar.close,
            volume: bar.volume,
            close_only: bar.close_only,
        }
    }
}

fn normalize_bar(bar: &HistoryBarSnapshot) -> Option<NormalizedBar> {
    if !bar.close.is_finite() {
        return None;
    }
    let open = bar.open.filter(|value| value.is_finite());
    let high = bar.high.filter(|value| value.is_finite());
    let low = bar.low.filter(|value| value.is_finite());
    let close_only = open.is_none() || high.is_none() || low.is_none();
    let open = open.unwrap_or(bar.close);
    let high = high.unwrap_or(open.max(bar.close));
    let low = low.unwrap_or(open.min(bar.close));
    Some(NormalizedBar {
        open_time: bar.open_time.clone(),
        close_time: bar.close_time.clone(),
        open,
        high: high.max(open).max(bar.close),
        low: low.min(open).min(bar.close),
        close: bar.close,
        volume: bar
            .volume
            .filter(|value| value.is_finite() && *value >= 0.0),
        close_only,
    })
}

fn sum_optional(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    let mut total = 0.0;
    let mut seen = false;
    for value in values.flatten() {
        total += value;
        seen = true;
    }
    seen.then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_compression_preserves_ohlcv_semantics() {
        let buckets = compressed_bars(&bars(), 2);

        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets[0].open, 10.0);
        assert_eq!(buckets[0].close_time.as_deref(), Some("2"));
        assert_eq!(buckets[0].high, 15.0);
        assert_eq!(buckets[0].low, 8.0);
        assert_eq!(buckets[0].close, 14.0);
        assert_eq!(buckets[0].volume, Some(300.0));
        assert!(!buckets[0].close_only);
    }

    #[test]
    fn close_only_bars_fallback_without_losing_price() {
        let buckets = compressed_bars(
            &[HistoryBarSnapshot {
                open_time: "t".to_string(),
                close_time: Some("t+1".to_string()),
                open: None,
                high: None,
                low: None,
                close: 42.0,
                volume: None,
                quote_volume: None,
                trades: None,
                repaired: false,
            }],
            10,
        );

        assert_eq!(buckets[0].open, 42.0);
        assert_eq!(buckets[0].close_time.as_deref(), Some("t+1"));
        assert_eq!(buckets[0].high, 42.0);
        assert_eq!(buckets[0].low, 42.0);
        assert!(buckets[0].close_only);
    }

    #[test]
    fn moving_average_and_vwap_compute_expected_values() {
        let buckets = compressed_bars(&bars(), 10);
        assert_eq!(moving_average(&buckets, 2)[0], None);
        assert_eq!(moving_average(&buckets, 2)[1], Some(12.0));

        let vwap = vwap(&buckets);
        assert_near(vwap[0], 10.0);
        assert_near(vwap[1], 11.777777777777779);
        assert_near(vwap[2], 13.222222222222221);
    }

    fn assert_near(actual: Option<f64>, expected: f64) {
        let actual = actual.expect("expected vwap value");
        assert!((actual - expected).abs() < f64::EPSILON * 16.0);
    }

    fn bars() -> Vec<HistoryBarSnapshot> {
        vec![
            bar("1", 10.0, 12.0, 8.0, 10.0, 100.0),
            bar("2", 11.0, 15.0, 9.0, 14.0, 200.0),
            bar("3", 14.0, 16.0, 13.0, 15.0, 300.0),
        ]
    }

    fn bar(
        open_time: &str,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
    ) -> HistoryBarSnapshot {
        HistoryBarSnapshot {
            open_time: open_time.to_string(),
            close_time: Some(open_time.to_string()),
            open: Some(open),
            high: Some(high),
            low: Some(low),
            close,
            volume: Some(volume),
            quote_volume: None,
            trades: None,
            repaired: false,
        }
    }
}
