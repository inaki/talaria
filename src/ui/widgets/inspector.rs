//! Peek panel: one tool, thinking block, usage, or help.
//!
//! Does not rewrite the transcript. Copy is a first-class verb.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::session::UsageSnapshot;
use crate::theme;
use crate::ui::keys::is_ctrl_u;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorKind {
    Tool,
    Thinking,
    Usage,
    Help,
}

#[derive(Debug, Clone)]
pub struct Inspector {
    pub kind: InspectorKind,
    pub title: String,
    pub subtitle: String,
    pub lines: Vec<String>,
    pub scroll: u16,
    pub loading: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InspectorAction {
    Close,
    Copy,
}

impl Inspector {
    pub fn tool(title: impl Into<String>, subtitle: impl Into<String>, body: Vec<String>) -> Self {
        Self {
            kind: InspectorKind::Tool,
            title: title.into(),
            subtitle: subtitle.into(),
            lines: body,
            scroll: 0,
            loading: false,
        }
    }

    pub fn thinking(body: Vec<String>) -> Self {
        Self {
            kind: InspectorKind::Thinking,
            title: "thinking".into(),
            subtitle: String::new(),
            lines: body,
            scroll: 0,
            loading: false,
        }
    }

    pub fn usage_loading() -> Self {
        Self {
            kind: InspectorKind::Usage,
            title: "usage".into(),
            subtitle: String::new(),
            lines: vec!["loading…".into()],
            scroll: 0,
            loading: true,
        }
    }

    pub fn set_usage(&mut self, u: &UsageSnapshot) {
        self.loading = false;
        self.subtitle = u.model.clone();
        let mut lines = Vec::new();
        if u.calls > 0 {
            lines.push(format!(
                "calls {}   in {}   out {}   total {}",
                u.calls, u.input, u.output, u.total
            ));
        }
        if u.context_max > 0 {
            lines.push(format!(
                "context  {} / {}  ({}%)",
                u.context_used, u.context_max, u.context_percent
            ));
        }
        if let Some(c) = u.cost_usd {
            lines.push(format!("cost  ${c:.4}"));
        }
        for line in &u.credits_lines {
            lines.push(line.clone());
        }
        if lines.is_empty() {
            lines.push("no API calls yet".into());
        }
        self.lines = lines;
        self.scroll = 0;
    }

    pub fn help(lines: Vec<String>) -> Self {
        Self {
            kind: InspectorKind::Help,
            title: "help".into(),
            subtitle: format!("Talaria v{}", env!("CARGO_PKG_VERSION")),
            lines,
            scroll: 0,
            loading: false,
        }
    }

    pub fn body_text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<InspectorAction> {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
            return Some(InspectorAction::Close);
        }
        if matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C')) || is_ctrl_u(&key) {
            return Some(InspectorAction::Copy);
        }
        match key.code {
            KeyCode::Up | KeyCode::PageUp => {
                let step = if key.code == KeyCode::PageUp { 8 } else { 1 };
                self.scroll = self.scroll.saturating_sub(step);
            }
            KeyCode::Down | KeyCode::PageDown => {
                let step = if key.code == KeyCode::PageDown { 8 } else { 1 };
                let max = self.lines.len().saturating_sub(1) as u16;
                self.scroll = self.scroll.saturating_add(step).min(max);
            }
            _ => {}
        }
        None
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let rect = inspector_rect(area);
        f.render_widget(Clear, rect);
        let title = if self.subtitle.is_empty() {
            format!(" {} ", self.title)
        } else {
            format!(" {} · {} ", self.title, self.subtitle)
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                title,
                theme::agent().add_modifier(Modifier::BOLD),
            ))
            .border_style(theme::hairline())
            .style(Style::default().bg(theme::SURFACE()).fg(theme::TEXT()));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let vis = inner.height.max(1) as usize;
        let total = self.lines.len();
        let start = (self.scroll as usize).min(total.saturating_sub(1));
        let end = (start + vis).min(total);
        let hidden = total.saturating_sub(end);
        let mut out: Vec<Line> = self.lines[start..end]
            .iter()
            .map(|l| Line::from(Span::styled(l.clone(), theme::text())))
            .collect();
        if hidden > 0 {
            out.push(Line::from(Span::styled(
                format!("+{hidden} lines · ↓"),
                theme::dim(),
            )));
        }
        out.push(Line::from(""));
        out.push(Line::from(Span::styled(
            "c copy  ·  Esc close",
            theme::dim().add_modifier(Modifier::DIM),
        )));
        f.render_widget(Paragraph::new(out).wrap(Wrap { trim: false }), inner);
    }
}

pub fn inspector_rect(area: Rect) -> Rect {
    let w = area.width.saturating_sub(6).min(84).max(28);
    let h = area.height.saturating_sub(6).min(28).max(10);
    Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + 1,
        width: w,
        height: h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_joins_body() {
        let ins = Inspector::tool("terminal", "done", vec!["$ ls".into(), "a".into()]);
        assert!(ins.body_text().contains("$ ls"));
        assert!(ins.body_text().contains('\n'));
    }

    #[test]
    fn usage_fills_from_snapshot() {
        let mut ins = Inspector::usage_loading();
        assert!(ins.loading);
        ins.set_usage(&UsageSnapshot {
            calls: 2,
            input: 10,
            output: 4,
            total: 14,
            context_used: 100,
            context_max: 1000,
            context_percent: 10,
            cost_usd: Some(0.01),
            model: "ox".into(),
            credits_lines: vec!["$1 left".into()],
        });
        assert!(!ins.loading);
        assert!(ins.lines.iter().any(|l| l.contains("calls 2")));
        assert_eq!(ins.subtitle, "ox");
    }

    #[test]
    fn esc_closes() {
        let mut ins = Inspector::help(vec!["hi".into()]);
        assert_eq!(
            ins.handle_key(KeyEvent::from(KeyCode::Esc)),
            Some(InspectorAction::Close)
        );
    }
}
