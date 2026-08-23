use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::theme;
use crate::ui::screens::chat::TimelineItem;
use crate::ui::widgets::markdown_to_lines;

use super::Chat;

pub(super) fn draw(chat: &mut Chat, f: &mut Frame, area: Rect) {
    let hints_h = 1;
    let composer_h = chat.composer.visual_lines().saturating_add(2);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(composer_h),
            Constraint::Length(hints_h),
        ])
        .split(area);

    draw_status(chat, f, chunks[0]);
    draw_transcript(chat, f, chunks[1]);
    draw_composer(chat, f, chunks[2]);
    if chat.slash.is_active() {
        chat.slash.render(f, chunks[2]);
    }
    f.render_widget(chat.key_hints_impl(), chunks[3]);

    super::overlay::draw_overlay(&chat.overlay, f, area);
    if chat.confirm_quit {
        draw_quit_modal(f, area);
    }
}

fn draw_status(chat: &Chat, f: &mut Frame, area: Rect) {
    let alive = if chat.gateway_alive {
        "alive"
    } else {
        "offline"
    };
    let spin = if chat.streaming || chat.thinking {
        format!("{} ", chat.spinner.glyph())
    } else {
        String::new()
    };
    let notice = chat.notice.as_deref().unwrap_or("");
    let line = Line::from(vec![
        Span::styled(spin, theme::accent()),
        Span::styled(format!(" {}", chat.model), theme::accent()),
        Span::styled(format!("  ·  {}", chat.session_id), theme::dim()),
        Span::styled(
            format!("  ·  {alive}"),
            if chat.gateway_alive {
                Style::default().fg(theme::SUCCESS())
            } else {
                theme::error()
            },
        ),
        Span::styled(format!("  {notice}"), theme::dim()),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn draw_transcript(chat: &mut Chat, f: &mut Frame, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    for item in &chat.items {
        match item {
            TimelineItem::User(t) => {
                lines.push(Line::from(Span::styled("you", theme::user())));
                for l in t.split('\n') {
                    lines.push(Line::from(Span::styled(format!("  {l}"), theme::user())));
                }
                lines.push(Line::from(""));
            }
            TimelineItem::Assistant { text, streaming } => {
                let label = if *streaming { "hermes ▍" } else { "hermes" };
                lines.push(Line::from(Span::styled(label, theme::accent())));
                if *streaming {
                    for l in text.split('\n') {
                        lines.push(Line::from(Span::styled(
                            format!("  {l}"),
                            theme::assistant(),
                        )));
                    }
                } else {
                    for mut line in markdown_to_lines(text) {
                        let mut spans = vec![Span::raw("  ")];
                        spans.extend(line.spans.drain(..));
                        line.spans = spans;
                        lines.push(line);
                    }
                }
                lines.push(Line::from(""));
            }
            TimelineItem::Tool {
                name,
                args,
                preview,
                result,
                error,
                done,
                ..
            } => {
                let mark = if *done { "└" } else { "│" };
                lines.push(Line::from(Span::styled(
                    format!("┌ tool  {name}"),
                    theme::tool(),
                )));
                if !args.is_empty() {
                    lines.push(Line::from(Span::styled(format!("│ {args}"), theme::dim())));
                }
                if !preview.is_empty() && result.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("│ {preview}"),
                        theme::dim(),
                    )));
                }
                if !result.is_empty() {
                    for l in result.split('\n').take(8) {
                        lines.push(Line::from(Span::styled(format!("│ {l}"), theme::text())));
                    }
                }
                if let Some(err) = error {
                    lines.push(Line::from(Span::styled(format!("│ {err}"), theme::error())));
                }
                lines.push(Line::from(Span::styled(format!("{mark}"), theme::tool())));
                lines.push(Line::from(""));
            }
            TimelineItem::Status(s) => {
                lines.push(Line::from(Span::styled(s.clone(), theme::dim())));
            }
            TimelineItem::Error(s) => {
                lines.push(Line::from(Span::styled(s.clone(), theme::error())));
                lines.push(Line::from(""));
            }
            TimelineItem::Subagent { label, .. } => {
                lines.push(Line::from(Span::styled(
                    format!("  ▸ {label}"),
                    theme::tool(),
                )));
            }
        }
    }

    let paragraph = Paragraph::new(lines.clone()).wrap(Wrap { trim: false });
    let total = paragraph.line_count(area.width) as u16;
    let max_scroll = total.saturating_sub(area.height);
    if chat.follow {
        chat.scroll = max_scroll;
    } else {
        chat.scroll = chat.scroll.min(max_scroll);
        if chat.scroll >= max_scroll {
            chat.follow = true;
        }
    }
    let paragraph = paragraph.scroll((chat.scroll, 0));
    f.render_widget(paragraph, area);
}

fn draw_composer(chat: &Chat, f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::accent())
        .title(" › ");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let text = if chat.slash.is_active() {
        Paragraph::new(chat.slash.prompt_value()).style(theme::accent())
    } else if chat.composer.draft.is_empty() {
        Paragraph::new(Span::styled("Ask Hermes…  (/ for commands)", theme::dim()))
    } else {
        Paragraph::new(cursor_line(&chat.composer.draft, chat.composer.cursor_pos))
            .style(theme::text())
            .wrap(Wrap { trim: false })
    };
    f.render_widget(text, inner);
}

fn draw_quit_modal(f: &mut Frame, area: Rect) {
    let w = 36u16.min(area.width.saturating_sub(2));
    let h = 5u16;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title("quit")
        .style(Style::default().bg(theme::SURFACE()).fg(theme::TEXT()));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    f.render_widget(
        Paragraph::new("Quit hermes-rust?\n y to confirm  ·  Esc to cancel"),
        inner,
    );
}

fn cursor_line(draft: &str, cursor: usize) -> String {
    let cur = if draft.is_char_boundary(cursor) {
        cursor.min(draft.len())
    } else {
        draft.len()
    };
    format!("{}▍{}", &draft[..cur], &draft[cur..])
}
