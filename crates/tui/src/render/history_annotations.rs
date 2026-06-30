use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

use crate::chart::series::CandleBucket;
use crate::history_chart::ChartWarning;
use crate::theme::ThemeConfig;

use super::history::{ChartContext, ChartMode};

pub(super) fn render_warning_band(
    buffer: &mut Buffer,
    area: Rect,
    mode: ChartMode,
    warnings: &[ChartWarning],
    theme: &ThemeConfig,
) {
    let Some(first) = warnings.first() else {
        return;
    };
    if area.width < 24 || area.height < 4 {
        return;
    }
    let y = if mode == ChartMode::Workbench && area.width >= 72 && area.height >= 12 {
        area.y + 2
    } else {
        area.y
    };
    let suffix = if warnings.len() > 1 {
        format!(" +{} more", warnings.len() - 1)
    } else {
        String::new()
    };
    let line = format!("warning: {}{suffix}", first.message);
    let width = line.chars().count().min(area.width as usize);
    let text = clipped_prefix(&line, width);
    buffer.set_string(
        area.x,
        y,
        format!("{text:<width$}"),
        theme.warning_style().add_modifier(Modifier::BOLD),
    );
}

pub(super) fn render_crosshair(buffer: &mut Buffer, area: Rect, column: u16, theme: &ThemeConfig) {
    for row in area.y..area.bottom() {
        if buffer[(column, row)].symbol() == " " {
            buffer.set_string(column, row, "┊", theme.muted_style());
        }
    }
}

pub(super) fn render_hover_tooltip(
    buffer: &mut Buffer,
    area: Rect,
    hover_row: u16,
    column: u16,
    bucket: &CandleBucket,
    context: ChartContext<'_>,
    theme: &ThemeConfig,
) {
    if area.width < 24 || area.height < 3 {
        return;
    }
    let lines = hover_tooltip_lines(bucket, context);
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
    let y = tooltip_y(area, hover_row, lines.len() as u16);
    let style = theme.selected_style();
    for (offset, line) in lines.iter().enumerate().take(area.height as usize) {
        let y = y + offset as u16;
        let clipped = clipped_prefix(line, width);
        buffer.set_string(x, y, format!("{clipped:<width$}"), style);
    }
}

fn tooltip_y(area: Rect, hover_row: u16, tooltip_height: u16) -> u16 {
    if tooltip_height >= area.height {
        return area.y;
    }
    let top = area.y;
    let bottom = area.bottom().saturating_sub(tooltip_height);
    if (top..top + tooltip_height).contains(&hover_row) {
        bottom
    } else {
        top
    }
}

fn hover_tooltip_lines(bucket: &CandleBucket, context: ChartContext<'_>) -> [String; 3] {
    let volume = bucket
        .volume
        .map(super::widgets::format_volume)
        .unwrap_or_else(|| "-".to_string());
    let freshness = context.fetched_at.map(freshness_time).unwrap_or("-");
    [
        format!(
            "{} O{} H{}",
            bucket_time_range(bucket),
            super::widgets::format_price(bucket.open),
            super::widgets::format_price(bucket.high)
        ),
        format!(
            "L{} C{} V{} {}",
            super::widgets::format_price(bucket.low),
            super::widgets::format_price(bucket.close),
            volume,
            bucket_change(bucket)
        ),
        format!(
            "{} {} {} {} @{}",
            context.provider, context.session, context.range, context.interval, freshness
        ),
    ]
}

fn bucket_change(bucket: &CandleBucket) -> String {
    if bucket.open.abs() <= f64::EPSILON {
        return "chg -".to_string();
    }
    let value = (bucket.close / bucket.open - 1.0) * 100.0;
    format!("chg {value:+.2}%")
}

fn bucket_time_range(bucket: &CandleBucket) -> String {
    match bucket.close_time.as_deref() {
        Some(close_time) if close_time != bucket.open_time => {
            format!("{}-{}", bucket.open_time, close_time)
        }
        _ => bucket.open_time.clone(),
    }
}

fn freshness_time(value: &str) -> &str {
    if let Some(time) = value
        .split_whitespace()
        .last()
        .filter(|time| *time != value)
    {
        return time;
    }
    let Some((_, time)) = value.split_once('T') else {
        return value;
    };
    let end = time
        .char_indices()
        .find_map(|(index, character)| matches!(character, 'Z' | '+' | '-').then_some(index))
        .unwrap_or(time.len());
    &time[..end]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_chart::ChartWarningKind;

    #[test]
    fn warning_band_summarizes_chart_data_quality() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 48, 6));
        let warnings = vec![
            ChartWarning {
                kind: ChartWarningKind::CloseOnly,
                message: "close-only bars=3".to_string(),
            },
            ChartWarning {
                kind: ChartWarningKind::Provider,
                message: "provider fallback failed".to_string(),
            },
        ];

        render_warning_band(
            &mut buffer,
            Rect::new(0, 0, 48, 6),
            ChartMode::Cockpit,
            &warnings,
            &ThemeConfig::default(),
        );

        assert!(row_text(&buffer, 0).starts_with("warning: close-only bars=3 +1 more"));
        assert!(buffer[(0, 0)].style().add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn warning_band_leaves_small_areas_empty() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 12, 3));
        let warnings = vec![ChartWarning {
            kind: ChartWarningKind::Provider,
            message: "fallback failed".to_string(),
        }];

        render_warning_band(
            &mut buffer,
            Rect::new(0, 0, 12, 3),
            ChartMode::Cockpit,
            &warnings,
            &ThemeConfig::default(),
        );

        assert!(buffer.content().iter().all(|cell| cell.symbol() == " "));
    }

    #[test]
    fn hover_tooltip_includes_change_and_market_context() {
        let bucket = CandleBucket {
            open_time: "09:30".to_string(),
            close_time: Some("09:35".to_string()),
            open: 100.0,
            high: 105.0,
            low: 98.0,
            close: 102.5,
            volume: Some(12_000.0),
            close_only: false,
        };

        let lines = hover_tooltip_lines(
            &bucket,
            ChartContext {
                provider: "yahoo",
                session: "extended",
                interval: "5m",
                range: "5d",
                fetched_at: Some("2026-06-25T09:30:00+08:00"),
            },
        );

        assert!(lines[1].contains("chg +2.50%"));
        assert_eq!(lines[2], "yahoo extended 5d 5m @09:30:00");
    }

    #[test]
    fn hover_tooltip_avoids_the_hovered_top_rows() {
        let bucket = CandleBucket {
            open_time: "09:30".to_string(),
            close_time: Some("09:35".to_string()),
            open: 100.0,
            high: 105.0,
            low: 98.0,
            close: 102.5,
            volume: Some(12_000.0),
            close_only: false,
        };
        let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 8));

        render_hover_tooltip(
            &mut buffer,
            Rect::new(0, 0, 40, 8),
            1,
            1,
            &bucket,
            ChartContext {
                provider: "yahoo",
                session: "extended",
                interval: "5m",
                range: "5d",
                fetched_at: Some("2026-06-25T09:30:00+08:00"),
            },
            &ThemeConfig::default(),
        );

        assert!(row_text(&buffer, 0).trim().is_empty());
        assert!(row_text(&buffer, 5).contains("09:30-09:35"));
    }

    #[test]
    fn freshness_time_handles_space_and_rfc3339_formats() {
        assert_eq!(freshness_time("2026-06-25 09:30:00"), "09:30:00");
        assert_eq!(freshness_time("2026-06-25T09:30:00+08:00"), "09:30:00");
        assert_eq!(freshness_time("2026-06-25T01:30:00Z"), "01:30:00");
    }

    fn row_text(buffer: &Buffer, y: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>()
    }
}
