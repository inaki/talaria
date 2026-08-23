//! FIGlet big-text helper (same family as the official `HERMES-AGENT` banner).
//!
//! The Ink TUI does **not** call figlet at runtime: `ui-tui/src/banner.ts`
//! ships pre-rendered **ANSI Shadow** output. This module wraps `figrs` so we
//! can regenerate that look (or any other title) from a string.

use figrs::{Figlet, FigletOptions};

/// Font used for the official `HERMES-AGENT` wordmark.
pub const ANSI_SHADOW: &str = "ANSI Shadow";

/// Render `text` with a FIGlet font. Empty vec on unknown font / empty input.
pub fn render(text: &str, font: &str) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    let opt = FigletOptions {
        font: font.to_string(),
        ..FigletOptions::default()
    };
    let Ok(out) = Figlet::text(text.to_string(), opt) else {
        return Vec::new();
    };
    let mut lines: Vec<String> = out.text.lines().map(|l| l.trim_end().to_string()).collect();
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines
}

pub fn render_ansi_shadow(text: &str) -> Vec<String> {
    render(text, ANSI_SHADOW)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_shadow_hermes_agent_is_block_rows() {
        let lines = render_ansi_shadow("HERMES-AGENT");
        assert!(
            lines.len() >= 5,
            "expected a multi-row FIGlet banner, got {lines:?}"
        );
        let joined = lines.join("\n");
        assert!(
            joined.contains('█') || joined.contains('#') || joined.contains('╗'),
            "ANSI Shadow should use block/box glyphs: {joined}"
        );
    }
}
