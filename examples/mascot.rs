//! Winged-sneaker mascot as Unicode Braille (gold wings, mint sneaker).
//!
//! ```sh
//! cargo run --example mascot
//! ```

use ratatui::style::Color;
use talaria::ui::widgets::mascot_lines;

fn main() {
    for line in mascot_lines() {
        for span in line.spans {
            match span.style.fg {
                Some(Color::Rgb(r, g, b)) => print!("\x1b[38;2;{r};{g};{b}m{}", span.content),
                _ => print!("{}", span.content),
            }
        }
        print!("\x1b[0m\n");
    }
}
