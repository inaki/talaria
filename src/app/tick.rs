use crate::session::{SessionApi, SessionCommand, SessionEvent};
use crate::ui::screens::CurrentScreen;

use super::App;

pub fn on_tick(app: &mut App) {
    let mut drained = Vec::new();
    if let Some(rx) = app.events.as_mut() {
        while let Ok(ev) = rx.try_recv() {
            drained.push(ev);
        }
    }
    for ev in drained {
        apply(app, ev);
    }
    let CurrentScreen::Chat(chat) = &mut app.screen;
    if chat.take_pending_usage() {
        if let Some(session) = &app.session {
            session.send(SessionCommand::FetchUsage);
        }
    }
    app.screen.as_screen_mut().tick();
}

fn apply(app: &mut App, ev: SessionEvent) {
    let CurrentScreen::Chat(chat) = &mut app.screen;
    chat.apply_event(ev);
    if let Some(text) = chat.take_pending_send() {
        chat.push_user(text.clone());
        if let Some(session) = &app.session {
            session.send(SessionCommand::Submit { text });
        }
    }
}
