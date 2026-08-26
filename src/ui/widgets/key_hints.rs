use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::theme;

#[derive(Debug, Clone)]
pub struct KeyHints {
    items: Vec<(&'static str, &'static str)>,
}

impl KeyHints {
    pub fn new(items: Vec<(&'static str, &'static str)>) -> Self {
        Self { items }
    }

    /// Idle composer: only actions that do something right now.
    pub fn idle(has_tools: bool, can_rewind: bool) -> Self {
        let mut items = vec![("Ctrl+K", "palette"), ("/", "commands"), ("!", "shell")];
        if has_tools {
            items.push(("Ctrl+O", "expand tool"));
        }
        if can_rewind {
            items.push(("/rewind", "edit"));
        }
        Self::new(items)
    }

    pub fn returning() -> Self {
        Self::new(vec![
            ("1–3", "resume"),
            ("Enter", "resume"),
            ("Ctrl+K", "palette"),
            ("/", "sessions"),
        ])
    }

    pub fn streaming() -> Self {
        Self::new(vec![
            ("Enter", "steer"),
            ("Ctrl+Enter", "queue"),
            ("Esc", "interrupt"),
        ])
    }

    pub fn slash() -> Self {
        Self::new(vec![("↑↓", "select"), ("Enter", "run"), ("Esc", "close")])
    }

    pub fn overlay() -> Self {
        Self::new(vec![
            ("↑↓", "select"),
            ("Enter", "confirm"),
            ("Esc", "dismiss"),
        ])
    }

    pub fn confirm_quit() -> Self {
        Self::new(vec![("y", "quit"), ("Esc", "cancel")])
    }
}

impl Widget for KeyHints {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut spans: Vec<Span> = Vec::new();
        for (i, (key, desc)) in self.items.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled("  ·  ", theme::dim()));
            }
            spans.push(Span::styled(*key, theme::accent()));
            spans.push(Span::styled(format!(" {desc}"), theme::dim()));
        }
        Paragraph::new(Line::from(spans)).render(area, buf);
    }
}
