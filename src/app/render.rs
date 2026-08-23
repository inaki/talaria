use ratatui::layout::Rect;
use ratatui::widgets::Block;
use ratatui::Frame;

use crate::theme;

use super::App;

pub fn draw(app: &mut App, f: &mut Frame) {
    let area = f.area();
    f.render_widget(
        Block::default().style(ratatui::style::Style::default().bg(theme::BACKGROUND())),
        area,
    );
    app.screen.as_screen_mut().render(
        f,
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height,
        },
    );
}
