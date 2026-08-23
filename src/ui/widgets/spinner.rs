//! Named spinner. Default is `circleHalves` (`◐ ◓ ◑ ◒` at 50ms).

use ratatui::style::Style;
use ratatui::text::Span;

#[derive(Debug, Clone, Copy)]
pub struct SpinnerSpec {
    pub name: &'static str,
    pub interval_ms: u64,
    pub frames: &'static [&'static str],
}

pub const CIRCLE_HALVES: SpinnerSpec = SpinnerSpec {
    name: "circleHalves",
    interval_ms: 50,
    frames: &["◐", "◓", "◑", "◒"],
};

#[derive(Debug)]
pub struct Spinner {
    spec: &'static SpinnerSpec,
    frame: usize,
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new(&CIRCLE_HALVES)
    }
}

impl Spinner {
    pub fn new(spec: &'static SpinnerSpec) -> Self {
        Self { spec, frame: 0 }
    }

    pub fn name(&self) -> &'static str {
        self.spec.name
    }

    pub fn interval_ms(&self) -> u64 {
        self.spec.interval_ms
    }

    pub fn tick(&mut self) {
        if self.spec.frames.is_empty() {
            return;
        }
        self.frame = (self.frame + 1) % self.spec.frames.len();
    }

    pub fn glyph(&self) -> &'static str {
        self.spec.frames.get(self.frame).copied().unwrap_or("◐")
    }

    pub fn span(&self, style: Style) -> Span<'static> {
        Span::styled(self.glyph(), style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_halves_spec() {
        let s = Spinner::default();
        assert_eq!(s.name(), "circleHalves");
        assert_eq!(s.interval_ms(), 50);
        assert_eq!(s.glyph(), "◐");
        let mut s = s;
        s.tick();
        assert_eq!(s.glyph(), "◓");
        s.tick();
        assert_eq!(s.glyph(), "◑");
        s.tick();
        assert_eq!(s.glyph(), "◒");
        s.tick();
        assert_eq!(s.glyph(), "◐");
    }
}
