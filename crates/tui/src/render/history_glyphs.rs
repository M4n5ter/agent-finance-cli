use ratatui::buffer::Buffer;
use ratatui::style::Style;

use crate::history_chart::PricePoint;

#[derive(Debug, Clone, Copy)]
pub(super) struct CandleShape {
    pub high: PricePoint,
    pub low: PricePoint,
    pub open: PricePoint,
    pub close: PricePoint,
}

pub(super) fn render_split_candle(
    buffer: &mut Buffer,
    wick_x: u16,
    body_x: u16,
    shape: CandleShape,
    style: Style,
) {
    render_vertical_segment(buffer, wick_x, shape.high, shape.low, wick_symbol, style);
    render_vertical_segment(buffer, body_x, shape.open, shape.close, body_symbol, style);
}

pub(super) fn render_dense_candle(buffer: &mut Buffer, x: u16, shape: CandleShape, style: Style) {
    let wick = SegmentSlots::between(shape.high, shape.low);
    let body = SegmentSlots::between(shape.open, shape.close);
    let top_row = shape
        .high
        .row
        .min(shape.low.row)
        .min(shape.open.row)
        .min(shape.close.row);
    let bottom_row = shape
        .high
        .row
        .max(shape.low.row)
        .max(shape.open.row)
        .max(shape.close.row);
    for row in top_row..=bottom_row {
        let mask = braille_mask(row, wick, body);
        if mask != 0 {
            buffer.set_string(x, row, braille_symbol(mask), style);
        }
    }
}

pub(super) fn render_close_only_candle(
    buffer: &mut Buffer,
    wick_x: u16,
    marker_x: u16,
    shape: CandleShape,
    candle_width: u16,
    style: Style,
) {
    render_vertical_segment(buffer, wick_x, shape.high, shape.low, wick_symbol, style);
    buffer.set_string(
        marker_x,
        shape.close.row,
        close_only_symbol(candle_width),
        style,
    );
}

pub(super) fn close_only_symbol(candle_width: u16) -> &'static str {
    if candle_width > 1 { "◆" } else { "•" }
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

#[derive(Debug, Clone, Copy)]
struct SegmentSlots {
    top: u32,
    bottom: u32,
}

impl SegmentSlots {
    fn between(start: PricePoint, end: PricePoint) -> Self {
        let start = start.slot();
        let end = end.slot();
        Self {
            top: start.min(end),
            bottom: start.max(end),
        }
    }

    fn contains(self, slot: u32) -> bool {
        (self.top..=self.bottom).contains(&slot)
    }
}

fn braille_mask(row: u16, wick: SegmentSlots, body: SegmentSlots) -> u8 {
    let row_top = u32::from(row) * 4;
    (0..4).fold(0u8, |mask, slot| {
        let absolute = row_top + slot;
        let wick_mask = if wick.contains(absolute) {
            left_braille_bit(slot)
        } else {
            0
        };
        let body_mask = if body.contains(absolute) {
            right_braille_bit(slot)
        } else {
            0
        };
        mask | wick_mask | body_mask
    })
}

fn braille_symbol(mask: u8) -> String {
    char::from_u32(0x2800 + u32::from(mask))
        .expect("braille mask is always a valid Unicode scalar")
        .to_string()
}

const fn left_braille_bit(slot: u32) -> u8 {
    match slot {
        0 => 0b0000_0001,
        1 => 0b0000_0010,
        2 => 0b0000_0100,
        3 => 0b0100_0000,
        _ => 0,
    }
}

const fn right_braille_bit(slot: u32) -> u8 {
    match slot {
        0 => 0b0000_1000,
        1 => 0b0001_0000,
        2 => 0b0010_0000,
        3 => 0b1000_0000,
        _ => 0,
    }
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
