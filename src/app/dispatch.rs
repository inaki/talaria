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
        ScreenAction::ShellExec { command } => {
            let crate::ui::screens::CurrentScreen::Chat(chat) = &app.screen;
            let cwd = chat.cwd.clone();
            if let Some(session) = &app.session {
                session.send(SessionCommand::ShellExec { command, cwd });
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
        ScreenAction::SteerSubagent { subagent_id, text } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::SteerSubagent { subagent_id, text });
            }
        }
        ScreenAction::OpenUsage => {
            let crate::ui::screens::CurrentScreen::Chat(chat) = &mut app.screen;
            chat.open_usage();
            if let Some(session) = &app.session {
                session.send(SessionCommand::FetchUsage);
            }
        }
        ScreenAction::NewSession => {
            let crate::ui::screens::CurrentScreen::Chat(chat) = &mut app.screen;
            let cwd = chat.cwd.clone();
            chat.begin_new_session();
            if let Some(session) = &app.session {
                session.send(SessionCommand::Create {
                    cwd: Some(cwd),
                    cols: app.last_cols,
                });
            }
        }
        ScreenAction::CloseLive { session_id } => {
            let crate::ui::screens::CurrentScreen::Chat(chat) = &mut app.screen;
            if chat.session_id == session_id {
                chat.begin_new_session();
            }
            if let Some(session) = &app.session {
                session.send(SessionCommand::CloseSession {
                    session_id,
                    cols: app.last_cols,
                });
                session.send(SessionCommand::ListActive);
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
        ScreenAction::OpenModel { refresh } => {
            let crate::ui::screens::CurrentScreen::Chat(chat) = &mut app.screen;
            chat.open_model();
            if let Some(session) = &app.session {
                session.send(SessionCommand::FetchModelOptions { refresh });
            }
        }
        ScreenAction::SaveModelKey { slug, api_key } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::SaveModelKey { slug, api_key });
            }
        }
        ScreenAction::DisconnectModel { slug } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::DisconnectModel { slug });
            }
        }
        ScreenAction::SetModel {
            value,
            confirm_expensive_model,
        } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::SetConfig {
                    key: "model".into(),
                    value,
                    confirm_expensive_model,
                });
            }
        }
        ScreenAction::OpenSkills => {
            let crate::ui::screens::CurrentScreen::Chat(chat) = &mut app.screen;
            chat.open_skills();
            if let Some(session) = &app.session {
                session.send(SessionCommand::FetchSkills);
            }
        }
        ScreenAction::InstallSkill { query } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::InstallSkill { query });
            }
        }
        ScreenAction::OpenPlugins => {
            let crate::ui::screens::CurrentScreen::Chat(chat) = &mut app.screen;
            chat.open_plugins();
            if let Some(session) = &app.session {
                session.send(SessionCommand::FetchPlugins);
            }
        }
        ScreenAction::TogglePlugin { key, enable } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::TogglePlugin { key, enable });
            }
        }
        ScreenAction::OpenMcp => {
            let crate::ui::screens::CurrentScreen::Chat(chat) = &mut app.screen;
            chat.open_mcp();
            if let Some(session) = &app.session {
                session.send(SessionCommand::FetchMcpServers);
                session.send(SessionCommand::FetchMcpCatalog);
            }
        }
        ScreenAction::AddMcp { name, preset } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::AddMcp { name, preset });
            }
        }
        ScreenAction::RemoveMcp { name } => {
            if let Some(session) = &app.session {
                session.send(SessionCommand::RemoveMcp { name });
            }
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
