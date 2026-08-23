use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::theme;

#[derive(Debug, Clone, Copy)]
pub struct KeyHints<'a> {
    items: &'a [(&'a str, &'a str)],
}

impl<'a> KeyHints<'a> {
    pub fn new(items: &'a [(&'a str, &'a str)]) -> Self {
        Self { items }
    }

    pub fn idle() -> Self {
        Self::new(&[
            ("Enter", "send"),
            ("/", "commands"),
            ("Esc", "interrupt"),
            ("Ctrl+O", "tool"),
            ("Ctrl+V", "paste img"),
            ("Ctrl+C", "quit"),
        ])
    }

    pub fn streaming() -> Self {
        Self::new(&[("Esc", "interrupt"), ("Ctrl+C", "quit")])
    }

    pub fn slash() -> Self {
        Self::new(&[("↑↓", "select"), ("Enter", "run"), ("Esc", "close")])
    }

    pub fn overlay() -> Self {
        Self::new(&[("↑↓", "select"), ("Enter", "confirm"), ("Esc", "dismiss")])
    }

    pub fn confirm_quit() -> Self {
        Self::new(&[("y", "quit"), ("Esc", "cancel")])
    }
}

impl Widget for KeyHints<'_> {
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
