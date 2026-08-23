//! Talaria splash: ANSI Shadow wordmark plus the Hermes caduceus.
//!
//! **Wordmark:** live FIGlet font **ANSI Shadow** via [`crate::ui::widgets::big_text`].
//!
//! **Caduceus:** hand-composed Unicode Braille bitmap (U+2800–U+28FF), same
//! constant as Ink `CADUCEUS_ART` / `HERMES_CADUCEUS` in `hermes_cli/banner.py`.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::theme;

use super::big_text;

/// Official Ink `CADUCEUS_ART` (Braille, 15 rows).
const CADUCEUS: &[&str] = &[
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣀⡀⠀⣀⣀⠀⢀⣀⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⢀⣠⣴⣾⣿⣿⣇⠸⣿⣿⠇⣸⣿⣿⣷⣦⣄⡀⠀⠀⠀⠀⠀⠀",
    "⠀⢀⣠⣴⣶⠿⠋⣩⡿⣿⡿⠻⣿⡇⢠⡄⢸⣿⠟⢿⣿⢿⣍⠙⠿⣶⣦⣄⡀⠀",
    "⠀⠀⠉⠉⠁⠶⠟⠋⠀⠉⠀⢀⣈⣁⡈⢁⣈⣁⡀⠀⠉⠀⠙⠻⠶⠈⠉⠉⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣴⣿⡿⠛⢁⡈⠛⢿⣿⣦⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠿⣿⣦⣤⣈⠁⢠⣴⣿⠿⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠉⠻⢿⣿⣦⡉⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠘⢷⣦⣈⠛⠃⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢠⣴⠦⠈⠙⠿⣦⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠸⣿⣤⡈⠁⢤⣿⠇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠉⠛⠷⠄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣀⠑⢶⣄⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣿⠁⢰⡆⠈⡿⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠳⠈⣡⠞⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
];

/// Ink `CADUC_GRADIENT` indexes into `[primary, accent, border, muted]`.
const CADUCEUS_GRADIENT: &[u8] = &[2, 2, 1, 1, 0, 0, 1, 1, 2, 2, 3, 3, 3, 3, 3];
/// Ink `LOGO_GRADIENT` for the six ANSI Shadow rows.
const LOGO_GRADIENT: &[u8] = &[0, 0, 1, 1, 2, 2];

const TAGLINE: &str = "Unofficial TUI host for Hermes Agent";

fn tone(i: u8) -> Style {
    match i {
        0 => theme::accent(),
        1 => theme::tool(),
        2 => Style::default().fg(theme::INPUT_BORDER()),
        _ => theme::dim(),
    }
}

fn colorize(art: &[&str], gradient: &[u8]) -> Vec<Line<'static>> {
    art.iter()
        .enumerate()
        .map(|(i, text)| {
            let g = gradient.get(i).copied().unwrap_or(3);
            Line::from(Span::styled((*text).to_string(), tone(g)))
        })
        .collect()
}

/// `TALARIA` wordmark: live ANSI Shadow, else a plain title.
pub fn logo_lines() -> Vec<Line<'static>> {
    let generated = big_text::render_ansi_shadow("TALARIA");
    if generated.len() >= 5 {
        let g = LOGO_GRADIENT;
        return generated
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let gi = g.get(i).copied().unwrap_or(2);
                Line::from(Span::styled(text.clone(), tone(gi)))
            })
            .collect();
    }
    vec![Line::from(Span::styled("TALARIA", tone(0)))]
}

pub fn caduceus_lines() -> Vec<Line<'static>> {
    colorize(CADUCEUS, CADUCEUS_GRADIENT)
}

pub fn caduceus_width() -> u16 {
    CADUCEUS
        .iter()
        .map(|s| s.chars().count() as u16)
        .max()
        .unwrap_or(0)
}

pub fn tagline() -> &'static str {
    TAGLINE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caduceus_is_braille() {
        let sample = CADUCEUS[0];
        assert!(
            sample
                .chars()
                .any(|c| ('\u{2800}'..='\u{28FF}').contains(&c)),
            "caduceus must be Unicode Braille, not FIGlet"
        );
        assert_eq!(CADUCEUS.len(), 15);
    }

    #[test]
    fn tagline_does_not_claim_nous() {
        let t = tagline();
        assert!(t.to_ascii_lowercase().contains("unofficial"), "{t}");
        assert!(!t.contains("Nous Research"), "{t}");
        assert!(!t.contains("Digital Gods"), "{t}");
    }

    #[test]
    fn logo_is_talaria() {
        let generated = big_text::render_ansi_shadow("TALARIA");
        assert!(
            generated.len() >= 5,
            "expected a multi-row TALARIA wordmark, got {generated:?}"
        );
        let lines = logo_lines();
        assert!(!lines.is_empty());
    }
}
