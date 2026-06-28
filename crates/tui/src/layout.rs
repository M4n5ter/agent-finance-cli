use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::config::{
    LayoutConfig, MAX_LEFT_MAIN_RATIO, MAX_LEFT_RATIO, MAX_MAIN_RATIO, MIN_LEFT_RATIO,
    MIN_MAIN_RATIO, MIN_RIGHT_RATIO,
};
use crate::model::{FloatingKind, FloatingPane, FloatingSize, Panel};

const MIN_PANEL_WIDTH: u16 = 18;
const MIN_PANEL_HEIGHT: u16 = 4;
const STATUS_HEIGHT: u16 = 1;
const MIDDLE_COLUMN_SPECS: [ColumnSpec; 3] = [
    ColumnSpec::new(
        &[
            Panel::Settings,
            Panel::OrderTicket,
            Panel::OpenOrders,
            Panel::IntentReview,
            Panel::RiskAudit,
        ],
        &[
            (Panel::Settings, 34),
            (Panel::OrderTicket, 34),
            (Panel::OpenOrders, 24),
            (Panel::IntentReview, 22),
            (Panel::RiskAudit, 20),
            (Panel::Account, 36),
            (Panel::TransferTicket, 22),
            (Panel::FuturesState, 22),
            (Panel::Quote, 18),
            (Panel::History, 12),
        ],
    ),
    ColumnSpec::new(
        &[Panel::Account, Panel::TransferTicket, Panel::FuturesState],
        &[
            (Panel::Account, 36),
            (Panel::TransferTicket, 22),
            (Panel::FuturesState, 22),
            (Panel::Quote, 10),
            (Panel::History, 10),
        ],
    ),
    ColumnSpec::new(&[], &[(Panel::Quote, 1), (Panel::History, 1)]),
];

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CockpitLayout {
    panels: PanelRects,
    pub status: Rect,
    pub floating: Vec<FloatingRect>,
    open_panels: Vec<Panel>,
    columns: Option<DockedColumns>,
}

impl CockpitLayout {
    pub fn panel_rect(&self, panel: Panel) -> Option<Rect> {
        if !self.open_panels.contains(&panel) {
            return None;
        }

        Some(self.panels.get(panel))
    }

    pub fn panel_at(&self, x: u16, y: u16) -> Option<Panel> {
        self.open_panels.iter().copied().find(|panel| {
            self.panel_rect(*panel)
                .is_some_and(|rect| contains(rect, x, y))
        })
    }

    pub fn hit_test(&self, x: u16, y: u16) -> Option<LayoutHit> {
        if let Some(floating) = self
            .floating
            .iter()
            .rev()
            .find(|floating| contains(floating.rect, x, y))
        {
            if floating_resize_handle_contains(floating.rect, x, y) {
                return Some(LayoutHit::FloatingResize(floating.kind));
            }
            return Some(LayoutHit::Floating(floating.kind));
        }
        if contains(self.status, x, y) {
            return Some(LayoutHit::Status);
        }
        if let Some(split) = self.docked_split_at(x, y) {
            return Some(LayoutHit::DockedSplit(split));
        }
        self.panel_at(x, y).map(LayoutHit::Panel)
    }

    fn docked_split_at(&self, x: u16, y: u16) -> Option<DockedColumnSplit> {
        let columns = self.columns.as_ref()?;
        columns.visible_splits().into_iter().find_map(|split| {
            let boundary = split.left_rect.x.saturating_add(split.left_rect.width);
            (contains(split.left_rect, x, y) || contains(split.right_rect, x, y))
                .then_some(split.kind)
                .filter(|_kind| near_column_boundary(x, boundary))
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct FloatingRect {
    pub kind: FloatingKind,
    pub rect: Rect,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
struct DockedColumns {
    left: Rect,
    middle: Rect,
    right: Rect,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DockedColumnSplit {
    LeftMain,
    MainRight,
    LeftRight,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LayoutHit {
    Floating(FloatingKind),
    FloatingResize(FloatingKind),
    DockedSplit(DockedColumnSplit),
    Panel(Panel),
    Status,
}

pub fn build(
    area: Rect,
    config: &LayoutConfig,
    floating: &[FloatingPane],
    open_panels: &[Panel],
) -> CockpitLayout {
    let [body, status] = split_vertical(
        area,
        [Constraint::Min(0), Constraint::Length(STATUS_HEIGHT)],
    );
    let compact = body.width < MIN_PANEL_WIDTH * 3 || body.height < MIN_PANEL_HEIGHT * 3;
    let open_panels = normalized_open_panels(open_panels);

    let (panels, columns) = if compact {
        (compact_layout(body, &open_panels), None)
    } else {
        let columns = docked_columns(body, config, &open_panels);
        (wide_layout(columns, &open_panels), Some(columns))
    };

    let floating = floating
        .iter()
        .map(|pane| FloatingRect {
            kind: pane.kind,
            rect: floating_rect(body, pane.size),
        })
        .collect::<Vec<_>>();

    CockpitLayout {
        panels,
        status,
        floating,
        open_panels,
        columns,
    }
}

pub fn resize_docked_columns(
    area: Rect,
    split: DockedColumnSplit,
    x: u16,
    config: &LayoutConfig,
    open_panels: &[Panel],
) -> LayoutConfig {
    let [body, _status] = split_vertical(
        area,
        [Constraint::Min(0), Constraint::Length(STATUS_HEIGHT)],
    );
    if body.width == 0 {
        return config.clone();
    }

    let pointer_ratio =
        (((u32::from(x.saturating_sub(body.x)) * 100) / u32::from(body.width)).min(100)) as u16;
    let mut next = config.clone();
    let active = active_docked_groups(config, &normalized_open_panels(open_panels));
    match split {
        DockedColumnSplit::LeftMain => {
            let left_and_main = config.left_ratio.saturating_add(config.main_ratio);
            let max_left = MAX_LEFT_RATIO.min(left_and_main.saturating_sub(MIN_MAIN_RATIO));
            let pointer_ratio = if active_has_right(&active) {
                pointer_ratio
            } else {
                scale_visible_ratio(pointer_ratio, left_and_main)
            };
            next.left_ratio = pointer_ratio.clamp(MIN_LEFT_RATIO, max_left);
            next.main_ratio = left_and_main.saturating_sub(next.left_ratio);
        }
        DockedColumnSplit::MainRight => {
            let max_main = MAX_MAIN_RATIO.min(MAX_LEFT_MAIN_RATIO.saturating_sub(next.left_ratio));
            let main_and_right = 100u16.saturating_sub(config.left_ratio);
            let main_ratio = if active_has_left(&active) {
                pointer_ratio.saturating_sub(next.left_ratio)
            } else {
                scale_visible_ratio(pointer_ratio, main_and_right)
            };
            next.main_ratio = main_ratio.clamp(MIN_MAIN_RATIO, max_main);
        }
        DockedColumnSplit::LeftRight => {
            let left_and_right = 100u16.saturating_sub(config.main_ratio);
            let pointer_ratio = scale_visible_ratio(pointer_ratio, left_and_right);
            let max_left = MAX_LEFT_RATIO.min(left_and_right.saturating_sub(MIN_RIGHT_RATIO));
            next.left_ratio = pointer_ratio.clamp(MIN_LEFT_RATIO, max_left);
        }
    }
    next.normalize();
    next
}

pub fn resize_floating(area: Rect, x: u16, y: u16) -> FloatingSize {
    let [body, _status] = split_vertical(
        area,
        [Constraint::Min(0), Constraint::Length(STATUS_HEIGHT)],
    );
    if body.width == 0 || body.height == 0 {
        return FloatingSize::resized(FloatingSize::MIN_RATIO, FloatingSize::MIN_RATIO);
    }

    let center_x = body.x.saturating_add(body.width / 2);
    let center_y = body.y.saturating_add(body.height / 2);
    let width = x.abs_diff(center_x).saturating_mul(2).max(MIN_PANEL_WIDTH);
    let height = y.abs_diff(center_y).saturating_mul(2).max(MIN_PANEL_HEIGHT);
    let width_ratio = ratio_of(width, body.width);
    let height_ratio = ratio_of(height, body.height);
    FloatingSize::resized(width_ratio, height_ratio)
}

fn docked_columns(area: Rect, config: &LayoutConfig, open_panels: &[Panel]) -> DockedColumns {
    let active = active_docked_groups(config, open_panels);

    if active.len() == 1 {
        return DockedColumns::single(active[0].0, area);
    }

    let total_weight = active
        .iter()
        .map(|(_group, weight)| u32::from(*weight))
        .sum::<u32>()
        .max(1);
    let constraints = active
        .iter()
        .map(|(_group, weight)| Constraint::Ratio(u32::from(*weight), total_weight))
        .collect::<Vec<_>>();
    let areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    let mut columns = DockedColumns::default();
    for ((group, _weight), rect) in active.into_iter().zip(areas.iter().copied()) {
        columns.set(group, rect);
    }
    columns
}

fn active_docked_groups(config: &LayoutConfig, open_panels: &[Panel]) -> Vec<(DockedGroup, u16)> {
    let right_ratio = 100u16.saturating_sub(config.left_ratio + config.main_ratio);
    [
        (
            DockedGroup::Left,
            config.left_ratio,
            &[Panel::Watchlist, Panel::ProviderHealth, Panel::TaskLog][..],
        ),
        (
            DockedGroup::Middle,
            config.main_ratio,
            &[
                Panel::Settings,
                Panel::OrderTicket,
                Panel::OpenOrders,
                Panel::IntentReview,
                Panel::RiskAudit,
                Panel::Account,
                Panel::TransferTicket,
                Panel::FuturesState,
                Panel::Quote,
                Panel::History,
            ][..],
        ),
        (
            DockedGroup::Right,
            right_ratio.max(MIN_RIGHT_RATIO),
            &[Panel::Evidence, Panel::Polymarket, Panel::Research][..],
        ),
    ]
    .into_iter()
    .filter(|(_group, _weight, panels)| panels.iter().any(|panel| open_panels.contains(panel)))
    .map(|(group, weight, _panels)| (group, weight))
    .collect()
}

fn active_has_left(active: &[(DockedGroup, u16)]) -> bool {
    active
        .iter()
        .any(|(group, _weight)| *group == DockedGroup::Left)
}

fn active_has_right(active: &[(DockedGroup, u16)]) -> bool {
    active
        .iter()
        .any(|(group, _weight)| *group == DockedGroup::Right)
}

fn scale_visible_ratio(pointer_ratio: u16, active_ratio: u16) -> u16 {
    ((u32::from(pointer_ratio) * u32::from(active_ratio)) / 100) as u16
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum DockedGroup {
    Left,
    Middle,
    Right,
}

impl DockedColumns {
    fn single(group: DockedGroup, area: Rect) -> Self {
        let mut columns = Self::default();
        columns.set(group, area);
        columns
    }

    fn set(&mut self, group: DockedGroup, rect: Rect) {
        match group {
            DockedGroup::Left => self.left = rect,
            DockedGroup::Middle => self.middle = rect,
            DockedGroup::Right => self.right = rect,
        }
    }

    fn visible_splits(self) -> Vec<VisibleSplit> {
        let visible = [
            (DockedGroup::Left, self.left),
            (DockedGroup::Middle, self.middle),
            (DockedGroup::Right, self.right),
        ]
        .into_iter()
        .filter(|(_group, rect)| has_area(*rect))
        .collect::<Vec<_>>();

        visible
            .windows(2)
            .filter_map(|pair| visible_split(pair[0], pair[1]))
            .collect()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct VisibleSplit {
    kind: DockedColumnSplit,
    left_rect: Rect,
    right_rect: Rect,
}

fn visible_split(left: (DockedGroup, Rect), right: (DockedGroup, Rect)) -> Option<VisibleSplit> {
    let kind = match (left.0, right.0) {
        (DockedGroup::Left, DockedGroup::Middle) => DockedColumnSplit::LeftMain,
        (DockedGroup::Middle, DockedGroup::Right) => DockedColumnSplit::MainRight,
        (DockedGroup::Left, DockedGroup::Right) => DockedColumnSplit::LeftRight,
        _ => return None,
    };
    Some(VisibleSplit {
        kind,
        left_rect: left.1,
        right_rect: right.1,
    })
}

fn wide_layout(columns: DockedColumns, open_panels: &[Panel]) -> PanelRects {
    let mut rects = PanelRects::default();
    assign_weighted_column(
        &mut rects,
        columns.left,
        &[
            (Panel::Watchlist, 55),
            (Panel::ProviderHealth, 25),
            (Panel::TaskLog, 20),
        ],
        open_panels,
    );
    assign_middle_column(&mut rects, columns.middle, open_panels);
    assign_weighted_column(
        &mut rects,
        columns.right,
        &[
            (Panel::Evidence, 36),
            (Panel::Polymarket, 28),
            (Panel::Research, 36),
        ],
        open_panels,
    );
    rects
}

fn compact_layout(area: Rect, open_panels: &[Panel]) -> PanelRects {
    let mut rects = PanelRects::default();
    assign_weighted_column(
        &mut rects,
        area,
        &Panel::ALL.map(|panel| (panel, 1)),
        open_panels,
    );
    rects
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
struct PanelRects {
    rects: [Rect; Panel::ALL.len()],
}

impl PanelRects {
    fn get(&self, panel: Panel) -> Rect {
        self.rects[panel.order()]
    }

    fn set(&mut self, panel: Panel, rect: Rect) {
        self.rects[panel.order()] = non_empty(rect);
    }
}

fn assign_middle_column(rects: &mut PanelRects, area: Rect, open_panels: &[Panel]) {
    let spec = MIDDLE_COLUMN_SPECS
        .iter()
        .find(|spec| spec.matches(open_panels))
        .expect("fallback middle column spec is always available");
    if !spec.anchor_panels.is_empty() {
        assign_weighted_column(rects, area, spec.panels, open_panels);
        return;
    }

    match (
        open_panels.contains(&Panel::Quote),
        open_panels.contains(&Panel::History),
    ) {
        (true, true) => {
            let [quote, history] = split_vertical(
                area,
                [Constraint::Length(9.min(area.height)), Constraint::Min(0)],
            );
            rects.set(Panel::Quote, quote);
            rects.set(Panel::History, history);
        }
        (true, false) => rects.set(Panel::Quote, area),
        (false, true) => rects.set(Panel::History, area),
        (false, false) => {}
    }
}

#[derive(Debug, Clone, Copy)]
struct ColumnSpec {
    anchor_panels: &'static [Panel],
    panels: &'static [(Panel, u32)],
}

impl ColumnSpec {
    const fn new(anchor_panels: &'static [Panel], panels: &'static [(Panel, u32)]) -> Self {
        Self {
            anchor_panels,
            panels,
        }
    }

    fn matches(self, open_panels: &[Panel]) -> bool {
        self.anchor_panels.is_empty()
            || self
                .anchor_panels
                .iter()
                .any(|panel| open_panels.contains(panel))
    }
}

fn assign_weighted_column(
    rects: &mut PanelRects,
    area: Rect,
    specs: &[(Panel, u32)],
    open_panels: &[Panel],
) {
    let active = specs
        .iter()
        .copied()
        .filter(|(panel, _weight)| open_panels.contains(panel))
        .collect::<Vec<_>>();
    if active.is_empty() {
        return;
    }
    if active.len() == 1 {
        rects.set(active[0].0, area);
        return;
    }

    let total_weight = active
        .iter()
        .map(|(_panel, weight)| *weight)
        .sum::<u32>()
        .max(1);
    let constraints = active
        .iter()
        .map(|(_panel, weight)| Constraint::Ratio(*weight, total_weight))
        .collect::<Vec<_>>();
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);
    for ((panel, _weight), rect) in active.into_iter().zip(areas.iter().copied()) {
        rects.set(panel, rect);
    }
}

fn normalized_open_panels(open_panels: &[Panel]) -> Vec<Panel> {
    Panel::ALL
        .into_iter()
        .filter(|panel| open_panels.contains(panel))
        .collect()
}

fn floating_rect(area: Rect, size: FloatingSize) -> Rect {
    let width = floating_dimension(area.width, size.width_ratio, MIN_PANEL_WIDTH);
    let height = floating_dimension(area.height, size.height_ratio, MIN_PANEL_HEIGHT);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn floating_dimension(total: u16, ratio: u16, minimum: u16) -> u16 {
    let maximum = total.max(1) as u32;
    if maximum < minimum as u32 {
        return maximum as u16;
    }

    ((total as u32 * u32::from(ratio)) / 100)
        .max(1)
        .clamp(minimum as u32, maximum) as u16
}

fn ratio_of(value: u16, total: u16) -> u16 {
    if total == 0 {
        return FloatingSize::MIN_RATIO;
    }

    (((u32::from(value) * 100) / u32::from(total)).min(100)) as u16
}

fn split_vertical<const N: usize>(area: Rect, constraints: [Constraint; N]) -> [Rect; N] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area)
        .to_vec()
        .try_into()
        .unwrap_or_else(|_| [Rect::default(); N])
}

fn non_empty(rect: Rect) -> Rect {
    Rect {
        width: rect.width.max(1),
        height: rect.height.max(1),
        ..rect
    }
}

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

fn floating_resize_handle_contains(rect: Rect, x: u16, y: u16) -> bool {
    if !contains(rect, x, y) {
        return false;
    }

    let handle_x = rect.x.saturating_add(rect.width.saturating_sub(2));
    let handle_y = rect.y.saturating_add(rect.height.saturating_sub(1));
    x >= handle_x && y >= handle_y
}

fn has_area(rect: Rect) -> bool {
    rect.width > 0 && rect.height > 0
}

fn near_column_boundary(x: u16, boundary: u16) -> bool {
    x.abs_diff(boundary) <= 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FloatingPane;

    #[test]
    fn wide_layout_preserves_all_docked_panels_and_status_bar() {
        let layout = build(
            Rect::new(0, 0, 160, 48),
            &LayoutConfig::default(),
            &[],
            &Panel::ALL,
        );

        assert_eq!(layout.status.height, 1);
        for panel in [
            Panel::Watchlist,
            Panel::Quote,
            Panel::History,
            Panel::Evidence,
            Panel::Polymarket,
            Panel::Research,
            Panel::ProviderHealth,
            Panel::TaskLog,
        ] {
            let rect = layout.panel_rect(panel).expect("panel should be open");
            assert!(rect.width > 0, "{panel:?} should have width");
            assert!(rect.height > 0, "{panel:?} should have height");
        }
    }

    #[test]
    fn compact_layout_does_not_generate_zero_sized_panels() {
        let layout = build(
            Rect::new(0, 0, 42, 16),
            &LayoutConfig::default(),
            &[],
            &Panel::ALL,
        );

        assert!(layout.panel_rect(Panel::History).unwrap().width > 0);
        assert!(layout.panel_rect(Panel::Evidence).unwrap().height > 0);
        assert_eq!(layout.status.height, 1);
    }

    #[test]
    fn layout_does_not_invent_panels_for_empty_input() {
        let layout = build(Rect::new(0, 0, 160, 48), &LayoutConfig::default(), &[], &[]);

        assert!(
            Panel::ALL
                .iter()
                .all(|panel| layout.panel_rect(*panel).is_none())
        );
        assert_eq!(layout.panel_at(2, 2), None);
    }

    #[test]
    fn floating_rects_are_clamped_and_keep_stack_order() {
        let layout = build(
            Rect::new(0, 0, 100, 32),
            &LayoutConfig::default(),
            &[
                FloatingPane::new(FloatingKind::Help),
                FloatingPane::new(FloatingKind::CommandPalette),
            ],
            &Panel::ALL,
        );

        assert_eq!(layout.floating[0].kind, FloatingKind::Help);
        assert_eq!(layout.floating[1].kind, FloatingKind::CommandPalette);
        for floating in layout.floating {
            assert!(floating.rect.width <= 100);
            assert!(floating.rect.height <= 31);
            assert!(floating.rect.width >= MIN_PANEL_WIDTH);
            assert!(floating.rect.height >= MIN_PANEL_HEIGHT);
        }
    }

    #[test]
    fn floating_rects_fit_tiny_terminals() {
        let layout = build(
            Rect::new(0, 0, 1, 1),
            &LayoutConfig::default(),
            &[FloatingPane::new(FloatingKind::CommandPalette)],
            &Panel::ALL,
        );

        assert_eq!(layout.floating[0].rect, Rect::new(0, 0, 1, 1));
    }

    #[test]
    fn floating_rects_use_pane_size_and_resize_handle_hit_test() {
        let mut pane = FloatingPane::new(FloatingKind::Help);
        pane.size = FloatingSize::resized(80, 50);
        let layout = build(
            Rect::new(0, 0, 100, 41),
            &LayoutConfig::default(),
            &[pane],
            &Panel::ALL,
        );
        let floating = layout.floating[0];

        assert_eq!(floating.rect.width, 80);
        assert_eq!(floating.rect.height, 20);
        assert_eq!(
            layout.hit_test(floating.rect.right() - 1, floating.rect.bottom() - 1),
            Some(LayoutHit::FloatingResize(FloatingKind::Help))
        );
        assert_eq!(
            layout.hit_test(floating.rect.x + 1, floating.rect.y + 1),
            Some(LayoutHit::Floating(FloatingKind::Help))
        );
    }

    #[test]
    fn floating_resize_produces_visible_clamped_rect() {
        let size = resize_floating(Rect::new(0, 0, 100, 41), 90, 35);
        let mut pane = FloatingPane::new(FloatingKind::Help);
        pane.size = size;
        let layout = build(
            Rect::new(0, 0, 100, 41),
            &LayoutConfig::default(),
            &[pane],
            &Panel::ALL,
        );
        let rect = layout.floating[0].rect;
        let default_help = floating_rect(
            Rect::new(0, 0, 100, 40),
            FloatingSize::default_for(FloatingKind::Help),
        );
        let default_palette = floating_rect(
            Rect::new(0, 0, 100, 40),
            FloatingSize::default_for(FloatingKind::CommandPalette),
        );

        assert!(rect.width > default_help.width);
        assert!(rect.height > default_palette.height);
        assert!(rect.right() <= 100);
        assert!(rect.bottom() <= 40);
    }

    #[test]
    fn wide_layout_maps_points_to_panels_and_split_handles() {
        let area = Rect::new(0, 0, 160, 48);
        let config = LayoutConfig::default();
        let layout = build(area, &config, &[], &Panel::ALL);

        assert_eq!(layout.panel_at(2, 2), Some(Panel::Watchlist));
        for panel in Panel::ALL {
            let rect = layout.panel_rect(panel).expect("panel is open");
            assert_eq!(
                layout.panel_at(rect.x + rect.width / 2, rect.y + rect.height / 2),
                Some(panel),
                "{panel:?} center should hit its own panel"
            );
        }

        assert_eq!(
            layout.hit_test(layout.panel_rect(Panel::Watchlist).unwrap().right(), 2),
            Some(LayoutHit::DockedSplit(DockedColumnSplit::LeftMain))
        );
        assert_eq!(
            layout.hit_test(layout.panel_rect(Panel::Quote).unwrap().right(), 2),
            Some(LayoutHit::DockedSplit(DockedColumnSplit::MainRight))
        );
    }

    #[test]
    fn closed_panels_do_not_hit_test_or_reserve_space() {
        let area = Rect::new(0, 0, 160, 48);
        let open = [
            Panel::Watchlist,
            Panel::Quote,
            Panel::Evidence,
            Panel::ProviderHealth,
            Panel::TaskLog,
        ];
        let layout = build(area, &LayoutConfig::default(), &[], &open);

        assert_eq!(layout.panel_rect(Panel::History), None);
        assert_eq!(layout.panel_rect(Panel::Polymarket), None);
        assert_eq!(layout.panel_rect(Panel::Research), None);
        let quote = layout.panel_rect(Panel::Quote).expect("quote is open");
        assert!(quote.height > 30);
        assert_ne!(layout.panel_at(150, 36), Some(Panel::Research));
    }

    #[test]
    fn intent_review_keeps_middle_layout_when_order_ticket_is_closed() {
        let open = [Panel::Watchlist, Panel::IntentReview, Panel::Evidence];
        let layout = build(
            Rect::new(0, 0, 160, 48),
            &LayoutConfig::default(),
            &[],
            &open,
        );

        let review = layout
            .panel_rect(Panel::IntentReview)
            .expect("intent review should be visible");
        assert_eq!(
            layout.panel_at(review.x + 1, review.y + 1),
            Some(Panel::IntentReview)
        );
        assert_eq!(layout.panel_rect(Panel::OrderTicket), None);
    }

    #[test]
    fn closed_column_groups_release_wide_layout_space() {
        let area = Rect::new(0, 0, 160, 48);
        let open = [Panel::Watchlist, Panel::Quote, Panel::History];

        let layout = build(area, &LayoutConfig::default(), &[], &open);

        let quote = layout.panel_rect(Panel::Quote).expect("quote is open");
        let history = layout.panel_rect(Panel::History).expect("history is open");
        assert_eq!(quote.x + quote.width, area.width);
        assert_eq!(history.x + history.width, area.width);
        assert_eq!(
            layout.hit_test(159, 2),
            Some(LayoutHit::Panel(Panel::Quote))
        );
        assert_eq!(
            layout.hit_test(159, 20),
            Some(LayoutHit::Panel(Panel::History))
        );
    }

    #[test]
    fn visible_split_resize_uses_closed_column_group_context() {
        let area = Rect::new(0, 0, 160, 48);
        let config = LayoutConfig::default();
        let open = [Panel::Watchlist, Panel::Quote, Panel::History];
        let layout = build(area, &config, &[], &open);
        let watchlist = layout.panel_rect(Panel::Watchlist).unwrap();
        let x = watchlist.right().saturating_add(10);

        let resized = resize_docked_columns(area, DockedColumnSplit::LeftMain, x, &config, &open);

        assert_eq!(100 - resized.left_ratio - resized.main_ratio, 30);
        assert!(resized.left_ratio > config.left_ratio);
        assert!(resized.main_ratio < config.main_ratio);
    }

    #[test]
    fn closed_middle_group_exposes_left_right_split() {
        let area = Rect::new(0, 0, 160, 48);
        let config = LayoutConfig::default();
        let open = [Panel::Watchlist, Panel::Evidence, Panel::Polymarket];
        let layout = build(area, &config, &[], &open);
        let watchlist = layout.panel_rect(Panel::Watchlist).unwrap();

        assert_eq!(
            layout.hit_test(watchlist.right(), 2),
            Some(LayoutHit::DockedSplit(DockedColumnSplit::LeftRight))
        );

        let resized = resize_docked_columns(
            area,
            DockedColumnSplit::LeftRight,
            watchlist.right().saturating_add(12),
            &config,
            &open,
        );
        assert_eq!(resized.main_ratio, config.main_ratio);
        assert!(resized.left_ratio > config.left_ratio);
    }

    #[test]
    fn resizing_docked_columns_clamps_to_usable_ratios() {
        let area = Rect::new(0, 0, 160, 48);
        let config = LayoutConfig::default();

        let narrow_left =
            resize_docked_columns(area, DockedColumnSplit::LeftMain, 4, &config, &Panel::ALL);
        assert_eq!(narrow_left.left_ratio, 15);

        let wide_main = resize_docked_columns(
            area,
            DockedColumnSplit::MainRight,
            150,
            &config,
            &Panel::ALL,
        );
        assert_eq!(wide_main.main_ratio, 56);
        assert!(wide_main.left_ratio + wide_main.main_ratio <= MAX_LEFT_MAIN_RATIO);
    }

    #[test]
    fn resizing_left_main_preserves_right_column_share() {
        let area = Rect::new(0, 0, 160, 48);
        let config = LayoutConfig::default();
        let initial_right = 100 - config.left_ratio - config.main_ratio;

        let resized =
            resize_docked_columns(area, DockedColumnSplit::LeftMain, 56, &config, &Panel::ALL);

        assert_eq!(100 - resized.left_ratio - resized.main_ratio, initial_right);
        assert_eq!(resized.left_ratio, 35);
        assert_eq!(resized.main_ratio, 35);
    }

    #[test]
    fn resizing_left_main_does_not_borrow_from_right_when_main_is_minimum() {
        let area = Rect::new(0, 0, 160, 48);
        let config = LayoutConfig {
            left_ratio: 24,
            main_ratio: 35,
        };
        let initial_right = 100 - config.left_ratio - config.main_ratio;

        let resized =
            resize_docked_columns(area, DockedColumnSplit::LeftMain, 56, &config, &Panel::ALL);

        assert_eq!(100 - resized.left_ratio - resized.main_ratio, initial_right);
        assert_eq!(resized.left_ratio, 24);
        assert_eq!(resized.main_ratio, 35);
    }

    #[test]
    fn floating_hit_test_blocks_docked_panel_passthrough() {
        let layout = build(
            Rect::new(0, 0, 160, 48),
            &LayoutConfig::default(),
            &[FloatingPane::new(FloatingKind::Help)],
            &Panel::ALL,
        );
        let floating = layout.floating[0];

        assert_eq!(
            layout.hit_test(floating.rect.x + 1, floating.rect.y + 1),
            Some(LayoutHit::Floating(FloatingKind::Help))
        );
    }

    #[test]
    fn resizing_docked_columns_uses_full_wide_terminal_range() {
        let area = Rect::new(0, 0, 1_000, 48);
        let config = LayoutConfig::default();

        let resized = resize_docked_columns(
            area,
            DockedColumnSplit::MainRight,
            920,
            &config,
            &Panel::ALL,
        );

        assert_eq!(resized.main_ratio, 56);
    }
}
