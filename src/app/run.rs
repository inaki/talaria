//! Terminal bootstrap and the main event loop.
//!
//! One Tokio runtime. The ratatui loop stays synchronous. Gateway I/O runs
//! on `Handle::spawn`. The loop holds `rt.enter()` so `Handle::current()`
//! never panics.

use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEventKind, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::discover;
use crate::gateway::GatewayClient;
use crate::logging::log_line;
use crate::session::{
    GatewaySession, MockScenario, MockSession, SessionApi, SessionCommand, SessionKind,
};
use crate::ui::screens::CurrentScreen;
use crate::user_messages;

use super::{App, RunOptions};

pub fn run_tui() -> io::Result<()> {
    run_tui_with_options(RunOptions::default())
}

pub fn run_tui_with_options(options: RunOptions) -> io::Result<()> {
    crate::logging::init_file_logging();
    install_panic_hook();

    let theme_id = crate::theme::resolve_startup_theme(options.theme.as_deref());
    let _ = crate::theme::apply_theme(theme_id);
    crate::theme::sync_terminal_canvas_hard();
    log_line(&format!("theme: {theme_id}"));

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut app = App::new();
    app.tokio_handle = Some(rt.handle().clone());

    let _enter = rt.enter();
    attach_session(&mut app, &options, rt.handle());

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    // Disambiguate Esc so Shift+Enter is Enter+SHIFT. Do not enable
    // REPORT_ALL_KEYS_AS_ESCAPE_CODES — that reports physical `1`+SHIFT
    // instead of `!`.
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableBracketedPaste,
        EnableMouseCapture,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, &mut app);

    execute!(
        terminal.backend_mut(),
        PopKeyboardEnhancementFlags,
        DisableMouseCapture,
        DisableBracketedPaste,
        LeaveAlternateScreen
    )?;
    disable_raw_mode()?;

    if let Some(session) = &app.session {
        session.send(SessionCommand::Shutdown);
        // Give the session task a beat to reap the child.
        std::thread::sleep(Duration::from_millis(150));
    }

    result
}

fn attach_session(app: &mut App, options: &RunOptions, handle: &tokio::runtime::Handle) {
    if options.dev || options.mock.is_some() {
        let scenario = options
            .mock
            .as_deref()
            .map(MockScenario::parse)
            .unwrap_or(MockScenario::Streaming);
        log_line(&format!("session: mock {scenario:?}"));
        let mut mock = MockSession::start(scenario, handle);
        app.events = mock.take_events();
        mock.send(SessionCommand::Create {
            cwd: std::env::current_dir()
                .ok()
                .map(|p| p.display().to_string()),
            cols: 80,
        });
        app.session = Some(SessionKind::Mock(mock));
        return;
    }

    match discover::discover() {
        Ok(found) => {
            log_line(&format!(
                "discovered python={} src_root={:?}",
                found.python.display(),
                found.src_root
            ));
            let opts = found.spawn_args();
            match handle.block_on(GatewayClient::spawn(opts)) {
                Ok(client) => {
                    let mut live = GatewaySession::start(client, handle);
                    app.events = live.take_events();
                    live.send(SessionCommand::Create {
                        cwd: std::env::current_dir()
                            .ok()
                            .map(|p| p.display().to_string()),
                        cols: 80,
                    });
                    app.session = Some(SessionKind::Live(live));
                }
                Err(e) => {
                    let msg = user_messages::gateway_failed(&e);
                    log_line(&format!("spawn failed: {msg}"));
                    let CurrentScreen::Chat(chat) = &mut app.screen;
                    chat.apply_event(crate::session::SessionEvent::Error { message: msg });
                }
            }
        }
        Err(e) => {
            let msg = user_messages::discovery_failed(&e);
            log_line(&format!("discover failed: {msg}"));
            let CurrentScreen::Chat(chat) = &mut app.screen;
            chat.apply_event(crate::session::SessionEvent::Error { message: msg });
        }
    }
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    let tick_rate = Duration::from_millis(80);
    let mut last_tick = Instant::now();

    loop {
        if app.should_quit {
            break;
        }

        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(k) => {
                    if !matches!(k.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                        continue;
                    }
                    super::input::on_key(app, k);
                }
                Event::Paste(text) => {
                    let _ = app.screen.as_screen_mut().handle_paste(text);
                }
                Event::Mouse(m) => {
                    let area = terminal.size()?;
                    let _ = app.screen.as_screen_mut().handle_mouse(
                        m,
                        ratatui::layout::Rect {
                            x: 0,
                            y: 0,
                            width: area.width,
                            height: area.height,
                        },
                    );
                }
                Event::Resize(cols, rows) => {
                    app.last_cols = cols;
                    app.last_rows = rows;
                    if let Some(session) = &app.session {
                        session.send(SessionCommand::Resize { cols, rows });
                    }
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            super::tick::on_tick(app);
            last_tick = Instant::now();
        }

        if crate::theme::take_canvas_dirty() {
            terminal.clear()?;
        }
        terminal.draw(|f| super::render::draw(app, f))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        let _ = event::poll(timeout)?;
    }
    Ok(())
}

fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut stdout = io::stdout();
        let _ = execute!(
            stdout,
            DisableMouseCapture,
            DisableBracketedPaste,
            LeaveAlternateScreen
        );
        let _ = disable_raw_mode();
        original(info);
    }));
}
