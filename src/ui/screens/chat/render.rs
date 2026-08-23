use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::theme;
use crate::ui::screens::chat::TimelineItem;
use crate::ui::widgets::markdown_to_lines;

use super::Chat;

pub(super) fn draw(chat: &mut Chat, f: &mut Frame, area: Rect) {
    let hints_h = 1;
    // 1 content row + top/bottom border, then grows with Shift+Enter.
    let composer_h = chat.composer.visual_lines().saturating_add(2).max(3);
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
        chat.slash.render_above_input(f, chunks[2]);
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
            TimelineItem::User { text: t, .. } => {
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
            TimelineItem::Thinking { text, live } => {
                let label = if *live { "thinking ▍" } else { "thinking" };
                lines.push(Line::from(Span::styled(label, theme::dim())));
                let body: Vec<&str> = text.split('\n').collect();
                let start = body.len().saturating_sub(8);
                for l in &body[start..] {
                    lines.push(Line::from(Span::styled(format!("  {l}"), theme::dim())));
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
                expanded,
                ..
            } => {
                let state = if *done { "done" } else { "running" };
                if !*expanded {
                    let hint = if result.is_empty() {
                        preview.clone()
                    } else {
                        result.lines().next().unwrap_or("").to_string()
                    };
                    lines.push(Line::from(Span::styled(
                        format!("▸ tool  {name}  [{state}]  {hint}"),
                        theme::tool(),
                    )));
                    lines.push(Line::from(Span::styled("  Ctrl+O expand", theme::dim())));
                } else {
                    let mark = if *done { "└" } else { "│" };
                    lines.push(Line::from(Span::styled(
                        format!("┌ tool  {name}  [{state}]"),
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
                        for l in result.split('\n').take(12) {
                            lines.push(Line::from(Span::styled(format!("│ {l}"), theme::text())));
                        }
                    }
                    if let Some(err) = error {
                        lines.push(Line::from(Span::styled(format!("│ {err}"), theme::error())));
                    }
                    lines.push(Line::from(Span::styled(
                        format!("{mark}  Ctrl+O collapse"),
                        theme::tool(),
                    )));
                }
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
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::INPUT_BORDER()))
        .style(
            Style::default()
                .bg(theme::BACKGROUND())
                .fg(theme::INPUT_BORDER()),
        );
    let inner = block.inner(area);
    f.render_widget(block, area);
    let prefix = "› ";
    let indent = " ".repeat(prefix.chars().count());
    let accent = theme::accent().add_modifier(Modifier::BOLD);
    let body = theme::text();

    let lines = if chat.slash.is_active() {
        vec![Line::from(vec![
            Span::styled(prefix, accent),
            Span::styled(chat.slash.prompt_value(), theme::accent()),
        ])]
    } else if chat.composer.draft.is_empty() {
        vec![Line::from(vec![
            Span::styled(prefix, accent),
            Span::styled(" ", theme::cursor_block_style()),
            Span::styled("Ask Hermes…  (/ for commands)", theme::dim()),
        ])]
    } else {
        composer_lines(
            &chat.composer.draft,
            chat.composer.cursor_pos,
            prefix,
            &indent,
            accent,
            body,
        )
    };
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
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

fn composer_lines(
    draft: &str,
    cursor: usize,
    prefix: &str,
    indent: &str,
    accent: Style,
    body: Style,
) -> Vec<Line<'static>> {
    let text_lines: Vec<&str> = draft.split('\n').collect();
    let (cursor_line, cursor_col) = crate::ui::widgets::offset_to_line_col(draft, cursor);
    let mut lines = Vec::new();
    for (li, line) in text_lines.iter().enumerate() {
        let mut spans = Vec::new();
        if li == 0 {
            spans.push(Span::styled(prefix.to_string(), accent));
        } else {
            spans.push(Span::raw(indent.to_string()));
        }
        if li == cursor_line {
            let col = cursor_col.min(line.len());
            let before = line[..col].to_string();
            let at = line[col..].chars().next();
            let after_start = col + at.map(|c| c.len_utf8()).unwrap_or(0);
            let after = line[after_start.min(line.len())..].to_string();
            spans.push(Span::styled(before, body));
            let ch = at.map(|c| c.to_string()).unwrap_or_else(|| " ".into());
            spans.push(Span::styled(ch, theme::cursor_block_style()));
            spans.push(Span::styled(after, body));
        } else {
            spans.push(Span::styled((*line).to_string(), body));
        }
        lines.push(Line::from(spans));
    }
    lines
}
