//! Management sheet: searchable list, optional tabs, does not steal the stream.
//!
//! Wide terminals: docks on the right. Narrow: a tall overlay above the composer.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::session::{ActiveSession, RewindTurn, SavedSession, SpawnTreeEntry, SubagentRow};
use crate::theme;
use crate::ui::keys::{is_ctrl_d, is_ctrl_n, typed_char};
use crate::ui::widgets::selector;
use unicode_width::UnicodeWidthStr;

const VISIBLE: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetTab {
    Saved,
    Live,
}

#[derive(Debug, Clone)]
pub enum SheetKind {
    Sessions {
        tab: SheetTab,
        saved: Vec<SavedSession>,
        live: Vec<ActiveSession>,
    },
    Trees {
        entries: Vec<SpawnTreeEntry>,
    },
    Theme {
        saved_id: &'static str,
    },
    Custom,
    Rewind {
        turns: Vec<RewindTurn>,
        confirming: bool,
    },
    Agents {
        agents: Vec<SubagentRow>,
        steering: bool,
        draft: String,
    },
}

#[derive(Debug, Clone)]
pub struct SheetRow {
    pub id: String,
    pub title: String,
    pub subtitle: String,
}

#[derive(Debug, Clone)]
pub struct Sheet {
    pub kind: SheetKind,
    pub query: String,
    pub selected: usize,
    pub scroll: usize,
    pub loading: bool,
    pub notice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetAction {
    Close,
    /// Key consumed by the sheet; Chat should not close it.
    Handled,
    Confirm {
        id: String,
    },
    New,
    CloseLive {
        id: String,
    },
    Save,
    RestoreTheme {
        id: &'static str,
    },
    ApplyTheme {
        id: &'static str,
    },
    ToggleCustom,
    Rewind {
        text: String,
        truncate_before_row_id: i64,
        confirm_empty_truncate: bool,
    },
    InterruptAgent {
        id: String,
    },
    SteerAgent {
        id: String,
        text: String,
    },
}

impl Sheet {
    pub fn sessions() -> Self {
        Self {
            kind: SheetKind::Sessions {
                tab: SheetTab::Saved,
                saved: Vec::new(),
                live: Vec::new(),
            },
            query: String::new(),
            selected: 0,
            scroll: 0,
            loading: true,
            notice: None,
        }
    }

    pub fn trees() -> Self {
        Self {
            kind: SheetKind::Trees {
                entries: Vec::new(),
            },
            query: String::new(),
            selected: 0,
            scroll: 0,
            loading: true,
            notice: None,
        }
    }

    pub fn theme() -> Self {
        let saved_id = crate::theme::current_theme_id();
        let selected = crate::theme::themes()
            .iter()
            .position(|t| t.id == saved_id)
            .unwrap_or(0);
        Self {
            kind: SheetKind::Theme { saved_id },
            query: String::new(),
            selected,
            scroll: 0,
            loading: false,
            notice: None,
        }
    }

    pub fn custom() -> Self {
        Self {
            kind: SheetKind::Custom,
            query: String::new(),
            selected: 0,
            scroll: 0,
            loading: false,
            notice: None,
        }
    }

    pub fn rewind(turns: Vec<RewindTurn>) -> Self {
        Self {
            kind: SheetKind::Rewind {
                turns,
                confirming: false,
            },
            query: String::new(),
            selected: 0,
            scroll: 0,
            loading: true,
            notice: None,
        }
    }

    pub fn agents(agents: Vec<SubagentRow>) -> Self {
        Self {
            kind: SheetKind::Agents {
                agents,
                steering: false,
                draft: String::new(),
            },
            query: String::new(),
            selected: 0,
            scroll: 0,
            loading: true,
            notice: None,
        }
    }

    pub fn title(&self) -> &'static str {
        match self.kind {
            SheetKind::Sessions { .. } => "sessions",
            SheetKind::Trees { .. } => "trees",
            SheetKind::Theme { .. } => "skin",
            SheetKind::Custom => "custom",
            SheetKind::Rewind {
                confirming: true, ..
            } => "confirm rewind",
            SheetKind::Rewind { .. } => "rewind",
            SheetKind::Agents { .. } => "agents",
        }
    }

    pub fn footer(&self) -> &'static str {
        match &self.kind {
            SheetKind::Sessions { tab, .. } => match tab {
                SheetTab::Saved => "Enter open  ·  Ctrl+N new  ·  Tab live  ·  Esc",
                SheetTab::Live => "Enter open  ·  Ctrl+D close  ·  Ctrl+N new  ·  Esc",
            },
            SheetKind::Trees { .. } => "Enter load  ·  Ctrl+S save  ·  Esc",
            SheetKind::Theme { .. } => "↑↓ preview  ·  Enter save  ·  Esc restore",
            SheetKind::Custom => "Enter toggle  ·  Esc",
            SheetKind::Rewind {
                confirming: true, ..
            } => "y / Enter confirm  ·  n / Esc",
            SheetKind::Rewind { .. } => "Enter select  ·  Esc",
            SheetKind::Agents { steering: true, .. } => "Enter steer  ·  Esc cancel",
            SheetKind::Agents { .. } => "Enter interrupt  ·  s steer  ·  Esc",
        }
    }

    pub fn set_saved(&mut self, saved: Vec<SavedSession>) {
        if let SheetKind::Sessions { saved: slot, .. } = &mut self.kind {
            *slot = saved;
        }
        self.loading = false;
        self.clamp_selection();
    }

    pub fn set_live(&mut self, live: Vec<ActiveSession>) {
        if let SheetKind::Sessions { live: slot, .. } = &mut self.kind {
            *slot = live;
        }
        self.loading = false;
        self.clamp_selection();
    }

    pub fn set_trees(&mut self, entries: Vec<SpawnTreeEntry>) {
        self.kind = SheetKind::Trees { entries };
        self.loading = false;
        self.clamp_selection();
    }

    pub fn set_rewind_turns(&mut self, turns: Vec<RewindTurn>) {
        if let SheetKind::Rewind {
            turns: slot,
            confirming,
        } = &mut self.kind
        {
            *slot = turns;
            *confirming = false;
        }
        self.loading = false;
        self.clamp_selection();
    }

    pub fn set_agents(&mut self, agents: Vec<SubagentRow>) {
        if let SheetKind::Agents {
            agents: slot,
            steering,
            draft,
        } = &mut self.kind
        {
            *slot = agents;
            *steering = false;
            draft.clear();
        }
        self.loading = false;
        self.clamp_selection();
    }

    /// Id of the highlighted row. Callers switch on this rather than on
    /// `selected`, so inserting a row cannot silently rebind a toggle.
    pub fn selected_id(&self) -> Option<String> {
        self.rows().get(self.selected).map(|r| r.id.clone())
    }

    pub fn rows(&self) -> Vec<SheetRow> {
        let q = self.query.to_ascii_lowercase();
        match &self.kind {
            SheetKind::Sessions { tab, saved, live } => match tab {
                SheetTab::Saved => saved
                    .iter()
                    .filter(|s| row_matches(&q, &s.title, &s.id, &s.preview))
                    .map(|s| SheetRow {
                        id: s.id.clone(),
                        title: if s.title.is_empty() {
                            s.id.clone()
                        } else {
                            s.title.clone()
                        },
                        subtitle: session_sub(s),
                    })
                    .collect(),
                SheetTab::Live => live
                    .iter()
                    .filter(|s| row_matches(&q, s.title.as_deref().unwrap_or(""), &s.id, &s.status))
                    .map(|s| SheetRow {
                        id: s.id.clone(),
                        title: s
                            .title
                            .clone()
                            .filter(|t| !t.is_empty())
                            .unwrap_or_else(|| s.id.clone()),
                        subtitle: format!(
                            "{}{}",
                            s.status,
                            if s.current { " · current" } else { "" }
                        ),
                    })
                    .collect(),
            },
            SheetKind::Trees { entries } => entries
                .iter()
                .filter(|e| row_matches(&q, &e.label, &e.path, ""))
                .map(|e| SheetRow {
                    id: e.path.clone(),
                    title: e.label.clone(),
                    subtitle: format!("{} nodes", e.count),
                })
                .collect(),
            SheetKind::Theme { saved_id } => crate::theme::themes()
                .iter()
                .filter(|t| row_matches(&q, t.label, t.id, t.description))
                .map(|t| SheetRow {
                    id: t.id.to_string(),
                    title: t.label.to_string(),
                    subtitle: format!(
                        "{}{}",
                        t.description,
                        if t.id == *saved_id { "  (saved)" } else { "" }
                    ),
                })
                .collect(),
            SheetKind::Custom => {
                let prefs = crate::prefs::get();
                vec![
                    SheetRow {
                        id: "status_bar".into(),
                        title: format!(
                            "status bar  {}",
                            if prefs.status_bar { "on" } else { "off" }
                        ),
                        subtitle: "model · context · clocks".into(),
                    },
                    SheetRow {
                        id: "key_hints".into(),
                        title: format!(
                            "key hints   {}",
                            if prefs.key_hints { "on" } else { "off" }
                        ),
                        subtitle: "shortcut row under the prompt".into(),
                    },
                    SheetRow {
                        id: "rail".into(),
                        title: format!("rail        {}", if prefs.rail { "on" } else { "off" }),
                        subtitle: format!(
                            "right panel · needs {}+ cols",
                            crate::ui::widgets::RAIL_MIN_WIDTH
                        ),
                    },
                ]
            }
            SheetKind::Rewind { turns, .. } => turns
                .iter()
                .filter(|t| row_matches(&q, &t.text, &t.row_id.to_string(), ""))
                .map(|t| SheetRow {
                    id: t.row_id.to_string(),
                    title: format!("#{}  {}", t.row_id, truncate(&t.text, 48)),
                    subtitle: String::new(),
                })
                .collect(),
            SheetKind::Agents { agents, .. } => agents
                .iter()
                .filter(|a| row_matches(&q, &a.goal, &a.id, &a.status))
                .map(|a| SheetRow {
                    id: a.id.clone(),
                    title: a.goal.clone(),
                    subtitle: format!("{} · {}", a.id, a.status),
                })
                .collect(),
        }
    }

    fn preview_theme(&self) {
        if !matches!(self.kind, SheetKind::Theme { .. }) {
            return;
        }
        if let Some(row) = self.rows().get(self.selected) {
            if crate::theme::apply_theme(&row.id) {
                crate::theme::sync_terminal_canvas();
            }
        }
    }

    fn handle_kind_key(&mut self, key: KeyEvent) -> Option<SheetAction> {
        match &mut self.kind {
            SheetKind::Theme { saved_id } => {
                if key.code == KeyCode::Esc {
                    let id = *saved_id;
                    let _ = crate::theme::apply_theme(id);
                    crate::theme::sync_terminal_canvas();
                    return Some(SheetAction::RestoreTheme { id });
                }
                None
            }
            SheetKind::Rewind { turns, confirming } => {
                if *confirming {
                    if matches!(
                        key.code,
                        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N')
                    ) {
                        *confirming = false;
                        return Some(SheetAction::Handled);
                    }
                    if matches!(
                        key.code,
                        KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y')
                    ) {
                        let turn = turns.get(self.selected).cloned();
                        return turn.map(|t| SheetAction::Rewind {
                            text: t.text,
                            truncate_before_row_id: t.row_id,
                            confirm_empty_truncate: t.first,
                        });
                    }
                    return Some(SheetAction::Handled);
                }
                if key.code == KeyCode::Enter && !turns.is_empty() && !self.loading {
                    *confirming = true;
                    return Some(SheetAction::Handled);
                }
                None
            }
            SheetKind::Agents {
                agents,
                steering,
                draft,
            } => {
                if *steering {
                    if key.code == KeyCode::Esc {
                        *steering = false;
                        draft.clear();
                        return Some(SheetAction::Handled);
                    }
                    if key.code == KeyCode::Backspace {
                        draft.pop();
                        return Some(SheetAction::Handled);
                    }
                    if key.code == KeyCode::Enter {
                        if draft.trim().is_empty() {
                            return Some(SheetAction::Handled);
                        }
                        let text = std::mem::take(draft);
                        let id = agents.get(self.selected).map(|a| a.id.clone());
                        *steering = false;
                        return id.map(|id| SheetAction::SteerAgent { id, text });
                    }
                    if let Some(c) = typed_char(&key) {
                        draft.push(c);
                    }
                    return Some(SheetAction::Handled);
                }
                if matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S'))
                    && self.query.is_empty()
                    && !agents.is_empty()
                {
                    *steering = true;
                    draft.clear();
                    return Some(SheetAction::Handled);
                }
                if key.code == KeyCode::Enter && !self.loading {
                    return agents
                        .get(self.selected)
                        .map(|a| SheetAction::InterruptAgent { id: a.id.clone() });
                }
                None
            }
            SheetKind::Custom => {
                if matches!(
                    key.code,
                    KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('l') | KeyCode::Char('h')
                ) {
                    return Some(SheetAction::ToggleCustom);
                }
                None
            }
            _ => None,
        }
    }

    fn clamp_selection(&mut self) {
        let n = self.rows().len();
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

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<SheetAction> {
        if let Some(act) = self.handle_kind_key(key) {
            return Some(act);
        }
        if key.code == KeyCode::Esc {
            return Some(SheetAction::Close);
        }
        if matches!(key.code, KeyCode::Tab | KeyCode::Left | KeyCode::Right) {
            if let SheetKind::Sessions { tab, .. } = &mut self.kind {
                *tab = match tab {
                    SheetTab::Saved => SheetTab::Live,
                    SheetTab::Live => SheetTab::Saved,
                };
                self.selected = 0;
                self.scroll = 0;
                self.query.clear();
            }
            return None;
        }
        if is_ctrl_n(&key) {
            return Some(SheetAction::New);
        }
        if is_ctrl_d(&key) {
            if let SheetKind::Sessions {
                tab: SheetTab::Live,
                ..
            } = &self.kind
            {
                if let Some(row) = self.rows().get(self.selected) {
                    return Some(SheetAction::CloseLive { id: row.id.clone() });
                }
            }
            return None;
        }
        if matches!(
            key.code,
            KeyCode::Char('s') | KeyCode::Char('S')
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
        ) && matches!(self.kind, SheetKind::Trees { .. })
        {
            return Some(SheetAction::Save);
        }
        match key.code {
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                self.clamp_selection();
                self.preview_theme();
                None
            }
            KeyCode::Down => {
                let n = self.rows().len();
                if n > 0 {
                    self.selected = (self.selected + 1).min(n - 1);
                    self.clamp_selection();
                }
                self.preview_theme();
                None
            }
            KeyCode::Enter => {
                if self.loading {
                    return None;
                }
                if let SheetKind::Theme { .. } = self.kind {
                    let id = self.rows().get(self.selected).map(|r| r.id.clone());
                    if let Some(id) = id {
                        if let Some(t) = crate::theme::theme_by_id(&id) {
                            let _ = crate::theme::apply_theme(t.id);
                            crate::theme::save_theme_id(t.id);
                            crate::theme::sync_terminal_canvas_hard();
                            return Some(SheetAction::ApplyTheme { id: t.id });
                        }
                    }
                }
                if matches!(self.kind, SheetKind::Custom) {
                    return Some(SheetAction::ToggleCustom);
                }
                self.rows()
                    .get(self.selected)
                    .map(|row| SheetAction::Confirm { id: row.id.clone() })
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
        let rect = sheet_rect(area);
        f.render_widget(Clear, rect);
        let tabs = match &self.kind {
            SheetKind::Sessions { tab, .. } => match tab {
                SheetTab::Saved => " saved  live",
                SheetTab::Live => " saved  LIVE",
            },
            _ => "",
        };
        let title = if tabs.is_empty() {
            format!(" {} ", self.title())
        } else {
            format!(" {} ·{tabs} ", self.title())
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

        let inner = inner.inner(Margin {
            horizontal: 1,
            vertical: 0,
        });
        let mut lines: Vec<Line> = Vec::new();
        if let SheetKind::Agents {
            steering: true,
            draft,
            ..
        } = &self.kind
        {
            lines.push(Line::from(Span::styled(
                format!("steer › {draft}▍"),
                theme::accent(),
            )));
        } else if !matches!(
            self.kind,
            SheetKind::Custom
                | SheetKind::Rewind {
                    confirming: true,
                    ..
                }
        ) {
            let filter = if self.query.is_empty() {
                "/ filter".to_string()
            } else {
                format!("/ {}", self.query)
            };
            lines.push(Line::from(Span::styled(filter, theme::dim())));
        }
        let confirming_rewind = matches!(
            self.kind,
            SheetKind::Rewind {
                confirming: true,
                ..
            }
        );
        if let SheetKind::Rewind {
            confirming: true,
            turns,
        } = &self.kind
        {
            let preview = turns
                .get(self.selected)
                .map(|t| t.text.as_str())
                .unwrap_or("");
            lines.push(Line::from(Span::styled(
                truncate(preview, 72),
                theme::text(),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Drop this turn and everything after it?",
                theme::dim(),
            )));
        }
        if self.loading {
            lines.push(Line::from(Span::styled("loading…", theme::dim())));
        }
        if let Some(n) = &self.notice {
            lines.push(Line::from(Span::styled(n.clone(), theme::accent())));
        }
        if !confirming_rewind {
            let rows = self.rows();
            if rows.is_empty() && !self.loading {
                lines.push(Line::from(Span::styled("nothing here", theme::dim())));
            }
            let cols = inner.width as usize;
            let window = VISIBLE.min(rows.len().saturating_sub(self.scroll));
            let end = self.scroll + window;
            for (i, row) in rows.iter().enumerate().take(end).skip(self.scroll) {
                let sel = i == self.selected;
                let mark = selector::prefix(sel);
                let title = pad_cols(&format!("{mark}{}", row.title), cols);
                if sel {
                    lines.push(Line::from(Span::styled(title, theme::selected())));
                } else {
                    lines.push(Line::from(Span::styled(title, theme::text())));
                }
                if !row.subtitle.is_empty() {
                    lines.push(Line::from(Span::styled(
                        pad_cols(&format!("    {}", row.subtitle), cols),
                        theme::dim(),
                    )));
                }
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(self.footer(), theme::dim())));
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }
}

fn session_sub(s: &SavedSession) -> String {
    let mut bits = Vec::new();
    if s.message_count > 0 {
        bits.push(format!("{} turns", s.message_count));
    }
    if !s.source.is_empty() {
        bits.push(s.source.clone());
    }
    if !s.preview.is_empty() {
        bits.push(truncate(&s.preview, 36));
    }
    bits.join(" · ")
}

fn row_matches(q: &str, title: &str, id: &str, extra: &str) -> bool {
    if q.is_empty() {
        return true;
    }
    title.to_ascii_lowercase().contains(q)
        || id.to_ascii_lowercase().contains(q)
        || extra.to_ascii_lowercase().contains(q)
}

fn pad_cols(s: &str, cols: usize) -> String {
    let mut out = s.to_string();
    let mut n = UnicodeWidthStr::width(out.as_str());
    while n < cols {
        out.push(' ');
        n += 1;
    }
    out
}

fn truncate(s: &str, n: usize) -> String {
    let mut it = s.chars();
    let head: String = it.by_ref().take(n).collect();
    if it.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// Sheet chrome around hub-drawn lines (model / skills / plugins / MCP).
pub fn paint_hosted(f: &mut Frame, area: Rect, title: &str, lines: Vec<Line<'static>>) {
    let rect = sheet_rect(area);
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title.to_string(),
            theme::agent().add_modifier(Modifier::BOLD),
        ))
        .border_style(theme::hairline())
        .style(Style::default().bg(theme::SURFACE()).fg(theme::TEXT()));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        inner.inner(Margin {
            horizontal: 1,
            vertical: 0,
        }),
    );
}

pub fn sheet_rect(area: Rect) -> Rect {
    if area.width >= 90 {
        let w = (area.width / 5 * 2).clamp(34, 46);
        let h = area.height.saturating_sub(3).max(10);
        Rect {
            x: area.x + area.width.saturating_sub(w).saturating_sub(1),
            y: area.y,
            width: w,
            height: h,
        }
    } else {
        let w = area.width.saturating_sub(2).min(72).max(24);
        let h = area.height.saturating_sub(5).max(10);
        Rect {
            x: area.x + (area.width.saturating_sub(w)) / 2,
            y: area.y,
            width: w,
            height: h,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_narrows_saved_sessions() {
        let mut s = Sheet::sessions();
        s.set_saved(vec![
            SavedSession {
                id: "a".into(),
                title: "Auth refactor".into(),
                preview: "jwt".into(),
                source: "tui".into(),
                message_count: 12,
            },
            SavedSession {
                id: "b".into(),
                title: "notes".into(),
                preview: String::new(),
                source: "cli".into(),
                message_count: 2,
            },
        ]);
        assert_eq!(s.rows().len(), 2);
        s.query = "auth".into();
        assert_eq!(s.rows().len(), 1);
        assert_eq!(s.rows()[0].id, "a");
    }

    #[test]
    fn enter_confirms_selected_id() {
        let mut s = Sheet::trees();
        s.set_trees(vec![SpawnTreeEntry {
            path: "tree.json".into(),
            label: "run".into(),
            count: 3,
        }]);
        let act = s.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            act,
            Some(SheetAction::Confirm {
                id: "tree.json".into()
            })
        );
    }

    #[test]
    fn wide_sheet_docks_right() {
        let r = sheet_rect(Rect::new(0, 0, 120, 40));
        assert!(r.x > 60, "{r:?}");
        let n = sheet_rect(Rect::new(0, 0, 80, 24));
        assert!(n.x < 10, "{n:?}");
    }
}
