use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use crate::action_line_view::{ActionLine, ActionSpan};
use crate::command::ActionId;
use crate::model::Panel;
use crate::mouse_target::MouseTarget;
use crate::theme::ThemeConfig;

pub(crate) type PanelActionLine = ActionLine<ActionId>;
pub(crate) type PanelActionSpan = ActionSpan<ActionId>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct RenderedPanelActionLine {
    pub line: Line<'static>,
    pub actions: Vec<PanelActionSpan>,
}

pub(crate) fn panel_action_span_at(
    actions: &[PanelActionSpan],
    content_column: u16,
) -> Option<PanelActionSpan> {
    actions
        .iter()
        .find(|span| (span.start..span.end).contains(&content_column))
        .cloned()
}

pub(crate) fn render_panel_action_line(
    action_line: &PanelActionLine,
    theme: &ThemeConfig,
    panel: Panel,
    mouse_target: Option<MouseTarget>,
) -> RenderedPanelActionLine {
    RenderedPanelActionLine {
        line: styled_panel_action_line(action_line, theme, panel, mouse_target),
        actions: action_line.actions.clone(),
    }
}

pub(crate) fn styled_panel_action_line(
    action_line: &PanelActionLine,
    theme: &ThemeConfig,
    panel: Panel,
    mouse_target: Option<MouseTarget>,
) -> Line<'static> {
    let mut spans = Vec::new();
    let mut cursor = 0usize;

    for action in &action_line.actions {
        push_text_span(
            &mut spans,
            action_line.text_before(action.byte_start, cursor),
            theme.text_style(),
        );
        let hovered =
            mouse_target.is_some_and(|target| target.panel_action_hovered(panel, action.action));
        let style = if hovered {
            theme.selected_style().add_modifier(Modifier::BOLD)
        } else {
            theme.accent_style()
        };
        push_text_span(&mut spans, action_line.action_text(action), style);
        cursor = action.byte_end;
    }

    push_text_span(
        &mut spans,
        action_line.text_after(cursor),
        theme.text_style(),
    );
    Line::from(spans)
}

fn push_text_span(spans: &mut Vec<Span<'static>>, text: &str, style: ratatui::style::Style) {
    if !text.is_empty() {
        spans.push(Span::styled(text.to_string(), style));
    }
}
