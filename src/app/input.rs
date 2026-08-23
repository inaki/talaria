use crossterm::event::KeyEvent;

use super::App;

pub fn on_key(app: &mut App, key: KeyEvent) {
    if let Some(action) = app.screen.as_screen_mut().handle_key(key) {
        super::dispatch::dispatch(app, action);
    }
}
