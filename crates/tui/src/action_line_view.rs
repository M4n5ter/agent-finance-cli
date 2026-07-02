use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ActionLine<A> {
    pub text: String,
    pub actions: Vec<ActionSpan<A>>,
    width: u16,
    used_width: u16,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ActionSpan<A> {
    pub label: String,
    pub start: u16,
    pub end: u16,
    pub byte_start: usize,
    pub byte_end: usize,
    pub action: A,
}

impl<A> ActionLine<A> {
    pub(crate) fn new(text: impl AsRef<str>, width: u16) -> Self {
        let mut line = Self {
            text: String::new(),
            actions: Vec::new(),
            width,
            used_width: 0,
        };
        line.push_visible_text(text.as_ref());
        line
    }

    pub(crate) fn push_visible_text(&mut self, text: &str) {
        let remaining = self.remaining_width();
        if remaining == 0 {
            return;
        }

        let mut byte_end = 0usize;
        let mut added_width = 0u16;
        for (index, character) in text.char_indices() {
            let character_width = character.width().unwrap_or(0) as u16;
            if added_width.saturating_add(character_width) > remaining {
                break;
            }
            byte_end = index + character.len_utf8();
            added_width = added_width.saturating_add(character_width);
        }

        self.text.push_str(&text[..byte_end]);
        self.used_width = self.used_width.saturating_add(added_width);
    }

    pub(crate) fn push_visible_action(&mut self, label: impl AsRef<str>, action: A) {
        let label = label.as_ref();
        let label_width = UnicodeWidthStr::width(label) as u16;
        if label_width > self.remaining_width() {
            return;
        }

        let start = self.used_width;
        let byte_start = self.text.len();
        self.text.push_str(label);
        self.used_width = self.used_width.saturating_add(label_width);
        self.actions.push(ActionSpan {
            label: label.to_string(),
            start,
            end: self.used_width,
            byte_start,
            byte_end: self.text.len(),
            action,
        });
    }

    pub(crate) fn text_before(&self, byte_end: usize, cursor: usize) -> &str {
        &self.text[cursor..byte_end]
    }

    pub(crate) fn action_text(&self, span: &ActionSpan<A>) -> &str {
        &self.text[span.byte_start..span.byte_end]
    }

    pub(crate) fn text_after(&self, cursor: usize) -> &str {
        &self.text[cursor..]
    }

    fn remaining_width(&self) -> u16 {
        self.width.saturating_sub(self.used_width)
    }
}

impl<A: Copy> ActionLine<A> {
    pub(crate) fn action_at(&self, content_column: u16) -> Option<ActionSpan<A>> {
        self.actions
            .iter()
            .find(|span| (span.start..span.end).contains(&content_column))
            .cloned()
    }
}

pub(crate) fn right_aligned_action_line<A: Clone>(
    width: u16,
    text: &str,
    min_gap: u16,
    actions: &[(&str, A)],
) -> ActionLine<A> {
    let action_width = actions_width(actions);
    let mut line = ActionLine::new("", width);
    if actions.is_empty() || width <= action_width {
        line.push_visible_text(text);
        return line;
    }

    let text_width = width.saturating_sub(action_width).saturating_sub(min_gap);
    line.push_visible_text(&truncate_to_width(text, text_width));
    line.push_visible_text(
        &" ".repeat(width.saturating_sub(action_width + line.used_width) as usize),
    );
    for (index, (label, action)) in actions.iter().enumerate() {
        if index > 0 {
            line.push_visible_text(" ");
        }
        line.push_visible_action(label, action.clone());
    }
    line
}

fn actions_width<A>(actions: &[(&str, A)]) -> u16 {
    let labels_width = actions
        .iter()
        .map(|(label, _)| UnicodeWidthStr::width(*label) as u16)
        .fold(0u16, u16::saturating_add);
    labels_width.saturating_add(actions.len().saturating_sub(1) as u16)
}

fn truncate_to_width(text: &str, width: u16) -> String {
    let mut output = String::new();
    let mut used = 0u16;
    for character in text.chars() {
        let character_width = character.width().unwrap_or(0) as u16;
        if used.saturating_add(character_width) > width {
            break;
        }
        output.push(character);
        used = used.saturating_add(character_width);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    enum TestAction {
        Run,
    }

    #[test]
    fn action_spans_use_terminal_cells_without_slicing_utf8_boundaries() {
        let mut line = ActionLine::new("市场", 12);
        line.push_visible_text("  ");
        line.push_visible_action("[run]", TestAction::Run);

        let span = line.action_at(6).expect("action starts after cjk text");

        assert_eq!(line.text, "市场  [run]");
        assert_eq!(span.start, 6);
        assert_eq!(span.end, 11);
        assert_eq!(line.action_text(&span), "[run]");
        assert_eq!(line.action_at(5), None);
    }

    #[test]
    fn visible_text_truncates_at_cell_boundary() {
        let mut line = ActionLine::<TestAction>::new("市场数据", 4);
        line.push_visible_text("x");

        assert_eq!(line.text, "市场");
        assert_eq!(UnicodeWidthStr::width(line.text.as_str()), 4);
        assert!(line.actions.is_empty());
    }

    #[test]
    fn right_aligned_actions_reserve_visible_action_width() {
        let line =
            right_aligned_action_line(20, "BTCUSDT position", 2, &[("[state]", TestAction::Run)]);

        let action = line.action_at(13).expect("right aligned action");

        assert_eq!(line.text, "BTCUSDT pos  [state]");
        assert_eq!(action.start, 13);
        assert_eq!(action.end, 20);
        assert_eq!(line.action_at(12), None);
    }

    #[test]
    fn right_aligned_actions_are_hidden_when_narrow() {
        let line = right_aligned_action_line(7, "BTCUSDT", 2, &[("[state]", TestAction::Run)]);

        assert_eq!(line.text, "BTCUSDT");
        assert!(line.actions.is_empty());
        assert_eq!(line.action_at(0), None);
    }
}
