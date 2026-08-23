//! Named palettes (herald_v2 pattern). Widgets read tokens via accessors.
//!
//! Call [`apply_theme`] at startup (CLI / env / saved file) and from `/theme`.
//! `gateway.ready` skin overlay applies only while the GitHub default is active.

#![allow(non_snake_case)]

use std::sync::RwLock;

use ratatui::style::{Color, Modifier, Style};
use serde_json::Value;

use crate::paths::{create_private_dir_all, HermesRustPaths};

pub const GITHUB_ID: &str = "github";
pub const GOLD_ID: &str = "gold";
pub const HERMES_ID: &str = "hermes";

#[derive(Clone, Copy)]
struct Palette {
    background: Color,
    surface: Color,
    primary: Color,
    text: Color,
    text_dim: Color,
    success: Color,
    warning: Color,
    error: Color,
    user: Color,
    tool: Color,
    selected_text: Color,
    selection_bg: Option<Color>,
    separator: Color,
    /// Prompt / composer outline. Distinct from `separator` (hairline chrome).
    input_border: Color,
}

#[derive(Clone, Copy)]
pub struct ThemeDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    palette: Palette,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

/// Existing GitHub-ish dark (default).
pub const GITHUB: ThemeDefinition = ThemeDefinition {
    id: GITHUB_ID,
    label: "GitHub Dark",
    description: "Neutral dark canvas (default).",
    palette: Palette {
        background: rgb(0x0d, 0x11, 0x17),
        surface: rgb(0x16, 0x1b, 0x22),
        primary: rgb(0x58, 0xa6, 0xff),
        text: rgb(0xe6, 0xed, 0xf3),
        text_dim: rgb(0x8b, 0x94, 0x9e),
        success: rgb(0x3f, 0xb9, 0x50),
        warning: rgb(0xd2, 0x99, 0x22),
        error: rgb(0xf8, 0x51, 0x49),
        user: rgb(0xa3, 0x71, 0xf7),
        tool: rgb(0x79, 0xc0, 0xff),
        selected_text: rgb(0xe6, 0xed, 0xf3),
        selection_bg: Some(rgb(0x21, 0x26, 0x2d)),
        separator: rgb(0x30, 0x36, 0x3d),
        input_border: rgb(0x58, 0xa6, 0xff),
    },
};

/// Bronze / gold on navy (banner + status chrome).
pub const GOLD: ThemeDefinition = ThemeDefinition {
    id: GOLD_ID,
    label: "Gold",
    description: "Bronze, gold, and amber on dark navy.",
    palette: Palette {
        background: rgb(0x1a, 0x1a, 0x2e), // status / completion bg
        surface: rgb(0x3a, 0x3a, 0x55),    // active completion
        primary: rgb(0xff, 0xd7, 0x00),    // gold — titles, status strong
        text: rgb(0xff, 0xf8, 0xdc),       // cornsilk — prompt / body
        text_dim: rgb(0xb8, 0x86, 0x0b),   // dark goldenrod
        success: rgb(0x4c, 0xaf, 0x50),
        warning: rgb(0xff, 0xa7, 0x26),
        error: rgb(0xef, 0x53, 0x50),
        user: rgb(0xda, 0xa5, 0x20), // goldenrod — labels
        tool: rgb(0xff, 0xbf, 0x00), // amber — UI accent
        selected_text: rgb(0xff, 0xf8, 0xdc),
        selection_bg: Some(rgb(0x33, 0x33, 0x55)),
        separator: rgb(0x8b, 0x86, 0x82), // warm gray — session chrome
        input_border: rgb(0xcd, 0x7f, 0x32), // bronze — input rule
    },
};

/// Official Hermes brand: intense blue canvas, lime accent.
pub const HERMES: ThemeDefinition = ThemeDefinition {
    id: HERMES_ID,
    label: "Hermes",
    description: "Brand blue (#0000f2) with lime accent.",
    palette: Palette {
        background: rgb(0x00, 0x00, 0xf2), // --color-hermes
        surface: rgb(0x00, 0x29, 0xde),    // secondary blue overlays
        primary: rgb(0xed, 0xff, 0x45),    // --color-hermes-accent
        text: rgb(0xf5, 0xf5, 0xf5),       // --color-hermes-fg
        text_dim: rgb(0x99, 0xaa, 0xff),
        success: rgb(0x4c, 0xaf, 0x50),
        warning: rgb(0xff, 0xa7, 0x26),
        error: rgb(0xef, 0x53, 0x50),
        user: rgb(0xed, 0xff, 0x45),
        tool: rgb(0xed, 0xff, 0x45),
        selected_text: rgb(0x00, 0x00, 0xf2),
        selection_bg: Some(rgb(0xed, 0xff, 0x45)),
        separator: rgb(0x00, 0x29, 0xde),
        input_border: rgb(0xed, 0xff, 0x45),
    },
};

const THEMES: &[ThemeDefinition] = &[GITHUB, GOLD, HERMES];

static PALETTE: RwLock<Palette> = RwLock::new(GITHUB.palette);
static THEME_ID: RwLock<&'static str> = RwLock::new(GITHUB_ID);

fn pal() -> Palette {
    PALETTE.read().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn themes() -> &'static [ThemeDefinition] {
    THEMES
}

pub fn theme_by_id(id: &str) -> Option<&'static ThemeDefinition> {
    let want = id.trim();
    THEMES.iter().find(|t| t.id.eq_ignore_ascii_case(want))
}

pub fn current_theme_id() -> &'static str {
    *THEME_ID.read().unwrap_or_else(|e| e.into_inner())
}

pub fn current_theme_label() -> &'static str {
    theme_by_id(current_theme_id())
        .map(|t| t.label)
        .unwrap_or(GITHUB.label)
}

/// Swap the active palette. Does not touch the terminal; call
/// [`sync_terminal_canvas`] from the TUI after applying.
pub fn apply_theme(id: &str) -> bool {
    let Some(next) = theme_by_id(id) else {
        return false;
    };
    if let Ok(mut g) = PALETTE.write() {
        *g = next.palette;
    }
    if let Ok(mut g) = THEME_ID.write() {
        *g = next.id;
    }
    true
}

pub fn resolve_startup_theme(cli: Option<&str>) -> &'static str {
    if let Some(id) = cli.and_then(theme_by_id) {
        return id.id;
    }
    if let Ok(env) = std::env::var("HERMES_RUST_THEME") {
        if let Some(id) = theme_by_id(&env) {
            return id.id;
        }
    }
    if let Some(saved) = load_saved_theme_id() {
        if let Some(id) = theme_by_id(&saved) {
            return id.id;
        }
    }
    GITHUB_ID
}

pub fn load_saved_theme_id() -> Option<String> {
    let path = HermesRustPaths::from_env().theme_file();
    let s = std::fs::read_to_string(path).ok()?;
    let id = s.trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

pub fn save_theme_id(id: &str) {
    let Some(def) = theme_by_id(id) else {
        return;
    };
    let paths = HermesRustPaths::from_env();
    create_private_dir_all(&paths.root);
    let path = paths.theme_file();
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    if let Ok(mut f) = opts.open(path) {
        use std::io::Write;
        let _ = writeln!(f, "{}", def.id);
    }
}

/// Soft canvas sync: default background without `Clear(All)` (arrow-key preview).
pub fn sync_terminal_canvas() {
    sync_terminal_canvas_inner(false);
}

/// Hard canvas sync on startup and when committing a theme.
pub fn sync_terminal_canvas_hard() {
    sync_terminal_canvas_inner(true);
}

fn sync_terminal_canvas_inner(hard_clear: bool) {
    use std::io::stdout;

    use crossterm::execute;
    use crossterm::style::SetBackgroundColor;
    use crossterm::terminal::{Clear, ClearType};
    use crossterm::tty::IsTty;

    if !stdout().is_tty() {
        return;
    }
    let bg = match pal().background {
        Color::Rgb(r, g, b) => crossterm::style::Color::Rgb { r, g, b },
        Color::Black => crossterm::style::Color::Black,
        Color::White => crossterm::style::Color::White,
        _ => crossterm::style::Color::Rgb { r: 0, g: 0, b: 0 },
    };
    if hard_clear {
        let _ = execute!(stdout(), SetBackgroundColor(bg), Clear(ClearType::All));
    } else {
        let _ = execute!(stdout(), SetBackgroundColor(bg));
    }
}

pub fn BACKGROUND() -> Color {
    pal().background
}
pub fn SURFACE() -> Color {
    pal().surface
}
pub fn PRIMARY() -> Color {
    pal().primary
}
pub fn TEXT() -> Color {
    pal().text
}
pub fn TEXT_DIM() -> Color {
    pal().text_dim
}
pub fn SUCCESS() -> Color {
    pal().success
}
pub fn WARNING() -> Color {
    pal().warning
}
pub fn ERROR() -> Color {
    pal().error
}
pub fn USER() -> Color {
    pal().user
}
pub fn TOOL() -> Color {
    pal().tool
}
pub fn SELECTED_TEXT() -> Color {
    pal().selected_text
}
pub fn SELECTION_BG() -> Option<Color> {
    pal().selection_bg
}
pub fn SEPARATOR() -> Color {
    pal().separator
}
pub fn INPUT_BORDER() -> Color {
    pal().input_border
}

pub fn text() -> Style {
    Style::default().fg(TEXT())
}
pub fn dim() -> Style {
    Style::default().fg(TEXT_DIM())
}
pub fn accent() -> Style {
    Style::default().fg(PRIMARY())
}
pub fn error() -> Style {
    Style::default().fg(ERROR())
}
pub fn user() -> Style {
    Style::default().fg(USER()).add_modifier(Modifier::BOLD)
}
pub fn assistant() -> Style {
    Style::default().fg(TEXT())
}
pub fn tool() -> Style {
    Style::default().fg(TOOL())
}

/// Block caret: invert the prompt canvas (never a hardcoded swatch).
pub fn cursor_block_style() -> Style {
    Style::default()
        .fg(BACKGROUND())
        .bg(TEXT())
        .add_modifier(Modifier::BOLD)
}

/// Overlay palette from a Hermes `GatewaySkin` object.
/// Ignored when a named theme other than GitHub is active.
pub fn apply_skin(skin: &Value) {
    if current_theme_id() != GITHUB_ID {
        return;
    }
    let Some(map) = colors_object(skin) else {
        return;
    };
    let mut p = pal();
    if let Some(c) = color_in(map, &["background", "bg", "canvas", "bg_primary"]) {
        p.background = c;
        p.surface = shift(c, 10);
        p.selection_bg = Some(shift(c, 18));
        p.separator = shift(c, 28);
    }
    if let Some(c) = color_in(map, &["foreground", "fg", "text", "fg_primary"]) {
        p.text = c;
        p.selected_text = c;
    }
    if let Some(c) = color_in(map, &["muted", "dim", "comment", "fg_secondary"]) {
        p.text_dim = c;
    }
    if let Some(c) = color_in(map, &["accent", "primary", "blue", "link"]) {
        p.primary = c;
        p.tool = c;
        p.input_border = c;
    }
    if let Some(c) = color_in(map, &["error", "red", "danger"]) {
        p.error = c;
    }
    if let Some(c) = color_in(map, &["warning", "yellow", "orange"]) {
        p.warning = c;
    }
    if let Some(c) = color_in(map, &["success", "green"]) {
        p.success = c;
    }
    if let Some(c) = color_in(map, &["purple", "magenta", "user"]) {
        p.user = c;
    }
    if let Ok(mut g) = PALETTE.write() {
        *g = p;
    }
    crate::logging::log_line("theme: applied gateway.ready skin");
}

fn colors_object(skin: &Value) -> Option<&Value> {
    if !skin.is_object() {
        return None;
    }
    skin.get("dark_colors")
        .or_else(|| skin.get("colors"))
        .or_else(|| skin.get("palette"))
        .or(Some(skin))
}

fn color_in(obj: &Value, keys: &[&str]) -> Option<Color> {
    for k in keys {
        if let Some(c) = obj.get(*k).and_then(parse_color) {
            return Some(c);
        }
    }
    None
}

fn parse_color(v: &Value) -> Option<Color> {
    let s = v.as_str()?.trim();
    parse_hex(s)
}

fn parse_hex(s: &str) -> Option<Color> {
    let s = s.strip_prefix('#').unwrap_or(s);
    match s.len() {
        3 => {
            let r = u8::from_str_radix(&s[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&s[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&s[2..3].repeat(2), 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

fn shift(c: Color, delta: u8) -> Color {
    match c {
        Color::Rgb(r, g, b) => Color::Rgb(
            r.saturating_add(delta),
            g.saturating_add(delta),
            b.saturating_add(delta),
        ),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    static THEME_LOCK: Mutex<()> = Mutex::new(());

    fn restore_github() {
        let _ = apply_theme(GITHUB_ID);
    }

    #[test]
    fn gold_and_hermes_palettes() {
        let _g = THEME_LOCK.lock().unwrap();
        assert!(apply_theme("gold"));
        assert_eq!(current_theme_id(), GOLD_ID);
        assert_eq!(BACKGROUND(), rgb(0x1a, 0x1a, 0x2e));
        assert_eq!(PRIMARY(), rgb(0xff, 0xd7, 0x00));
        assert_eq!(INPUT_BORDER(), rgb(0xcd, 0x7f, 0x32));
        assert_eq!(SEPARATOR(), rgb(0x8b, 0x86, 0x82));
        assert_eq!(SUCCESS(), rgb(0x4c, 0xaf, 0x50));
        assert_eq!(ERROR(), rgb(0xef, 0x53, 0x50));
        assert_eq!(WARNING(), rgb(0xff, 0xa7, 0x26));

        assert!(apply_theme("HERMES"));
        assert_eq!(current_theme_id(), HERMES_ID);
        assert_eq!(BACKGROUND(), rgb(0x00, 0x00, 0xf2));
        assert_eq!(PRIMARY(), rgb(0xed, 0xff, 0x45));
        assert_eq!(TEXT(), rgb(0xf5, 0xf5, 0xf5));
        assert_eq!(SURFACE(), rgb(0x00, 0x29, 0xde));
        assert_eq!(INPUT_BORDER(), rgb(0xed, 0xff, 0x45));

        assert!(!apply_theme("not-a-theme"));
        restore_github();
        assert_eq!(current_theme_id(), GITHUB_ID);
    }

    #[test]
    fn skin_ignored_on_named_theme() {
        let _g = THEME_LOCK.lock().unwrap();
        assert!(apply_theme(GOLD_ID));
        apply_skin(&json!({
            "colors": { "background": "#000000", "accent": "#ff00aa", "foreground": "#ffffff" }
        }));
        assert_eq!(BACKGROUND(), rgb(0x1a, 0x1a, 0x2e));
        restore_github();
    }

    #[test]
    fn parses_hex_and_applies_skin() {
        let _g = THEME_LOCK.lock().unwrap();
        restore_github();
        apply_skin(&json!({
            "name": "test",
            "colors": { "background": "#000000", "accent": "#ff00aa", "foreground": "#ffffff" }
        }));
        assert_eq!(BACKGROUND(), Color::Rgb(0, 0, 0));
        assert_eq!(PRIMARY(), Color::Rgb(0xff, 0x00, 0xaa));
        assert_eq!(TEXT(), Color::Rgb(0xff, 0xff, 0xff));
        restore_github();
    }
}
