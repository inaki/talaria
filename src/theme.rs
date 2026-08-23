//! Single source of truth for colors. No raw `Rgb` outside this file.
//!
//! Default is a dark GitHub-ish palette. `apply_skin` overlays whatever
//! `gateway.ready` `payload.skin` we can parse (object, not a string).

#![allow(non_snake_case)]

use std::sync::RwLock;

use ratatui::style::{Color, Modifier, Style};
use serde_json::Value;

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
}

const DEFAULT: Palette = Palette {
    background: Color::Rgb(0x0d, 0x11, 0x17),
    surface: Color::Rgb(0x16, 0x1b, 0x22),
    primary: Color::Rgb(0x58, 0xa6, 0xff),
    text: Color::Rgb(0xe6, 0xed, 0xf3),
    text_dim: Color::Rgb(0x8b, 0x94, 0x9e),
    success: Color::Rgb(0x3f, 0xb9, 0x50),
    warning: Color::Rgb(0xd2, 0x99, 0x22),
    error: Color::Rgb(0xf8, 0x51, 0x49),
    user: Color::Rgb(0xa3, 0x71, 0xf7),
    tool: Color::Rgb(0x79, 0xc0, 0xff),
};

static PALETTE: RwLock<Palette> = RwLock::new(DEFAULT);

fn pal() -> Palette {
    PALETTE.read().unwrap_or_else(|e| e.into_inner()).clone()
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

/// Overlay palette from a Hermes `GatewaySkin` object. Unknown shapes are ignored.
pub fn apply_skin(skin: &Value) {
    let Some(map) = colors_object(skin) else {
        return;
    };
    let mut p = pal();
    if let Some(c) = color_in(map, &["background", "bg", "canvas", "bg_primary"]) {
        p.background = c;
        p.surface = shift(c, 10);
    }
    if let Some(c) = color_in(map, &["foreground", "fg", "text", "fg_primary"]) {
        p.text = c;
    }
    if let Some(c) = color_in(map, &["muted", "dim", "comment", "fg_secondary"]) {
        p.text_dim = c;
    }
    if let Some(c) = color_in(map, &["accent", "primary", "blue", "link"]) {
        p.primary = c;
        p.tool = c;
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

    #[test]
    fn parses_hex_and_applies_skin() {
        apply_skin(&json!({
            "name": "test",
            "colors": { "background": "#000000", "accent": "#ff00aa", "foreground": "#ffffff" }
        }));
        assert_eq!(BACKGROUND(), Color::Rgb(0, 0, 0));
        assert_eq!(PRIMARY(), Color::Rgb(0xff, 0x00, 0xaa));
        assert_eq!(TEXT(), Color::Rgb(0xff, 0xff, 0xff));
        // restore default so other tests are not tinted
        if let Ok(mut g) = PALETTE.write() {
            *g = DEFAULT;
        }
    }
}
