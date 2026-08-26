//! Live design-system gallery. Not shipped in the `talaria` binary.
//!
//! ```sh
//! cargo run --example gallery
//! ```
//!
//! ↑/↓ move · Enter interact · t theme · q quit

fn main() -> std::io::Result<()> {
    talaria::ui::gallery::run()
}
