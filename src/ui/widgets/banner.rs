//! Official Hermes splash art.
//!
//! **Wordmark:** FIGlet font **ANSI Shadow**, pre-rendered in the Ink TUI
//! (`ui-tui/src/banner.ts` `LOGO_ART`) and regenerated here via
//! [`crate::ui::widgets::big_text`].
//!
//! **Caduceus (image 2):** not FIGlet. It is a **hand-composed Unicode
//! Braille bitmap**. Each cell is U+2800–U+28FF (2×4 dots). They drew the
//! staff as pixels and encoded each 2×4 window as one braille character,
//! then colorized rows with a gold→amber→bronze gradient. Same constant as
//! `HERMES_CADUCEUS` in `hermes_cli/banner.py`.

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

const TAGLINE: &str = "Nous Research · Messenger of the Digital Gods";

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

/// `HERMES-AGENT` wordmark: live ANSI Shadow, else the Ink pre-render.
pub fn logo_lines() -> Vec<Line<'static>> {
    let generated = big_text::render_ansi_shadow("HERMES-AGENT");
    if generated.len() >= 5 {
        let rows: Vec<&str> = generated.iter().map(String::as_str).collect();
        let g = LOGO_GRADIENT;
        return rows
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let gi = g.get(i).copied().unwrap_or(2);
                Line::from(Span::styled((*text).to_string(), tone(gi)))
            })
            .collect();
    }
    // Fallback: official pre-rendered ANSI Shadow (Ink `LOGO_ART`).
    colorize(
        &[
            "██╗  ██╗███████╗██████╗ ███╗   ███╗███████╗███████╗       █████╗  ██████╗ ███████╗███╗   ██╗████████╗",
            "██║  ██║██╔════╝██╔══██╗████╗ ████║██╔════╝██╔════╝      ██╔══██╗██╔════╝ ██╔════╝████╗  ██║╚══██╔══╝",
            "███████║█████╗  ██████╔╝██╔████╔██║█████╗  ███████╗█████╗███████║██║  ███╗█████╗  ██╔██╗ ██║   ██║   ",
            "██╔══██║██╔══╝  ██╔══██╗██║╚██╔╝██║██╔══╝  ╚════██║╚════╝██╔══██║██║   ██║██╔══╝  ██║╚██╗██║   ██║   ",
            "██║  ██║███████╗██║  ██║██║ ╚═╝ ██║███████╗███████║      ██║  ██║╚██████╔╝███████╗██║ ╚████║   ██║   ",
            "╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝╚═╝     ╚═╝╚══════╝╚══════╝      ╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚═╝  ╚═══╝   ╚═╝   ",
        ],
        LOGO_GRADIENT,
    )
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
}
