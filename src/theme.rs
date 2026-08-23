//! Named palettes (herald_v2 pattern). Widgets read tokens via accessors.
//!
//! Call [`apply_theme`] at startup (CLI / env / saved file) and from `/skin`.
//! `gateway.ready` skin overlay applies only while the GitHub default is active.

#![allow(non_snake_case)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;

use ratatui::style::{Color, Modifier, Style};
use serde_json::Value;

use crate::paths::{create_private_dir_all, ensure_private_file, HermesRustPaths};

pub const GITHUB_ID: &str = "github";

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

/// Hermes `default` — classic gold / kawaii (skin_engine.py).
pub const DEFAULT: ThemeDefinition = ThemeDefinition {
    id: "default",
    label: "default",
    description: "Classic Hermes — gold and kawaii",
    palette: Palette {
        background: rgb(0x1a, 0x1a, 0x2e),
        surface: rgb(0x33, 0x33, 0x55),
        primary: rgb(0xff, 0xd7, 0x00),
        text: rgb(0xff, 0xf8, 0xdc),
        text_dim: rgb(0xb8, 0x86, 0x0b),
        success: rgb(0x4c, 0xaf, 0x50),
        warning: rgb(0xff, 0xa7, 0x26),
        error: rgb(0xef, 0x53, 0x50),
        user: rgb(0xda, 0xa5, 0x20),
        tool: rgb(0xff, 0xbf, 0x00),
        selected_text: rgb(0xff, 0xf8, 0xdc),
        selection_bg: Some(rgb(0x3a, 0x3a, 0x55)),
        separator: rgb(0x8b, 0x86, 0x82),
        input_border: rgb(0xcd, 0x7f, 0x32),
    },
};

pub const ARES: ThemeDefinition = ThemeDefinition {
    id: "ares",
    label: "ares",
    description: "War-god theme — crimson and bronze",
    palette: Palette {
        background: rgb(0x2a, 0x12, 0x12),
        surface: rgb(0x5c, 0x22, 0x1d),
        primary: rgb(0xc7, 0xa9, 0x6b),
        text: rgb(0xf1, 0xe6, 0xcf),
        text_dim: rgb(0x90, 0x51, 0x51),
        success: rgb(0x4c, 0xaf, 0x50),
        warning: rgb(0xff, 0xa7, 0x26),
        error: rgb(0xef, 0x53, 0x50),
        user: rgb(0xc7, 0xa9, 0x6b),
        tool: rgb(0xdd, 0x4a, 0x3a),
        selected_text: rgb(0xf1, 0xe6, 0xcf),
        selection_bg: Some(rgb(0x69, 0x26, 0x20)),
        separator: rgb(0x6e, 0x58, 0x4b),
        input_border: rgb(0xa9, 0x33, 0x33),
    },
};

pub const MONO: ThemeDefinition = ThemeDefinition {
    id: "mono",
    label: "mono",
    description: "Monochrome — clean grayscale",
    palette: Palette {
        background: rgb(0x1f, 0x1f, 0x1f),
        surface: rgb(0x46, 0x46, 0x46),
        primary: rgb(0xe6, 0xed, 0xf3),
        text: rgb(0xc9, 0xd1, 0xd9),
        text_dim: rgb(0x60, 0x60, 0x60),
        success: rgb(0x88, 0x88, 0x88),
        warning: rgb(0x99, 0x99, 0x99),
        error: rgb(0xcc, 0xcc, 0xcc),
        user: rgb(0x88, 0x88, 0x88),
        tool: rgb(0xaa, 0xaa, 0xaa),
        selected_text: rgb(0xe6, 0xed, 0xf3),
        selection_bg: Some(rgb(0x50, 0x50, 0x50)),
        separator: rgb(0x5e, 0x5e, 0x5e),
        input_border: rgb(0x60, 0x60, 0x60),
    },
};

pub const SLATE: ThemeDefinition = ThemeDefinition {
    id: "slate",
    label: "slate",
    description: "Cool blue — developer-focused",
    palette: Palette {
        background: rgb(0x15, 0x1c, 0x2f),
        surface: rgb(0x32, 0x48, 0x67),
        primary: rgb(0x7e, 0xb8, 0xf6),
        text: rgb(0xc9, 0xd1, 0xd9),
        text_dim: rgb(0x54, 0x5e, 0x6b),
        success: rgb(0x63, 0xd0, 0xa6),
        warning: rgb(0xe6, 0xa8, 0x55),
        error: rgb(0xf7, 0xa0, 0x72),
        user: rgb(0x8e, 0xa8, 0xff),
        tool: rgb(0x7e, 0xb8, 0xf6),
        selected_text: rgb(0xc9, 0xd1, 0xd9),
        selection_bg: Some(rgb(0x3a, 0x53, 0x75)),
        separator: rgb(0x54, 0x5e, 0x6b),
        input_border: rgb(0x41, 0x69, 0xe1),
    },
};

pub const DAYLIGHT: ThemeDefinition = ThemeDefinition {
    id: "daylight",
    label: "daylight",
    description: "Light theme for bright terminals",
    palette: Palette {
        background: rgb(0xe5, 0xed, 0xf8),
        surface: rgb(0xdb, 0xea, 0xfe),
        primary: rgb(0x0f, 0x17, 0x2a),
        text: rgb(0x11, 0x18, 0x27),
        text_dim: rgb(0x47, 0x55, 0x69),
        success: rgb(0x15, 0x80, 0x3d),
        warning: rgb(0xb4, 0x53, 0x09),
        error: rgb(0xb9, 0x1c, 0x1c),
        user: rgb(0x0f, 0x76, 0x6e),
        tool: rgb(0x25, 0x63, 0xeb),
        selected_text: rgb(0x0f, 0x17, 0x2a),
        selection_bg: Some(rgb(0xd3, 0xe0, 0xfb)),
        separator: rgb(0x64, 0x74, 0x8b),
        input_border: rgb(0x6e, 0x94, 0xbe),
    },
};

pub const WARM_LIGHT: ThemeDefinition = ThemeDefinition {
    id: "warm-lightmode",
    label: "warm-lightmode",
    description: "Warm light mode — dark brown/gold text",
    palette: Palette {
        background: rgb(0xf5, 0xf0, 0xe8),
        surface: rgb(0xe8, 0xdc, 0xc8),
        primary: rgb(0x5c, 0x3d, 0x11),
        text: rgb(0x2c, 0x18, 0x10),
        text_dim: rgb(0x8b, 0x73, 0x55),
        success: rgb(0x2e, 0x7d, 0x32),
        warning: rgb(0xe6, 0x51, 0x00),
        error: rgb(0xc6, 0x28, 0x28),
        user: rgb(0x5c, 0x3d, 0x11),
        tool: rgb(0x8b, 0x45, 0x13),
        selected_text: rgb(0x2c, 0x18, 0x10),
        selection_bg: Some(rgb(0xe8, 0xda, 0xd0)),
        separator: rgb(0xa0, 0x84, 0x5c),
        input_border: rgb(0x8b, 0x69, 0x14),
    },
};

pub const POSEIDON: ThemeDefinition = ThemeDefinition {
    id: "poseidon",
    label: "poseidon",
    description: "Ocean-god theme — deep blue and seafoam",
    palette: Palette {
        background: rgb(0x0f, 0x24, 0x40),
        surface: rgb(0x25, 0x4d, 0x73),
        primary: rgb(0xa9, 0xdf, 0xff),
        text: rgb(0xea, 0xf7, 0xff),
        text_dim: rgb(0x44, 0x63, 0x8f),
        success: rgb(0x4c, 0xaf, 0x50),
        warning: rgb(0xff, 0xa7, 0x26),
        error: rgb(0xef, 0x53, 0x50),
        user: rgb(0xa9, 0xdf, 0xff),
        tool: rgb(0x5d, 0xb8, 0xf5),
        selected_text: rgb(0xea, 0xf7, 0xff),
        selection_bg: Some(rgb(0x2a, 0x58, 0x7f)),
        separator: rgb(0x49, 0x68, 0x84),
        input_border: rgb(0x2a, 0x6f, 0xb9),
    },
};

pub const SISYPHUS: ThemeDefinition = ThemeDefinition {
    id: "sisyphus",
    label: "sisyphus",
    description: "Sisyphean theme — austere grayscale",
    palette: Palette {
        background: rgb(0x20, 0x20, 0x20),
        surface: rgb(0x58, 0x58, 0x58),
        primary: rgb(0xf5, 0xf5, 0xf5),
        text: rgb(0xd3, 0xd3, 0xd3),
        text_dim: rgb(0x5c, 0x5c, 0x5c),
        success: rgb(0x91, 0x91, 0x91),
        warning: rgb(0xb7, 0xb7, 0xb7),
        error: rgb(0xe7, 0xe7, 0xe7),
        user: rgb(0xd3, 0xd3, 0xd3),
        tool: rgb(0xe7, 0xe7, 0xe7),
        selected_text: rgb(0xf5, 0xf5, 0xf5),
        selection_bg: Some(rgb(0x66, 0x66, 0x66)),
        separator: rgb(0x65, 0x65, 0x65),
        input_border: rgb(0x65, 0x65, 0x65),
    },
};

pub const CHARIZARD: ThemeDefinition = ThemeDefinition {
    id: "charizard",
    label: "charizard",
    description: "Volcanic theme — burnt orange and ember",
    palette: Palette {
        background: rgb(0x2b, 0x16, 0x0e),
        surface: rgb(0x4a, 0x1b, 0x07),
        primary: rgb(0xff, 0xd3, 0x9a),
        text: rgb(0xff, 0xf0, 0xd4),
        text_dim: rgb(0xc5, 0x8a, 0x45),
        success: rgb(0x4c, 0xaf, 0x50),
        warning: rgb(0xff, 0xa7, 0x26),
        error: rgb(0xef, 0x53, 0x50),
        user: rgb(0xff, 0xd3, 0x9a),
        tool: rgb(0xf2, 0x9c, 0x38),
        selected_text: rgb(0xff, 0xf0, 0xd4),
        selection_bg: Some(rgb(0x5a, 0x26, 0x0d)),
        separator: rgb(0x7b, 0x59, 0x3a),
        input_border: rgb(0xc7, 0x5b, 0x1d),
    },
};

const THEMES: &[ThemeDefinition] = &[
    GITHUB, DEFAULT, ARES, MONO, SLATE, DAYLIGHT, WARM_LIGHT, POSEIDON, SISYPHUS, CHARIZARD,
];

static PALETTE: RwLock<Palette> = RwLock::new(GITHUB.palette);
static THEME_ID: RwLock<&'static str> = RwLock::new(GITHUB_ID);
/// Set when the palette changes. The event loop calls `Terminal::clear`
/// so ratatui does not skip cells after a skin swap.
static CANVAS_DIRTY: AtomicBool = AtomicBool::new(false);

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
    CANVAS_DIRTY.store(true, Ordering::Relaxed);
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
    if let Ok(mut f) = opts.open(&path) {
        ensure_private_file(&path);
        use std::io::Write;
        let _ = writeln!(f, "{}", def.id);
    }
}

/// Mark the next ratatui frame as a full redraw (arrow-key preview).
pub fn sync_terminal_canvas() {
    CANVAS_DIRTY.store(true, Ordering::Relaxed);
}

/// Same as [`sync_terminal_canvas`]. Never `Clear(All)` while ratatui owns
/// the alternate screen — that desyncs the backend buffer and punches holes
/// in the splash (logo, status, composer).
pub fn sync_terminal_canvas_hard() {
    CANVAS_DIRTY.store(true, Ordering::Relaxed);
}

pub fn take_canvas_dirty() -> bool {
    CANVAS_DIRTY.swap(false, Ordering::Relaxed)
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
    fn default_and_ares_palettes() {
        let _g = THEME_LOCK.lock().unwrap();
        assert!(apply_theme("default"));
        assert_eq!(current_theme_id(), "default");
        assert_eq!(BACKGROUND(), rgb(0x1a, 0x1a, 0x2e));
        assert_eq!(PRIMARY(), rgb(0xff, 0xd7, 0x00));
        assert_eq!(INPUT_BORDER(), rgb(0xcd, 0x7f, 0x32));
        assert_eq!(SEPARATOR(), rgb(0x8b, 0x86, 0x82));

        assert!(apply_theme("ARES"));
        assert_eq!(current_theme_id(), "ares");
        assert_eq!(BACKGROUND(), rgb(0x2a, 0x12, 0x12));
        assert_eq!(PRIMARY(), rgb(0xc7, 0xa9, 0x6b));
        assert_eq!(TOOL(), rgb(0xdd, 0x4a, 0x3a));

        assert!(!apply_theme("gold"));
        assert!(!apply_theme("not-a-theme"));
        restore_github();
        assert_eq!(current_theme_id(), GITHUB_ID);
    }

    #[test]
    fn apply_theme_marks_canvas_dirty() {
        let _g = THEME_LOCK.lock().unwrap();
        let _ = take_canvas_dirty();
        assert!(apply_theme("ares"));
        assert!(take_canvas_dirty());
        assert!(!take_canvas_dirty());
        restore_github();
        let _ = take_canvas_dirty();
    }

    #[test]
    fn lists_official_skins_and_github() {
        let ids: Vec<&str> = themes().iter().map(|t| t.id).collect();
        assert_eq!(
            ids,
            [
                "github",
                "default",
                "ares",
                "mono",
                "slate",
                "daylight",
                "warm-lightmode",
                "poseidon",
                "sisyphus",
                "charizard",
            ]
        );
    }

    #[test]
    fn skin_ignored_on_named_theme() {
        let _g = THEME_LOCK.lock().unwrap();
        assert!(apply_theme("default"));
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
