//! Live design-system gallery.
//!
//! Dev-only. Run with:
//!
//! ```sh
//! cargo run --example gallery
//! ```
//!
//! Left: every chrome piece. Right: the real widget on the active theme.
//! `t` cycles skins. Enter focuses interactive samples. `q` quits.
//!
//! Add a sample: variant on [`Sample`], list it in `Sample::ALL`, title +
//! render + optional keys.

use std::io::{self, stdout};
use std::time::{Duration, Instant};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::{Frame, Terminal};

use crate::session::{
    ActiveSession, RewindTurn, SavedSession, SpawnTreeEntry, SubagentRow, UsageSnapshot,
};
use crate::theme;
use crate::ui::screens::chat::status::{self, Meter};
use crate::ui::screens::chat::{Overlay, TimelineItem};
use crate::ui::screens::{Chat, Screen};
use crate::ui::widgets::{
    markdown_to_lines_at, Inspector, KeyHints, Palette, Sheet, SlashItem, SlashMenu, Spinner,
    TextComposer,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sample {
    Wordmark,
    Caduceus,
    Tokens,
    Spinner,
    KeyHints,
    Markdown,
    Composer,
    Meter,
    Document,
    EmptySplash,
    EmptyReturning,
    SlashMenu,
    Palette,
    SheetSessions,
    SheetTrees,
    SheetTheme,
    SheetCustom,
    SheetAgents,
    SheetRewind,
    InspectorTool,
    InspectorUsage,
    InspectorHelp,
    ModalApproval,
    ModalQuit,
}

impl Sample {
    const ALL: &'static [Sample] = &[
        Sample::Wordmark,
        Sample::Caduceus,
        Sample::Tokens,
        Sample::Spinner,
        Sample::KeyHints,
        Sample::Markdown,
        Sample::Composer,
        Sample::Meter,
        Sample::Document,
        Sample::EmptySplash,
        Sample::EmptyReturning,
        Sample::SlashMenu,
        Sample::Palette,
        Sample::SheetSessions,
        Sample::SheetTrees,
        Sample::SheetTheme,
        Sample::SheetCustom,
        Sample::SheetAgents,
        Sample::SheetRewind,
        Sample::InspectorTool,
        Sample::InspectorUsage,
        Sample::InspectorHelp,
        Sample::ModalApproval,
        Sample::ModalQuit,
    ];

    fn title(self) -> &'static str {
        match self {
            Sample::Wordmark => "Wordmark",
            Sample::Caduceus => "Caduceus (agent)",
            Sample::Tokens => "Theme tokens",
            Sample::Spinner => "Spinner",
            Sample::KeyHints => "Key hints",
            Sample::Markdown => "Markdown",
            Sample::Composer => "Composer",
            Sample::Meter => "Meter",
            Sample::Document => "Document",
            Sample::EmptySplash => "Empty splash",
            Sample::EmptyReturning => "Empty returning",
            Sample::SlashMenu => "Slash menu",
            Sample::Palette => "Palette",
            Sample::SheetSessions => "Sheet — sessions",
            Sample::SheetTrees => "Sheet — trees",
            Sample::SheetTheme => "Sheet — skin",
            Sample::SheetCustom => "Sheet — custom",
            Sample::SheetAgents => "Sheet — agents",
            Sample::SheetRewind => "Sheet — rewind",
            Sample::InspectorTool => "Inspector — tool",
            Sample::InspectorUsage => "Inspector — usage",
            Sample::InspectorHelp => "Inspector — help",
            Sample::ModalApproval => "Modal — approve",
            Sample::ModalQuit => "Modal — quit",
        }
    }

    fn interactive(self) -> bool {
        matches!(
            self,
            Sample::Composer
                | Sample::SlashMenu
                | Sample::Palette
                | Sample::SheetSessions
                | Sample::SheetTrees
                | Sample::SheetTheme
                | Sample::SheetCustom
                | Sample::SheetAgents
                | Sample::SheetRewind
                | Sample::InspectorTool
                | Sample::InspectorUsage
                | Sample::InspectorHelp
        )
    }
}

struct App {
    selected: usize,
    theme_idx: usize,
    focused: bool,
    spinner: Spinner,
    composer: TextComposer,
    slash: SlashMenu,
    palette: Palette,
    sheet: Sheet,
    trees: Sheet,
    inspector: Inspector,
    usage: Inspector,
}

impl App {
    fn new() -> Self {
        let mut slash = SlashMenu::default();
        slash.set_commands(catalog());
        slash.open();
        let mut palette = Palette::default();
        palette.set_catalog(catalog());
        palette.open();
        let mut sheet = Sheet::sessions();
        sheet.set_saved(vec![
            SavedSession {
                id: "20260825_auth".into(),
                title: "auth refactor".into(),
                preview: "jwt middleware".into(),
                source: "tui".into(),
                message_count: 12,
            },
            SavedSession {
                id: "20260824_notes".into(),
                title: "notes".into(),
                preview: "cargo test".into(),
                source: "cli".into(),
                message_count: 4,
            },
        ]);
        sheet.set_live(vec![ActiveSession {
            id: "live-1".into(),
            title: Some("current".into()),
            status: "idle".into(),
            current: true,
        }]);
        let mut trees = Sheet::trees();
        trees.set_trees(vec![
            SpawnTreeEntry {
                path: "trees/run-a.json".into(),
                label: "deploy preview".into(),
                count: 6,
            },
            SpawnTreeEntry {
                path: "trees/run-b.json".into(),
                label: "investigate flaky".into(),
                count: 3,
            },
        ]);
        let inspector = Inspector::tool(
            "terminal",
            "done",
            vec![
                "$ git status".into(),
                String::new(),
                "On branch docs/redesign".into(),
                "Changes not staged for commit:".into(),
                "  modified:   src/ui/gallery.rs".into(),
            ],
        );
        let mut usage = Inspector::usage_loading();
        usage.set_usage(&sample_usage());
        let mut composer = TextComposer::default();
        composer.insert_str("How does session.resume work?");
        Self {
            selected: 0,
            theme_idx: theme::themes()
                .iter()
                .position(|t| t.id == theme::current_theme_id())
                .unwrap_or(0),
            focused: false,
            spinner: Spinner::default(),
            composer,
            slash,
            palette,
            sheet,
            trees,
            inspector,
            usage,
        }
    }

    fn sample(&self) -> Sample {
        Sample::ALL[self.selected]
    }

    fn cycle_theme(&mut self) {
        let themes = theme::themes();
        self.theme_idx = (self.theme_idx + 1) % themes.len();
        theme::apply_theme(themes[self.theme_idx].id);
    }

    fn on_key(&mut self, key: KeyEvent) -> bool {
        if self.focused {
            if matches!(key.code, KeyCode::Esc) {
                self.focused = false;
                return false;
            }
            self.on_focused_key(key);
            return false;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => true,
            KeyCode::Char('t') => {
                self.cycle_theme();
                false
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                } else {
                    self.selected = Sample::ALL.len() - 1;
                }
                false
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1) % Sample::ALL.len();
                false
            }
            KeyCode::Enter if self.sample().interactive() => {
                self.focused = true;
                false
            }
            _ => false,
        }
    }

    fn on_focused_key(&mut self, key: KeyEvent) {
        match self.sample() {
            Sample::Composer => {
                if key.code == KeyCode::Enter {
                    self.composer.insert_newline();
                } else {
                    let _ = self.composer.handle_key(key);
                }
            }
            Sample::SlashMenu => match key.code {
                KeyCode::Up => self.slash.move_up(),
                KeyCode::Down => self.slash.move_down(),
                KeyCode::Backspace => {
                    let _ = self.slash.backspace();
                    if !self.slash.is_active() {
                        self.slash.open();
                    }
                }
                KeyCode::Char(c) if c.is_ascii() && !c.is_control() => self.slash.append(c),
                _ => {}
            },
            Sample::Palette => {
                let _ = self.palette.handle_key(key);
                if !self.palette.is_active() {
                    self.palette.open();
                }
            }
            Sample::SheetSessions => {
                let _ = self.sheet.handle_key(key);
            }
            Sample::SheetTrees => {
                let _ = self.trees.handle_key(key);
            }
            Sample::SheetTheme | Sample::SheetCustom => {}
            Sample::SheetAgents | Sample::SheetRewind => {}
            Sample::InspectorTool => {
                let _ = self.inspector.handle_key(key);
            }
            Sample::InspectorUsage => {
                let _ = self.usage.handle_key(key);
            }
            Sample::InspectorHelp => {}
            _ => {}
        }
    }
}

/// Run the gallery until quit. Restores the terminal on exit or panic.
pub fn run() -> io::Result<()> {
    let _ = theme::apply_theme(theme::current_theme_id());
    enable_raw_mode()?;
    struct Cleanup;
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = disable_raw_mode();
            let _ = execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen);
        }
    }
    let _cleanup = Cleanup;
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new();
    let mut last_spin = Instant::now();

    loop {
        if last_spin.elapsed() >= Duration::from_millis(app.spinner.interval_ms()) {
            app.spinner.tick();
            last_spin = Instant::now();
        }
        if theme::take_canvas_dirty() {
            terminal.clear()?;
        }
        terminal.draw(|f| draw(f, &mut app))?;
        if event::poll(Duration::from_millis(80))? {
            match event::read()? {
                Event::Key(k) if matches!(k.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                    if app.on_key(k) {
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    f.render_widget(Clear, area);
    f.render_widget(
        Block::default().style(Style::default().bg(theme::BACKGROUND()).fg(theme::TEXT())),
        area,
    );
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(area);
    draw_header(f, rows[0]);
    draw_body(f, rows[1], app);
    draw_footer(f, rows[2], app);
}

fn draw_header(f: &mut Frame, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" TALARIA  ", theme::agent().add_modifier(Modifier::BOLD)),
            Span::styled("gallery", theme::text()),
            Span::styled("  · design system", theme::dim()),
        ])),
        cols[0],
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("theme  ", theme::dim()),
            Span::styled(theme::current_theme_label(), theme::accent()),
        ]))
        .alignment(Alignment::Right),
        cols[1],
    );
}

fn draw_body(f: &mut Frame, area: Rect, app: &mut App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(20)])
        .split(area);
    draw_list(f, cols[0], app);
    let sample = app.sample();
    let (border, title_style) = if app.focused {
        (theme::accent(), theme::accent())
    } else {
        (Style::default().fg(theme::SEPARATOR()), theme::dim())
    };
    let title = if app.focused {
        format!(" {}  · live ", sample.title())
    } else {
        format!(" {} ", sample.title())
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border)
        .title(Span::styled(
            title,
            title_style.add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(theme::BACKGROUND()));
    let inner = block.inner(cols[1]);
    f.render_widget(block, cols[1]);
    render_sample(f, inner, sample, app);
}

fn draw_list(f: &mut Frame, area: Rect, app: &App) {
    let mut lines = Vec::new();
    for (i, sample) in Sample::ALL.iter().enumerate() {
        let sel = i == app.selected;
        let mark = if sel { "▸ " } else { "  " };
        let style = if sel {
            theme::accent().add_modifier(Modifier::BOLD)
        } else {
            theme::dim()
        };
        lines.push(Line::from(Span::styled(
            format!("{mark}{}", sample.title()),
            style,
        )));
    }
    f.render_widget(Paragraph::new(lines), area);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<(&str, &str)> = if app.focused {
        let mut v = match app.sample() {
            Sample::Composer => vec![("type", "edit"), ("Shift+Enter", "newline")],
            Sample::SlashMenu | Sample::Palette => vec![("type", "filter"), ("↑↓", "select")],
            Sample::SheetSessions
            | Sample::SheetTrees
            | Sample::SheetTheme
            | Sample::SheetCustom
            | Sample::SheetAgents
            | Sample::SheetRewind => {
                vec![("type", "filter"), ("↑↓", "select")]
            }
            Sample::InspectorTool | Sample::InspectorUsage | Sample::InspectorHelp => {
                vec![("↑↓", "scroll"), ("c", "copy")]
            }
            _ => vec![],
        };
        v.push(("Esc", "back"));
        v
    } else {
        let mut v = vec![("↑↓", "move")];
        if app.sample().interactive() {
            v.push(("Enter", "interact"));
        }
        v.push(("t", "theme"));
        v.push(("q", "quit"));
        v
    };
    f.render_widget(KeyHints::new(items), area);
}

fn render_sample(f: &mut Frame, area: Rect, sample: Sample, app: &mut App) {
    match sample {
        Sample::Wordmark => {
            let lines = crate::ui::widgets::logo_lines();
            f.render_widget(Paragraph::new(lines), area);
        }
        Sample::Caduceus => {
            let mut lines = crate::ui::widgets::caduceus_lines();
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Hermes Agent mark — not Talaria’s logo",
                theme::dim(),
            )));
            f.render_widget(Paragraph::new(lines), area);
        }
        Sample::Tokens => render_tokens(f, area),
        Sample::Spinner => {
            let line = Line::from(vec![
                app.spinner.span(theme::accent()),
                Span::raw("  "),
                Span::styled(app.spinner.name(), theme::dim()),
                Span::styled("  live turn", theme::text()),
            ]);
            f.render_widget(Paragraph::new(line), inset(area, 2, 1));
        }
        Sample::KeyHints => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(2),
                    Constraint::Length(2),
                    Constraint::Length(2),
                ])
                .split(inset(area, 1, 1));
            f.render_widget(KeyHints::idle(true, true), chunks[0]);
            f.render_widget(KeyHints::streaming(), chunks[1]);
            f.render_widget(KeyHints::overlay(), chunks[2]);
        }
        Sample::Markdown => {
            let src = "# Heading\n\nA paragraph with **bold** and `code`.\n\n```rust\npub async fn verify_token(token: &str) -> Result<Claims, AuthError> {\n    let decoded = jsonwebtoken::decode::<Claims>(\n        token,\n        &KEYS.decoding,\n        &Validation::default(),\n    )?;\n\n    // Validate expiration claim explicitly\n}\n```\n";
            let w = area.width.saturating_sub(4) as usize;
            f.render_widget(
                Paragraph::new(markdown_to_lines_at(src, w)),
                inset(area, 1, 1),
            );
        }
        Sample::Composer => render_composer(f, area, app),
        Sample::Meter => {
            let meter = Meter {
                model: "stealth/ox-alpha".into(),
                git_branch: "docs/redesign".into(),
                usage: sample_usage(),
                session: Duration::from_secs(9 * 3600 + 15 * 60),
                turn: Some(Duration::from_secs(12)),
                turn_live: true,
                idle: None,
                right: "session.resume".into(),
                spinner: Some(app.spinner.glyph()),
                model_global: true,
            };
            f.render_widget(
                Paragraph::new(status::line(&meter, area.width)),
                inset(area, 1, 1),
            );
        }
        Sample::Document => {
            let mut chat = document_chat();
            chat.render(f, area);
        }
        Sample::EmptySplash => {
            let mut chat = Chat::new();
            chat.gateway_alive = true;
            chat.model = "ox-alpha".into();
            chat.notice = None;
            chat.items.clear();
            chat.render(f, area);
        }
        Sample::EmptyReturning => {
            let mut chat = Chat::new();
            chat.gateway_alive = true;
            chat.model = "ox-alpha".into();
            chat.notice = None;
            chat.items.clear();
            chat.recent_sessions = vec![
                SavedSession {
                    id: "20260825_auth".into(),
                    title: "auth refactor".into(),
                    preview: "jwt middleware".into(),
                    source: "tui".into(),
                    message_count: 12,
                },
                SavedSession {
                    id: "20260824_notes".into(),
                    title: "notes".into(),
                    preview: "cargo test".into(),
                    source: "cli".into(),
                    message_count: 4,
                },
                SavedSession {
                    id: "20260823_plan".into(),
                    title: "planning".into(),
                    preview: "returning workspace".into(),
                    source: "tui".into(),
                    message_count: 7,
                },
            ];
            chat.render(f, area);
        }
        Sample::SlashMenu => render_slash(f, area, app),
        Sample::Palette => {
            app.palette.render(f, area);
        }
        Sample::SheetSessions => {
            let mut chat = document_chat();
            chat.sheet = Some(crate::ui::screens::chat::OpenSheet::List(app.sheet.clone()));
            chat.render(f, area);
        }
        Sample::SheetTrees => {
            let mut chat = document_chat();
            chat.sheet = Some(crate::ui::screens::chat::OpenSheet::List(app.trees.clone()));
            chat.render(f, area);
        }
        Sample::SheetTheme => {
            let mut chat = document_chat();
            chat.sheet = Some(crate::ui::screens::chat::OpenSheet::List(Sheet::theme()));
            chat.render(f, area);
        }
        Sample::SheetCustom => {
            let mut chat = document_chat();
            chat.sheet = Some(crate::ui::screens::chat::OpenSheet::List(Sheet::custom()));
            chat.render(f, area);
        }
        Sample::SheetAgents => {
            let mut chat = document_chat();
            let mut sheet = Sheet::agents(vec![
                SubagentRow {
                    id: "a1".into(),
                    goal: "scan failing tests".into(),
                    status: "running".into(),
                },
                SubagentRow {
                    id: "a2".into(),
                    goal: "draft changelog".into(),
                    status: "idle".into(),
                },
            ]);
            sheet.loading = false;
            chat.sheet = Some(crate::ui::screens::chat::OpenSheet::List(sheet));
            chat.render(f, area);
        }
        Sample::SheetRewind => {
            let mut chat = document_chat();
            let mut sheet = Sheet::rewind(vec![
                RewindTurn {
                    row_id: 1,
                    text: "How does session.resume work?".into(),
                    first: true,
                },
                RewindTurn {
                    row_id: 7,
                    text: "Show me the queue path.".into(),
                    first: false,
                },
            ]);
            sheet.loading = false;
            chat.sheet = Some(crate::ui::screens::chat::OpenSheet::List(sheet));
            chat.render(f, area);
        }
        Sample::InspectorTool => {
            let mut chat = document_chat();
            chat.inspector = Some(app.inspector.clone());
            chat.render(f, area);
        }
        Sample::InspectorUsage => {
            let mut chat = document_chat();
            chat.inspector = Some(app.usage.clone());
            chat.render(f, area);
        }
        Sample::InspectorHelp => {
            let mut chat = document_chat();
            chat.inspector = Some(Inspector::help(vec![
                "Native TUI host for Hermes Agent".into(),
                String::new(),
                "Ctrl+K           command palette".into(),
                "Enter            send (steer if a turn is running)".into(),
                "Ctrl+Enter       queue follow-up".into(),
            ]));
            chat.render(f, area);
        }
        Sample::ModalApproval => {
            let mut chat = document_chat();
            chat.overlay = Overlay::Approval {
                command: "rm -rf dist".into(),
                description: "Delete the build output.".into(),
                choices: vec!["once".into(), "always".into(), "deny".into()],
                selected: 0,
                request_id: Some("r1".into()),
                allow_permanent: true,
            };
            chat.render(f, area);
        }
        Sample::ModalQuit => {
            let mut chat = document_chat();
            chat.confirm_quit = true;
            chat.render(f, area);
        }
    }
}

fn render_tokens(f: &mut Frame, area: Rect) {
    let swatches: &[(&str, ratatui::style::Color)] = &[
        ("canvas", theme::BACKGROUND()),
        ("surface", theme::SURFACE()),
        ("accent", theme::PRIMARY()),
        ("text", theme::TEXT()),
        ("dim", theme::TEXT_DIM()),
        ("user", theme::USER()),
        ("tool", theme::TOOL()),
        ("agent", theme::AGENT()),
        ("ok", theme::SUCCESS()),
        ("warn", theme::WARNING()),
        ("danger", theme::ERROR()),
        ("input", theme::INPUT_BORDER()),
    ];
    let mut lines = vec![Line::from(Span::styled(
        format!("skin  {}", theme::current_theme_label()),
        theme::dim(),
    ))];
    for (name, color) in swatches {
        lines.push(Line::from(vec![
            Span::styled("  ██  ", Style::default().fg(*color)),
            Span::styled(format!("{name:<8}"), theme::text()),
        ]));
    }
    f.render_widget(Paragraph::new(lines), inset(area, 1, 1));
}

fn render_composer(f: &mut Frame, area: Rect, app: &App) {
    let mut chat = Chat::new();
    chat.items.clear();
    chat.notice = None;
    chat.composer = TextComposer::default();
    chat.composer.insert_str(&app.composer.draft);
    chat.composer.cursor_pos = app.composer.cursor_pos;
    chat.model = "ox-alpha".into();
    chat.usage = sample_usage();
    chat.gateway_alive = true;
    chat.render(f, area);
}

fn render_slash(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::INPUT_BORDER()))
        .title(" composer ");
    let inner = block.inner(chunks[1]);
    f.render_widget(block, chunks[1]);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("› ", theme::accent()),
            Span::styled(app.slash.prompt_value(), theme::accent()),
        ])),
        inner,
    );
    app.slash.render_above_input(f, chunks[1]);
}

fn document_chat() -> Chat {
    let mut chat = Chat::new();
    chat.gateway_alive = true;
    chat.model = "ox-alpha".into();
    chat.session_id = "s1".into();
    chat.notice = None;
    chat.usage = sample_usage();
    chat.items = vec![
        TimelineItem::User {
            text: "How does session.resume work?".into(),
            row_id: Some(1),
        },
        TimelineItem::Assistant {
            text: "session.resume loads a saved transcript by id.\n\n```rust\npub async fn verify_token(token: &str) -> Result<Claims, AuthError> {\n    let decoded = jsonwebtoken::decode::<Claims>(token, &KEYS.decoding, &Validation::default())?;\n\n    // Validate expiration claim explicitly\n}\n```".into(),
            streaming: false,
        },
        TimelineItem::Thinking {
            text: "checking row ids…".into(),
            live: false,
        },
        TimelineItem::Tool {
            tool_id: "t1".into(),
            name: "session.list".into(),
            args: "{\"limit\":80}".into(),
            preview: String::new(),
            result: "3 saved".into(),
            error: None,
            done: true,
            expanded: false,
        },
        TimelineItem::Shell {
            command: "git status".into(),
            output: "On branch main".into(),
            code: Some(0),
            running: false,
        },
        TimelineItem::Status("queued · will send after this turn".into()),
    ];
    chat
}

fn sample_usage() -> UsageSnapshot {
    UsageSnapshot {
        calls: 4,
        input: 12_400,
        output: 1_100,
        total: 13_500,
        context_used: 25_700,
        context_max: 1_000_000,
        context_percent: 2,
        cost_usd: Some(0.04),
        model: "ox-alpha".into(),
        credits_lines: vec!["$9.12 remaining".into()],
    }
}

fn catalog() -> Vec<SlashItem> {
    vec![
        SlashItem {
            name: "compress".into(),
            help: "summarize context".into(),
        },
        SlashItem {
            name: "yolo".into(),
            help: "skip approvals".into(),
        },
        SlashItem {
            name: "title".into(),
            help: "name this session".into(),
        },
        SlashItem {
            name: "rewind".into(),
            help: "regenerate from a turn".into(),
        },
    ]
}

fn inset(area: Rect, dx: u16, dy: u16) -> Rect {
    Rect {
        x: area.x + dx,
        y: area.y + dy,
        width: area.width.saturating_sub(dx.saturating_mul(2)),
        height: area.height.saturating_sub(dy.saturating_mul(2)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_have_unique_titles() {
        let mut seen = std::collections::BTreeSet::new();
        for s in Sample::ALL {
            assert!(seen.insert(s.title()), "duplicate {}", s.title());
        }
        assert_eq!(seen.len(), Sample::ALL.len());
    }
}
