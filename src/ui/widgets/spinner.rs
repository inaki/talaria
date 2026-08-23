const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Debug, Default)]
pub struct Spinner {
    frame: usize,
}

impl Spinner {
    pub fn tick(&mut self) {
        self.frame = (self.frame + 1) % FRAMES.len();
    }

    pub fn glyph(&self) -> &'static str {
        FRAMES[self.frame]
    }
}
