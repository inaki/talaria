//! Drop-up slash palette. Commands come from `commands.catalog`, not a hardcoded list.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};
use ratatui::Frame;

use crate::theme;

const VISIBLE: usize = 8;

#[derive(Debug, Clone)]
pub struct SlashItem {
    pub name: String,
    pub help: String,
}

#[derive(Debug, Default)]
pub struct SlashMenu {
    commands: Vec<SlashItem>,
    query: String,
    selected: usize,
    scroll: usize,
    active: bool,
}

impl SlashMenu {
    pub fn set_commands(&mut self, commands: Vec<SlashItem>) {
        self.commands = commands;
        self.selected = 0;
        self.scroll = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
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
        self.scroll = 0;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn prompt_value(&self) -> String {
        if self.query.is_empty() {
            "/".into()
        } else {
            format!("/{}", self.query)
        }
    }

    pub fn append(&mut self, c: char) {
        if !self.active {
            return;
        }
        self.query.push(c);
        self.selected = 0;
        self.scroll = 0;
    }

    /// Returns true if the menu closed (backspace on empty query).
    pub fn backspace(&mut self) -> bool {
        if !self.active {
            return false;
        }
        if self.query.is_empty() {
            self.close();
            return true;
        }
        self.query.pop();
        self.selected = 0;
        self.scroll = 0;
        false
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.ensure_visible();
        }
    }

    pub fn move_down(&mut self) {
        let n = self.filtered().len();
        if n == 0 {
            return;
        }
        if self.selected + 1 < n {
            self.selected += 1;
            self.ensure_visible();
        }
    }

    fn ensure_visible(&mut self) {
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + VISIBLE {
            self.scroll = self.selected + 1 - VISIBLE;
        }
    }

    pub fn filtered(&self) -> Vec<&SlashItem> {
        let q = self.query.to_ascii_lowercase();
        // If the user typed args (`resume foo`), filter on the command token.
        let token = q.split_whitespace().next().unwrap_or(&q);
        self.commands
            .iter()
            .filter(|c| {
                if token.is_empty() {
                    return true;
                }
                c.name.to_ascii_lowercase().contains(token)
                    || c.help.to_ascii_lowercase().contains(token)
            })
            .collect()
    }

    /// Command line to dispatch, including any typed args.
    pub fn selected_dispatch(&self) -> Option<String> {
        let filtered = self.filtered();
        if filtered.is_empty() {
            if self.query.is_empty() {
                return None;
            }
            return Some(format!("/{}", self.query.trim()));
        }
        let idx = self.selected.min(filtered.len() - 1);
        let name = &filtered[idx].name;
        let rest = self
            .query
            .split_once(char::is_whitespace)
            .map(|(_, r)| r.trim())
            .unwrap_or("");
        if rest.is_empty() {
            Some(format!("/{name}"))
        } else {
            Some(format!("/{name} {rest}"))
        }
    }

    pub fn height(&self) -> u16 {
        let n = self.filtered().len().clamp(1, VISIBLE) as u16;
        n.saturating_add(2)
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        if !self.active {
            return;
        }
        let h = self.height().min(area.height);
        let y = area.y.saturating_add(area.height.saturating_sub(h));
        let rect = Rect {
            x: area.x,
            y,
            width: area.width,
            height: h,
        };
        f.render_widget(Clear, rect);
        let filtered = self.filtered();
        let items: Vec<ListItem> = if filtered.is_empty() {
            vec![ListItem::new(Span::styled(
                "no matching commands",
                theme::dim(),
            ))]
        } else {
            filtered
                .iter()
                .skip(self.scroll)
                .take(VISIBLE)
                .map(|c| {
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("/{}  ", c.name), theme::accent()),
                        Span::styled(c.help.clone(), theme::dim()),
                    ]))
                })
                .collect()
        };
        let mut state = ListState::default();
        if !filtered.is_empty() {
            state.select(Some(self.selected.saturating_sub(self.scroll)));
        }
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" commands ")
                    .border_style(theme::accent())
                    .style(Style::default().bg(theme::SURFACE()).fg(theme::TEXT())),
            )
            .highlight_style(
                Style::default()
                    .fg(theme::TEXT())
                    .add_modifier(Modifier::BOLD)
                    .bg(theme::BACKGROUND()),
            );
        f.render_stateful_widget(list, rect, &mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_and_dispatch_with_args() {
        let mut m = SlashMenu::default();
        m.set_commands(vec![
            SlashItem {
                name: "resume".into(),
                help: "resume a session".into(),
            },
            SlashItem {
                name: "help".into(),
                help: "show help".into(),
            },
        ]);
        m.open();
        m.append('r');
        assert_eq!(m.filtered().len(), 1);
        assert_eq!(m.selected_dispatch().as_deref(), Some("/resume"));
        m.append(' ');
        m.append('a');
        m.append('b');
        assert_eq!(m.selected_dispatch().as_deref(), Some("/resume ab"));
    }
}
