use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::keys::{is_ctrl_c, is_ctrl_o, is_ctrl_v, typed_char};
use crate::ui::screens::chat::overlay;
use crate::ui::screens::ScreenAction;
use crate::ui::widgets::ComposerAction;

use super::Chat;

pub(super) fn on_key(chat: &mut Chat, key: KeyEvent) -> Option<ScreenAction> {
    if chat.confirm_quit {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => return Some(ScreenAction::Quit),
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                chat.confirm_quit = false;
                return None;
            }
            _ if is_ctrl_c(&key) => return Some(ScreenAction::Quit),
            _ => return None,
        }
    }

    if is_ctrl_c(&key) {
        chat.confirm_quit = true;
        return None;
    }

    if is_ctrl_o(&key) && !chat.overlay.is_open() {
        chat.toggle_last_tool();
        return None;
    }

    if is_ctrl_v(&key) && !chat.overlay.is_open() && !chat.slash.is_active() {
        return Some(ScreenAction::ClipboardPaste);
    }

    if chat.overlay.is_open() {
        return overlay::handle_overlay_key(&mut chat.overlay, key);
    }

    if chat.slash.is_active() {
        return slash_key(chat, key);
    }

    if key.code == KeyCode::Esc {
        if chat.streaming || chat.thinking {
            chat.finish_turn();
            return Some(ScreenAction::Interrupt);
        }
        return None;
    }

    if key.code == KeyCode::PageUp {
        chat.follow = false;
        chat.scroll = chat.scroll.saturating_sub(8);
        return None;
    }
    if key.code == KeyCode::PageDown {
        chat.scroll = chat.scroll.saturating_add(8);
        return None;
    }

    if chat.composer.draft.is_empty() {
        if let Some('/') = typed_char(&key) {
            chat.slash.open();
            return None;
        }
    }

    match chat.composer.handle_key(key) {
        ComposerAction::Submit(text) => {
            if let Some(cmd) = parse_slash_submit(&text) {
                return local_or_dispatch(chat, &cmd);
            }
            if text.trim() == "!" {
                chat.notice = Some("Usage: !<command>  e.g. !pwd".into());
                return None;
            }
            if let Some(cmd) = crate::shell::command_from_input(&text) {
                chat.overlay = super::Overlay::BangConfirm {
                    command: cmd.to_string(),
                    cwd: chat.cwd.clone(),
                };
                return None;
            }
            if chat.streaming || chat.thinking {
                chat.items
                    .push(super::TimelineItem::Status(format!("steer: {text}")));
                return Some(ScreenAction::Steer { text });
            }
            chat.push_user(text.clone());
            Some(ScreenAction::SubmitPrompt(text))
        }
        ComposerAction::None => None,
    }
}

fn slash_key(chat: &mut Chat, key: KeyEvent) -> Option<ScreenAction> {
    match key.code {
        KeyCode::Esc => {
            chat.slash.close();
            None
        }
        KeyCode::Up => {
            chat.slash.move_up();
            None
        }
        KeyCode::Down => {
            chat.slash.move_down();
            None
        }
        KeyCode::Enter => {
            let cmd = chat.slash.selected_dispatch();
            chat.slash.close();
            match cmd {
                Some(c) => local_or_dispatch(chat, &c),
                None => None,
            }
        }
        KeyCode::Backspace => {
            let _ = chat.slash.backspace();
            None
        }
        _ => {
            if let Some(c) = typed_char(&key) {
                chat.slash.append(c);
            }
            None
        }
    }
}

fn parse_slash_submit(text: &str) -> Option<String> {
    let t = text.trim();
    if t.starts_with('/') {
        Some(t.to_string())
    } else {
        None
    }
}

fn local_or_dispatch(chat: &mut Chat, cmd: &str) -> Option<ScreenAction> {
    let (name, rest) = split_cmd(cmd);
    match name {
        "clear" => {
            chat.items.clear();
            chat.scroll = 0;
            chat.notice = Some("transcript cleared".into());
            None
        }
        "quit" | "exit" => Some(ScreenAction::Quit),
        "resume" if rest.is_empty() => Some(ScreenAction::OpenSessions),
        "sessions" if rest == "new" => Some(ScreenAction::NewSession),
        "sessions" => Some(ScreenAction::OpenSessions),
        "resume" => Some(ScreenAction::ResumeSaved {
            session_id: rest.to_string(),
        }),
        "help" => {
            chat.open_help();
            None
        }
        "branch" => Some(ScreenAction::Branch {
            title: if rest.is_empty() {
                None
            } else {
                Some(rest.to_string())
            },
        }),
        "agents" => Some(ScreenAction::OpenAgents),
        "trees" => Some(ScreenAction::OpenSpawnTrees),
        "rewind" | "restore" => Some(ScreenAction::OpenRewind),
        "skin" | "theme" if rest.is_empty() => Some(ScreenAction::OpenTheme),
        "skin" | "theme" => {
            if crate::theme::apply_theme(rest) {
                crate::theme::save_theme_id(rest);
                crate::theme::sync_terminal_canvas_hard();
                chat.notice = Some(format!("skin {}", crate::theme::current_theme_label()));
            } else {
                chat.notice = Some(format!(
                    "unknown skin {rest} (github, default, ares, mono, slate, …)"
                ));
            }
            None
        }
        "model" if rest.is_empty() => Some(ScreenAction::OpenModel { refresh: false }),
        "model" if rest == "--refresh" => Some(ScreenAction::OpenModel { refresh: true }),
        "model" => Some(ScreenAction::SetModel {
            value: rest.to_string(),
            confirm_expensive_model: false,
        }),
        "skills" if rest.is_empty() => Some(ScreenAction::OpenSkills),
        "skills" if rest.starts_with("install ") => Some(ScreenAction::InstallSkill {
            query: rest["install ".len()..].trim().to_string(),
        }),
        "plugins" if rest.is_empty() => Some(ScreenAction::OpenPlugins),
        "mcp" => Some(ScreenAction::OpenMcp),
        "usage" => Some(ScreenAction::OpenUsage),
        "custom" => {
            chat.open_custom();
            None
        }
        _ => {
            chat.items
                .push(super::TimelineItem::Status(format!("/{name}")));
            Some(ScreenAction::DispatchCommand {
                command: cmd.to_string(),
            })
        }
    }
}

fn split_cmd(cmd: &str) -> (&str, &str) {
    let s = cmd.trim().trim_start_matches('/');
    match s.split_once(char::is_whitespace) {
        Some((n, r)) => (n, r.trim()),
        None => (s, ""),
    }
}

impl Chat {
    pub(crate) fn handle_paste_inner(&mut self, text: String) -> Option<ScreenAction> {
        if let super::Overlay::Model(picker) = &mut self.overlay {
            if picker.stage == super::model_picker::ModelStage::Key && !picker.key_saving {
                picker.key_input.push_str(text.trim());
                return None;
            }
        }
        if self.overlay.is_open() || self.slash.is_active() || self.confirm_quit {
            return None;
        }
        if let Some(path) = image_path_from_paste(&text) {
            return Some(ScreenAction::AttachImage { path });
        }
        self.composer.insert_str(&text);
        None
    }
}

fn image_path_from_paste(text: &str) -> Option<String> {
    let t = text.trim();
    if t.contains('\n') || t.contains(' ') {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    if lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".webp")
        || lower.ends_with(".gif")
    {
        Some(t.to_string())
    } else {
        None
    }
}
