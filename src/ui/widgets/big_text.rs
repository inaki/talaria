//! FIGlet big-text helper for the Talaria splash wordmark.

use figrs::{Figlet, FigletOptions};
use unicode_width::UnicodeWidthStr;

/// Font used for the `TALARIA` wordmark.
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

/// ANSI Shadow with no horizontal smush: each letter keeps its full cell
/// width so lower rows (the T stem, A feet, …) stay under their tops.
pub fn render_ansi_shadow(text: &str) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    let mut glyphs: Vec<Vec<String>> = Vec::new();
    let mut widths: Vec<usize> = Vec::new();
    for c in text.chars() {
        if c.is_whitespace() {
            glyphs.push(Vec::new());
            widths.push(3);
            continue;
        }
        let g = render(&c.to_string(), ANSI_SHADOW);
        if g.is_empty() {
            continue;
        }
        let w = g.iter().map(|l| l.width()).max().unwrap_or(0);
        glyphs.push(g);
        widths.push(w);
    }
    if glyphs.is_empty() {
        return Vec::new();
    }
    let rows = glyphs.iter().map(|g| g.len()).max().unwrap_or(0);
    let mut out = vec![String::new(); rows];
    for (glyph, w) in glyphs.iter().zip(widths) {
        for (r, row) in out.iter_mut().enumerate() {
            let line = glyph.get(r).map(String::as_str).unwrap_or("");
            row.push_str(line);
            let pad = w.saturating_sub(line.width());
            if pad > 0 {
                row.push_str(&" ".repeat(pad));
            }
        }
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_shadow_talaria_is_block_rows() {
        let lines = render_ansi_shadow("TALARIA");
        assert!(
            lines.len() >= 5,
            "expected a multi-row FIGlet banner, got {lines:?}"
        );
        let joined = lines.join("\n");
        assert!(
            joined.contains('█') || joined.contains('#') || joined.contains('╗'),
            "ANSI Shadow should use block/box glyphs: {joined}"
        );
        let titled = render_ansi_shadow("TALARIA - CLIENT");
        assert!(
            titled.iter().map(|l| l.width()).max().unwrap_or(0)
                > lines.iter().map(|l| l.width()).max().unwrap_or(0),
            "TALARIA - CLIENT should be wider than TALARIA"
        );
    }

    #[test]
    fn ansi_shadow_does_not_smush_t_into_the_rest() {
        let t = render("T", ANSI_SHADOW);
        let a = render("A", ANSI_SHADOW);
        let ta = render_ansi_shadow("TA");
        let t_w = t.iter().map(|l| l.width()).max().unwrap_or(0);
        assert!(t_w > 0, "T glyph should have width");
        let a_rows = a.len().max(t.len());
        assert_eq!(ta.len(), a_rows.max(t.len()));
        for r in 0..ta.len() {
            let row = &ta[r];
            assert!(
                row.width() >= t_w,
                "row {r} is narrower than T ({t_w}): {row:?}"
            );
            let rest = {
                let mut w = 0usize;
                let mut idx = 0usize;
                for (i, ch) in row.char_indices() {
                    if w >= t_w {
                        idx = i;
                        break;
                    }
                    w += unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                    idx = i + ch.len_utf8();
                }
                &row[idx..]
            };
            let a_line = a.get(r).map(String::as_str).unwrap_or("");
            assert!(
                rest.starts_with(a_line) || a_line.is_empty(),
                "row {r}: after T should be A, got rest={rest:?} a={a_line:?}\nfull={row:?}"
            );
        }
    }
}
