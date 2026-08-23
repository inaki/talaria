//! Native skills / plugins / MCP overlays. No fallback to the original TUI.

use crossterm::event::{KeyCode, KeyEvent};

use crate::session::{McpCatalogEntry, McpServer, PluginRow};
use crate::theme;
use crate::ui::screens::ScreenAction;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

const VISIBLE: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubKey {
    None,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillsStage {
    Category,
    Skill,
}

#[derive(Debug, Clone)]
pub struct SkillsHub {
    pub groups: Vec<(String, Vec<String>)>,
    pub stage: SkillsStage,
    pub cat_idx: usize,
    pub skill_idx: usize,
    pub loading: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
}

impl SkillsHub {
    pub fn loading() -> Self {
        Self {
            groups: Vec::new(),
            stage: SkillsStage::Category,
            cat_idx: 0,
            skill_idx: 0,
            loading: true,
            error: None,
            notice: None,
        }
    }

    fn category(&self) -> Option<&(String, Vec<String>)> {
        self.groups.get(self.cat_idx)
    }

    fn selected_skill(&self) -> Option<&str> {
        self.category()?.1.get(self.skill_idx).map(|s| s.as_str())
    }
}

pub fn skills_key(hub: &mut SkillsHub, key: KeyEvent) -> (HubKey, Option<ScreenAction>) {
    if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
        if hub.stage == SkillsStage::Skill {
            hub.stage = SkillsStage::Category;
            hub.skill_idx = 0;
            return (HubKey::None, None);
        }
        return (HubKey::Close, None);
    }
    if hub.loading {
        return (HubKey::None, None);
    }
    match hub.stage {
        SkillsStage::Category => {
            nav(&mut hub.cat_idx, hub.groups.len(), key);
            if key.code == KeyCode::Enter && !hub.groups.is_empty() {
                hub.stage = SkillsStage::Skill;
                hub.skill_idx = 0;
            }
        }
        SkillsStage::Skill => {
            let n = hub.category().map(|g| g.1.len()).unwrap_or(0);
            nav(&mut hub.skill_idx, n, key);
            if key.code == KeyCode::Enter || matches!(key.code, KeyCode::Char('i')) {
                if let Some(name) = hub.selected_skill().map(|s| s.to_string()) {
                    hub.notice = Some(format!("installing {name}…"));
                    return (
                        HubKey::None,
                        Some(ScreenAction::InstallSkill { query: name }),
                    );
                }
            }
        }
    }
    (HubKey::None, None)
}

pub fn skills_lines(hub: &SkillsHub) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled(
            "Skills",
            theme::accent().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Enter opens  ·  i/Enter installs  ·  Esc back",
            theme::dim(),
        )),
    ];
    if hub.loading {
        lines.push(Line::from(Span::styled("loading…", theme::dim())));
        return lines;
    }
    if let Some(err) = &hub.error {
        lines.push(Line::from(Span::styled(
            format!("error: {err}"),
            theme::error(),
        )));
        return lines;
    }
    if let Some(n) = &hub.notice {
        lines.push(Line::from(Span::styled(n.clone(), theme::accent())));
    }
    match hub.stage {
        SkillsStage::Category => {
            if hub.groups.is_empty() {
                lines.push(Line::from(Span::styled(
                    "no skills installed",
                    theme::dim(),
                )));
            }
            let offset = window_start(hub.groups.len(), hub.cat_idx, VISIBLE);
            for (i, (name, members)) in hub.groups.iter().enumerate().skip(offset).take(VISIBLE) {
                let mark = if i == hub.cat_idx { "▸ " } else { "  " };
                let style = if i == hub.cat_idx {
                    theme::accent()
                } else {
                    theme::text()
                };
                lines.push(Line::from(Span::styled(
                    format!("{mark}{name}  ({})", members.len()),
                    style,
                )));
            }
        }
        SkillsStage::Skill => {
            let (cat, skills) = hub
                .category()
                .map(|(n, s)| (n.as_str(), s.as_slice()))
                .unwrap_or(("", &[]));
            lines.push(Line::from(Span::styled(format!("{cat}"), theme::dim())));
            if skills.is_empty() {
                lines.push(Line::from(Span::styled("empty category", theme::dim())));
            }
            let offset = window_start(skills.len(), hub.skill_idx, VISIBLE);
            for (i, name) in skills.iter().enumerate().skip(offset).take(VISIBLE) {
                let mark = if i == hub.skill_idx { "▸ " } else { "  " };
                let style = if i == hub.skill_idx {
                    theme::accent()
                } else {
                    theme::text()
                };
                lines.push(Line::from(Span::styled(format!("{mark}{name}"), style)));
            }
        }
    }
    lines
}

#[derive(Debug, Clone)]
pub struct PluginsHub {
    pub plugins: Vec<PluginRow>,
    pub selected: usize,
    pub loading: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
}

impl PluginsHub {
    pub fn loading() -> Self {
        Self {
            plugins: Vec::new(),
            selected: 0,
            loading: true,
            error: None,
            notice: None,
        }
    }
}

pub fn plugins_key(hub: &mut PluginsHub, key: KeyEvent) -> (HubKey, Option<ScreenAction>) {
    if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
        return (HubKey::Close, None);
    }
    if hub.loading {
        return (HubKey::None, None);
    }
    nav(&mut hub.selected, hub.plugins.len(), key);
    if key.code == KeyCode::Enter {
        if let Some(p) = hub.plugins.get(hub.selected) {
            let enable = p.status != "enabled";
            hub.notice = Some(format!(
                "{} {}",
                if enable { "enabling" } else { "disabling" },
                p.name
            ));
            return (
                HubKey::None,
                Some(ScreenAction::TogglePlugin {
                    key: p.key.clone(),
                    enable,
                }),
            );
        }
    }
    (HubKey::None, None)
}

pub fn plugins_lines(hub: &PluginsHub) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled(
            "Plugins",
            theme::accent().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled("Enter toggles  ·  Esc close", theme::dim())),
    ];
    if hub.loading {
        lines.push(Line::from(Span::styled("loading…", theme::dim())));
        return lines;
    }
    if let Some(err) = &hub.error {
        lines.push(Line::from(Span::styled(
            format!("error: {err}"),
            theme::error(),
        )));
        return lines;
    }
    if let Some(n) = &hub.notice {
        lines.push(Line::from(Span::styled(n.clone(), theme::accent())));
    }
    if hub.plugins.is_empty() {
        lines.push(Line::from(Span::styled("no plugins", theme::dim())));
    }
    let offset = window_start(hub.plugins.len(), hub.selected, VISIBLE);
    for (i, p) in hub.plugins.iter().enumerate().skip(offset).take(VISIBLE) {
        let mark = if i == hub.selected { "▸ " } else { "  " };
        let glyph = if p.status == "enabled" { "✓" } else { "✗" };
        let style = if i == hub.selected {
            theme::accent()
        } else {
            theme::text()
        };
        lines.push(Line::from(Span::styled(
            format!("{mark}{glyph} {}  {}", p.name, p.status),
            style,
        )));
    }
    lines
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpTab {
    Installed,
    Catalog,
}

#[derive(Debug, Clone)]
pub struct McpHub {
    pub tab: McpTab,
    pub installed: Vec<McpServer>,
    pub catalog: Vec<McpCatalogEntry>,
    pub selected: usize,
    pub loading: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub confirm_remove: Option<String>,
}

impl McpHub {
    pub fn loading() -> Self {
        Self {
            tab: McpTab::Installed,
            installed: Vec::new(),
            catalog: Vec::new(),
            selected: 0,
            loading: true,
            error: None,
            notice: None,
            confirm_remove: None,
        }
    }

    fn len(&self) -> usize {
        match self.tab {
            McpTab::Installed => self.installed.len(),
            McpTab::Catalog => self.catalog.len(),
        }
    }
}

pub fn mcp_key(hub: &mut McpHub, key: KeyEvent) -> (HubKey, Option<ScreenAction>) {
    if let Some(name) = hub.confirm_remove.clone() {
        match key.code {
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                hub.confirm_remove = None;
                return (HubKey::None, None);
            }
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                hub.confirm_remove = None;
                hub.notice = Some(format!("removing {name}…"));
                return (HubKey::None, Some(ScreenAction::RemoveMcp { name }));
            }
            _ => return (HubKey::None, None),
        }
    }
    if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
        return (HubKey::Close, None);
    }
    if matches!(key.code, KeyCode::Tab | KeyCode::Left | KeyCode::Right) {
        hub.tab = match hub.tab {
            McpTab::Installed => McpTab::Catalog,
            McpTab::Catalog => McpTab::Installed,
        };
        hub.selected = 0;
        return (HubKey::None, None);
    }
    if hub.loading {
        return (HubKey::None, None);
    }
    let n = hub.len();
    nav(&mut hub.selected, n, key);
    if key.code == KeyCode::Enter {
        match hub.tab {
            McpTab::Installed => {
                if let Some(s) = hub.installed.get(hub.selected) {
                    hub.confirm_remove = Some(s.name.clone());
                }
            }
            McpTab::Catalog => {
                if let Some(s) = hub.catalog.get(hub.selected) {
                    hub.notice = Some(format!("adding {}…", s.name));
                    return (
                        HubKey::None,
                        Some(ScreenAction::AddMcp {
                            name: s.name.clone(),
                            preset: s.name.clone(),
                        }),
                    );
                }
            }
        }
    }
    (HubKey::None, None)
}

pub fn mcp_lines(hub: &McpHub) -> Vec<Line<'static>> {
    if let Some(name) = &hub.confirm_remove {
        return vec![
            Line::from(Span::styled(
                format!("Remove {name}?"),
                theme::error().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "y / Enter confirm   ·   n / Esc cancel",
                theme::dim(),
            )),
        ];
    }
    let tabs = match hub.tab {
        McpTab::Installed => "[installed]   catalog",
        McpTab::Catalog => " installed   [catalog]",
    };
    let mut lines = vec![
        Line::from(Span::styled(
            "MCP servers",
            theme::accent().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(tabs, theme::accent())),
        Line::from(Span::styled(
            "Tab switches  ·  Enter add/remove  ·  Esc close",
            theme::dim(),
        )),
    ];
    if hub.loading {
        lines.push(Line::from(Span::styled("loading…", theme::dim())));
        return lines;
    }
    if let Some(err) = &hub.error {
        lines.push(Line::from(Span::styled(
            format!("error: {err}"),
            theme::error(),
        )));
        return lines;
    }
    if let Some(n) = &hub.notice {
        lines.push(Line::from(Span::styled(n.clone(), theme::accent())));
    }
    match hub.tab {
        McpTab::Installed => {
            if hub.installed.is_empty() {
                lines.push(Line::from(Span::styled(
                    "none configured — Tab for catalog",
                    theme::dim(),
                )));
            }
            let offset = window_start(hub.installed.len(), hub.selected, VISIBLE);
            for (i, s) in hub.installed.iter().enumerate().skip(offset).take(VISIBLE) {
                let mark = if i == hub.selected { "▸ " } else { "  " };
                let on = if s.enabled { "on" } else { "off" };
                let style = if i == hub.selected {
                    theme::accent()
                } else {
                    theme::text()
                };
                lines.push(Line::from(Span::styled(
                    format!("{mark}{}  {}  {on}", s.name, s.transport),
                    style,
                )));
            }
        }
        McpTab::Catalog => {
            if hub.catalog.is_empty() {
                lines.push(Line::from(Span::styled("empty catalog", theme::dim())));
            }
            let offset = window_start(hub.catalog.len(), hub.selected, VISIBLE);
            for (i, s) in hub.catalog.iter().enumerate().skip(offset).take(VISIBLE) {
                let mark = if i == hub.selected { "▸ " } else { "  " };
                let st = if s.installed { "installed" } else { "add" };
                let style = if i == hub.selected {
                    theme::accent()
                } else {
                    theme::text()
                };
                lines.push(Line::from(Span::styled(
                    format!("{mark}{}  {st}  {}", s.name, s.description),
                    style,
                )));
            }
        }
    }
    lines
}

fn nav(selected: &mut usize, len: usize, key: KeyEvent) {
    if len == 0 {
        return;
    }
    if key.code == KeyCode::Up {
        *selected = selected.saturating_sub(1);
    }
    if key.code == KeyCode::Down {
        *selected = (*selected + 1).min(len - 1);
    }
}

fn window_start(len: usize, selected: usize, visible: usize) -> usize {
    if len <= visible {
        return 0;
    }
    let mut start = selected.saturating_sub(visible / 2);
    if start + visible > len {
        start = len - visible;
    }
    start
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skills_enter_opens_category() {
        let mut hub = SkillsHub::loading();
        hub.loading = false;
        hub.groups = vec![("bundled".into(), vec!["plan".into()])];
        let _ = skills_key(&mut hub, KeyEvent::from(KeyCode::Enter));
        assert_eq!(hub.stage, SkillsStage::Skill);
        match skills_key(&mut hub, KeyEvent::from(KeyCode::Enter)) {
            (_, Some(ScreenAction::InstallSkill { query })) => assert_eq!(query, "plan"),
            other => panic!("{other:?}"),
        }
    }
}
