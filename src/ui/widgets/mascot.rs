//! Winged-sneaker mascot as Unicode Braille (same 2×4 pixel dots as caduceus).
//!
//! Source: `design/layer-winged-sneaker.png`, packed at compile time.
//! Wings are gold (`agent`); the sneaker is mint (`accent`).

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::theme;

include!(concat!(env!("OUT_DIR"), "/mascot_data.rs"));

pub fn mascot_art() -> String {
    let mut out = String::new();
    for (glyphs, _) in MASCOT_ROWS {
        out.push_str(glyphs);
        out.push('\n');
    }
    out
}

/// Braille mascot with per-cell wing vs sneaker color.
pub fn mascot_lines() -> Vec<Line<'static>> {
    MASCOT_ROWS
        .iter()
        .map(|(glyphs, tones)| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            for (ch, t) in glyphs.chars().zip(tones.chars()) {
                let style = match t {
                    '1' => theme::agent(),
                    '2' => theme::accent(),
                    _ => Style::default(),
                };
                if let Some(last) = spans.last_mut() {
                    if last.style == style {
                        last.content.to_mut().push(ch);
                        continue;
                    }
                }
                spans.push(Span::styled(ch.to_string(), style));
            }
            Line::from(spans)
        })
        .collect()
}

pub fn mascot_width() -> u16 {
    MASCOT_ROWS
        .iter()
        .map(|(g, _)| g.chars().count() as u16)
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mascot_is_braille_like_caduceus() {
        let sample = MASCOT_ROWS[0].0;
        assert!(
            sample
                .chars()
                .any(|c| ('\u{2800}'..='\u{28FF}').contains(&c)),
            "mascot must be Unicode Braille, not half-blocks"
        );
        let art = mascot_art();
        assert!(!art.contains('▄') && !art.contains('▀'));
        assert!(MASCOT_ROWS.len() >= 8, "expected a multi-row mascot");
        assert_eq!(mascot_width(), 24);
        let lines = mascot_lines();
        assert_eq!(lines.len(), MASCOT_ROWS.len());
    }

    #[test]
    fn wings_and_sneaker_use_different_tones() {
        let mut wing = 0usize;
        let mut sneaker = 0usize;
        for (_, tones) in MASCOT_ROWS {
            wing += tones.chars().filter(|c| *c == '1').count();
            sneaker += tones.chars().filter(|c| *c == '2').count();
        }
        assert!(wing > 8, "expected a wing region, got {wing} cells");
        assert!(
            sneaker > 8,
            "expected a sneaker region, got {sneaker} cells"
        );
        let lines = mascot_lines();
        let gold = theme::agent().fg;
        let mint = theme::accent().fg;
        let has_gold = lines.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.style.fg == gold && s.content.chars().any(|c| c != '\u{2800}'))
        });
        let has_mint = lines.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.style.fg == mint && s.content.chars().any(|c| c != '\u{2800}'))
        });
        assert!(has_gold, "wings should render in agent gold");
        assert!(has_mint, "sneaker should render in accent mint");
        assert_ne!(gold, mint);
    }

    #[test]
    fn inner_wing_peak_is_gold() {
        // Right-hand peak in the top rows is the inner wing, not the tongue.
        let mut wing = 0usize;
        let mut sneaker = 0usize;
        for (_, tones) in MASCOT_ROWS.iter().take(8) {
            let skip = tones.chars().count() / 4;
            for (i, t) in tones.chars().enumerate() {
                if i < skip {
                    continue;
                }
                match t {
                    '1' => wing += 1,
                    '2' => sneaker += 1,
                    _ => {}
                }
            }
        }
        assert!(
            wing > sneaker && wing > 8,
            "inner wing should be gold, got wing={wing} sneaker={sneaker}"
        );
    }

    #[test]
    fn shoe_collar_is_mint() {
        // Under the inner wing, toward the laces — the upper shoe, not feathers.
        let n = MASCOT_ROWS.len();
        let mut wing = 0usize;
        let mut sneaker = 0usize;
        let start = n / 3;
        let end = (n * 2) / 3;
        for (_, tones) in MASCOT_ROWS.iter().take(end).skip(start) {
            let cols = tones.chars().count();
            for (i, t) in tones.chars().enumerate() {
                if i < cols * 5 / 12 {
                    continue;
                }
                match t {
                    '1' => wing += 1,
                    '2' => sneaker += 1,
                    _ => {}
                }
            }
        }
        assert!(
            sneaker > wing && sneaker > 8,
            "shoe collar should be mint, got wing={wing} sneaker={sneaker}"
        );
    }
}
