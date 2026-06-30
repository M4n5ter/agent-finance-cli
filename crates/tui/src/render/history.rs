use agent_finance_market::history_snapshot::HistoryBarSnapshot;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;

use crate::chart::ChartWindow;
use crate::chart::series::{CandleBucket, compressed_bars, moving_average, vwap};
use crate::chart_overlay::{ChartOverlayKind, ChartOverlayLine};
use crate::history_chart::{ChartAreas, PriceBounds, PricePoint, bucket_capacity, visible_bars};
use crate::mouse_target::MousePosition;
use crate::theme::ThemeConfig;

pub(super) fn chart<'a>(
    bars: &'a [HistoryBarSnapshot],
    theme: &'a ThemeConfig,
    hover: Option<MousePosition>,
    mode: ChartMode,
    view: ChartView,
    overlays: &'a [ChartOverlayLine],
) -> CandlestickChart<'a> {
    CandlestickChart {
        bars,
        theme,
        hover,
        mode,
        view,
        overlays,
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CandlestickChart<'a> {
    bars: &'a [HistoryBarSnapshot],
    theme: &'a ThemeConfig,
    hover: Option<MousePosition>,
    mode: ChartMode,
    view: ChartView,
    overlays: &'a [ChartOverlayLine],
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct ChartView {
    pub window: ChartWindow,
    pub cursor_bps: Option<u16>,
    pub selected_overlay_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum ChartMode {
    Cockpit,
    Workbench,
}

impl Widget for CandlestickChart<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.width < 8 || area.height < 4 {
            return;
        }
        let visible = visible_bars(self.bars, self.view.window);
        let buckets = compressed_bars(visible, bucket_capacity(area));
        if buckets.is_empty() {
            return;
        }
        let geometry = ChartGeometry::for_buckets(area, buckets.len());

        let areas = ChartAreas::from(area);
        let bounds = PriceBounds::from_buckets_and_prices(
            &buckets,
            self.overlays.iter().map(|line| line.price),
        );
        render_reference_lines(buffer, areas.price, bounds, &buckets, self.mode, self.theme);
        render_chart_overlay_lines(
            buffer,
            areas.price,
            bounds,
            self.overlays,
            self.view.selected_overlay_index,
            self.theme,
        );
        render_overlays(buffer, areas.price, bounds, &buckets, geometry, self.theme);
        render_candles(buffer, areas.price, bounds, &buckets, geometry, self.theme);
        render_volume(buffer, areas.volume, &buckets, geometry, self.theme);
        render_reference_labels(buffer, areas.price, bounds, &buckets, self.mode, self.theme);
        render_chart_overlay_labels(
            buffer,
            areas.price,
            bounds,
            self.overlays,
            self.view.selected_overlay_index,
            self.mode,
            self.theme,
        );
        render_price_labels(buffer, areas.price, bounds, self.theme);
        render_time_labels(buffer, areas.time, &buckets, self.theme);
        render_workbench_legend(buffer, area, &buckets, self.mode, self.theme);
        render_cursor(
            buffer,
            area,
            self.view.cursor_bps,
            self.view.window,
            &buckets,
            geometry,
            self.theme,
        );
        render_hover(buffer, area, self.hover, &buckets, geometry, self.theme);
    }
}

#[derive(Debug, Clone, Copy)]
struct ChartGeometry {
    area: Rect,
    bucket_count: usize,
    candle_width: u16,
}

impl ChartGeometry {
    fn for_buckets(area: Rect, bucket_count: usize) -> Self {
        let candle_width = if area.width >= 48 { 2 } else { 1 };
        Self {
            area,
            bucket_count: bucket_count.max(1),
            candle_width,
        }
    }

    fn candle_x(self, index: usize) -> Option<u16> {
        if index >= self.bucket_count {
            return None;
        }
        let offset = if self.bucket_count <= 1 || self.area.width <= 1 {
            self.area.width / 2
        } else {
            ((index as u32 * u32::from(self.area.width - 1)) / (self.bucket_count - 1) as u32)
                as u16
        };
        let x = self.area.x.checked_add(offset)?;
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
        if self.bucket_count <= 1 || self.area.width <= 1 {
            return Some(0);
        }
        let offset = u32::from(column - self.area.x);
        let width = u32::from(self.area.width - 1);
        let index = (offset * (self.bucket_count - 1) as u32 + width / 2) / width;
        Some(index as usize)
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
        let high = bounds.point(area, bucket.high);
        let low = bounds.point(area, bucket.low);
        let open = bounds.point(area, bucket.open);
        let close = bounds.point(area, bucket.close);
        render_vertical_segment(buffer, wick_x, high, low, wick_symbol, style);
        if bucket.close_only {
            buffer.set_string(body_x, close.row, close_only_symbol(geometry), style);
        } else {
            render_vertical_segment(buffer, body_x, open, close, body_symbol, style);
        }
    }
}

fn render_vertical_segment(
    buffer: &mut Buffer,
    x: u16,
    start: PricePoint,
    end: PricePoint,
    symbol: fn(u8) -> &'static str,
    style: Style,
) {
    let top_slot = start.slot().min(end.slot());
    let bottom_slot = start.slot().max(end.slot());
    let top_row = start.row.min(end.row);
    let bottom_row = start.row.max(end.row);
    for row in top_row..=bottom_row {
        let row_top = u32::from(row) * 4;
        let mask = (0..4).fold(0u8, |mask, slot| {
            if (top_slot..=bottom_slot).contains(&(row_top + slot)) {
                mask | (1 << slot)
            } else {
                mask
            }
        });
        if mask != 0 {
            buffer.set_string(x, row, symbol(mask), style);
        }
    }
}

fn render_reference_lines(
    buffer: &mut Buffer,
    area: Rect,
    bounds: PriceBounds,
    buckets: &[CandleBucket],
    mode: ChartMode,
    theme: &ThemeConfig,
) {
    let Some(last) = buckets.last() else {
        return;
    };
    render_horizontal_reference_line(
        buffer,
        area,
        bounds.row(area, last.close),
        theme.neutral_style(),
    );
    if mode == ChartMode::Workbench
        && let Some(first) = buckets.first()
    {
        render_horizontal_reference_line(
            buffer,
            area,
            bounds.row(area, first.open),
            theme.accent_style(),
        );
        let high = buckets
            .iter()
            .max_by(|left, right| left.high.total_cmp(&right.high))
            .expect("buckets are non-empty");
        let low = buckets
            .iter()
            .min_by(|left, right| left.low.total_cmp(&right.low))
            .expect("buckets are non-empty");
        render_horizontal_reference_line(
            buffer,
            area,
            bounds.row(area, high.high),
            theme.success_style(),
        );
        render_horizontal_reference_line(
            buffer,
            area,
            bounds.row(area, low.low),
            theme.danger_style(),
        );
    }
}

fn render_horizontal_reference_line(buffer: &mut Buffer, area: Rect, row: u16, style: Style) {
    for x in area.x..area.x + area.width {
        if buffer[(x, row)].symbol() == " " {
            buffer.set_string(x, row, "·", style);
        }
    }
}

fn render_reference_labels(
    buffer: &mut Buffer,
    area: Rect,
    bounds: PriceBounds,
    buckets: &[CandleBucket],
    mode: ChartMode,
    theme: &ThemeConfig,
) {
    if mode != ChartMode::Workbench || area.width < 18 {
        return;
    }
    let Some(first) = buckets.first() else {
        return;
    };
    let Some(last) = buckets.last() else {
        return;
    };
    write_right_clipped(
        buffer,
        Rect {
            y: bounds.row(area, last.close),
            ..area
        },
        "last",
        theme.muted_style(),
    );
    write_right_clipped(
        buffer,
        Rect {
            y: bounds.row(area, first.open),
            ..area
        },
        "open",
        theme.muted_style(),
    );
    if let Some(high) = buckets
        .iter()
        .max_by(|left, right| left.high.total_cmp(&right.high))
    {
        write_right_clipped(
            buffer,
            Rect {
                y: bounds.row(area, high.high),
                ..area
            },
            "high",
            theme.muted_style(),
        );
    }
    if let Some(low) = buckets
        .iter()
        .min_by(|left, right| left.low.total_cmp(&right.low))
    {
        write_right_clipped(
            buffer,
            Rect {
                y: bounds.row(area, low.low),
                ..area
            },
            "low",
            theme.muted_style(),
        );
    }
}

fn render_chart_overlay_lines(
    buffer: &mut Buffer,
    area: Rect,
    bounds: PriceBounds,
    overlays: &[ChartOverlayLine],
    selected_overlay_index: Option<usize>,
    theme: &ThemeConfig,
) {
    for (index, overlay) in overlays.iter().enumerate() {
        let selected = selected_overlay_index == Some(index);
        render_horizontal_reference_line(
            buffer,
            area,
            bounds.row(area, overlay.price),
            overlay_style(overlay.kind, selected, theme),
        );
    }
}

fn render_chart_overlay_labels(
    buffer: &mut Buffer,
    area: Rect,
    bounds: PriceBounds,
    overlays: &[ChartOverlayLine],
    selected_overlay_index: Option<usize>,
    mode: ChartMode,
    theme: &ThemeConfig,
) {
    if mode != ChartMode::Workbench || area.width < 32 {
        return;
    }
    for (index, overlay) in overlays.iter().take(8).enumerate() {
        let selected = selected_overlay_index == Some(index);
        let label = format!(
            "{}{} {}",
            if selected { "> " } else { "" },
            overlay.label,
            super::widgets::format_price(overlay.price)
        );
        write_left_clipped(
            buffer,
            Rect {
                y: bounds.row(area, overlay.price),
                ..area
            },
            &label,
            overlay_style(overlay.kind, selected, theme),
        );
    }
}

fn overlay_style(kind: ChartOverlayKind, selected: bool, theme: &ThemeConfig) -> Style {
    let style = match kind {
        ChartOverlayKind::Current => theme.accent_style(),
        ChartOverlayKind::PreviousClose
        | ChartOverlayKind::DayOpen
        | ChartOverlayKind::DayHigh
        | ChartOverlayKind::DayLow => theme.muted_style(),
        ChartOverlayKind::BuyOrder | ChartOverlayKind::LongPosition => theme.success_style(),
        ChartOverlayKind::SellOrder | ChartOverlayKind::ShortPosition => theme.danger_style(),
    };
    if selected {
        style.add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        style
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
    let high = format!("{:.2}", bounds.max());
    let low = format!("{:.2}", bounds.min());
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

fn render_workbench_legend(
    buffer: &mut Buffer,
    area: Rect,
    buckets: &[CandleBucket],
    mode: ChartMode,
    theme: &ThemeConfig,
) {
    if mode != ChartMode::Workbench || area.width < 72 || area.height < 12 {
        return;
    }
    let Some(first) = buckets.first() else {
        return;
    };
    let Some(last) = buckets.last() else {
        return;
    };
    let high = buckets
        .iter()
        .map(|bucket| bucket.high)
        .fold(f64::NEG_INFINITY, f64::max);
    let low = buckets
        .iter()
        .map(|bucket| bucket.low)
        .fold(f64::INFINITY, f64::min);
    let volume = buckets
        .iter()
        .filter_map(|bucket| bucket.volume)
        .sum::<f64>();
    let change = if first.open.abs() > f64::EPSILON {
        Some((last.close / first.open - 1.0) * 100.0)
    } else {
        None
    };
    let lines = [
        format!(
            "O {}  H {}  L {}  C {}",
            super::widgets::format_price(first.open),
            super::widgets::format_price(high),
            super::widgets::format_price(low),
            super::widgets::format_price(last.close)
        ),
        format!(
            "change {}  volume {}  overlays MA20 MA50 VWAP",
            change
                .map(|value| format!("{value:+.2}%"))
                .unwrap_or_else(|| "-".to_string()),
            super::widgets::format_volume(volume)
        ),
    ];
    for (offset, line) in lines.into_iter().enumerate() {
        let width = line.chars().count().min(area.width as usize);
        let text = clipped_prefix(&line, width);
        buffer.set_string(
            area.x,
            area.y + offset as u16,
            format!("{text:<width$}"),
            theme.selected_style(),
        );
    }
}

fn render_cursor(
    buffer: &mut Buffer,
    area: Rect,
    cursor_bps: Option<u16>,
    window: ChartWindow,
    buckets: &[CandleBucket],
    geometry: ChartGeometry,
    theme: &ThemeConfig,
) {
    let Some(cursor_bps) = cursor_bps else {
        return;
    };
    let Some(index) = window.cursor_bucket_index(cursor_bps, buckets.len()) else {
        return;
    };
    let Some(column) = geometry.body_x(index) else {
        return;
    };
    for row in area.y..area.bottom() {
        buffer.set_string(column, row, "┃", theme.accent_style());
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
        if buffer[(column, row)].symbol() == " " {
            buffer.set_string(column, row, "┊", theme.muted_style());
        }
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

fn body_symbol(mask: u8) -> &'static str {
    match mask {
        0b0001 => "▔",
        0b0010 | 0b0100 | 0b0110 => "━",
        0b1000 => "▁",
        0b0011 | 0b0111 => "▀",
        0b1100 | 0b1110 => "▄",
        0b1111 => "█",
        _ => "█",
    }
}

fn wick_symbol(mask: u8) -> &'static str {
    const SYMBOLS: [&str; 16] = [
        " ", "⠁", "⠂", "⠃", "⠄", "⠅", "⠆", "⠇", "⡀", "⡁", "⡂", "⡃", "⡄", "⡅", "⡆", "⡇",
    ];
    SYMBOLS[usize::from(mask & 0b1111)]
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

    #[test]
    fn candle_renderer_separates_spike_wick_from_small_body() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 4));
        let area = Rect::new(0, 0, 4, 4);
        let bucket = CandleBucket {
            open_time: "09:30".to_string(),
            close_time: Some("09:35".to_string()),
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 100.5,
            volume: Some(1_000.0),
            close_only: false,
        };
        let geometry = ChartGeometry {
            area,
            bucket_count: 1,
            candle_width: 2,
        };

        render_candles(
            &mut buffer,
            area,
            PriceBounds::new(90.0, 110.0),
            &[bucket],
            geometry,
            &ThemeConfig::default(),
        );

        assert_eq!(buffer[(2, 0)].symbol(), "⡇");
        assert_eq!(buffer[(2, 1)].symbol(), "⡇");
        assert_eq!(buffer[(2, 2)].symbol(), "⡇");
        assert_eq!(buffer[(2, 3)].symbol(), "⡇");
        assert_eq!(buffer[(3, 1)].symbol(), "▁");
        assert_eq!(buffer[(3, 2)].symbol(), "▔");
    }

    #[test]
    fn hover_crosshair_does_not_erase_existing_chart_glyphs() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 3, 3));
        buffer.set_string(1, 0, "⡇", Style::default());
        buffer.set_string(1, 1, "█", Style::default());

        render_crosshair(
            &mut buffer,
            Rect::new(0, 0, 3, 3),
            1,
            &ThemeConfig::default(),
        );

        assert_eq!(buffer[(1, 0)].symbol(), "⡇");
        assert_eq!(buffer[(1, 1)].symbol(), "█");
        assert_eq!(buffer[(1, 2)].symbol(), "┊");
    }

    #[test]
    fn selected_overlay_label_is_visible_in_workbench() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 5));
        let area = Rect::new(0, 0, 40, 5);
        let overlays = vec![
            ChartOverlayLine {
                price: 100.0,
                label: "cur".to_string(),
                kind: ChartOverlayKind::Current,
            },
            ChartOverlayLine {
                price: 95.0,
                label: "buy order".to_string(),
                kind: ChartOverlayKind::BuyOrder,
            },
        ];

        render_chart_overlay_labels(
            &mut buffer,
            area,
            PriceBounds::new(90.0, 110.0),
            &overlays,
            Some(0),
            ChartMode::Workbench,
            &ThemeConfig::default(),
        );

        assert!(row_text(&buffer, 2).starts_with("> cur"));
        assert!(
            buffer[(0, 2)]
                .style()
                .add_modifier
                .contains(Modifier::REVERSED)
        );
    }

    fn row_text(buffer: &Buffer, y: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>()
    }
}
