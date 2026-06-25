use crate::args::HistoryAdjustment;
use crate::model::OhlcBar;

pub fn apply_history_adjustment_and_repair(
    bars: &mut [OhlcBar],
    adjustment: HistoryAdjustment,
    repair: bool,
) -> bool {
    let repair_applied = if repair {
        repair_obvious_100x_errors(bars)
    } else {
        false
    };
    apply_adjustment(bars, adjustment);
    repair_applied
}

fn apply_adjustment(bars: &mut [OhlcBar], adjustment: HistoryAdjustment) {
    if adjustment == HistoryAdjustment::Raw {
        return;
    }
    for bar in bars {
        let Some(adj_close) = bar.adj_close else {
            continue;
        };
        if bar.close == 0.0 {
            continue;
        }
        let ratio = adj_close / bar.close;
        bar.open = bar.open.map(|value| value * ratio);
        bar.high = bar.high.map(|value| value * ratio);
        bar.low = bar.low.map(|value| value * ratio);
        if adjustment == HistoryAdjustment::Auto {
            bar.close = adj_close;
        }
    }
}

fn repair_obvious_100x_errors(bars: &mut [OhlcBar]) -> bool {
    let closes = bars.iter().map(|bar| bar.close).collect::<Vec<_>>();
    let mut repaired = false;
    for (index, bar) in bars.iter_mut().enumerate() {
        let Some(reference) = neighbor_reference(&closes, index) else {
            continue;
        };
        if reference <= 0.0 {
            continue;
        }
        let close = bar.close;
        let factor = if within(close / 100.0, reference) {
            Some(0.01)
        } else if within(close * 100.0, reference) {
            Some(100.0)
        } else {
            None
        };
        let Some(factor) = factor else {
            continue;
        };
        scale_bar(bar, factor);
        bar.repaired = true;
        repaired = true;
    }
    repaired
}

fn neighbor_reference(closes: &[f64], index: usize) -> Option<f64> {
    let previous = (index > 0)
        .then(|| closes[index - 1])
        .filter(|value| *value > 0.0);
    let next = closes.get(index + 1).copied().filter(|value| *value > 0.0);
    match (previous, next) {
        (Some(previous), Some(next)) => Some((previous + next) / 2.0),
        (Some(previous), None) => Some(previous),
        (None, Some(next)) => Some(next),
        (None, None) => None,
    }
}

fn within(value: f64, reference: f64) -> bool {
    value >= reference * 0.8 && value <= reference * 1.2
}

fn scale_bar(bar: &mut OhlcBar, factor: f64) {
    bar.open = bar.open.map(|value| value * factor);
    bar.high = bar.high.map(|value| value * factor);
    bar.low = bar.low.map(|value| value * factor);
    bar.close *= factor;
    bar.adj_close = bar.adj_close.map(|value| value * factor);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(open: f64, high: f64, low: f64, close: f64, adj_close: Option<f64>) -> OhlcBar {
        OhlcBar {
            symbol: "TST".to_string(),
            provider: "fixture".to_string(),
            open_time: "2026-01-01T00:00:00Z".to_string(),
            close_time: None,
            open: Some(open),
            high: Some(high),
            low: Some(low),
            close,
            adj_close,
            volume: Some(100.0),
            quote_volume: None,
            trades: None,
            dividend: None,
            stock_split: None,
            capital_gain: None,
            repaired: false,
        }
    }

    #[test]
    fn auto_adjust_replaces_close_and_scales_ohl() {
        let mut bars = vec![bar(100.0, 110.0, 90.0, 100.0, Some(50.0))];
        let repaired =
            apply_history_adjustment_and_repair(&mut bars, HistoryAdjustment::Auto, false);
        assert!(!repaired);
        assert_eq!(bars[0].open, Some(50.0));
        assert_eq!(bars[0].high, Some(55.0));
        assert_eq!(bars[0].low, Some(45.0));
        assert_eq!(bars[0].close, 50.0);
    }

    #[test]
    fn back_adjust_scales_ohl_but_keeps_raw_close() {
        let mut bars = vec![bar(100.0, 110.0, 90.0, 100.0, Some(50.0))];
        apply_history_adjustment_and_repair(&mut bars, HistoryAdjustment::Back, false);
        assert_eq!(bars[0].open, Some(50.0));
        assert_eq!(bars[0].close, 100.0);
    }

    #[test]
    fn repair_marks_and_scales_isolated_100x_error() {
        let mut bars = vec![
            bar(100.0, 102.0, 98.0, 100.0, Some(100.0)),
            bar(10100.0, 10200.0, 9800.0, 10100.0, Some(10100.0)),
            bar(102.0, 103.0, 99.0, 102.0, Some(102.0)),
        ];
        let repaired = apply_history_adjustment_and_repair(&mut bars, HistoryAdjustment::Raw, true);
        assert!(repaired);
        assert!(bars[1].repaired);
        assert_eq!(bars[1].close, 101.0);
        assert_eq!(bars[1].high, Some(102.0));
    }
}
