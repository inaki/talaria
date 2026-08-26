use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;
use unicode_width::UnicodeWidthChar;

use crate::theme;
use crate::ui::screens::chat::TimelineItem;
use crate::ui::widgets::markdown_to_lines_at;

use super::Chat;

/// Same 1-cell gutter as the gap between a rounded panel and the terminal edge.
const EDGE_PAD: u16 = 1;

pub(super) fn draw(chat: &mut Chat, f: &mut Frame, area: Rect) {
    let show_bar = crate::prefs::status_bar();
    let show_hints = crate::prefs::key_hints();
    // 1 content row + top/bottom border. Soft-wrap and Shift+Enter grow the box.
    let wrap_cols = area.width.saturating_sub(4); // borders + "› "
    let composer_h = chat
        .composer
        .visual_lines(wrap_cols)
        .saturating_add(2)
        .max(3);
    let mut constraints = vec![Constraint::Min(3)];
    if show_bar {
        constraints.push(Constraint::Length(EDGE_PAD));
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(composer_h));
    if show_hints {
        constraints.push(Constraint::Length(1));
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut i = 0;
    let transcript = chunks[i];
    draw_transcript(chat, f, transcript);
    if chat.notice_expires.is_some() {
        if let Some(n) = chat.notice.as_deref() {
            draw_toast(f, transcript, n);
        }
    }
    i += 1;
    if show_bar {
        i += 1; // EDGE_PAD above the meter
        draw_meter(chat, f, chunks[i]);
        i += 1;
    }
    let composer = chunks[i];
    i += 1;
    draw_composer(chat, f, composer);
    if chat.slash.is_active() {
        chat.slash.render_above_input(f, composer);
    }
    if show_hints {
        f.render_widget(chat.key_hints_impl(), gutter(chunks[i]));
    }

    if let Some(sheet) = &chat.sheet {
        sheet.render(f, area);
    }
    if let Some(ins) = &chat.inspector {
        ins.render(f, area);
    }
    chat.palette.render(f, area);

    super::overlay::draw_overlay(
        &chat.overlay,
        f,
        area,
        chat.update_available.as_deref(),
        (!chat.version.is_empty()).then_some(chat.version.as_str()),
    );
    if chat.confirm_quit {
        draw_quit_modal(f, area);
    }
}

fn gutter(area: Rect) -> Rect {
    area.inner(Margin {
        horizontal: EDGE_PAD,
        vertical: 0,
    })
}

fn draw_meter(chat: &Chat, f: &mut Frame, area: Rect) {
    let area = gutter(area);
    let meter = chat.meter();
    f.render_widget(
        Paragraph::new(super::status::line(&meter, area.width)),
        area,
    );
}

fn draw_transcript(chat: &mut Chat, f: &mut Frame, area: Rect) {
    if chat.items.is_empty() && !chat.streaming {
        chat.transcript_area = area;
        chat.tool_hits.clear();
        draw_splash(chat, f, area);
        return;
    }
    chat.transcript_area = area;
    chat.tool_hits.clear();
    let mut lines: Vec<Line> = Vec::new();
    for _ in 0..EDGE_PAD {
        lines.push(Line::from(""));
    }
    let col_w = area.width.max(12);
    for (i, item) in chat.items.iter().enumerate() {
        let hit_start = lines.len() as u16;
        match item {
            TimelineItem::User { text: t, .. } => {
                let mut first = true;
                let indent = " ".repeat(EDGE_PAD as usize);
                for l in t.split('\n') {
                    if first {
                        lines.push(Line::from(vec![
                            Span::raw(indent.clone()),
                            Span::styled("● ", theme::user().add_modifier(Modifier::BOLD)),
                            Span::styled(l.to_string(), theme::text()),
                        ]));
                        first = false;
                    } else {
                        lines.push(Line::from(Span::styled(
                            format!("{indent}  {l}"),
                            theme::text(),
                        )));
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
                    body = markdown_to_lines_at(text, panel_width(col_w).saturating_sub(2));
                }
                lines.extend(assistant_card(body, *streaming, col_w));
                lines.push(Line::from(""));
            }
            TimelineItem::Thinking { text, live } => {
                let raw: Vec<&str> = text.split('\n').collect();
                let start = raw.len().saturating_sub(8);
                let italic = theme::dim().add_modifier(Modifier::ITALIC);
                let is_last = i + 1 == chat.items.len();
                let mut first = true;
                for l in &raw[start..] {
                    let mut spans = vec![Span::styled("  ", italic)];
                    if first && *live && is_last {
                        spans.push(chat.spinner.span(theme::accent()));
                        spans.push(Span::raw(" "));
                    }
                    spans.push(Span::styled((*l).to_string(), italic));
                    lines.push(Line::from(spans));
                    first = false;
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
                    let hint = first_tool_hint(result, preview, args);
                    lines.push(tool_chip(name, state, hint, panel_width(col_w)));
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

    let width = area.width.max(1) as usize;
    let logical = lines;
    let mut lines: Vec<Line> = Vec::with_capacity(logical.len());
    let mut remap: Vec<u16> = Vec::with_capacity(logical.len() + 1);
    for line in logical {
        remap.push(lines.len() as u16);
        lines.extend(wrap_line(line, width));
    }
    remap.push(lines.len() as u16);
    for (start, len, _) in chat.tool_hits.iter_mut() {
        let s = (*start as usize).min(remap.len().saturating_sub(1));
        let e = s
            .saturating_add(*len as usize)
            .min(remap.len().saturating_sub(1));
        *start = remap[s];
        *len = remap[e].saturating_sub(remap[s]);
    }

    chat.plain_rows = lines.iter().map(line_plain).collect();
    if let Some((a, b)) = chat.selection {
        lines = lines
            .into_iter()
            .enumerate()
            .map(|(i, line)| highlight_line(line, i as u16, a, b))
            .collect();
    }

    let total = lines.len() as u16;
    let max_scroll = total.saturating_sub(area.height);
    if chat.follow {
        chat.scroll = max_scroll;
    } else {
        chat.scroll = chat.scroll.min(max_scroll);
        if chat.scroll >= max_scroll {
            chat.follow = true;
        }
    }
    let paragraph = Paragraph::new(lines).scroll((chat.scroll, 0));
    f.render_widget(paragraph, area);
}

fn draw_splash(chat: &mut Chat, f: &mut Frame, area: Rect) {
    let (lines, hits) = splash_layout(chat, area);
    chat.recent_hits = hits;
    chat.plain_rows = lines.iter().map(line_plain).collect();
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

#[cfg(test)]
fn splash_lines(chat: &Chat, area: Rect) -> Vec<Line<'static>> {
    splash_layout(chat, area).0
}

fn splash_layout(chat: &Chat, area: Rect) -> (Vec<Line<'static>>, Vec<(u16, u16, usize)>) {
    if chat.is_returning_empty() {
        returning_lines(chat, area)
    } else {
        (first_run_lines(chat, area), Vec::new())
    }
}

fn first_run_lines(chat: &Chat, area: Rect) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();
    for _ in 0..EDGE_PAD {
        lines.push(Line::from(""));
    }

    let logo = crate::ui::widgets::logo_lines();
    let logo_w = logo.iter().map(|l| l.width() as u16).max().unwrap_or(0);
    let logo_h = logo.len() as u16;
    if area.width >= logo_w && area.height > logo_h.saturating_add(10) {
        lines.extend(logo);
    } else {
        lines.push(Line::from(Span::styled(
            "TALARIA",
            theme::accent().add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(Span::styled(
        crate::ui::widgets::tagline(),
        theme::dim(),
    )));
    lines.push(Line::from(""));

    let model = chat.model.rsplit('/').next().unwrap_or(&chat.model);
    lines.push(Line::from(vec![
        Span::styled("⚕ ", theme::agent()),
        Span::styled("Hermes Agent", theme::agent()),
        Span::styled(" · ", theme::dim()),
        Span::styled(model.to_string(), theme::text()),
    ]));
    if !chat.cwd.is_empty() {
        lines.push(Line::from(Span::styled(chat.cwd.clone(), theme::dim())));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Ctrl+K", theme::accent()),
        Span::styled(" palette", theme::dim()),
        Span::styled("   ", theme::dim()),
        Span::styled("/", theme::accent()),
        Span::styled(" commands", theme::dim()),
        Span::styled("   ", theme::dim()),
        Span::styled("!", theme::accent()),
        Span::styled(" shell", theme::dim()),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("Talaria is ready.", theme::text())));
    splash_footer(chat, &mut lines);
    lines
}

fn returning_lines(chat: &Chat, area: Rect) -> (Vec<Line<'static>>, Vec<(u16, u16, usize)>) {
    let mut lines: Vec<Line> = Vec::new();
    let mut hits: Vec<(u16, u16, usize)> = Vec::new();
    for _ in 0..EDGE_PAD {
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        "TALARIA",
        theme::accent().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        crate::ui::widgets::tagline(),
        theme::dim(),
    )));
    lines.push(Line::from(""));

    let model = chat.model.rsplit('/').next().unwrap_or(&chat.model);
    lines.push(Line::from(vec![
        Span::styled("⚕ ", theme::agent()),
        Span::styled(model.to_string(), theme::text()),
    ]));
    if !chat.cwd.is_empty() {
        lines.push(Line::from(Span::styled(chat.cwd.clone(), theme::dim())));
    }
    lines.push(Line::from(""));

    let budget = (area.width as usize).saturating_sub(8).max(12);
    let recents = chat.recent_for_empty();
    for (i, s) in recents.iter().enumerate() {
        let start = lines.len() as u16;
        let selected = i == chat.recent_selected;
        let mark = if selected { "▸ " } else { "  " };
        let num = format!("{}.", i + 1);
        let title = if s.title.is_empty() {
            s.id.clone()
        } else {
            s.title.clone()
        };
        let row = format!("{mark}{num:<3}{}", ellipsize(&title, budget));
        if selected {
            lines.push(Line::from(Span::styled(row, theme::selected())));
        } else {
            lines.push(Line::from(Span::styled(row, theme::text())));
        }
        let preview = if s.preview.is_empty() {
            format!("{} msgs", s.message_count)
        } else {
            s.preview.clone()
        };
        lines.push(Line::from(vec![
            Span::raw("      "),
            Span::styled(ellipsize(&preview, budget), theme::dim()),
        ]));
        hits.push((start, (lines.len() as u16).saturating_sub(start), i));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("1–3", theme::accent()),
        Span::styled(" resume", theme::dim()),
        Span::styled("   ", theme::dim()),
        Span::styled("Enter", theme::accent()),
        Span::styled(" open", theme::dim()),
        Span::styled("   ", theme::dim()),
        Span::styled("Ctrl+K", theme::accent()),
        Span::styled(" palette", theme::dim()),
    ]));
    splash_footer(chat, &mut lines);
    (lines, hits)
}

fn splash_footer(chat: &Chat, lines: &mut Vec<Line<'static>>) {
    if let Some(v) = &chat.update_available {
        lines.push(Line::from(Span::styled(
            crate::update::notice_line(v),
            Style::default().fg(theme::WARNING()),
        )));
    }
    let mut vers = format!("Talaria v{}", env!("CARGO_PKG_VERSION"));
    if !chat.version.is_empty() {
        vers.push_str(&format!(" · Hermes v{}", chat.version));
    }
    lines.push(Line::from(Span::styled(vers, theme::dim())));
}

fn border_style() -> Style {
    theme::hairline()
}

fn rule_style() -> Style {
    theme::accent()
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

fn first_tool_hint<'a>(result: &'a str, preview: &'a str, args: &'a str) -> &'a str {
    first_nonempty_line(result)
        .or_else(|| first_nonempty_line(preview))
        .or_else(|| first_nonempty_line(args))
        .unwrap_or("")
}

fn first_nonempty_line(s: &str) -> Option<&str> {
    s.lines().map(str::trim).find(|l| !l.is_empty())
}

fn panel_width(col_w: u16) -> usize {
    (col_w as usize).max(12)
}

/// Official brand mark (staff of Asclepius), same glyph as Hermes CLI `response_label`.
const RESPONSE_MARK: &str = "⚕";

/// Document turn: mint left rule, gold author. Fill only while streaming.
fn assistant_card(body: Vec<Line<'static>>, streaming: bool, width: u16) -> Vec<Line<'static>> {
    let width = panel_width(width);
    let inner = width.saturating_sub(2);
    let mut title = vec![
        Span::styled("│ ", rule_style()),
        Span::styled("● ", theme::accent()),
        Span::styled(RESPONSE_MARK, theme::agent()),
        Span::raw(" "),
        Span::styled("Hermes", theme::agent().add_modifier(Modifier::BOLD)),
    ];
    if streaming {
        title.push(Span::styled(" ▍", theme::dim()));
    }
    let mut out = vec![paint_card(
        pad_rule_line(Line::from(title), inner, width),
        streaming,
    )];
    if body.is_empty() {
        let placeholder = if streaming {
            Line::from(Span::styled("▍", theme::dim()))
        } else {
            Line::from("")
        };
        out.push(paint_card(rule_row(placeholder, inner, width), streaming));
    } else {
        for line in body {
            for row in wrap_line(line, inner) {
                out.push(paint_card(rule_row(row, inner, width), streaming));
            }
        }
    }
    out
}

fn rule_row(content: Line<'static>, inner: usize, _width: usize) -> Line<'static> {
    let mut spans = vec![Span::styled("│ ", rule_style())];
    let content_w = content.width().min(inner);
    spans.extend(content.spans);
    spans.push(Span::raw(" ".repeat(inner.saturating_sub(content_w))));
    Line::from(spans)
}

fn pad_rule_line(mut line: Line<'static>, inner: usize, _width: usize) -> Line<'static> {
    let used = line.width().saturating_sub(2);
    if used < inner {
        line.spans.push(Span::raw(" ".repeat(inner - used)));
    }
    line
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
        .border_style(theme::hairline())
        .style(
            Style::default()
                .bg(theme::BACKGROUND())
                .fg(theme::SEPARATOR()),
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
            inner.width.saturating_sub(indent.chars().count() as u16),
        )
    };
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn line_plain(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

fn highlight_line(line: Line<'static>, row: u16, a: (u16, u16), b: (u16, u16)) -> Line<'static> {
    let (s, e) = if a <= b { (a, b) } else { (b, a) };
    if row < s.0 || row > e.0 {
        return line;
    }
    let from = if row == s.0 { s.1 } else { 0 };
    let to = if row == e.0 {
        e.1.saturating_add(1)
    } else {
        u16::MAX
    };
    highlight_cols(line, from, to)
}

fn highlight_cols(line: Line<'static>, from: u16, to: u16) -> Line<'static> {
    if from >= to {
        return line;
    }
    let hi = Style::default()
        .fg(theme::BACKGROUND())
        .bg(theme::PRIMARY());
    let line_style = line.style;
    let mut out: Vec<Span> = Vec::new();
    let mut col = 0u16;
    for span in line.spans {
        let style = span.style;
        let mut buf = String::new();
        let mut buf_hi = false;
        for ch in span.content.chars() {
            let cw = UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
            if cw == 0 {
                buf.push(ch);
                continue;
            }
            let is_hi = col >= from && col < to;
            if is_hi != buf_hi && !buf.is_empty() {
                let st = if buf_hi { hi } else { style };
                out.push(Span::styled(std::mem::take(&mut buf), st));
            }
            buf_hi = is_hi;
            buf.push(ch);
            col = col.saturating_add(cw);
        }
        if !buf.is_empty() {
            let st = if buf_hi { hi } else { style };
            out.push(Span::styled(buf, st));
        }
    }
    Line::from(out).style(line_style)
}

fn draw_toast(f: &mut Frame, area: Rect, text: &str) {
    if area.width < 6 || area.height == 0 {
        return;
    }
    let label = format!(" {text} ");
    let w = (label.chars().count() as u16)
        .min(area.width.saturating_sub(2))
        .max(1);
    let rect = Rect {
        x: area.x + area.width.saturating_sub(w).saturating_sub(1),
        y: area.y + area.height.saturating_sub(1),
        width: w,
        height: 1,
    };
    let ok = text.starts_with("Copied") || text.starts_with("Wrote");
    let style = if ok {
        Style::default()
            .fg(theme::BACKGROUND())
            .bg(theme::SUCCESS())
    } else {
        Style::default()
            .fg(theme::BACKGROUND())
            .bg(theme::WARNING())
    };
    f.render_widget(Paragraph::new(Span::styled(label, style)), rect);
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
        Paragraph::new("Quit talaria?\n y to confirm  ·  Esc to cancel"),
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
    wrap_cols: u16,
) -> Vec<Line<'static>> {
    let width = wrap_cols.max(1) as usize;
    let (cursor_line, cursor_col) = crate::ui::widgets::offset_to_line_col(draft, cursor);
    let mut lines = Vec::new();
    let mut cursor_row = 0usize;
    for (li, line) in draft.split('\n').enumerate() {
        let chunks = crate::ui::widgets::wrap_chunks(line, width);
        let n = chunks.len();
        for (ci, &(start, end)) in chunks.iter().enumerate() {
            let piece = &line[start..end];
            let mut spans = Vec::new();
            if li == 0 && ci == 0 {
                spans.push(Span::styled(prefix.to_string(), accent));
            } else {
                spans.push(Span::raw(indent.to_string()));
            }
            let cursor_here = li == cursor_line
                && cursor_col >= start
                && (cursor_col < end || (ci + 1 == n && cursor_col >= end));
            if cursor_here {
                cursor_row = lines.len();
                let col = cursor_col.saturating_sub(start).min(piece.len());
                let before = piece[..col].to_string();
                let at = piece[col..].chars().next();
                let after_start = col + at.map(|c| c.len_utf8()).unwrap_or(0);
                let after = piece[after_start.min(piece.len())..].to_string();
                spans.push(Span::styled(before, body));
                let ch = at.map(|c| c.to_string()).unwrap_or_else(|| " ".into());
                spans.push(Span::styled(ch, theme::cursor_block_style()));
                spans.push(Span::styled(after, body));
            } else {
                spans.push(Span::styled(piece.to_string(), body));
            }
            lines.push(Line::from(spans));
        }
    }
    const MAX: usize = 6;
    if lines.len() > MAX {
        let start = (cursor_row + 1).saturating_sub(MAX).min(lines.len() - MAX);
        lines = lines[start..start + MAX].to_vec();
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
        assert!(vis.iter().all(|row| row.starts_with('│')), "{vis:?}");
        assert!(!vis[0].starts_with('╭'), "{vis:?}");
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
    fn assistant_card_fills_while_streaming() {
        let lines = assistant_card(vec![Line::from("hello")], true, 40);
        assert!(
            lines.iter().all(|l| l.style.bg == Some(theme::SURFACE())),
            "streaming assistant card uses Talaria surface fill"
        );
    }

    #[test]
    fn splash_is_talaria_host_not_hermes_brochure() {
        let mut chat = super::super::Chat::new();
        chat.model = "ox-alpha".into();
        chat.cwd = "/tmp".into();
        chat.tools = vec![("core".into(), vec!["terminal".into()])];
        chat.skills = vec![("dev".into(), vec!["review".into()])];
        let t = visible(&splash_lines(&chat, Rect::new(0, 0, 40, 12))).join("\n");
        assert!(t.contains("TALARIA"), "{t}");
        assert!(t.contains("Native TUI host for Hermes Agent"), "{t}");
        assert!(t.contains("Talaria is ready."), "{t}");
        assert!(t.contains("Ctrl+K"), "{t}");
        assert!(t.contains("Hermes Agent"), "{t}");
        assert!(t.contains("ox-alpha"), "{t}");
        assert!(!t.contains("Welcome to Hermes"), "{t}");
        assert!(!t.contains("Nous Research"), "{t}");
        assert!(!t.contains("Available Tools"), "{t}");
        assert!(!t.contains("Available Skills"), "{t}");
        assert!(!t.contains("CLIENT"), "{t}");
    }

    #[test]
    fn returning_splash_lists_sessions_not_brochure() {
        let mut chat = super::super::Chat::new();
        chat.model = "ox-alpha".into();
        chat.cwd = "/tmp".into();
        chat.recent_sessions = vec![
            crate::session::SavedSession {
                id: "a".into(),
                title: "auth refactor".into(),
                preview: "jwt middleware".into(),
                source: "tui".into(),
                message_count: 12,
            },
            crate::session::SavedSession {
                id: "b".into(),
                title: "notes".into(),
                preview: "cargo test".into(),
                source: "cli".into(),
                message_count: 4,
            },
        ];
        let t = visible(&splash_lines(&chat, Rect::new(0, 0, 80, 24))).join("\n");
        assert!(t.contains("TALARIA"), "{t}");
        assert!(t.contains("auth refactor"), "{t}");
        assert!(t.contains("notes"), "{t}");
        assert!(t.contains("1–3"), "{t}");
        assert!(!t.contains("Talaria is ready."), "{t}");
        assert!(!t.contains("Welcome to Hermes"), "{t}");
        assert!(!t.contains("Available Tools"), "{t}");
    }

    #[test]
    fn tool_panel_keeps_surface_fill() {
        let lines = rounded_panel(
            vec![Span::styled(" terminal", theme::tool())],
            vec![Line::from("ls")],
            40,
            false,
            true,
        );
        assert!(
            lines.iter().all(|l| l.style.bg == Some(theme::SURFACE())),
            "tool/shell panels keep the surface fill"
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
    fn highlight_covers_inclusive_cells() {
        let line = Line::from("abcdef");
        let hi = highlight_line(line, 0, (0, 1), (0, 3));
        let vis = visible(&[hi]);
        assert_eq!(vis[0], "abcdef");
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
