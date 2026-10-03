//! Native `/model` overlay. Same RPCs as `hermes --tui` ModelPicker.
//!
//! Enter writes Hermes `--global` by default (shared with `hermes --tui`).
//! Ctrl+G opts into `--session` for this run only.

use crossterm::event::{KeyCode, KeyEvent};

use crate::session::ModelProvider;
use crate::theme;
use crate::ui::keys::{is_ctrl_d, is_ctrl_g, is_ctrl_u, typed_char};
use crate::ui::screens::ScreenAction;
use crate::ui::widgets::selector;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub const VISIBLE: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelStage {
    Provider,
    Model,
    Key,
    Disconnect,
}

#[derive(Clone)]
pub struct ModelPicker {
    pub stage: ModelStage,
    pub providers: Vec<ModelProvider>,
    pub current_model: String,
    pub provider_idx: usize,
    pub model_idx: usize,
    pub filter: String,
    pub persist_global: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub key_input: String,
    pub key_saving: bool,
    pub key_error: Option<String>,
    pub pending_value: Option<String>,
    pub confirm_message: Option<String>,
}

impl std::fmt::Debug for ModelPicker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelPicker")
            .field("stage", &self.stage)
            .field("current_model", &self.current_model)
            .field("loading", &self.loading)
            .field(
                "key_input",
                &if self.key_input.is_empty() { "" } else { "***" },
            )
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum ModelKey {
    None,
    Close,
    Action(ScreenAction),
}

impl ModelPicker {
    pub fn loading() -> Self {
        Self {
            stage: ModelStage::Provider,
            providers: Vec::new(),
            current_model: String::new(),
            provider_idx: 0,
            model_idx: 0,
            filter: String::new(),
            persist_global: true,
            loading: true,
            error: None,
            key_input: String::new(),
            key_saving: false,
            key_error: None,
            pending_value: None,
            confirm_message: None,
        }
    }

    pub fn apply_options(&mut self, providers: Vec<ModelProvider>, model: String) {
        let current = providers.iter().position(|p| p.is_current).unwrap_or(0);
        self.providers = providers;
        self.current_model = model;
        self.provider_idx = current;
        self.model_idx = 0;
        self.stage = ModelStage::Provider;
        self.filter.clear();
        self.loading = false;
        self.error = None;
        self.key_saving = false;
    }

    pub fn filtered_providers(&self) -> Vec<&ModelProvider> {
        if self.stage != ModelStage::Provider || self.filter.trim().is_empty() {
            return self.providers.iter().collect();
        }
        let q = self.filter.to_ascii_lowercase();
        self.providers
            .iter()
            .filter(|p| provider_haystack(p).to_ascii_lowercase().contains(&q))
            .collect()
    }

    pub fn selected_provider(&self) -> Option<&ModelProvider> {
        self.filtered_providers()
            .get(self.provider_idx)
            .copied()
            .or_else(|| self.providers.get(self.provider_idx))
    }

    pub fn filtered_models(&self) -> Vec<&str> {
        let Some(p) = self.selected_provider() else {
            return Vec::new();
        };
        if self.stage != ModelStage::Model || self.filter.trim().is_empty() {
            return p.models.iter().map(|s| s.as_str()).collect();
        }
        let q = self.filter.to_ascii_lowercase();
        p.models
            .iter()
            .filter(|m| m.to_ascii_lowercase().contains(&q))
            .map(|s| s.as_str())
            .collect()
    }

    fn clamp_selection(&mut self) {
        let n = match self.stage {
            ModelStage::Provider => self.filtered_providers().len(),
            ModelStage::Model => self.filtered_models().len(),
            _ => 0,
        };
        if n == 0 {
            return;
        }
        match self.stage {
            ModelStage::Provider => self.provider_idx = self.provider_idx.min(n - 1),
            ModelStage::Model => self.model_idx = self.model_idx.min(n - 1),
            _ => {}
        }
    }

    fn switch_value(&self, model: &str, provider_slug: &str) -> String {
        let scope = if self.persist_global {
            "--global"
        } else {
            "--session"
        };
        format!("{model} --provider {provider_slug} {scope}")
    }
}

fn provider_haystack(p: &ModelProvider) -> String {
    format!("{} {} {}", p.name, p.slug, p.models.join(" "))
}

pub fn on_key(picker: &mut ModelPicker, key: KeyEvent) -> ModelKey {
    if picker.confirm_message.is_some() {
        return confirm_key(picker, key);
    }
    if picker.loading {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
            return ModelKey::Close;
        }
        return ModelKey::None;
    }
    match picker.stage {
        ModelStage::Key => key_stage(picker, key),
        ModelStage::Disconnect => disconnect_stage(picker, key),
        ModelStage::Provider | ModelStage::Model => list_stage(picker, key),
    }
}

fn confirm_key(picker: &mut ModelPicker, key: KeyEvent) -> ModelKey {
    match key.code {
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
            picker.confirm_message = None;
            picker.pending_value = None;
            ModelKey::None
        }
        KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
            let value = picker.pending_value.take().unwrap_or_default();
            picker.confirm_message = None;
            if value.is_empty() {
                ModelKey::None
            } else {
                ModelKey::Action(ScreenAction::SetModel {
                    value,
                    confirm_expensive_model: true,
                })
            }
        }
        _ => ModelKey::None,
    }
}

fn key_stage(picker: &mut ModelPicker, key: KeyEvent) -> ModelKey {
    if picker.key_saving {
        return ModelKey::None;
    }
    if key.code == KeyCode::Esc {
        picker.stage = ModelStage::Provider;
        picker.key_input.clear();
        picker.key_error = None;
        return ModelKey::None;
    }
    if is_ctrl_u(&key) {
        picker.key_input.clear();
        return ModelKey::None;
    }
    if key.code == KeyCode::Backspace {
        picker.key_input.pop();
        return ModelKey::None;
    }
    if key.code == KeyCode::Enter {
        let key_text = picker.key_input.trim().to_string();
        if key_text.is_empty() {
            return ModelKey::None;
        }
        let Some(slug) = picker.selected_provider().map(|p| p.slug.clone()) else {
            return ModelKey::None;
        };
        picker.key_saving = true;
        picker.key_error = None;
        return ModelKey::Action(ScreenAction::SaveModelKey {
            slug,
            api_key: key_text,
        });
    }
    if let Some(c) = typed_char(&key) {
        picker.key_input.push(c);
    }
    ModelKey::None
}

fn disconnect_stage(picker: &mut ModelPicker, key: KeyEvent) -> ModelKey {
    if picker.key_saving {
        return ModelKey::None;
    }
    if matches!(
        key.code,
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N')
    ) {
        picker.stage = ModelStage::Provider;
        return ModelKey::None;
    }
    if matches!(
        key.code,
        KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y')
    ) {
        let Some(slug) = picker.selected_provider().map(|p| p.slug.clone()) else {
            picker.stage = ModelStage::Provider;
            return ModelKey::None;
        };
        picker.key_saving = true;
        return ModelKey::Action(ScreenAction::DisconnectModel { slug });
    }
    ModelKey::None
}

fn list_stage(picker: &mut ModelPicker, key: KeyEvent) -> ModelKey {
    if key.code == KeyCode::Esc {
        if !picker.filter.is_empty() {
            remember_provider(picker);
            picker.filter.clear();
            picker.model_idx = 0;
            picker.clamp_selection();
            return ModelKey::None;
        }
        if picker.stage == ModelStage::Model {
            picker.stage = ModelStage::Provider;
            picker.model_idx = 0;
            picker.filter.clear();
            remember_provider(picker);
            return ModelKey::None;
        }
        return ModelKey::Close;
    }
    if matches!(key.code, KeyCode::Char('q')) && picker.filter.is_empty() {
        return ModelKey::Close;
    }
    if is_ctrl_u(&key) {
        picker.filter.clear();
        picker.clamp_selection();
        return ModelKey::None;
    }
    if is_ctrl_g(&key) {
        picker.persist_global = !picker.persist_global;
        return ModelKey::None;
    }
    if is_ctrl_d(&key) && picker.stage == ModelStage::Provider {
        if picker.selected_provider().is_some_and(|p| p.authenticated) {
            remember_provider(picker);
            picker.stage = ModelStage::Disconnect;
            picker.filter.clear();
        }
        return ModelKey::None;
    }

    let count = if picker.stage == ModelStage::Provider {
        picker.filtered_providers().len()
    } else {
        picker.filtered_models().len()
    };
    let sel = if picker.stage == ModelStage::Provider {
        picker.provider_idx
    } else {
        picker.model_idx
    };

    if key.code == KeyCode::Up && sel > 0 {
        if picker.stage == ModelStage::Provider {
            picker.provider_idx -= 1;
        } else {
            picker.model_idx -= 1;
        }
        return ModelKey::None;
    }
    if key.code == KeyCode::Down && count > 0 && sel + 1 < count {
        if picker.stage == ModelStage::Provider {
            picker.provider_idx += 1;
        } else {
            picker.model_idx += 1;
        }
        return ModelKey::None;
    }

    if key.code == KeyCode::Enter {
        return list_enter(picker);
    }
    if key.code == KeyCode::Backspace {
        picker.filter.pop();
        if picker.stage == ModelStage::Provider {
            picker.provider_idx = 0;
        } else {
            picker.model_idx = 0;
        }
        picker.clamp_selection();
        return ModelKey::None;
    }
    if let Some(c) = typed_char(&key) {
        if c != 'q' || !picker.filter.is_empty() || picker.stage == ModelStage::Model {
            picker.filter.push(c);
            if picker.stage == ModelStage::Provider {
                picker.provider_idx = 0;
            } else {
                picker.model_idx = 0;
            }
        }
    }
    ModelKey::None
}

fn list_enter(picker: &mut ModelPicker) -> ModelKey {
    if picker.stage == ModelStage::Provider {
        let Some(p) = picker.selected_provider().cloned() else {
            return ModelKey::None;
        };
        remember_provider(picker);
        if !p.authenticated {
            if p.auth_type == "api_key" && p.key_env.is_some() {
                picker.stage = ModelStage::Key;
                picker.key_input.clear();
                picker.key_error = None;
                picker.filter.clear();
            } else if let Some(existing) =
                picker.providers.iter_mut().find(|row| row.slug == p.slug)
            {
                existing.warning = Some("OAuth — this picker only pastes API keys".into());
            }
            return ModelKey::None;
        }
        picker.stage = ModelStage::Model;
        picker.model_idx = 0;
        picker.filter.clear();
        return ModelKey::None;
    }
    let slug = picker
        .selected_provider()
        .map(|p| p.slug.clone())
        .unwrap_or_default();
    let models = picker.filtered_models();
    let Some(model) = models.get(picker.model_idx).map(|s| (*s).to_string()) else {
        picker.stage = ModelStage::Provider;
        return ModelKey::None;
    };
    drop(models);
    let value = picker.switch_value(&model, &slug);
    picker.pending_value = Some(value.clone());
    ModelKey::Action(ScreenAction::SetModel {
        value,
        confirm_expensive_model: false,
    })
}

fn remember_provider(picker: &mut ModelPicker) {
    let slug = picker
        .selected_provider()
        .map(|p| p.slug.clone())
        .unwrap_or_default();
    if slug.is_empty() {
        return;
    }
    if let Some(i) = picker.providers.iter().position(|p| p.slug == slug) {
        picker.provider_idx = i;
    }
}

pub fn lines(picker: &ModelPicker) -> Vec<Line<'static>> {
    if picker.confirm_message.is_some() {
        return confirm_lines(picker);
    }
    if picker.loading {
        return vec![
            Line::from(Span::styled("loading models…", theme::dim())),
            Line::from(""),
            Line::from(Span::styled("Esc cancel", theme::dim())),
        ];
    }
    if let Some(err) = &picker.error {
        return vec![
            Line::from(Span::styled(format!("error: {err}"), theme::error())),
            Line::from(""),
            Line::from(Span::styled("Esc/q cancel", theme::dim())),
        ];
    }
    match picker.stage {
        ModelStage::Key => key_lines(picker),
        ModelStage::Disconnect => disconnect_lines(picker),
        ModelStage::Provider => provider_lines(picker),
        ModelStage::Model => model_lines(picker),
    }
}

fn confirm_lines(picker: &ModelPicker) -> Vec<Line<'static>> {
    vec![
        Line::from(Span::styled(
            "Expensive model selection",
            theme::error().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            picker
                .confirm_message
                .clone()
                .unwrap_or_else(|| "This model has unusually high known pricing.".into()),
            theme::text(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "y / Enter switch anyway   ·   n / Esc cancel",
            theme::dim(),
        )),
    ]
}

fn key_lines(picker: &ModelPicker) -> Vec<Line<'static>> {
    let p = picker.selected_provider();
    let name = p.map(|x| x.name.as_str()).unwrap_or("provider");
    let env = p.and_then(|x| x.key_env.as_deref()).unwrap_or("API_KEY");
    let masked = if picker.key_input.is_empty() {
        "(empty)".to_string()
    } else {
        "•".repeat(picker.key_input.chars().count().min(40))
    };
    let status = if picker.key_saving {
        "saving…".to_string()
    } else if let Some(err) = &picker.key_error {
        format!("error: {err}")
    } else {
        String::new()
    };
    vec![
        Line::from(Span::styled(
            format!("Configure {name}"),
            theme::accent().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Paste your API key (saved to ~/.hermes/.env)",
            theme::dim(),
        )),
        Line::from(""),
        Line::from(Span::styled(format!("{env}:"), theme::dim())),
        Line::from(Span::styled(
            format!("  {masked}{}", if picker.key_saving { "" } else { "▍" }),
            theme::accent(),
        )),
        Line::from(""),
        Line::from(Span::styled(status, theme::error())),
        Line::from(Span::styled(
            "Enter save  ·  Ctrl+U clear  ·  Esc back",
            theme::dim(),
        )),
    ]
}

fn disconnect_lines(picker: &ModelPicker) -> Vec<Line<'static>> {
    let name = picker
        .selected_provider()
        .map(|p| p.name.as_str())
        .unwrap_or("provider");
    vec![
        Line::from(Span::styled(
            format!("Disconnect {name}?"),
            theme::accent().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("This removes saved credentials for {name}."),
            theme::text(),
        )),
        Line::from(Span::styled(
            "You can re-authenticate later by selecting it again.",
            theme::dim(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            if picker.key_saving {
                "disconnecting…"
            } else {
                "y / Enter confirm   ·   n / Esc cancel"
            },
            theme::dim(),
        )),
    ]
}

fn provider_lines(picker: &ModelPicker) -> Vec<Line<'static>> {
    let rows = picker.filtered_providers();
    let warning = picker
        .selected_provider()
        .and_then(|p| p.warning.clone())
        .unwrap_or_default();
    let mut lines = vec![
        Line::from(Span::styled(
            "Select provider (step 1/2)",
            theme::accent().add_modifier(Modifier::BOLD),
        )),
        persist_scope_line(picker.persist_global),
        Line::from(vec![
            Span::styled("● ", theme::agent()),
            Span::styled(
                or_unknown(&picker.current_model).to_string(),
                theme::agent(),
            ),
        ]),
        Line::from(Span::styled(filter_hint(&picker.filter), theme::dim())),
    ];
    lines.extend(warning_slot(&warning));
    if rows.is_empty() {
        lines.push(Line::from(Span::styled(
            if picker.filter.is_empty() {
                "no providers available"
            } else {
                "no providers match"
            },
            theme::dim(),
        )));
    } else {
        let offset = window_start(rows.len(), picker.provider_idx, VISIBLE);
        if offset > 0 {
            lines.push(Line::from(Span::styled(
                format!(" ↑ {offset} more"),
                theme::dim(),
            )));
        }
        for (idx, row) in rows.iter().enumerate().skip(offset).take(VISIBLE) {
            let sel = picker.provider_idx == idx;
            let auth = if !row.authenticated {
                "○"
            } else if row.is_current {
                "*"
            } else {
                "●"
            };
            let suffix = if !row.authenticated {
                if row.auth_type == "api_key" {
                    "(no key)".to_string()
                } else {
                    "(needs setup)".to_string()
                }
            } else {
                format!("{} models", row.total_models)
            };
            let style = if sel {
                theme::accent()
            } else if !row.authenticated {
                theme::dim()
            } else {
                theme::text()
            };
            lines.push(Line::from(vec![
                Span::styled(selector::prefix(sel), style),
                Span::styled(format!("{auth} {} · {suffix}", row.name), style),
            ]));
        }
        let rest = rows.len().saturating_sub(offset + VISIBLE);
        if rest > 0 {
            lines.push(Line::from(Span::styled(
                format!(" ↓ {rest} more"),
                theme::dim(),
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "↑/↓  Enter  ·  ^d disconnect  ·  Esc/q close",
        theme::dim(),
    )));
    lines
}

fn model_lines(picker: &ModelPicker) -> Vec<Line<'static>> {
    let name = picker
        .selected_provider()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "(unknown provider)".into());
    let models = picker.filtered_models();
    let mut lines = vec![
        Line::from(Span::styled(
            "Select model (step 2/2)",
            theme::accent().add_modifier(Modifier::BOLD),
        )),
        persist_scope_line(picker.persist_global),
        Line::from(Span::styled(format!("{name} · Esc back"), theme::dim())),
        Line::from(Span::styled(filter_hint(&picker.filter), theme::dim())),
    ];
    if models.is_empty() {
        lines.push(Line::from(Span::styled(
            if picker.filter.is_empty() {
                "no models listed for this provider"
            } else {
                "no models match filter"
            },
            theme::dim(),
        )));
    } else {
        let offset = window_start(models.len(), picker.model_idx, VISIBLE);
        if offset > 0 {
            lines.push(Line::from(Span::styled(
                format!(" ↑ {offset} more"),
                theme::dim(),
            )));
        }
        for (idx, model) in models.iter().enumerate().skip(offset).take(VISIBLE) {
            let sel = picker.model_idx == idx;
            let current = *model == picker.current_model;
            let mark = if current && !sel { "●" } else { "" };
            let style = if sel {
                theme::accent().add_modifier(Modifier::BOLD)
            } else if current {
                theme::agent()
            } else {
                theme::text()
            };
            let label = if mark.is_empty() {
                (*model).to_string()
            } else {
                format!("{mark} {model}")
            };
            lines.push(Line::from(vec![
                Span::styled(selector::prefix(sel), style),
                Span::styled(label, style),
            ]));
        }
        let rest = models.len().saturating_sub(offset + VISIBLE);
        if rest > 0 {
            lines.push(Line::from(Span::styled(
                format!(" ↓ {rest} more"),
                theme::dim(),
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "↑/↓ select  ·  Enter switch  ·  Esc back",
        theme::dim(),
    )));
    lines
}

/// Always two rows so a 1- or 2-line warning never shoves the provider list.
fn warning_slot(text: &str) -> [Line<'static>; 2] {
    const COLS: usize = 32;
    let blank = || Line::from("");
    if text.is_empty() {
        return [blank(), blank()];
    }
    let full = format!("warning: {text}");
    let (first, rest) = split_at_width(&full, COLS);
    let second = if rest.is_empty() {
        String::new()
    } else {
        ellipsize_width(&rest, COLS)
    };
    [
        Line::from(Span::styled(first, theme::error())),
        Line::from(Span::styled(second, theme::error())),
    ]
}

fn split_at_width(s: &str, cols: usize) -> (String, String) {
    if display_width(s) <= cols {
        return (s.to_string(), String::new());
    }
    let mut w = 0usize;
    let mut break_at = 0usize;
    let mut last_space = 0usize;
    for (i, ch) in s.char_indices() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > cols {
            break;
        }
        w += cw;
        break_at = i + ch.len_utf8();
        if ch.is_whitespace() {
            last_space = break_at;
        }
    }
    let cut = if last_space > 0 { last_space } else { break_at };
    let (a, b) = s.split_at(cut);
    (a.trim_end().to_string(), b.trim_start().to_string())
}

fn ellipsize_width(s: &str, cols: usize) -> String {
    if display_width(s) <= cols {
        return s.to_string();
    }
    let keep = cols.saturating_sub(1);
    let mut out = String::new();
    let mut w = 0usize;
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > keep {
            break;
        }
        out.push(ch);
        w += cw;
    }
    out.push('…');
    out
}

fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

fn persist_scope_line(global: bool) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            " global ",
            if global {
                theme::selected()
            } else {
                theme::dim()
            },
        ),
        Span::raw(" "),
        Span::styled(
            " session ",
            if !global {
                theme::selected()
            } else {
                theme::dim()
            },
        ),
        Span::styled("  Ctrl+G", theme::dim()),
    ])
}

fn filter_hint(filter: &str) -> String {
    if filter.is_empty() {
        "type to filter · ↑/↓ select".into()
    } else {
        format!("filter: {filter}▍")
    }
}

fn or_unknown(s: &str) -> &str {
    if s.is_empty() {
        "(unknown)"
    } else {
        s
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
    use crossterm::event::KeyModifiers;

    fn prov(slug: &str, auth: bool, models: &[&str]) -> ModelProvider {
        ModelProvider {
            slug: slug.into(),
            name: slug.into(),
            authenticated: auth,
            is_current: slug == "openrouter",
            auth_type: "api_key".into(),
            key_env: Some(format!("{}_KEY", slug.to_ascii_uppercase())),
            models: models.iter().map(|s| (*s).to_string()).collect(),
            total_models: models.len() as u64,
            warning: None,
        }
    }

    fn picker() -> ModelPicker {
        let mut p = ModelPicker::loading();
        p.apply_options(
            vec![
                prov("openrouter", true, &["openrouter/a", "openrouter/b"]),
                prov("anthropic", false, &[]),
            ],
            "openrouter/a".into(),
        );
        p
    }

    #[test]
    fn enter_on_configured_provider_opens_models() {
        let mut p = picker();
        assert!(matches!(
            on_key(&mut p, KeyEvent::from(KeyCode::Enter)),
            ModelKey::None
        ));
        assert_eq!(p.stage, ModelStage::Model);
        match on_key(&mut p, KeyEvent::from(KeyCode::Enter)) {
            ModelKey::Action(ScreenAction::SetModel { value, .. }) => {
                assert!(value.contains("openrouter/a"), "{value}");
                assert!(value.contains("--provider openrouter"), "{value}");
                assert!(value.contains("--global"), "{value}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn enter_on_unconfigured_api_key_prompts_for_key() {
        let mut p = picker();
        p.provider_idx = 1;
        assert!(matches!(
            on_key(&mut p, KeyEvent::from(KeyCode::Enter)),
            ModelKey::None
        ));
        assert_eq!(p.stage, ModelStage::Key);
    }

    fn visible(lines: &[Line]) -> String {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn warning_slot_is_always_two_rows() {
        assert_eq!(warning_slot("").len(), 2);
        assert_eq!(warning_slot("no key").len(), 2);
        assert_eq!(
            warning_slot("paste ANTHROPIC_API_KEY to activate and then restart").len(),
            2
        );
        let a = provider_lines(&picker());
        let mut p = picker();
        p.provider_idx = 1;
        p.providers[1].warning = Some("paste ANTHROPIC_API_KEY to activate".into());
        let b = provider_lines(&p);
        let idx_a = a.iter().position(|l| {
            l.spans
                .iter()
                .any(|s| s.content.as_ref().contains("OpenRouter"))
        });
        let idx_b = b.iter().position(|l| {
            l.spans
                .iter()
                .any(|s| s.content.as_ref().contains("OpenRouter"))
        });
        assert_eq!(idx_a, idx_b, "warning must not shift the provider list");
    }

    #[test]
    fn persist_chips_show_active_scope() {
        let p = picker();
        let t = visible(&provider_lines(&p));
        assert!(t.contains("session"), "{t}");
        assert!(t.contains("global"), "{t}");
        assert!(t.contains("Ctrl+G"), "{t}");
        assert!(p.persist_global);
        let t = visible(&model_lines(&p));
        assert!(t.contains("global"), "{t}");
    }

    #[test]
    fn ctrl_g_opts_into_session_only() {
        let mut p = picker();
        assert!(p.persist_global);
        p.stage = ModelStage::Model;
        let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
        on_key(&mut p, key);
        assert!(!p.persist_global);
        match on_key(&mut p, KeyEvent::from(KeyCode::Enter)) {
            ModelKey::Action(ScreenAction::SetModel { value, .. }) => {
                assert!(value.contains("--session"), "{value}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn filter_narrows_providers() {
        let mut p = picker();
        on_key(&mut p, KeyEvent::from(KeyCode::Char('a')));
        on_key(&mut p, KeyEvent::from(KeyCode::Char('n')));
        let rows = p.filtered_providers();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].slug, "anthropic");
    }
}
