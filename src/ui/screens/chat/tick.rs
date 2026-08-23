use super::Chat;

pub(super) fn on_tick(chat: &mut Chat) {
    if chat.streaming || chat.thinking {
        chat.spinner.tick();
    }
}
