use crate::model::{DerivedIndicator, HistoryBatch, OhlcBar};

pub fn compute_indicator(history: &HistoryBatch) -> DerivedIndicator {
    let bars = &history.bars;
    DerivedIndicator {
        symbol: history.symbol.clone(),
        provider: history.provider.clone(),
        latest_close: bars.last().map(|bar| bar.close),
        latest_time: bars.last().map(|bar| bar.open_time.clone()),
        return_1_bar_pct: return_pct(bars, 1),
        return_5_bar_pct: return_pct(bars, 5),
        return_20_bar_pct: return_pct(bars, 20),
        sma_20: sma(bars, 20),
        sma_50: sma(bars, 50),
        high_20: rolling_high(bars, 20),
        low_20: rolling_low(bars, 20),
        realized_vol_20_annualized_pct: realized_vol_annualized_pct(bars, 20),
    }
}

fn return_pct(bars: &[OhlcBar], lookback: usize) -> Option<f64> {
    if bars.len() <= lookback {
        return None;
    }
    let latest = bars.last()?.close;
    let base = bars.get(bars.len() - 1 - lookback)?.close;
    if base == 0.0 {
        None
    } else {
        Some((latest - base) / base * 100.0)
    }
}

fn sma(bars: &[OhlcBar], lookback: usize) -> Option<f64> {
    if bars.len() < lookback {
        return None;
    }
    let values = bars.iter().rev().take(lookback).map(|bar| bar.close);
    Some(values.sum::<f64>() / lookback as f64)
}

fn rolling_high(bars: &[OhlcBar], lookback: usize) -> Option<f64> {
    bars.iter()
        .rev()
        .take(lookback)
        .filter_map(|bar| bar.high.or(Some(bar.close)))
        .reduce(f64::max)
}

fn rolling_low(bars: &[OhlcBar], lookback: usize) -> Option<f64> {
    bars.iter()
        .rev()
        .take(lookback)
        .filter_map(|bar| bar.low.or(Some(bar.close)))
        .reduce(f64::min)
}

fn realized_vol_annualized_pct(bars: &[OhlcBar], lookback: usize) -> Option<f64> {
    if bars.len() <= lookback {
        return None;
    }
    let returns = bars
        .windows(2)
        .rev()
        .take(lookback)
        .filter_map(|window| {
            let previous = window.first()?.close;
            let latest = window.last()?.close;
            (previous > 0.0).then(|| (latest / previous).ln())
        })
        .collect::<Vec<_>>();
    if returns.len() < 2 {
        return None;
    }
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (returns.len() - 1) as f64;
    Some(variance.sqrt() * 252.0_f64.sqrt() * 100.0)
}
