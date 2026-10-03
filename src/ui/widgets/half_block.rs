//! `logo-art` half-block ANSI (`▄`/`▀` + 24-bit color) parsed for ratatui.
//!
//! Baked at compile time from `design/layer-winged-sneaker.png`. Splash still
//! uses the Braille mascot; this is the gallery exploration.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

const ANSI: &str = include_str!(concat!(env!("OUT_DIR"), "/mascot-logo-art.ansi"));

/// True-color half-block mascot from `logo-art`.
pub fn logo_art_lines() -> Vec<Line<'static>> {
    ansi_to_lines(ANSI)
}

fn ansi_to_lines(input: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut fg: Option<Color> = None;
    let mut bg: Option<Color> = None;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                let mut params = String::new();
                loop {
                    match chars.next() {
                        Some('m') | None => break,
                        Some(ch) => params.push(ch),
                    }
                }
                apply_sgr(&params, &mut fg, &mut bg);
            }
            continue;
        }
        if c == '\n' {
            lines.push(Line::from(std::mem::take(&mut spans)));
            continue;
        }
        if c == '\r' {
            continue;
        }
        let mut style = Style::default();
        if let Some(color) = fg {
            style = style.fg(color);
        }
        if let Some(color) = bg {
            style = style.bg(color);
        }
        if let Some(last) = spans.last_mut() {
            if last.style == style {
                last.content.to_mut().push(c);
                continue;
            }
        }
        spans.push(Span::styled(c.to_string(), style));
    }
    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }
    if lines.last().is_some_and(|line| line.spans.is_empty()) {
        lines.pop();
    }
    lines
}

fn apply_sgr(params: &str, fg: &mut Option<Color>, bg: &mut Option<Color>) {
    if params.is_empty() {
        *fg = None;
        *bg = None;
        return;
    }
    let nums: Vec<u16> = params.split(';').map(|p| p.parse().unwrap_or(0)).collect();
    let mut i = 0;
    while i < nums.len() {
        match nums[i] {
            0 => {
                *fg = None;
                *bg = None;
                i += 1;
            }
            39 => {
                *fg = None;
                i += 1;
            }
            49 => {
                *bg = None;
                i += 1;
            }
            38 if i + 4 < nums.len() && nums[i + 1] == 2 => {
                *fg = Some(Color::Rgb(
                    nums[i + 2] as u8,
                    nums[i + 3] as u8,
                    nums[i + 4] as u8,
                ));
                i += 5;
            }
            48 if i + 4 < nums.len() && nums[i + 1] == 2 => {
                *bg = Some(Color::Rgb(
                    nums[i + 2] as u8,
                    nums[i + 3] as u8,
                    nums[i + 4] as u8,
                ));
                i += 5;
            }
            _ => i += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_reads_half_block_truecolor() {
        let ansi = "\x1b[48;2;10;20;30;38;2;1;2;3m▄\x1b[m\n";
        let lines = ansi_to_lines(ansi);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans[0].content.as_ref(), "▄");
        assert_eq!(lines[0].spans[0].style.fg, Some(Color::Rgb(1, 2, 3)));
        assert_eq!(lines[0].spans[0].style.bg, Some(Color::Rgb(10, 20, 30)));
    }

    #[test]
    fn baked_logo_art_is_half_block() {
        let lines = logo_art_lines();
        assert!(lines.len() >= 8, "got {} lines", lines.len());
        let has_block = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|s| s.content.contains('▄') || s.content.contains('▀'))
        });
        assert!(has_block, "logo-art should emit ▄/▀");
        let has_rgb = lines.iter().any(|line| {
            line.spans.iter().any(|s| {
                matches!(s.style.fg, Some(Color::Rgb(_, _, _)))
                    || matches!(s.style.bg, Some(Color::Rgb(_, _, _)))
            })
        });
        assert!(has_rgb, "logo-art should carry 24-bit color");
    }
}
