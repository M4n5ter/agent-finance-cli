use ratatui::layout::Rect;
use unicode_width::UnicodeWidthStr;

use agent_finance_i18n::LocaleId;

use crate::i18n::TuiText;
use crate::model::WorkspaceKind;

const TAB_HORIZONTAL_PADDING: u16 = 2;
const TAB_DIVIDER_WIDTH: u16 = 1;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct WorkspaceTabSegment {
    pub workspace: WorkspaceKind,
    pub label: String,
    pub start: u16,
    pub end: u16,
    pub has_divider_after: bool,
}

pub(crate) fn workspace_tabs_width(locale: LocaleId) -> u16 {
    let text = TuiText::new(locale);
    WorkspaceKind::ALL
        .iter()
        .copied()
        .enumerate()
        .map(|(index, workspace)| {
            rendered_width(&workspace_tab_label(workspace, &text)) + divider_width_after(index)
        })
        .sum()
}

pub(crate) fn workspace_tab_at(area: Rect, column: u16, locale: LocaleId) -> Option<WorkspaceKind> {
    workspace_tab_segments(area, locale)
        .into_iter()
        .find(|segment| (segment.start..segment.end).contains(&column))
        .map(|segment| segment.workspace)
}

pub(crate) fn workspace_tab_segments(area: Rect, locale: LocaleId) -> Vec<WorkspaceTabSegment> {
    let text = TuiText::new(locale);
    let visible_right = area.x.saturating_add(area.width);
    let mut cursor = area.x;
    WorkspaceKind::ALL
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, workspace)| {
            let label = workspace_tab_label(workspace, &text);
            let width = rendered_width(&label);
            let start = cursor;
            let end = start.saturating_add(width).min(visible_right);
            let has_divider_after = index + 1 < WorkspaceKind::ALL.len();
            cursor = start
                .saturating_add(width)
                .saturating_add(divider_width_after(index));

            (start < end).then_some(WorkspaceTabSegment {
                workspace,
                label,
                start,
                end,
                has_divider_after,
            })
        })
        .collect()
}

fn workspace_tab_label(workspace: WorkspaceKind, text: &TuiText) -> String {
    let side_padding = " ".repeat((TAB_HORIZONTAL_PADDING / 2) as usize);
    format!(
        "{side_padding}{}{side_padding}",
        text.workspace_title(workspace)
    )
}

fn rendered_width(text: &str) -> u16 {
    UnicodeWidthStr::width(text) as u16
}

fn divider_width_after(index: usize) -> u16 {
    if index + 1 < WorkspaceKind::ALL.len() {
        TAB_DIVIDER_WIDTH
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_tracks_workspace_titles_and_chrome_padding() {
        let locale = LocaleId::EnUs;
        let text = TuiText::new(locale);
        let title_width = WorkspaceKind::ALL
            .iter()
            .map(|workspace| rendered_width(&text.workspace_title(*workspace)))
            .sum::<u16>();

        assert_eq!(
            workspace_tabs_width(locale),
            title_width
                + WorkspaceKind::ALL.len() as u16 * TAB_HORIZONTAL_PADDING
                + WorkspaceKind::ALL.len().saturating_sub(1) as u16 * TAB_DIVIDER_WIDTH
        );
        assert!(workspace_tabs_width(locale) < 80);
    }

    #[test]
    fn hit_testing_maps_rendered_tab_ranges() {
        let locale = LocaleId::EnUs;
        let text = TuiText::new(locale);
        let area = Rect::new(4, 10, 120, 1);
        let segments = workspace_tab_segments(area, locale);

        for segment in &segments {
            assert_eq!(
                workspace_tab_at(area, segment.start, locale),
                Some(segment.workspace)
            );
            assert_eq!(
                workspace_tab_at(area, segment.end - 1, locale),
                Some(segment.workspace)
            );
            assert_eq!(segment.label, workspace_tab_label(segment.workspace, &text));
        }

        let first_divider = segments
            .iter()
            .find(|segment| segment.has_divider_after)
            .expect("at least one divider")
            .end;
        assert_eq!(workspace_tab_at(area, first_divider, locale), None);
        assert_eq!(
            workspace_tab_at(area, area.x + workspace_tabs_width(locale), locale),
            None
        );
    }

    #[test]
    fn cjk_hit_testing_uses_terminal_cell_width() {
        let locale = LocaleId::ZhCn;
        let area = Rect::new(0, 0, 120, 1);
        let segments = workspace_tab_segments(area, locale);

        assert!(
            workspace_tabs_width(locale) > WorkspaceKind::ALL.len() as u16,
            "localized tab width must be measured in terminal cells"
        );
        for segment in &segments {
            assert_eq!(
                workspace_tab_at(area, segment.end - 1, locale),
                Some(segment.workspace)
            );
        }
    }
}
