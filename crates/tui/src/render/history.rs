use agent_finance_market::history_snapshot::HistoryBarSnapshot;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;

use crate::chart::ChartWindow;
use crate::chart::series::{CandleBucket, compressed_bars, moving_average, vwap};
use crate::chart_overlay::{ChartOverlayKind, ChartOverlayLine};
use crate::history_chart::{
    CandleLayout, ChartAreas, ChartWarning, PriceBounds, bucket_capacity, visible_bars,
};
use crate::mouse_target::MousePosition;
use crate::theme::ThemeConfig;

use super::history_annotations::render_warning_band;
use super::history_annotations::{render_crosshair, render_hover_tooltip};
use super::history_glyphs::{
    CandleShape, render_close_only_candle, render_dense_candle, render_split_candle, volume_symbol,
};

pub(super) fn chart<'a>(props: ChartProps<'a>) -> CandlestickChart<'a> {
    CandlestickChart { props }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ChartProps<'a> {
    pub bars: &'a [HistoryBarSnapshot],
    pub context: ChartContext<'a>,
    pub theme: &'a ThemeConfig,
    pub hover: Option<MousePosition>,
    pub mode: ChartMode,
    pub view: ChartView,
    pub overlays: &'a [ChartOverlayLine],
    pub warnings: &'a [ChartWarning],
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CandlestickChart<'a> {
    props: ChartProps<'a>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct ChartContext<'a> {
    pub provider: &'a str,
    pub session: &'a str,
    pub interval: &'a str,
    pub range: &'a str,
    pub fetched_at: Option<&'a str>,
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
        let props = self.props;
        let visible = visible_bars(props.bars, props.view.window);
        let buckets = compressed_bars(visible, bucket_capacity(area));
        if buckets.is_empty() {
            return;
        }
        let geometry = ChartGeometry::for_buckets(area, buckets.len());

        let areas = ChartAreas::from(area);
        let bounds = PriceBounds::from_buckets_and_prices(
            &buckets,
            props.overlays.iter().map(|line| line.price),
        );
        render_reference_lines(
            buffer,
            areas.price,
            bounds,
            &buckets,
            props.mode,
            props.theme,
        );
        render_chart_overlay_lines(
            buffer,
            areas.price,
            bounds,
            props.overlays,
            props.view.selected_overlay_index,
            props.theme,
        );
        render_overlays(buffer, areas.price, bounds, &buckets, geometry, props.theme);
        render_candles(buffer, areas.price, bounds, &buckets, geometry, props.theme);
        render_volume(buffer, areas.volume, &buckets, geometry, props.theme);
        render_reference_labels(
            buffer,
            areas.price,
            bounds,
            &buckets,
            props.mode,
            props.theme,
        );
        render_chart_overlay_labels(
            buffer,
            areas.price,
            bounds,
            props.overlays,
            props.view.selected_overlay_index,
            props.mode,
            props.theme,
        );
        render_price_labels(buffer, areas.price, bounds, props.theme);
        render_time_labels(buffer, areas.time, &buckets, props.theme);
        render_workbench_legend(buffer, area, &buckets, props.mode, props.theme);
        render_cursor(
            buffer,
            area,
            props.view.cursor_bps,
            props.view.window,
            &buckets,
            geometry,
            props.theme,
        );
        render_warning_band(buffer, area, props.mode, props.warnings, props.theme);
        render_hover(
            buffer,
            area,
            props.hover,
            &buckets,
            geometry,
            props.context,
            props.theme,
        );
    }
}

#[derive(Debug, Clone, Copy)]
struct ChartGeometry {
    area: Rect,
    bucket_count: usize,
    candle_layout: CandleLayout,
}

impl ChartGeometry {
    fn for_buckets(area: Rect, bucket_count: usize) -> Self {
        Self {
            area,
            bucket_count: bucket_count.max(1),
            candle_layout: CandleLayout::for_area(area),
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
            if self.candle_layout == CandleLayout::Split && x + 1 < self.area.right() {
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
        let shape = CandleShape {
            high,
            low,
            open,
            close,
        };
        if bucket.close_only {
            render_close_only_candle(
                buffer,
                wick_x,
                body_x,
                shape,
                geometry.candle_layout.candle_width(),
                style,
            );
        } else if geometry.candle_layout == CandleLayout::Dense {
            render_dense_candle(buffer, body_x, shape, style);
        } else {
            render_split_candle(buffer, wick_x, body_x, shape, style);
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
    if !geometry.candle_layout.shows_overlays() {
        return;
    }
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
        let total_slots = u32::from(area.height) * 8;
        let height = ((volume / max_volume) * f64::from(total_slots)).ceil() as u32;
        let height = height.max(1).min(total_slots);
        let style = candle_style(bucket, theme);
        let full_rows = height / 8;
        let partial = (height % 8) as u8;
        for offset in 0..full_rows.min(u32::from(area.height)) {
            buffer.set_string(x, area.y + area.height - 1 - offset as u16, "█", style);
        }
        if partial > 0 && full_rows < u32::from(area.height) {
            let row = area.y + area.height - 1 - full_rows as u16;
            buffer.set_string(x, row, volume_symbol(partial), style);
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
    context: ChartContext<'_>,
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
    render_hover_tooltip(
        buffer,
        ChartAreas::from(area).price,
        hover.row,
        column,
        bucket,
        context,
        theme,
    );
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

fn bucket_end_time(bucket: &CandleBucket) -> &str {
    bucket.close_time.as_deref().unwrap_or(&bucket.open_time)
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
            candle_layout: CandleLayout::Split,
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
    fn dense_candle_renderer_combines_wick_and_body_in_one_cell() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 4));
        let area = Rect::new(0, 0, 4, 4);
        let bucket = CandleBucket {
            open_time: "09:30".to_string(),
            close_time: Some("09:35".to_string()),
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 105.0,
            volume: Some(1_000.0),
            close_only: false,
        };
        let geometry = ChartGeometry {
            area,
            bucket_count: 1,
            candle_layout: CandleLayout::Dense,
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
        assert_eq!(buffer[(2, 1)].symbol(), "⣿");
        assert_eq!(buffer[(2, 2)].symbol(), "⡏");
        assert_eq!(buffer[(2, 3)].symbol(), "⡇");
    }

    #[test]
    fn close_only_candle_renderer_keeps_intrabar_range_visible() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 4));
        let area = Rect::new(0, 0, 4, 4);
        let bucket = CandleBucket {
            open_time: "09:30".to_string(),
            close_time: Some("09:35".to_string()),
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 100.0,
            volume: Some(1_000.0),
            close_only: true,
        };
        let geometry = ChartGeometry {
            area,
            bucket_count: 1,
            candle_layout: CandleLayout::Split,
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
        assert_eq!(buffer[(2, 3)].symbol(), "⡇");
        assert_eq!(buffer[(3, 2)].symbol(), "◆");
    }

    #[test]
    fn volume_renderer_preserves_sub_cell_height_precision() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 4));
        let area = Rect::new(0, 0, 4, 4);
        let geometry = ChartGeometry {
            area,
            bucket_count: 2,
            candle_layout: CandleLayout::Dense,
        };

        render_volume(
            &mut buffer,
            area,
            &[
                CandleBucket {
                    open_time: "09:30".to_string(),
                    close_time: Some("09:35".to_string()),
                    open: 100.0,
                    high: 100.0,
                    low: 100.0,
                    close: 100.0,
                    volume: Some(12.5),
                    close_only: false,
                },
                CandleBucket {
                    open_time: "09:35".to_string(),
                    close_time: Some("09:40".to_string()),
                    open: 101.0,
                    high: 101.0,
                    low: 101.0,
                    close: 101.0,
                    volume: Some(100.0),
                    close_only: false,
                },
            ],
            geometry,
            &ThemeConfig::default(),
        );

        assert_eq!(buffer[(0, 3)].symbol(), "▄");
        assert_eq!(buffer[(3, 0)].symbol(), "█");
        assert_eq!(buffer[(3, 1)].symbol(), "█");
        assert_eq!(buffer[(3, 2)].symbol(), "█");
        assert_eq!(buffer[(3, 3)].symbol(), "█");
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
