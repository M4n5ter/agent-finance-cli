use std::fmt;
use std::str::FromStr;

use agent_finance_market::args::HistorySession;
use agent_finance_market::is_likely_crypto_pair;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub mod series;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum ChartPreset {
    #[default]
    Auto,
    OneDay,
    FiveDays,
    OneMonth,
    ThreeMonths,
    SixMonths,
    OneYear,
}

impl ChartPreset {
    pub const ALL: [Self; 7] = [
        Self::Auto,
        Self::OneDay,
        Self::FiveDays,
        Self::OneMonth,
        Self::ThreeMonths,
        Self::SixMonths,
        Self::OneYear,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::OneDay => "1d",
            Self::FiveDays => "5d",
            Self::OneMonth => "1mo",
            Self::ThreeMonths => "3mo",
            Self::SixMonths => "6mo",
            Self::OneYear => "1y",
        }
    }

    pub const fn key(self) -> char {
        match self {
            Self::Auto => '0',
            Self::OneDay => '1',
            Self::FiveDays => '2',
            Self::OneMonth => '3',
            Self::ThreeMonths => '4',
            Self::SixMonths => '5',
            Self::OneYear => '6',
        }
    }

    pub fn from_key(key: char) -> Option<Self> {
        Self::ALL.into_iter().find(|preset| preset.key() == key)
    }

    pub fn command_id(self) -> String {
        format!("chart-preset-{}", self.label())
    }

    pub fn command_title(self) -> String {
        format!("Chart preset {}", self.label().to_ascii_uppercase())
    }

    pub const fn command_description(self) -> &'static str {
        match self {
            Self::Auto => "Use the asset-aware default chart range and interval",
            Self::OneDay => "Show one trading day on the history chart",
            Self::FiveDays => "Show five trading days on the history chart",
            Self::OneMonth => "Show one month on the history chart",
            Self::ThreeMonths => "Show three months on the history chart",
            Self::SixMonths => "Show six months on the history chart",
            Self::OneYear => "Show one year on the history chart",
        }
    }

    pub fn shift(self, direction: isize) -> Self {
        let index = Self::ALL
            .iter()
            .position(|preset| *preset == self)
            .unwrap_or_default() as isize;
        let next = (index + direction).rem_euclid(Self::ALL.len() as isize) as usize;
        Self::ALL[next]
    }

    pub fn request_for(self, symbol: &str) -> ChartHistoryRequest {
        let crypto = is_likely_crypto_pair(symbol);
        match (self, crypto) {
            (Self::Auto, true) => ChartHistoryRequest::new("1d", "1m", 288),
            (Self::Auto, false) => {
                ChartHistoryRequest::new("5d", "5m", 1_000).with_session(HistorySession::Extended)
            }
            (Self::OneDay, true) => ChartHistoryRequest::new("1d", "1m", 288),
            (Self::OneDay, false) => {
                ChartHistoryRequest::new("1d", "1m", 960).with_session(HistorySession::Extended)
            }
            (Self::FiveDays, true) => ChartHistoryRequest::new("5d", "5m", 288),
            (Self::FiveDays, false) => {
                ChartHistoryRequest::new("5d", "5m", 1_000).with_session(HistorySession::Extended)
            }
            (Self::OneMonth, _) => ChartHistoryRequest::new("1mo", "1d", 31),
            (Self::ThreeMonths, _) => ChartHistoryRequest::new("3mo", "1d", 66),
            (Self::SixMonths, _) => ChartHistoryRequest::new("6mo", "1d", 132),
            (Self::OneYear, _) => ChartHistoryRequest::new("1y", "1d", 252),
        }
    }
}

impl fmt::Display for ChartPreset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

impl FromStr for ChartPreset {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "1d" | "day" => Ok(Self::OneDay),
            "5d" | "5day" | "week" => Ok(Self::FiveDays),
            "1mo" | "1m" | "month" => Ok(Self::OneMonth),
            "3mo" | "3m" => Ok(Self::ThreeMonths),
            "6mo" | "6m" => Ok(Self::SixMonths),
            "1y" | "1yr" | "year" => Ok(Self::OneYear),
            _ => Err(format!("unknown chart preset {value}")),
        }
    }
}

impl Serialize for ChartPreset {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.label())
    }
}

impl<'de> Deserialize<'de> for ChartPreset {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ChartHistoryRequest {
    pub range: String,
    pub interval: String,
    pub limit: usize,
    pub session: HistorySession,
}

impl ChartHistoryRequest {
    fn new(range: &str, interval: &str, limit: usize) -> Self {
        Self {
            range: range.to_string(),
            interval: interval.to_string(),
            limit,
            session: HistorySession::Regular,
        }
    }

    const fn with_session(mut self, session: HistorySession) -> Self {
        self.session = session;
        self
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ChartState {
    preset: ChartPreset,
    window: ChartWindow,
    cursor_bps: Option<u16>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ChartWindow {
    start_bps: u16,
    end_bps: u16,
}

impl ChartState {
    pub const fn new(preset: ChartPreset) -> Self {
        Self {
            preset,
            window: ChartWindow::FULL,
            cursor_bps: None,
        }
    }

    pub const fn preset(&self) -> ChartPreset {
        self.preset
    }

    pub const fn window(&self) -> ChartWindow {
        self.window
    }

    pub const fn cursor_bps(&self) -> Option<u16> {
        self.cursor_bps
    }

    pub fn set_preset(&mut self, preset: ChartPreset) -> bool {
        let changed = self.preset != preset;
        self.preset = preset;
        if changed {
            self.reset_view();
        }
        changed
    }

    pub fn shift_preset(&mut self, direction: isize) -> bool {
        self.set_preset(self.preset.shift(direction))
    }

    pub fn move_cursor(&mut self, direction: isize) {
        let step = self.window.cursor_step();
        let current = self.cursor_bps.unwrap_or(self.window.end_bps);
        self.cursor_bps = Some(self.window.clamp_bps(offset_bps(current, direction, step)));
    }

    pub fn zoom_window(&mut self, direction: isize) {
        let anchor = self
            .cursor_bps
            .map(|cursor| self.window.clamp_bps(cursor))
            .unwrap_or_else(|| self.window.midpoint());
        self.window = self.window.zoom(direction, anchor);
        self.cursor_bps = Some(self.window.clamp_bps(anchor));
    }

    pub fn select_window(&mut self, start_bps: u16, end_bps: u16) -> bool {
        let Some(window) = ChartWindow::from_selection(start_bps, end_bps) else {
            return false;
        };
        self.window = window;
        self.cursor_bps = Some(window.midpoint());
        true
    }

    pub fn reset_view(&mut self) {
        self.window = ChartWindow::FULL;
        self.cursor_bps = None;
    }
}

impl ChartWindow {
    pub const FULL: Self = Self {
        start_bps: 0,
        end_bps: 10_000,
    };

    pub const MIN_SELECTION_SPAN_BPS: u16 = 500;

    pub const fn full(self) -> bool {
        self.start_bps == Self::FULL.start_bps && self.end_bps == Self::FULL.end_bps
    }

    pub const fn start_bps(self) -> u16 {
        self.start_bps
    }

    pub const fn end_bps(self) -> u16 {
        self.end_bps
    }

    pub const fn contains_bps(self, value: u16) -> bool {
        self.start_bps <= value && value <= self.end_bps
    }

    pub fn visible_range(self, len: usize) -> std::ops::Range<usize> {
        if len == 0 || self.full() {
            return 0..len;
        }
        let start = (len * usize::from(self.start_bps)) / 10_000;
        let mut end = (len * usize::from(self.end_bps)).div_ceil(10_000);
        end = end.clamp(start.saturating_add(1), len);
        start.min(len)..end
    }

    pub fn cursor_bucket_index(self, cursor_bps: u16, bucket_count: usize) -> Option<usize> {
        if bucket_count == 0 || !self.contains_bps(cursor_bps) {
            return None;
        }
        let relative = cursor_bps.saturating_sub(self.start_bps);
        Some(if bucket_count == 1 {
            0
        } else {
            (usize::from(relative) * (bucket_count - 1)) / usize::from(self.span())
        })
    }

    fn span(self) -> u16 {
        self.end_bps.saturating_sub(self.start_bps).max(1)
    }

    fn midpoint(self) -> u16 {
        self.start_bps + self.span() / 2
    }

    fn cursor_step(self) -> u16 {
        (self.span() / 20).max(1)
    }

    fn clamp_bps(self, value: u16) -> u16 {
        value.clamp(self.start_bps, self.end_bps)
    }

    fn zoom(self, direction: isize, anchor: u16) -> Self {
        if direction == 0 {
            return self;
        }
        let span = u32::from(self.span());
        let next_span = if direction > 0 {
            (span * 3 / 4).max(u32::from(Self::MIN_SELECTION_SPAN_BPS))
        } else {
            (span * 4 / 3).min(u32::from(Self::FULL.end_bps))
        } as u16;
        if next_span >= Self::FULL.end_bps {
            return Self::FULL;
        }
        let anchor = anchor.clamp(Self::FULL.start_bps, Self::FULL.end_bps);
        let half = next_span / 2;
        let mut start = anchor.saturating_sub(half);
        if start + next_span > Self::FULL.end_bps {
            start = Self::FULL.end_bps - next_span;
        }
        Self {
            start_bps: start,
            end_bps: start + next_span,
        }
    }

    fn from_selection(start_bps: u16, end_bps: u16) -> Option<Self> {
        let start = start_bps.min(end_bps).min(Self::FULL.end_bps);
        let end = start_bps.max(end_bps).min(Self::FULL.end_bps);
        (end.saturating_sub(start) >= Self::MIN_SELECTION_SPAN_BPS).then_some(Self {
            start_bps: start,
            end_bps: end,
        })
    }
}

fn offset_bps(value: u16, direction: isize, step: u16) -> u16 {
    if direction >= 0 {
        value.saturating_add(step.saturating_mul(direction as u16))
    } else {
        value.saturating_sub(step.saturating_mul(direction.unsigned_abs() as u16))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_chart_presets_to_asset_aware_requests() {
        let equity = ChartPreset::Auto.request_for("CRDO");
        assert_eq!(equity.range, "5d");
        assert_eq!(equity.interval, "5m");
        assert_eq!(equity.limit, 1_000);
        assert_eq!(equity.session, HistorySession::Extended);

        let day = ChartPreset::OneDay.request_for("CRDO");
        assert_eq!(day.range, "1d");
        assert_eq!(day.interval, "1m");
        assert_eq!(day.limit, 960);
        assert_eq!(day.session, HistorySession::Extended);

        let crypto = ChartPreset::Auto.request_for("BTCUSDT");
        assert_eq!(crypto.range, "1d");
        assert_eq!(crypto.interval, "1m");
        assert_eq!(crypto.session, HistorySession::Regular);

        let long = ChartPreset::OneYear.request_for("CRDO");
        assert_eq!(long.range, "1y");
        assert_eq!(long.interval, "1d");
        assert_eq!(long.limit, 252);
    }

    #[test]
    fn exposes_preset_keys_for_input_mapping() {
        for preset in ChartPreset::ALL {
            assert_eq!(ChartPreset::from_key(preset.key()), Some(preset));
        }
        assert_eq!(ChartPreset::from_key('7'), None);
    }

    #[test]
    fn chart_cursor_and_zoom_window_are_session_state() {
        let mut state = ChartState::new(ChartPreset::Auto);

        state.move_cursor(-1);
        assert_eq!(state.cursor_bps(), Some(9_500));

        state.zoom_window(1);
        assert_eq!(
            state.window(),
            ChartWindow {
                start_bps: 2_500,
                end_bps: 10_000
            }
        );
        assert_eq!(state.cursor_bps(), Some(9_500));

        state.move_cursor(-1);
        assert_eq!(state.cursor_bps(), Some(9_125));

        state.zoom_window(-1);
        assert_eq!(state.window(), ChartWindow::FULL);

        assert!(state.set_preset(ChartPreset::OneDay));
        assert_eq!(state.window(), ChartWindow::FULL);
        assert_eq!(state.cursor_bps(), None);
    }

    #[test]
    fn chart_window_selection_ignores_tiny_drags_and_sets_midpoint_cursor() {
        let mut state = ChartState::new(ChartPreset::Auto);

        assert!(!state.select_window(1_000, 1_200));
        assert_eq!(state.window(), ChartWindow::FULL);
        assert_eq!(state.cursor_bps(), None);

        assert!(state.select_window(7_000, 2_000));
        assert_eq!(
            state.window(),
            ChartWindow {
                start_bps: 2_000,
                end_bps: 7_000
            }
        );
        assert_eq!(state.cursor_bps(), Some(4_500));
    }
}
