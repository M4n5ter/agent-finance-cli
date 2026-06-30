use agent_finance_market::history_snapshot::HistoryBarSnapshot;
use ratatui::layout::Rect;

use crate::chart::ChartWindow;
use crate::chart::series::{CandleBucket, compressed_bars};

#[derive(Debug, Clone, Copy)]
pub(crate) struct ChartAreas {
    pub price: Rect,
    pub volume: Rect,
    pub time: Rect,
}

impl From<Rect> for ChartAreas {
    fn from(area: Rect) -> Self {
        let axis_height = 1;
        let volume_height = volume_height(area.height);
        let price_height = area
            .height
            .saturating_sub(volume_height)
            .saturating_sub(axis_height);
        let price = Rect {
            height: price_height,
            ..area
        };
        let volume = Rect {
            y: area.y + price_height,
            height: volume_height,
            ..area
        };
        let time = Rect {
            y: area.y + price_height + volume_height,
            height: axis_height,
            ..area
        };
        Self {
            price,
            volume,
            time,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PriceBounds {
    min: f64,
    max: f64,
}

impl PriceBounds {
    #[cfg(test)]
    pub(crate) const fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    pub(crate) fn from_buckets(buckets: &[CandleBucket]) -> Self {
        let (min, max) = buckets
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), bucket| {
                (min.min(bucket.low), max.max(bucket.high))
            });
        let scale = min.abs().max(max.abs()).max(f64::MIN_POSITIVE);
        let padding = ((max - min).abs() * 0.05).max(scale * 0.001);
        Self {
            min: min - padding,
            max: max + padding,
        }
    }

    pub(crate) fn row(self, area: Rect, price: f64) -> u16 {
        self.point(area, price).row
    }

    pub(crate) fn min(self) -> f64 {
        self.min
    }

    pub(crate) fn max(self) -> f64 {
        self.max
    }

    pub(crate) fn price_at_row(self, area: Rect, row: u16) -> f64 {
        if area.height <= 1 || (self.max - self.min).abs() <= f64::EPSILON {
            return (self.min + self.max) / 2.0;
        }
        let local_row = row.saturating_sub(area.y).min(area.height - 1);
        let slots = u32::from(area.height) * 4;
        let slot = u32::from(local_row) * 4 + 2;
        let ratio = 1.0 - (f64::from(slot) / f64::from(slots.saturating_sub(1))).clamp(0.0, 1.0);
        self.min + ratio * (self.max - self.min)
    }

    pub(crate) fn point(self, area: Rect, price: f64) -> PricePoint {
        if area.height <= 1 || (self.max - self.min).abs() <= f64::EPSILON {
            return PricePoint {
                row: area.y,
                cell_slot: 0,
            };
        }
        let ratio = ((price - self.min) / (self.max - self.min)).clamp(0.0, 1.0);
        let slots = u32::from(area.height) * 4;
        let slot = ((1.0 - ratio) * f64::from(slots.saturating_sub(1))).round() as u32;
        PricePoint {
            row: area.y + (slot / 4) as u16,
            cell_slot: (slot % 4) as u8,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct PricePoint {
    pub row: u16,
    cell_slot: u8,
}

impl PricePoint {
    pub(crate) fn slot(self) -> u32 {
        u32::from(self.row) * 4 + u32::from(self.cell_slot)
    }
}

pub(crate) fn chart_price_at_row(
    bars: &[HistoryBarSnapshot],
    window: ChartWindow,
    area: Rect,
    row: u16,
) -> Option<f64> {
    if area.width < 8 || area.height < 4 {
        return None;
    }
    let areas = ChartAreas::from(area);
    if row < areas.price.y || row >= areas.price.bottom() {
        return None;
    }
    let visible = visible_bars(bars, window);
    let buckets = compressed_bars(visible, bucket_capacity(area));
    (!buckets.is_empty())
        .then(|| PriceBounds::from_buckets(&buckets).price_at_row(areas.price, row))
}

pub(crate) fn chart_bps_at_column(area: Rect, window: ChartWindow, column: u16) -> Option<u16> {
    if column < area.x || column >= area.right() {
        return None;
    }
    if area.width <= 1 {
        return Some(window.start_bps());
    }
    let local_column = column.saturating_sub(area.x);
    let span = u32::from(window.end_bps().saturating_sub(window.start_bps()));
    let relative = (u32::from(local_column) * span) / u32::from(area.width - 1);
    Some(
        window
            .start_bps()
            .saturating_add(relative as u16)
            .min(10_000),
    )
}

pub(crate) fn visible_bars(
    bars: &[HistoryBarSnapshot],
    window: ChartWindow,
) -> &[HistoryBarSnapshot] {
    let range = window.visible_range(bars.len());
    &bars[range]
}

pub(crate) fn bucket_capacity(area: Rect) -> usize {
    if area.width >= 48 {
        usize::from(area.width / 2).max(1)
    } else {
        usize::from(area.width).max(1)
    }
}

pub(crate) fn volume_height(height: u16) -> u16 {
    match height {
        0..=7 => 0,
        8..=12 => 2,
        _ => (height / 5).clamp(3, 6),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_price_hit_test_maps_rows_to_visible_price_range() {
        let bars = [HistoryBarSnapshot {
            open_time: "09:30".to_string(),
            close_time: Some("09:35".to_string()),
            open: Some(100.0),
            high: Some(110.0),
            low: Some(90.0),
            close: 101.0,
            volume: Some(1_000.0),
            quote_volume: None,
            trades: None,
            repaired: false,
        }];
        let area = Rect::new(0, 0, 80, 20);

        let top =
            chart_price_at_row(&bars, ChartWindow::FULL, area, 0).expect("top row maps to price");
        let bottom =
            chart_price_at_row(&bars, ChartWindow::FULL, area, 14).expect("bottom maps to price");

        assert!(top > bottom);
        assert!(top <= 111.0);
        assert!(bottom >= 89.0);
        assert_eq!(chart_price_at_row(&bars, ChartWindow::FULL, area, 19), None);
    }

    #[test]
    fn chart_column_hit_test_maps_to_active_window_bps() {
        let area = Rect::new(10, 0, 101, 20);

        assert_eq!(chart_bps_at_column(area, ChartWindow::FULL, 10), Some(0));
        assert_eq!(
            chart_bps_at_column(area, ChartWindow::FULL, 60),
            Some(5_000)
        );
        assert_eq!(
            chart_bps_at_column(area, ChartWindow::FULL, 110),
            Some(10_000)
        );
        assert_eq!(chart_bps_at_column(area, ChartWindow::FULL, 111), None);
    }
}
