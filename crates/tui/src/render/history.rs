use agent_finance_market::history_snapshot::HistoryBarSnapshot;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use crate::chart::series::{CandleBucket, compressed_bars, moving_average, vwap};
use crate::mouse_target::MousePosition;
use crate::theme::ThemeConfig;

pub(super) fn chart<'a>(
    bars: &'a [HistoryBarSnapshot],
    theme: &'a ThemeConfig,
    hover: Option<MousePosition>,
) -> CandlestickChart<'a> {
    CandlestickChart { bars, theme, hover }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CandlestickChart<'a> {
    bars: &'a [HistoryBarSnapshot],
    theme: &'a ThemeConfig,
    hover: Option<MousePosition>,
}

impl Widget for CandlestickChart<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.width < 8 || area.height < 4 {
            return;
        }
        let geometry = ChartGeometry::for_area(area);
        let buckets = compressed_bars(self.bars, geometry.bucket_capacity());
        if buckets.is_empty() {
            return;
        }

        let axis_height = 1;
        let volume_height = volume_height(area.height);
        let price_height = area
            .height
            .saturating_sub(volume_height)
            .saturating_sub(axis_height);
        let price_area = Rect {
            height: price_height,
            ..area
        };
        let volume_area = Rect {
            y: area.y + price_height,
            height: volume_height,
            ..area
        };
        let time_area = Rect {
            y: area.y + price_height + volume_height,
            height: axis_height,
            ..area
        };
        let bounds = PriceBounds::from_buckets(&buckets);
        render_current_price_line(buffer, price_area, bounds, &buckets, self.theme);
        render_overlays(buffer, price_area, bounds, &buckets, geometry, self.theme);
        render_candles(buffer, price_area, bounds, &buckets, geometry, self.theme);
        render_volume(buffer, volume_area, &buckets, geometry, self.theme);
        render_price_labels(buffer, price_area, bounds, self.theme);
        render_time_labels(buffer, time_area, &buckets, self.theme);
        render_hover(buffer, area, self.hover, &buckets, geometry, self.theme);
    }
}

#[derive(Debug, Clone, Copy)]
struct ChartGeometry {
    area: Rect,
    candle_width: u16,
}

impl ChartGeometry {
    fn for_area(area: Rect) -> Self {
        let candle_width = if area.width >= 48 { 2 } else { 1 };
        Self { area, candle_width }
    }

    fn bucket_capacity(self) -> usize {
        usize::from(self.area.width / self.candle_width).max(1)
    }

    fn candle_x(self, index: usize) -> Option<u16> {
        let offset = index.checked_mul(usize::from(self.candle_width))?;
        let x = self.area.x.checked_add(offset as u16)?;
        (x < self.area.right()).then_some(x)
    }

    fn wick_x(self, index: usize) -> Option<u16> {
        self.candle_x(index)
    }

    fn body_x(self, index: usize) -> Option<u16> {
        self.candle_x(index).map(|x| {
            if self.candle_width > 1 && x + 1 < self.area.right() {
                x + 1
            } else {
                x
            }
        })
    }

    fn bucket_index_at(self, column: u16) -> Option<usize> {
        if column < self.area.x || column >= self.area.right() {
            return None;
        }
        Some(usize::from((column - self.area.x) / self.candle_width))
    }
}

#[derive(Debug, Clone, Copy)]
struct PriceBounds {
    min: f64,
    max: f64,
}

impl PriceBounds {
    fn from_buckets(buckets: &[CandleBucket]) -> Self {
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

    fn row(self, area: Rect, price: f64) -> u16 {
        if area.height <= 1 || (self.max - self.min).abs() <= f64::EPSILON {
            return area.y;
        }
        let ratio = ((price - self.min) / (self.max - self.min)).clamp(0.0, 1.0);
        area.y + area.height - 1 - (ratio * f64::from(area.height - 1)).round() as u16
    }
}

fn volume_height(height: u16) -> u16 {
    match height {
        0..=7 => 0,
        8..=12 => 2,
        _ => (height / 5).clamp(3, 6),
    }
}

fn render_candles(
    buffer: &mut Buffer,
    area: Rect,
    bounds: PriceBounds,
    buckets: &[CandleBucket],
    geometry: ChartGeometry,
    theme: &ThemeConfig,
) {
    for (index, bucket) in buckets.iter().enumerate() {
        let Some(wick_x) = geometry.wick_x(index) else {
            break;
        };
        let Some(body_x) = geometry.body_x(index) else {
            break;
        };
        let style = candle_style(bucket, theme);
        let high = bounds.row(area, bucket.high);
        let low = bounds.row(area, bucket.low);
        let open = bounds.row(area, bucket.open);
        let close = bounds.row(area, bucket.close);
        for row in high.min(low)..=high.max(low) {
            buffer.set_string(wick_x, row, "│", style);
        }
        if bucket.close_only {
            buffer.set_string(body_x, close, close_only_symbol(geometry), style);
        } else {
            for row in open.min(close)..=open.max(close) {
                buffer.set_string(body_x, row, body_symbol(bucket, geometry), style);
            }
        }
    }
}

fn render_current_price_line(
    buffer: &mut Buffer,
    area: Rect,
    bounds: PriceBounds,
    buckets: &[CandleBucket],
    theme: &ThemeConfig,
) {
    let Some(last) = buckets.last() else {
        return;
    };
    let row = bounds.row(area, last.close);
    for x in area.x..area.x + area.width {
        if buffer[(x, row)].symbol() == " " {
            buffer.set_string(x, row, "·", theme.neutral_style());
        }
    }
}

fn render_overlays(
    buffer: &mut Buffer,
    area: Rect,
    bounds: PriceBounds,
    buckets: &[CandleBucket],
    geometry: ChartGeometry,
    theme: &ThemeConfig,
) {
    render_series(
        buffer,
        area,
        bounds,
        &moving_average(buckets, 20),
        "∙",
        geometry,
        theme.accent_style(),
    );
    render_series(
        buffer,
        area,
        bounds,
        &moving_average(buckets, 50),
        "·",
        geometry,
        theme.warning_style(),
    );
    render_series(
        buffer,
        area,
        bounds,
        &vwap(buckets),
        "×",
        geometry,
        theme.prediction_style(),
    );
}

fn render_series(
    buffer: &mut Buffer,
    area: Rect,
    bounds: PriceBounds,
    series: &[Option<f64>],
    marker: &str,
    geometry: ChartGeometry,
    style: Style,
) {
    for (index, value) in series.iter().enumerate() {
        let Some(value) = value else {
            continue;
        };
        let Some(x) = geometry.body_x(index) else {
            break;
        };
        let row = bounds.row(area, *value);
        buffer.set_string(x, row, marker, style);
    }
}

fn render_volume(
    buffer: &mut Buffer,
    area: Rect,
    buckets: &[CandleBucket],
    geometry: ChartGeometry,
    theme: &ThemeConfig,
) {
    if area.height == 0 {
        return;
    }
    let max_volume = buckets
        .iter()
        .filter_map(|bucket| bucket.volume)
        .fold(0.0, f64::max);
    if max_volume <= 0.0 {
        return;
    }
    for (index, bucket) in buckets.iter().enumerate() {
        let Some(volume) = bucket.volume else {
            continue;
        };
        let Some(x) = geometry.body_x(index) else {
            break;
        };
        let height = ((volume / max_volume) * f64::from(area.height)).ceil() as u16;
        let style = candle_style(bucket, theme);
        for offset in 0..height.max(1).min(area.height) {
            buffer.set_string(x, area.y + area.height - 1 - offset, "█", style);
        }
    }
}

fn render_price_labels(buffer: &mut Buffer, area: Rect, bounds: PriceBounds, theme: &ThemeConfig) {
    let high = format!("{:.2}", bounds.max);
    let low = format!("{:.2}", bounds.min);
    write_right(
        buffer,
        area.x,
        area.y,
        area.width,
        &high,
        theme.muted_style(),
    );
    write_right(
        buffer,
        area.x,
        area.y + area.height.saturating_sub(1),
        area.width,
        &low,
        theme.muted_style(),
    );
}

fn render_time_labels(
    buffer: &mut Buffer,
    area: Rect,
    buckets: &[CandleBucket],
    theme: &ThemeConfig,
) {
    if let Some(first) = buckets.first() {
        write_left_clipped(buffer, area, &first.open_time, theme.muted_style());
    }
    if let Some(last) = buckets.last() {
        write_right_clipped(buffer, area, bucket_end_time(last), theme.muted_style());
    }
}

fn render_hover(
    buffer: &mut Buffer,
    area: Rect,
    hover: Option<MousePosition>,
    buckets: &[CandleBucket],
    geometry: ChartGeometry,
    theme: &ThemeConfig,
) {
    let Some(hover) = hover else {
        return;
    };
    if hover.column < area.x
        || hover.column >= area.right()
        || hover.row < area.y
        || hover.row >= area.bottom()
    {
        return;
    }
    let Some(index) = geometry.bucket_index_at(hover.column) else {
        return;
    };
    let Some(bucket) = buckets.get(index) else {
        return;
    };
    let column = geometry.body_x(index).unwrap_or(hover.column);
    render_crosshair(buffer, area, column, theme);
    render_hover_tooltip(buffer, area, column, bucket, theme);
}

fn render_crosshair(buffer: &mut Buffer, area: Rect, column: u16, theme: &ThemeConfig) {
    for row in area.y..area.bottom() {
        buffer.set_string(column, row, "┊", theme.muted_style());
    }
}

fn render_hover_tooltip(
    buffer: &mut Buffer,
    area: Rect,
    column: u16,
    bucket: &CandleBucket,
    theme: &ThemeConfig,
) {
    if area.width < 24 || area.height < 3 {
        return;
    }
    let volume = bucket
        .volume
        .map(super::widgets::format_volume)
        .unwrap_or_else(|| "-".to_string());
    let lines = [
        format!(
            "{} O{} H{}",
            bucket_time_range(bucket),
            super::widgets::format_price(bucket.open),
            super::widgets::format_price(bucket.high)
        ),
        format!(
            "L{} C{} V{}",
            super::widgets::format_price(bucket.low),
            super::widgets::format_price(bucket.close),
            volume
        ),
    ];
    let width = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or_default()
        .min(area.width as usize);
    let right_start = column.saturating_add(2);
    let axis_reserved = 8;
    let usable_right = area.right().saturating_sub(axis_reserved);
    let x = if right_start + width as u16 <= usable_right {
        right_start
    } else {
        column
            .saturating_sub(width as u16)
            .saturating_sub(2)
            .max(area.x)
    };
    let style = theme.selected_style();
    for (offset, line) in lines.iter().enumerate() {
        let y = area.y + offset as u16;
        let clipped = clipped_prefix(line, width);
        buffer.set_string(x, y, format!("{clipped:<width$}"), style);
    }
}

fn write_right(buffer: &mut Buffer, x: u16, y: u16, width: u16, text: &str, style: Style) {
    write_right_clipped(
        buffer,
        Rect {
            x,
            y,
            width,
            height: 1,
        },
        text,
        style,
    );
}

fn write_left_clipped(buffer: &mut Buffer, area: Rect, text: &str, style: Style) {
    let text = clipped_prefix(text, area.width as usize);
    buffer.set_string(area.x, area.y, text, style);
}

fn write_right_clipped(buffer: &mut Buffer, area: Rect, text: &str, style: Style) {
    let text = clipped_suffix(text, area.width as usize);
    let start = area.x + area.width.saturating_sub(text.chars().count() as u16);
    buffer.set_string(start, area.y, text, style);
}

fn candle_style(bucket: &CandleBucket, theme: &ThemeConfig) -> Style {
    if bucket.close > bucket.open {
        theme.success_style()
    } else if bucket.close < bucket.open {
        theme.danger_style()
    } else {
        theme.neutral_style()
    }
}

fn bucket_time_range(bucket: &CandleBucket) -> String {
    match bucket.close_time.as_deref() {
        Some(close_time) if close_time != bucket.open_time => {
            format!("{}-{}", bucket.open_time, close_time)
        }
        _ => bucket.open_time.clone(),
    }
}

fn bucket_end_time(bucket: &CandleBucket) -> &str {
    bucket.close_time.as_deref().unwrap_or(&bucket.open_time)
}

fn body_symbol(bucket: &CandleBucket, geometry: ChartGeometry) -> &'static str {
    if geometry.candle_width > 1 {
        return "█";
    }
    if bucket.close >= bucket.open {
        "█"
    } else {
        "▓"
    }
}

fn close_only_symbol(geometry: ChartGeometry) -> &'static str {
    if geometry.candle_width > 1 {
        "◆"
    } else {
        "•"
    }
}

fn clipped_prefix(text: &str, max_chars: usize) -> &str {
    if max_chars == 0 {
        return "";
    }
    if text.chars().count() <= max_chars {
        return text;
    }
    text.char_indices()
        .nth(max_chars)
        .map_or(text, |(index, _)| &text[..index])
}

fn clipped_suffix(text: &str, max_chars: usize) -> &str {
    if max_chars == 0 {
        return "";
    }
    let len = text.chars().count();
    if len <= max_chars {
        return text;
    }
    let skip = len - max_chars;
    text.char_indices()
        .nth(skip)
        .map_or(text, |(index, _)| &text[index..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_writes_are_clipped_to_their_rect() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 6, 2));
        write_left_clipped(
            &mut buffer,
            Rect::new(0, 0, 4, 1),
            "2026-06-30T09:30:00+08:00",
            Style::default(),
        );
        write_right_clipped(
            &mut buffer,
            Rect::new(0, 1, 4, 1),
            "2026-06-30T16:00:00+08:00",
            Style::default(),
        );

        assert_eq!(row_text(&buffer, 0), "2026  ");
        assert_eq!(row_text(&buffer, 1), "8:00  ");
    }

    fn row_text(buffer: &Buffer, y: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>()
    }
}
