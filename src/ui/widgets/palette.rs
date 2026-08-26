//! Command palette (Ctrl+K). Host actions first; gateway catalog mixed in when filtering.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::theme;
use crate::ui::keys::typed_char;
use crate::ui::widgets::SlashItem;

const VISIBLE: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteTag {
    Host,
    Hermes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteRun {
    /// Host verb handled in Chat (`new`, `sessions`, `rewind`, …).
    Host(&'static str),
    /// Slash line, including a leading `/`.
    Slash(String),
}

#[derive(Debug, Clone)]
pub struct PaletteItem {
    pub needle: String,
    pub label: String,
    pub hint: String,
    pub tag: PaletteTag,
    pub run: PaletteRun,
}

#[derive(Debug, Default)]
pub struct Palette {
    active: bool,
    query: String,
    selected: usize,
    scroll: usize,
    catalog: Vec<SlashItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteAction {
    Close,
    Run(PaletteRun),
}

impl Palette {
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn set_catalog(&mut self, commands: Vec<SlashItem>) {
        self.catalog = commands;
    }

    pub fn open(&mut self) {
        self.active = true;
        self.query.clear();
        self.selected = 0;
        self.scroll = 0;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.query.clear();
        self.selected = 0;
        self.scroll = 0;
    }

    pub fn host_items() -> Vec<PaletteItem> {
        [
            ("sessions", "Resume / sessions", "saved and live"),
            ("new", "New session", "session.create"),
            ("rewind", "Rewind", "regenerate from a user turn"),
            ("model", "Model", "switch or add a provider"),
            ("skills", "Skills", "browse and install"),
            ("plugins", "Plugins", "toggle"),
            ("mcp", "MCP servers", "add or remove"),
            ("agents", "Agents", "subagents"),
            ("trees", "Spawn trees", "list / load / save"),
            ("usage", "Usage", "tokens and cost"),
            ("theme", "Skin", "color theme"),
            ("custom", "Chrome", "meter and hints"),
            ("copy", "Copy last reply", "/copy"),
            ("help", "Help", "keys and commands"),
        ]
        .into_iter()
        .map(|(id, label, hint)| PaletteItem {
            needle: id.to_string(),
            label: label.into(),
            hint: hint.into(),
            tag: PaletteTag::Host,
            run: PaletteRun::Host(id),
        })
        .collect()
    }

    pub fn items(&self) -> Vec<PaletteItem> {
        let q = self.query.to_ascii_lowercase();
        let mut items = Self::host_items();
        if !q.is_empty() {
            for c in &self.catalog {
                if items
                    .iter()
                    .any(|i| matches!(&i.run, PaletteRun::Host(h) if *h == c.name))
                {
                    continue;
                }
                items.push(PaletteItem {
                    needle: c.name.clone(),
                    label: format!("/{}", c.name),
                    hint: c.help.clone(),
                    tag: PaletteTag::Hermes,
                    run: PaletteRun::Slash(format!("/{}", c.name)),
                });
            }
        }
        if q.is_empty() {
            return items;
        }
        items
            .into_iter()
            .filter(|i| {
                i.needle.to_ascii_lowercase().contains(&q)
                    || i.label.to_ascii_lowercase().contains(&q)
                    || i.hint.to_ascii_lowercase().contains(&q)
            })
            .collect()
    }

    fn clamp(&mut self) {
        let n = self.items().len();
        if n == 0 {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        self.selected = self.selected.min(n - 1);
        let window = VISIBLE.min(n);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + window {
            self.scroll = self.selected + 1 - window;
        }
        self.scroll = self.scroll.min(n.saturating_sub(window));
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<PaletteAction> {
        if !self.active {
            return None;
        }
        if key.code == KeyCode::Esc {
            return Some(PaletteAction::Close);
        }
        match key.code {
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                self.clamp();
                None
            }
            KeyCode::Down => {
                let n = self.items().len();
                if n > 0 {
                    self.selected = (self.selected + 1).min(n - 1);
                    self.clamp();
                }
                None
            }
            KeyCode::Enter => {
                let items = self.items();
                items
                    .get(self.selected)
                    .map(|i| PaletteAction::Run(i.run.clone()))
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.selected = 0;
                self.scroll = 0;
                None
            }
            _ => {
                if let Some(c) = typed_char(&key) {
                    if !c.is_control() {
                        self.query.push(c);
                        self.selected = 0;
                        self.scroll = 0;
                    }
                }
                None
            }
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        if !self.active {
            return;
        }
        let rect = palette_rect(area);
        f.render_widget(Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                " command ",
                theme::agent().add_modifier(Modifier::BOLD),
            ))
            .border_style(theme::hairline())
            .style(Style::default().bg(theme::SURFACE()).fg(theme::TEXT()));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let items = self.items();
        let mut lines = vec![Line::from(vec![
            Span::styled("› ", theme::accent()),
            Span::styled(
                if self.query.is_empty() {
                    "type to filter".into()
                } else {
                    self.query.clone()
                },
                if self.query.is_empty() {
                    theme::dim()
                } else {
                    theme::text()
                },
            ),
        ])];
        if items.is_empty() {
            lines.push(Line::from(Span::styled("no matches", theme::dim())));
        }
        let n = items.len();
        let window = VISIBLE.min(n.saturating_sub(self.scroll));
        let end = self.scroll + window;
        for (i, item) in items.iter().enumerate().take(end).skip(self.scroll) {
            let sel = i == self.selected;
            let mark = if sel { "▸ " } else { "  " };
            let tag = match item.tag {
                PaletteTag::Host => "",
                PaletteTag::Hermes => "  (Hermes)",
            };
            let label = format!("{mark}{}", item.label);
            let hint = format!("  {}{tag}", item.hint);
            if sel {
                lines.push(Line::from(Span::styled(
                    format!("{label}{hint}"),
                    theme::selected(),
                )));
            } else {
                lines.push(Line::from(vec![
                    Span::styled(label, theme::text()),
                    Span::styled(hint, theme::dim()),
                ]));
            }
        }
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
    }
}

pub fn palette_rect(area: Rect) -> Rect {
    let w = area.width.saturating_sub(8).min(72).max(36);
    let h = 14u16.min(area.height.saturating_sub(4)).max(8);
    Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + area.height.saturating_sub(h).saturating_sub(2) / 3,
        width: w,
        height: h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_is_host_only() {
        let mut p = Palette::default();
        p.set_catalog(vec![SlashItem {
            name: "compress".into(),
            help: "summarize".into(),
        }]);
        p.open();
        assert!(p.items().iter().all(|i| i.tag == PaletteTag::Host));
        assert!(p.items().iter().any(|i| i.needle == "new"));
        p.query = "comp".into();
        assert!(p
            .items()
            .iter()
            .any(|i| matches!(&i.run, PaletteRun::Slash(s) if s == "/compress")));
    }

    #[test]
    fn enter_runs_selected_host() {
        let mut p = Palette::default();
        p.open();
        let act = p.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(act, Some(PaletteAction::Run(PaletteRun::Host("sessions"))));
    }
}
