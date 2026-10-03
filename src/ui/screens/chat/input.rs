use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::keys::{is_ctrl_c, is_ctrl_enter, is_ctrl_k, is_ctrl_o, is_ctrl_v, typed_char};
use crate::ui::screens::chat::{overlay, OpenSheet};
use crate::ui::screens::ScreenAction;
use crate::ui::widgets::{
    ComposerAction, InspectorAction, PaletteAction, PaletteRun, SheetAction, SheetKind,
};

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

    if chat.overlay.is_open() {
        return overlay::handle_overlay_key(&mut chat.overlay, key);
    }

    if is_ctrl_k(&key) {
        if chat.palette.is_active() {
            chat.palette.close();
            return None;
        }
        chat.slash.close();
        chat.palette.set_catalog(chat.slash.commands().to_vec());
        chat.palette.open();
        return None;
    }

    if chat.palette.is_active() {
        return match chat.palette.handle_key(key) {
            Some(PaletteAction::Close) => {
                chat.palette.close();
                None
            }
            Some(PaletteAction::Run(run)) => {
                chat.palette.close();
                run_palette(chat, run)
            }
            None => None,
        };
    }

    if let Some(ins) = chat.inspector.as_mut() {
        match ins.handle_key(key) {
            Some(InspectorAction::Close) => {
                chat.inspector = None;
                return None;
            }
            Some(InspectorAction::Copy) => {
                let text = ins.body_text();
                if !text.is_empty() {
                    chat.apply_copy(&text);
                }
                return None;
            }
            None => return None,
        }
    }

    if let Some(open) = chat.sheet.as_mut() {
        return match open {
            OpenSheet::List(sheet) => match sheet.handle_key(key) {
                Some(
                    SheetAction::Close
                    | SheetAction::RestoreTheme { .. }
                    | SheetAction::ApplyTheme { .. },
                ) => {
                    chat.sheet = None;
                    None
                }
                Some(SheetAction::Handled) => None,
                Some(SheetAction::New) => {
                    chat.sheet = None;
                    Some(ScreenAction::NewSession)
                }
                Some(SheetAction::Save) => Some(ScreenAction::SaveSpawnTree),
                Some(SheetAction::CloseLive { id }) => {
                    Some(ScreenAction::CloseLive { session_id: id })
                }
                Some(SheetAction::ToggleCustom) => {
                    if let Some(OpenSheet::List(sheet)) = &chat.sheet {
                        match sheet.selected_id().as_deref() {
                            Some("status_bar") => {
                                crate::prefs::toggle_status_bar();
                            }
                            Some("key_hints") => {
                                crate::prefs::toggle_key_hints();
                            }
                            Some("rail") => {
                                let on = crate::prefs::toggle_rail();
                                let narrow = chat.transcript_area.width
                                    + crate::ui::widgets::RAIL_WIDTH
                                    < crate::ui::widgets::RAIL_MIN_WIDTH;
                                if on && narrow {
                                    chat.set_toast(format!(
                                        "rail on · hidden below {} cols",
                                        crate::ui::widgets::RAIL_MIN_WIDTH
                                    ));
                                }
                            }
                            _ => {}
                        }
                    }
                    None
                }
                Some(SheetAction::Rewind {
                    text,
                    truncate_before_row_id,
                    confirm_empty_truncate,
                }) => {
                    chat.sheet = None;
                    Some(ScreenAction::Rewind {
                        text,
                        truncate_before_row_id,
                        confirm_empty_truncate,
                    })
                }
                Some(SheetAction::InterruptAgent { id }) => {
                    Some(ScreenAction::InterruptSubagent { subagent_id: id })
                }
                Some(SheetAction::SteerAgent { id, text }) => {
                    chat.sheet = None;
                    Some(ScreenAction::SteerSubagent {
                        subagent_id: id,
                        text,
                    })
                }
                Some(SheetAction::Confirm { id }) => {
                    let kind = match &chat.sheet {
                        Some(OpenSheet::List(s)) => Some(s.kind.clone()),
                        _ => None,
                    };
                    chat.sheet = None;
                    match kind {
                        Some(SheetKind::Sessions { tab, .. }) => match tab {
                            crate::ui::widgets::SheetTab::Saved => {
                                Some(ScreenAction::ResumeSaved { session_id: id })
                            }
                            crate::ui::widgets::SheetTab::Live => {
                                Some(ScreenAction::ActivateLive { session_id: id })
                            }
                        },
                        Some(SheetKind::Trees { .. }) => {
                            Some(ScreenAction::LoadSpawnTree { path: id })
                        }
                        _ => None,
                    }
                }
                None => None,
            },
            OpenSheet::Model(picker) => match super::model_picker::on_key(picker, key) {
                super::model_picker::ModelKey::None => None,
                super::model_picker::ModelKey::Close => {
                    chat.sheet = None;
                    None
                }
                super::model_picker::ModelKey::Action(action) => Some(action),
            },
            OpenSheet::Skills(hub) => match super::hubs::skills_key(hub, key) {
                (super::hubs::HubKey::Close, _) => {
                    chat.sheet = None;
                    None
                }
                (super::hubs::HubKey::None, action) => action,
            },
            OpenSheet::Plugins(hub) => match super::hubs::plugins_key(hub, key) {
                (super::hubs::HubKey::Close, _) => {
                    chat.sheet = None;
                    None
                }
                (super::hubs::HubKey::None, action) => action,
            },
            OpenSheet::Mcp(hub) => match super::hubs::mcp_key(hub, key) {
                (super::hubs::HubKey::Close, _) => {
                    chat.sheet = None;
                    None
                }
                (super::hubs::HubKey::None, action) => action,
            },
        };
    }

    if is_ctrl_o(&key) {
        chat.toggle_last_tool();
        return None;
    }

    if is_ctrl_v(&key) && !chat.slash.is_active() {
        if let Some(text) = crate::clipboard::paste_text() {
            if let Some(action) = chat.attach_images_from(&text) {
                return Some(action);
            }
            if !text.trim().is_empty() {
                chat.composer.insert_str(&text);
                return None;
            }
        }
        return Some(ScreenAction::ClipboardPaste);
    }

    if chat.slash.is_active() {
        return slash_key(chat, key);
    }

    if key.code == KeyCode::Esc {
        if chat.clear_selection() {
            return None;
        }
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

    if is_ctrl_enter(&key) {
        let Some(text) = chat.composer.submit() else {
            return None;
        };
        chat.composer.persist();
        return submit_or_queue(chat, text);
    }

    if chat.composer.draft.is_empty() {
        if let Some('/') = typed_char(&key) {
            chat.slash.open();
            return None;
        }
        if chat.is_returning_empty() {
            match key.code {
                KeyCode::Up => {
                    chat.move_recent(-1);
                    return None;
                }
                KeyCode::Down => {
                    chat.move_recent(1);
                    return None;
                }
                KeyCode::Enter => return chat.resume_recent(chat.recent_selected),
                _ => {
                    if let Some(c) = typed_char(&key) {
                        if let Some(i) = "123".find(c) {
                            return chat.resume_recent(i);
                        }
                    }
                }
            }
        }
    }

    match chat.composer.handle_key(key) {
        ComposerAction::Submit(text) => submit_composer(chat, text),
        ComposerAction::None => None,
    }
}

fn submit_or_queue(chat: &mut Chat, text: String) -> Option<ScreenAction> {
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
        chat.followup_queue.push_back(text);
        let n = chat.followup_queue.len();
        chat.set_toast(if n == 1 {
            "queued · will send after this turn".into()
        } else {
            format!("queued ({n})")
        });
        return None;
    }
    chat.push_user(text.clone());
    Some(ScreenAction::SubmitPrompt(text))
}

fn submit_composer(chat: &mut Chat, text: String) -> Option<ScreenAction> {
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
            chat.clear_selection();
            chat.followup_queue.clear();
            chat.notice = Some("transcript cleared (view only · /new for a session)".into());
            None
        }
        "new" | "reset" => Some(ScreenAction::NewSession),
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
                    "unknown skin {rest} (talaria, talaria-light, github, default, ares, …)"
                ));
            }
            None
        }
        "model" if rest.is_empty() => Some(ScreenAction::OpenModel { refresh: false }),
        "model" if rest == "--refresh" => Some(ScreenAction::OpenModel { refresh: true }),
        "model" => Some(ScreenAction::SetModel {
            value: model_set_value(rest),
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
        "copy" => {
            chat.copy_assistant(rest);
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

fn run_palette(chat: &mut Chat, run: PaletteRun) -> Option<ScreenAction> {
    match run {
        PaletteRun::Slash(cmd) => local_or_dispatch(chat, &cmd),
        PaletteRun::Host(id) => match id {
            "new" => Some(ScreenAction::NewSession),
            "sessions" => Some(ScreenAction::OpenSessions),
            "rewind" => Some(ScreenAction::OpenRewind),
            "model" => Some(ScreenAction::OpenModel { refresh: false }),
            "skills" => Some(ScreenAction::OpenSkills),
            "plugins" => Some(ScreenAction::OpenPlugins),
            "mcp" => Some(ScreenAction::OpenMcp),
            "agents" => Some(ScreenAction::OpenAgents),
            "trees" => Some(ScreenAction::OpenSpawnTrees),
            "usage" => Some(ScreenAction::OpenUsage),
            "theme" | "skin" => Some(ScreenAction::OpenTheme),
            "custom" => {
                chat.open_custom();
                None
            }
            "copy" => {
                chat.copy_assistant("");
                None
            }
            "help" => {
                chat.open_help();
                None
            }
            other => local_or_dispatch(chat, &format!("/{other}")),
        },
    }
}

fn model_set_value(rest: &str) -> String {
    let r = rest.trim();
    if r.split_whitespace()
        .any(|t| t == "--global" || t == "--session")
    {
        r.to_string()
    } else {
        format!("{r} --global")
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
        let picker = match &mut self.sheet {
            Some(super::OpenSheet::Model(p)) => Some(p),
            _ => match &mut self.overlay {
                super::Overlay::Model(p) => Some(p),
                _ => None,
            },
        };
        if let Some(picker) = picker {
            if picker.stage == super::model_picker::ModelStage::Key && !picker.key_saving {
                picker.key_input.push_str(text.trim());
                return None;
            }
        }
        if self.overlay.is_open() || self.slash.is_active() || self.confirm_quit {
            return None;
        }
        if let Some(action) = self.attach_images_from(&text) {
            return Some(action);
        }
        self.composer.insert_str(&text);
        None
    }

    /// Turn a drop / paste of image paths into `[Image #n]` placeholders in the
    /// draft, plus the attach action. `None` when the paste is ordinary text.
    pub(crate) fn attach_images_from(&mut self, text: &str) -> Option<ScreenAction> {
        let paths = crate::attachments::image_paths_from_paste(text);
        if paths.is_empty() {
            return None;
        }
        let mut items = Vec::with_capacity(paths.len());
        for path in paths {
            let a = self.attachments.push(path);
            // Keep the placeholder a word: pad only when it would abut typing.
            if !self.composer.draft.is_empty() && !self.composer.draft.ends_with(' ') {
                self.composer.insert_str(" ");
            }
            self.composer.insert_str(&a.label());
            self.composer.insert_str(" ");
            items.push(a);
        }
        let n = self.attachments.len();
        self.set_toast(if items.len() == 1 {
            format!("{} attached", items[0].label())
        } else {
            format!("{} images attached ({n} this turn)", items.len())
        });
        Some(ScreenAction::AttachImages { items })
    }
}

#[cfg(test)]
mod attach_tests {
    use super::super::Chat;
    use super::*;

    const DROPPED: &str = r"/tmp/CleanShot\ 2026-08-26\ at\ 10.34.26@2x.png";

    #[test]
    fn drop_inserts_placeholder_and_returns_attach_action() {
        let mut chat = Chat::new();
        chat.composer.insert_str("tell me about");
        let action = chat.attach_images_from(DROPPED).expect("attach action");
        assert_eq!(chat.composer.draft, "tell me about [Image #1] ");
        match action {
            ScreenAction::AttachImages { items } => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].label(), "[Image #1]");
                // The escaped path is unescaped before it reaches the gateway.
                assert_eq!(
                    items[0].path,
                    "/tmp/CleanShot 2026-08-26 at 10.34.26@2x.png"
                );
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn second_drop_is_image_two_and_submit_resets() {
        let mut chat = Chat::new();
        chat.attach_images_from("/tmp/a.png").expect("first");
        chat.attach_images_from("/tmp/b.png").expect("second");
        assert_eq!(chat.composer.draft, "[Image #1] [Image #2] ");
        assert_eq!(chat.attachments.len(), 2);
        assert_eq!(chat.attachments.path_for(2), Some("/tmp/b.png"));

        chat.push_user("[Image #1] [Image #2] compare these".into());
        assert!(chat.attachments.is_empty());
        chat.attach_images_from("/tmp/c.png").expect("after submit");
        assert_eq!(chat.attachments.path_for(1), Some("/tmp/c.png"));
    }

    #[test]
    fn ordinary_paste_is_untouched() {
        let mut chat = Chat::new();
        assert!(chat.attach_images_from("just some text").is_none());
        assert!(chat.attachments.is_empty());
        assert_eq!(chat.composer.draft, "");
    }
}
