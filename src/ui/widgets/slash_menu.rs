//! Drop-up slash palette (herald_v2 SlashMenu layout).
//!
//! Commands still come from `commands.catalog` (`SlashItem`), not a hardcoded list.
//! Visual contract matches herald: `›` selector, aligned `/name` + help columns,
//! selection row, flush above the composer, hidden when the filter matches nothing.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::theme;

/// Max rows painted in the drop-up menu at once (window height).
const SLASH_MENU_VISIBLE: usize = 8;

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
    /// First filtered-index visible in the window (scroll offset).
    scroll: usize,
    active: bool,
}

impl SlashMenu {
    pub fn set_commands(&mut self, commands: Vec<SlashItem>) {
        self.commands = commands;
        self.selected = 0;
        self.scroll = 0;
    }

    pub fn commands(&self) -> &[SlashItem] {
        &self.commands
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

    /// Text shown in the prompt while the menu is open (`/` + filter query).
    pub fn prompt_value(&self) -> String {
        if self.query.is_empty() {
            "/".into()
        } else {
            format!("/{}", self.query)
        }
    }

    pub fn query(&self) -> &str {
        &self.query
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
        if self.active && self.selected > 0 {
            self.selected -= 1;
            self.ensure_selected_visible(self.filtered().len());
        }
    }

    pub fn move_down(&mut self) {
        if !self.active {
            return;
        }
        let len = self.filtered().len();
        let max = len.saturating_sub(1);
        if self.selected < max {
            self.selected += 1;
            self.ensure_selected_visible(len);
        }
    }

    fn ensure_selected_visible(&mut self, filtered_len: usize) {
        if filtered_len == 0 {
            self.scroll = 0;
            self.selected = 0;
            return;
        }
        self.selected = self.selected.min(filtered_len - 1);
        let window = SLASH_MENU_VISIBLE.min(filtered_len);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + window {
            self.scroll = self.selected + 1 - window;
        }
        let max_scroll = filtered_len.saturating_sub(window);
        self.scroll = self.scroll.min(max_scroll);
    }

    /// True when the filter includes whitespace — typed command plus args.
    pub fn has_typed_args(&self) -> bool {
        self.query.chars().any(char::is_whitespace)
    }

    pub fn filtered(&self) -> Vec<&SlashItem> {
        // Match the command name only (e.g. `/resume`), not help text.
        // If the user typed args (`resume foo`), filter on the command token.
        let q = self.query.to_ascii_lowercase();
        let token = q.split_whitespace().next().unwrap_or(&q);
        self.commands
            .iter()
            .filter(|c| {
                if token.is_empty() {
                    return true;
                }
                c.name.to_ascii_lowercase().contains(token)
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

    /// Drop-up menu anchored directly above the prompt input (`input_area`).
    pub fn render_above_input(&self, f: &mut Frame, input_area: Rect) {
        if !self.active {
            return;
        }

        let filtered = self.filtered();
        // Don't paint an empty box when the filter matches nothing.
        if filtered.is_empty() {
            return;
        }

        let total = filtered.len();
        let window = SLASH_MENU_VISIBLE.min(total);
        let scroll = self.scroll.min(total.saturating_sub(window));
        let selected = self.selected.min(total - 1);
        let end = (scroll + window).min(total);
        let visible = &filtered[scroll..end];

        let max_label = visible
            .iter()
            .map(|c| c.name.len() + 1) // leading '/'
            .max()
            .unwrap_or(0);

        let mut lines: Vec<Line> = Vec::new();
        for (row, cmd) in visible.iter().enumerate() {
            let idx = scroll + row;
            let selected_row = idx == selected;
            let bg = if selected_row {
                theme::SELECTION_BG()
            } else {
                None
            };
            // "› " and "  " are both exactly 2 terminal cells so the label
            // column never shifts when the selection moves.
            let selector = if selected_row {
                Span::styled("› ", row_style(theme::USER(), bg))
            } else {
                Span::raw("  ")
            };
            let label_style = if selected_row {
                row_style(theme::SELECTED_TEXT(), bg).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme::PRIMARY())
            };
            let label = format!("/{}", cmd.name);
            let padded_label = format!("{label:<max_label$}");
            lines.push(Line::from(vec![
                selector,
                Span::styled(padded_label, label_style),
                Span::raw("  "),
                Span::styled(
                    cmd.help.clone(),
                    if selected_row {
                        row_style(theme::SELECTED_TEXT(), bg)
                    } else {
                        Style::default().fg(theme::TEXT_DIM())
                    },
                ),
            ]));
        }

        let para = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::SEPARATOR()))
                    .style(Style::default().bg(theme::BACKGROUND())),
            )
            .wrap(Wrap { trim: false });

        let menu_height = (visible.len() as u16 + 2).min(12);
        let menu_area = menu_rect_above_input(input_area, menu_height);

        f.render_widget(Clear, menu_area);
        f.render_widget(para, menu_area);
    }
}

fn row_style(fg: ratatui::style::Color, bg: Option<ratatui::style::Color>) -> Style {
    if let Some(bg) = bg {
        Style::default().fg(fg).bg(bg)
    } else {
        Style::default().fg(fg)
    }
}

fn menu_rect_above_input(input_area: Rect, menu_height: u16) -> Rect {
    Rect {
        x: input_area.x,
        y: input_area.y.saturating_sub(menu_height),
        width: input_area.width,
        height: menu_height,
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

    #[test]
    fn filter_matches_name_not_help() {
        let mut m = SlashMenu::default();
        m.set_commands(vec![SlashItem {
            name: "clear".into(),
            help: "wipe the transcript".into(),
        }]);
        m.open();
        m.append('w');
        assert!(m.filtered().is_empty());
        m.backspace();
        m.append('c');
        assert_eq!(m.filtered().len(), 1);
    }

    #[test]
    fn menu_rect_sits_flush_above_input() {
        let input = Rect::new(0, 20, 80, 2);
        let menu = menu_rect_above_input(input, 5);
        assert_eq!(menu.y, 15);
        assert_eq!(menu.height, 5);
        assert_eq!(menu.x, input.x);
        assert_eq!(menu.width, input.width);
    }

    #[test]
    fn arrow_keys_scroll_window_through_full_list() {
        let cmds: Vec<SlashItem> = (0..12)
            .map(|i| SlashItem {
                name: format!("{}", (b'a' + i) as char),
                help: "x".into(),
            })
            .collect();
        let mut menu = SlashMenu::default();
        menu.set_commands(cmds);
        menu.open();
        assert_eq!(menu.selected, 0);
        assert_eq!(menu.scroll, 0);

        for _ in 0..8 {
            menu.move_down();
        }
        assert_eq!(menu.selected, 8);
        assert!(
            menu.scroll > 0,
            "window should scroll so selection stays visible"
        );
        assert!(menu.selected >= menu.scroll);
        assert!(menu.selected < menu.scroll + SLASH_MENU_VISIBLE);

        menu.move_up();
        assert_eq!(menu.selected, 7);
    }

    #[test]
    fn has_typed_args_detects_command_plus_args() {
        let mut menu = SlashMenu::default();
        menu.set_commands(vec![SlashItem {
            name: "resume".into(),
            help: "resume a session".into(),
        }]);
        menu.open();
        for c in "resume".chars() {
            menu.append(c);
        }
        assert!(!menu.has_typed_args());
        assert_eq!(menu.prompt_value(), "/resume");
        menu.append(' ');
        for c in "store-1".chars() {
            menu.append(c);
        }
        assert!(menu.has_typed_args());
        assert_eq!(menu.selected_dispatch().as_deref(), Some("/resume store-1"));
        assert_eq!(menu.query(), "resume store-1");
    }
}
