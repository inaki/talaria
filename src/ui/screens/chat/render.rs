use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;
use unicode_width::UnicodeWidthChar;

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
    if chat.items.is_empty() && !chat.streaming {
        draw_splash(chat, f, area);
        return;
    }
    chat.transcript_area = area;
    chat.tool_hits.clear();
    let mut lines: Vec<Line> = Vec::new();
    let col_w = area.width.max(12);
    for (i, item) in chat.items.iter().enumerate() {
        let hit_start = lines.len() as u16;
        match item {
            TimelineItem::User { text: t, .. } => {
                let mut first = true;
                for l in t.split('\n') {
                    if first {
                        lines.push(Line::from(vec![
                            Span::styled("● ", theme::user().add_modifier(Modifier::BOLD)),
                            Span::styled(l.to_string(), theme::user()),
                        ]));
                        first = false;
                    } else {
                        lines.push(Line::from(Span::styled(format!("  {l}"), theme::user())));
                    }
                }
                lines.push(Line::from(""));
            }
            TimelineItem::Assistant { text, streaming } => {
                let mut body: Vec<Line> = Vec::new();
                if *streaming {
                    for l in text.split('\n') {
                        body.push(Line::from(Span::styled(l.to_string(), theme::assistant())));
                    }
                } else {
                    body = markdown_to_lines(text);
                }
                lines.extend(assistant_card(body, *streaming, col_w));
                lines.push(Line::from(""));
            }
            TimelineItem::Thinking { text, live } => {
                let raw: Vec<&str> = text.split('\n').collect();
                let start = raw.len().saturating_sub(8);
                let body = raw[start..]
                    .iter()
                    .map(|l| Line::from(Span::styled((*l).to_string(), theme::dim())))
                    .collect();
                lines.extend(rounded_panel(
                    vec![Span::styled(" thinking", theme::dim())],
                    body,
                    panel_width(col_w),
                    *live,
                    true,
                ));
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
                        preview.as_str()
                    } else {
                        result.lines().next().unwrap_or("")
                    };
                    let selected = chat.selected_tool == Some(i);
                    lines.push(tool_chip(name, state, hint, panel_width(col_w)));
                    lines.push(Line::from(Span::styled(
                        if selected {
                            "  click / Ctrl+O collapse-or-expand  (selected)"
                        } else {
                            "  click or Ctrl+O expand"
                        },
                        if selected {
                            theme::accent()
                        } else {
                            theme::dim()
                        },
                    )));
                } else {
                    let mut body: Vec<Line> = Vec::new();
                    if !args.is_empty() {
                        body.push(Line::from(Span::styled(args.clone(), theme::dim())));
                    }
                    if !preview.is_empty() && result.is_empty() {
                        body.push(Line::from(Span::styled(preview.clone(), theme::dim())));
                    }
                    if !result.is_empty() {
                        for l in result.split('\n').take(12) {
                            body.push(Line::from(Span::styled(l.to_string(), theme::text())));
                        }
                    }
                    if let Some(err) = error {
                        body.push(Line::from(Span::styled(err.clone(), theme::error())));
                    }
                    body.push(Line::from(Span::styled(
                        if chat.selected_tool == Some(i) {
                            "click / Ctrl+O collapse  (selected)"
                        } else {
                            "click or Ctrl+O collapse"
                        },
                        theme::dim(),
                    )));
                    lines.extend(rounded_panel(
                        vec![Span::styled(format!(" ▸ {name}  [{state}]"), theme::tool())],
                        body,
                        panel_width(col_w),
                        !*done,
                        true,
                    ));
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
            TimelineItem::Shell {
                command,
                output,
                code,
                running,
            } => {
                let title = if *running {
                    format!(" $ {command}  [running]")
                } else {
                    format!(
                        " $ {command}  [{}]",
                        code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())
                    )
                };
                let body: Vec<Line> = if output.is_empty() {
                    Vec::new()
                } else {
                    output
                        .split('\n')
                        .take(40)
                        .map(|l| Line::from(Span::styled(l.to_string(), theme::text())))
                        .collect()
                };
                lines.extend(rounded_panel(
                    vec![Span::styled(title, theme::tool())],
                    body,
                    panel_width(col_w),
                    *running,
                    true,
                ));
                lines.push(Line::from(""));
            }
        }
        if matches!(item, TimelineItem::Tool { .. }) {
            chat.tool_hits
                .push((hit_start, (lines.len() as u16).saturating_sub(hit_start), i));
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

fn draw_splash(chat: &Chat, f: &mut Frame, area: Rect) {
    let logo = crate::ui::widgets::logo_lines();
    let logo_w = logo.iter().map(|l| l.width() as u16).max().unwrap_or(0);
    let tag = crate::ui::widgets::tagline();
    let mut logo_block: Vec<Line> = logo;
    logo_block.push(Line::from(Span::styled(format!("✦ {tag}"), theme::dim())));
    logo_block.push(Line::from(""));

    let logo_h = logo_block.len() as u16;
    let (banner_area, panel_area) = if area.width + 2 >= logo_w && area.height > logo_h + 10 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(logo_h), Constraint::Min(8)])
            .split(area);
        f.render_widget(
            Paragraph::new(logo_block).wrap(Wrap { trim: false }),
            chunks[0],
        );
        (chunks[0], chunks[1])
    } else {
        (Rect::default(), area)
    };
    let _ = banner_area;

    let caduceus = crate::ui::widgets::caduceus_lines();
    let cad_w = crate::ui::widgets::caduceus_width().saturating_add(4);
    let wide = panel_area.width >= 90 && cad_w + 40 < panel_area.width;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::INPUT_BORDER()))
        .style(Style::default().bg(theme::BACKGROUND()));
    let inner = block.inner(panel_area);
    f.render_widget(block, panel_area);

    if wide {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(cad_w), Constraint::Min(40)])
            .split(inner);
        draw_hero_column(chat, f, cols[0], &caduceus);
        draw_info_column(chat, f, cols[1], true);
    } else {
        draw_info_column(chat, f, inner, false);
    }
}

fn draw_hero_column(chat: &Chat, f: &mut Frame, area: Rect, caduceus: &[Line<'static>]) {
    let mut lines = caduceus.to_vec();
    lines.push(Line::from(""));
    let model = chat.model.rsplit('/').next().unwrap_or(&chat.model);
    lines.push(Line::from(vec![
        Span::styled(model.to_string(), theme::tool()),
        Span::styled(" · Nous Research", theme::dim()),
    ]));
    lines.push(Line::from(Span::styled(chat.cwd.clone(), theme::dim())));
    let sid = if chat.stored_session_id.is_empty() {
        chat.session_id.clone()
    } else {
        chat.stored_session_id.clone()
    };
    lines.push(Line::from(vec![
        Span::styled("Session: ", theme::user()),
        Span::styled(sid, theme::dim()),
    ]));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn draw_info_column(chat: &Chat, f: &mut Frame, area: Rect, wide: bool) {
    let mut lines: Vec<Line> = Vec::new();
    if wide {
        let mut title = String::from("Hermes Agent");
        if !chat.version.is_empty() {
            title.push_str(&format!(" v{}", chat.version));
        }
        if !chat.release_date.is_empty() {
            title.push_str(&format!(" ({})", chat.release_date));
        }
        lines.push(Line::from(Span::styled(
            title,
            theme::accent().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
    } else {
        let model = chat.model.rsplit('/').next().unwrap_or(&chat.model);
        lines.push(Line::from(vec![
            Span::styled(model.to_string(), theme::tool()),
            Span::styled(" · Nous Research", theme::dim()),
        ]));
        lines.push(Line::from(Span::styled(chat.cwd.clone(), theme::dim())));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        "Available Tools",
        theme::tool().add_modifier(Modifier::BOLD),
    )));
    if chat.tools.is_empty() {
        lines.push(Line::from(Span::styled(
            if chat.gateway_alive {
                "  scanning tools…"
            } else {
                "  (loading)"
            },
            theme::dim(),
        )));
    } else {
        append_grouped(&mut lines, &chat.tools, 8);
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Available Skills",
        theme::tool().add_modifier(Modifier::BOLD),
    )));
    if chat.skills.is_empty() {
        lines.push(Line::from(Span::styled("  (none yet)", theme::dim())));
    } else {
        append_grouped(&mut lines, &chat.skills, 8);
    }
    lines.push(Line::from(""));
    let n_tools: usize = chat.tools.iter().map(|(_, v)| v.len()).sum();
    let n_skills: usize = chat.skills.iter().map(|(_, v)| v.len()).sum();
    lines.push(Line::from(vec![
        Span::styled(format!("{n_tools} tools"), theme::text()),
        Span::styled(" · ", theme::dim()),
        Span::styled(format!("{n_skills} skills"), theme::text()),
        Span::styled(" · /help for commands", theme::dim()),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Welcome to Hermes Agent! Type your message or /help for commands.",
        theme::dim(),
    )));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn append_grouped(lines: &mut Vec<Line>, groups: &[(String, Vec<String>)], max: usize) {
    for (name, members) in groups.iter().take(max) {
        let body = if members.len() > 4 {
            format!(
                "{}, +{} more",
                members
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
                members.len() - 3
            )
        } else {
            members.join(", ")
        };
        lines.push(Line::from(vec![
            Span::styled(format!("  {name}: "), theme::user()),
            Span::styled(body, theme::text()),
        ]));
    }
    if groups.len() > max {
        lines.push(Line::from(Span::styled(
            format!("  (and {} more…)", groups.len() - max),
            theme::dim(),
        )));
    }
}

fn border_style() -> Style {
    Style::default().fg(theme::INPUT_BORDER())
}

fn b(s: &str) -> Span<'static> {
    Span::styled(s.to_string(), border_style())
}

fn paint_card(line: Line<'static>, fill: bool) -> Line<'static> {
    if fill {
        line.style(Style::default().bg(theme::SURFACE()))
    } else {
        line
    }
}

fn panel_width(col_w: u16) -> usize {
    (col_w as usize).max(12)
}

/// Official brand mark (staff of Asclepius), same glyph as Hermes CLI `response_label`.
const RESPONSE_MARK: &str = "⚕";

/// Rounded response panel: `╭─ ⚕ Hermes ───╮` / `│ body │` / `╰───────╯`.
fn assistant_card(body: Vec<Line<'static>>, streaming: bool, width: u16) -> Vec<Line<'static>> {
    rounded_panel(
        vec![
            Span::raw(" "),
            Span::styled(RESPONSE_MARK, theme::accent()),
            Span::raw(" "),
            Span::styled("Hermes", theme::accent().add_modifier(Modifier::BOLD)),
        ],
        body,
        panel_width(width),
        streaming,
        false,
    )
}

fn rounded_panel(
    title: Vec<Span<'static>>,
    body: Vec<Line<'static>>,
    width: usize,
    streaming: bool,
    fill: bool,
) -> Vec<Line<'static>> {
    let mut top = vec![b("╭─")];
    top.extend(title);
    if streaming {
        top.push(Span::styled(" ▍", theme::dim()));
    }
    top.push(Span::raw(" "));
    let used = Line::from(top.clone()).width();
    let dash = width.saturating_sub(used).saturating_sub(1);
    top.push(b(&"─".repeat(dash)));
    top.push(b("╮"));

    let inner = width.saturating_sub(4); // │␠ content ␠│
    let mut out = vec![paint_card(Line::from(top), fill)];
    if body.is_empty() {
        let placeholder = if streaming {
            Line::from(Span::styled("▍", theme::dim()))
        } else {
            Line::from("")
        };
        out.push(card_row(placeholder, inner, fill));
    } else {
        for line in body {
            for row in wrap_line(line, inner) {
                out.push(card_row(row, inner, fill));
            }
        }
    }
    out.push(paint_card(
        Line::from(vec![
            b("╰"),
            b(&"─".repeat(width.saturating_sub(2))),
            b("╯"),
        ]),
        fill,
    ));
    out
}

fn card_row(content: Line<'static>, inner: usize, fill: bool) -> Line<'static> {
    let mut spans = vec![b("│"), Span::raw(" ")];
    let content_w = content.width().min(inner);
    spans.extend(content.spans);
    spans.push(Span::raw(
        " ".repeat(inner.saturating_sub(content_w).saturating_add(1)),
    ));
    spans.push(b("│"));
    paint_card(Line::from(spans), fill)
}

fn tool_chip(name: &str, state: &str, hint: &str, width: usize) -> Line<'static> {
    let mut label = format!(" ▸ {name}  ·  {state}");
    if !hint.is_empty() {
        label.push_str("  ");
        label.push_str(hint);
    }
    let budget = width.saturating_sub(3); // ╭ … ╮
    if display_width(&label) > budget {
        label = ellipsize(&label, budget);
    }
    paint_card(
        Line::from(vec![b("╭"), Span::styled(label, theme::tool()), b(" ╮")]),
        true,
    )
}

fn display_width(s: &str) -> usize {
    s.chars()
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
        .sum()
}

fn ellipsize(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if display_width(s) <= max {
        return s.to_string();
    }
    if max <= 1 {
        return "…".to_string();
    }
    let keep = max.saturating_sub(1);
    let mut out = String::new();
    let mut w = 0usize;
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > keep {
            break;
        }
        out.push(ch);
        w += cw;
    }
    out.push('…');
    out
}

fn wrap_line(line: Line<'static>, max: usize) -> Vec<Line<'static>> {
    if max == 0 || line.width() <= max {
        return vec![line];
    }
    let mut rows: Vec<Line<'static>> = Vec::new();
    let mut cur: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut buf_style = Style::default();
    let mut w = 0usize;

    let flush_buf = |cur: &mut Vec<Span<'static>>, buf: &mut String, style: Style| {
        if !buf.is_empty() {
            cur.push(Span::styled(std::mem::take(buf), style));
        }
    };

    for span in line.spans {
        let style = span.style;
        for ch in span.content.chars() {
            let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
            if cw == 0 {
                if buf.is_empty() && cur.is_empty() {
                    continue;
                }
                if style != buf_style {
                    flush_buf(&mut cur, &mut buf, buf_style);
                    buf_style = style;
                }
                buf.push(ch);
                continue;
            }
            if w + cw > max && w > 0 {
                flush_buf(&mut cur, &mut buf, buf_style);
                rows.push(Line::from(std::mem::take(&mut cur)));
                w = 0;
            }
            if style != buf_style && !buf.is_empty() {
                flush_buf(&mut cur, &mut buf, buf_style);
            }
            buf_style = style;
            buf.push(ch);
            w += cw;
        }
    }
    flush_buf(&mut cur, &mut buf, buf_style);
    if !cur.is_empty() {
        rows.push(Line::from(cur));
    }
    if rows.is_empty() {
        rows.push(Line::from(""));
    }
    rows
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
            Span::styled("Ask Hermes…", theme::dim()),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn visible(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn assistant_card_uses_brand_title_and_even_rows() {
        let lines = assistant_card(vec![Line::from("hello")], false, 40);
        let vis = visible(&lines);
        assert!(vis[0].contains(RESPONSE_MARK), "{vis:?}");
        assert!(vis[0].contains("Hermes"), "{vis:?}");
        assert!(vis[0].starts_with('╭'), "{vis:?}");
        assert!(vis.last().unwrap().starts_with('╰'), "{vis:?}");
        let w = lines[0].width();
        assert_eq!(w, 40);
        assert!(
            lines.iter().all(|l| l.width() == w),
            "{:?}",
            lines.iter().map(|l| l.width()).collect::<Vec<_>>()
        );
        assert!(vis.iter().any(|row| row.contains("hello")), "{vis:?}");
    }

    #[test]
    fn assistant_card_has_no_surface_fill() {
        let lines = assistant_card(vec![Line::from("hello")], false, 40);
        assert!(
            lines.iter().all(|l| l.style.bg.is_none()),
            "assistant card should sit on the terminal background"
        );
    }

    #[test]
    fn thinking_panel_keeps_surface_fill() {
        let lines = rounded_panel(
            vec![Span::styled(" thinking", theme::dim())],
            vec![Line::from("hmm")],
            40,
            false,
            true,
        );
        assert!(
            lines.iter().all(|l| l.style.bg == Some(theme::SURFACE())),
            "thinking/tool panels keep the surface fill"
        );
    }

    #[test]
    fn assistant_card_marks_streaming() {
        let lines = assistant_card(vec![Line::from("x")], true, 32);
        let top = visible(&lines).remove(0);
        assert!(top.contains('▍'), "{top}");
        assert!(!top.contains('…'), "{top}");
    }

    #[test]
    fn wrap_line_keeps_rows_within_budget() {
        let rows = wrap_line(Line::from("abcdefghij"), 4);
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| r.width() <= 4));
    }

    #[test]
    fn tool_chip_hugs_label() {
        let line = tool_chip("terminal", "done", "ls -la", 80);
        let vis: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(vis.starts_with('╭'), "{vis}");
        assert!(vis.contains("terminal"), "{vis}");
        assert!(vis.ends_with('╮'), "{vis}");
        assert!(line.width() < 80, "{}", line.width());
    }
}
