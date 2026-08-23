use crate::session::{SessionApi, SessionCommand};
use crate::ui::screens::ScreenAction;

use super::App;

pub fn dispatch(app: &mut App, action: ScreenAction) {
    match action {
        ScreenAction::Quit => app.should_quit = true,
        ScreenAction::SubmitPrompt(text) => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::Submit { text });
            }
        }
        ScreenAction::Interrupt => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::Interrupt);
            }
        }
        ScreenAction::DispatchCommand { command } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::Dispatch { command });
            }
        }
        ScreenAction::ResumeSaved { session_id } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::Resume {
                    session_id,
                    cols: app.last_cols,
                });
            }
        }
        ScreenAction::ActivateLive { session_id } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::Activate { session_id });
            }
        }
        ScreenAction::RespondApproval { choice, request_id } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::RespondApproval { choice, request_id });
            }
        }
        ScreenAction::RespondClarify {
            request_id,
            answers,
        } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::RespondClarify {
                    request_id,
                    answers,
                });
            }
        }
        ScreenAction::RespondSudo {
            request_id,
            password,
        } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::RespondSudo {
                    request_id,
                    password,
                });
            }
        }
        ScreenAction::RespondSecret { request_id, value } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::RespondSecret { request_id, value });
            }
        }
        ScreenAction::OpenSessions => {
            let crate::ui::screens::CurrentScreen::Chat(chat) = &mut app.screen;
            chat.open_sessions();
            if let Some(session) = &app.session {
                session.send(SessionCommand::ListSaved { limit: 40 });
                session.send(SessionCommand::ListActive);
            }
        }
        ScreenAction::Steer { text } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::Steer { text });
            }
        }
        ScreenAction::Branch { title } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::Branch { title });
            }
        }
        ScreenAction::OpenAgents => {
            let crate::ui::screens::CurrentScreen::Chat(chat) = &mut app.screen;
            chat.open_agents();
            if let Some(session) = &app.session {
                session.send(SessionCommand::FetchDelegation);
            }
        }
        ScreenAction::InterruptSubagent { subagent_id } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::InterruptSubagent { subagent_id });
            }
        }
        ScreenAction::AttachImage { path } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::AttachImage { path });
            }
        }
        ScreenAction::ClipboardPaste => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::ClipboardPaste);
            }
        }
        ScreenAction::OpenSpawnTrees => {
            let crate::ui::screens::CurrentScreen::Chat(chat) = &mut app.screen;
            chat.open_spawn_trees();
            if let Some(session) = &app.session {
                session.send(SessionCommand::ListSpawnTrees);
            }
        }
        ScreenAction::LoadSpawnTree { path } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::LoadSpawnTree { path });
            }
        }
        ScreenAction::OpenRewind => {
            let crate::ui::screens::CurrentScreen::Chat(chat) = &mut app.screen;
            chat.open_rewind();
            if let Some(session) = &app.session {
                session.send(SessionCommand::FetchHistory);
            }
        }
        ScreenAction::OpenTheme => {
            let crate::ui::screens::CurrentScreen::Chat(chat) = &mut app.screen;
            chat.open_theme();
        }
        ScreenAction::Rewind {
            text,
            truncate_before_row_id,
            confirm_empty_truncate,
        } => {
            let crate::ui::screens::CurrentScreen::Chat(chat) = &mut app.screen;
            chat.apply_rewind_locally(truncate_before_row_id, text.clone());
            if let Some(session) = &app.session {
                session.send(SessionCommand::Interrupt);
                session.send(SessionCommand::Rewind {
                    text,
                    truncate_before_row_id,
                    confirm_empty_truncate,
                });
            }
        }
    }
}
