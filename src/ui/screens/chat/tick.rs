use std::time::Instant;

use super::Chat;

pub(super) fn on_tick(chat: &mut Chat) {
    if chat.streaming || chat.thinking {
        chat.spinner.tick();
    }
    if let Some(until) = chat.notice_expires {
        if Instant::now() >= until {
            chat.notice = None;
            chat.notice_expires = None;
        }
    }
}
