//! Fixed-width list selector column (Herald SelectableList).

use unicode_width::UnicodeWidthStr;

pub const COLS: usize = 2;
/// ASCII spaces get eaten by `Wrap { trim }` and some terminals.
const PAD: &str = "\u{00A0}";

/// Always `COLS` display cells: `▸` when selected, pads otherwise.
pub fn prefix(selected: bool) -> String {
    let g = if selected { "▸" } else { "" };
    let mut out = g.to_string();
    while UnicodeWidthStr::width(out.as_str()) < COLS {
        out.push_str(PAD);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_is_two_cells() {
        assert_eq!(UnicodeWidthStr::width(prefix(true).as_str()), COLS);
        assert_eq!(UnicodeWidthStr::width(prefix(false).as_str()), COLS);
    }
}
