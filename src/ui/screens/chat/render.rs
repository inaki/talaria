use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;
use unicode_width::UnicodeWidthChar;

use crate::theme;
use crate::ui::screens::chat::TimelineItem;
use crate::ui::widgets::{bordered_block, markdown_to_lines_at, mascot_lines, mascot_width};

use super::Chat;

/// Same 1-cell gutter as the gap between a rounded panel and the terminal edge.
const EDGE_PAD: u16 = 1;
/// Assistant body column: edge pad + "● ", so replies align under user text.
const ASSISTANT_INDENT: usize = EDGE_PAD as usize + 2;

pub(super) fn draw(chat: &mut Chat, f: &mut Frame, area: Rect) {
    let show_bar = crate::prefs::status_bar();
    let show_hints = crate::prefs::key_hints();

    // The rail takes columns off the right before anything else is laid out, so
    // the transcript (and therefore drag-copy) never overlaps it. Hosted
    // surfaces below still use the full `area` — they are modal over the rail.
    let (area, rail_area) = split_rail(area);
    if let Some(rail) = rail_area {
        draw_rail(chat, f, rail);
    }

    let dock = chat.overlay.is_composer_dock();
    let show_bar = show_bar && !dock;
    // 1 content row + top/bottom border. Soft-wrap and Shift+Enter grow the box.
    let wrap_cols = area.width.saturating_sub(4); // borders + "› "
    let composer_h = if dock {
        super::overlay::composer_dock_height(&chat.overlay)
    } else {
        chat.composer
            .visual_lines(wrap_cols)
            .saturating_add(2)
            .max(3)
    };
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
    if dock {
        super::overlay::paint_composer_dock(f, composer, &chat.overlay);
    } else {
        draw_composer(chat, f, composer);
        if chat.slash.is_active() {
            chat.slash.render_above_input(f, composer);
        }
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

/// `(body, rail)` — `rail` is `None` when the pref is off or the terminal is
/// too narrow to spare the columns.
fn split_rail(area: Rect) -> (Rect, Option<Rect>) {
    if !crate::ui::widgets::rail_visible(area.width) {
        return (area, None);
    }
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(crate::ui::widgets::RAIL_WIDTH),
        ])
        .split(area);
    (chunks[0], Some(chunks[1]))
}

fn draw_rail(chat: &Chat, f: &mut Frame, area: Rect) {
    let meter = chat.meter();
    crate::ui::widgets::render_rail(
        f,
        area,
        &crate::ui::widgets::RailData {
            title: &chat.session_title,
            model: &meter.model,
            git_branch: &meter.git_branch,
            usage: &meter.usage,
            agents: &chat.agents,
            tool_count: chat.tools.len(),
            skill_count: chat.skills.len(),
            cwd: &chat.cwd,
            version: &chat.version,
        },
    );
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
                lines.extend(user_block(t, col_w));
                lines.push(Line::from(""));
            }
            TimelineItem::Assistant { text, streaming } => {
                let mut body: Vec<Line> = Vec::new();
                if *streaming {
                    for l in text.split('\n') {
                        body.push(Line::from(Span::styled(l.to_string(), theme::assistant())));
                    }
                } else {
                    body = markdown_to_lines_at(
                        text,
                        panel_width(col_w).saturating_sub(ASSISTANT_INDENT),
                    );
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
                lines.extend(error_block(s, col_w));
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

fn splash_shows_mascot(area: Rect) -> bool {
    let left = area.width.saturating_mul(40) / 100;
    left >= mascot_width() && area.height >= 14
}

fn wordmark_lines(area: Rect, reserve_mascot: bool) -> Vec<Line<'static>> {
    let compact = vec![Line::from(Span::styled(
        "TALARIA",
        theme::accent().add_modifier(Modifier::BOLD),
    ))];
    let logo = crate::ui::widgets::logo_lines();
    let logo_w = logo.iter().map(|l| l.width() as u16).max().unwrap_or(0);
    let logo_h = logo.len() as u16;
    if area.width < logo_w.saturating_add(2) {
        return compact;
    }
    if reserve_mascot && area.height.saturating_sub(logo_h) < 12 {
        return compact;
    }
    if area.height <= logo_h.saturating_add(6) {
        return compact;
    }
    logo
}

fn draw_splash(chat: &mut Chat, f: &mut Frame, area: Rect) {
    let inner = area.inner(Margin {
        horizontal: EDGE_PAD,
        vertical: EDGE_PAD,
    });
    if inner.width == 0 || inner.height == 0 {
        chat.recent_hits.clear();
        chat.plain_rows.clear();
        return;
    }
    let show_mascot = splash_shows_mascot(inner);
    let wordmark = wordmark_lines(inner, show_mascot);
    let header_h = (wordmark.len() as u16).min(inner.height);
    let mut constraints = vec![Constraint::Length(header_h)];
    if inner.height > header_h.saturating_add(1) {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Min(1));
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);
    f.render_widget(
        Paragraph::new(wordmark).alignment(Alignment::Center),
        rows[0],
    );
    let body = *rows.last().unwrap();

    let copy_area = if show_mascot {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(body);
        let mascot = mascot_lines();
        let mh = mascot.len() as u16;
        let pane = cols[0];
        let mascot_area = if pane.height > mh {
            let pad = (pane.height - mh) / 2;
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(pad),
                    Constraint::Length(mh),
                    Constraint::Min(0),
                ])
                .split(pane)[1]
        } else {
            pane
        };
        f.render_widget(
            Paragraph::new(mascot).alignment(Alignment::Center),
            mascot_area,
        );
        cols[1]
    } else {
        body
    };

    let (copy, local_hits) = splash_copy(chat, copy_area);
    let row0 = copy_area.y.saturating_sub(area.y);
    let col0 = copy_area.x.saturating_sub(area.x);
    chat.recent_hits = local_hits
        .into_iter()
        .map(|(start, len, i)| (row0.saturating_add(start), len, col0, copy_area.width, i))
        .collect();
    let mut plain = vec![String::new(); row0 as usize];
    plain.extend(copy.iter().map(line_plain));
    chat.plain_rows = plain;
    f.render_widget(Paragraph::new(copy), copy_area);
}

#[cfg(test)]
fn splash_lines(chat: &Chat, area: Rect) -> Vec<Line<'static>> {
    splash_layout(chat, area).0
}

#[cfg(test)]
fn splash_layout(chat: &Chat, area: Rect) -> (Vec<Line<'static>>, Vec<(u16, u16, usize)>) {
    let show_mascot = splash_shows_mascot(area);
    let mut lines = wordmark_lines(area, show_mascot);
    lines.push(Line::from(""));
    let (copy, hits) = splash_copy(chat, area);
    let off = lines.len() as u16;
    lines.extend(copy);
    let hits = hits
        .into_iter()
        .map(|(start, len, i)| (off.saturating_add(start), len, i))
        .collect();
    (lines, hits)
}

fn splash_copy(chat: &Chat, area: Rect) -> (Vec<Line<'static>>, Vec<(u16, u16, usize)>) {
    if chat.is_returning_empty() {
        returning_copy(chat, area)
    } else {
        (first_run_copy(chat), Vec::new())
    }
}

fn first_run_copy(chat: &Chat) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();
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

fn returning_copy(chat: &Chat, area: Rect) -> (Vec<Line<'static>>, Vec<(u16, u16, usize)>) {
    let mut lines: Vec<Line> = Vec::new();
    let mut hits: Vec<(u16, u16, usize)> = Vec::new();
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

/// Your turn: a full-width surface band (one row of padding above and below)
/// so prompts stand apart from replies, which sit on the terminal background.
fn user_block(text: &str, width: u16) -> Vec<Line<'static>> {
    let width = panel_width(width);
    let band = Style::default().bg(theme::SURFACE());
    let inner = width.saturating_sub(ASSISTANT_INDENT + EDGE_PAD as usize);
    let lead = " ".repeat(EDGE_PAD as usize);
    let pad_row = || Line::from(Span::raw(" ".repeat(width))).style(band);
    let mut out = vec![pad_row()];
    let mut first = true;
    for l in text.split('\n') {
        for row in wrap_line(
            Line::from(Span::styled(l.to_string(), theme::text())),
            inner,
        ) {
            let mut spans = vec![Span::raw(lead.clone())];
            if first {
                spans.push(Span::styled(
                    "● ",
                    theme::user().add_modifier(Modifier::BOLD),
                ));
                first = false;
            } else {
                spans.push(Span::raw("  "));
            }
            spans.extend(row.spans);
            let used = Line::from(spans.clone()).width();
            spans.push(Span::raw(" ".repeat(width.saturating_sub(used))));
            out.push(Line::from(spans).style(band));
        }
    }
    out.push(pad_row());
    out
}

/// Document turn: accent bullet, first line of the reply beside it, the rest
/// aligned under it. No rule or card chrome — the terminal background.
fn assistant_card(body: Vec<Line<'static>>, streaming: bool, width: u16) -> Vec<Line<'static>> {
    let width = panel_width(width);
    let lead = " ".repeat(EDGE_PAD as usize);
    let body_indent = " ".repeat(ASSISTANT_INDENT);
    let inner = width.saturating_sub(ASSISTANT_INDENT);
    let bullet = || vec![Span::raw(lead.clone()), Span::styled("● ", theme::accent())];
    let mut rows: Vec<Line<'static>> = Vec::new();
    for line in body {
        if is_code_fence_line(&line) {
            // Fences are already sized to `inner`; wrapping would split the box.
            rows.push(line);
        } else {
            rows.extend(wrap_line(line, inner));
        }
    }
    if streaming {
        // Cursor trails the text; a fence (or nothing yet) gets its own row.
        let cursor = Span::styled("▍", theme::dim());
        match rows.last_mut() {
            Some(last) if !is_code_fence_line(last) && last.width() < inner => {
                last.spans.push(cursor)
            }
            _ => rows.push(Line::from(cursor)),
        }
    }
    // A fence can't share the bullet row; give the bullet its own line.
    let bullet_alone = rows.first().is_none_or(is_code_fence_line);
    let mut out = Vec::with_capacity(rows.len() + 1);
    if bullet_alone {
        out.push(Line::from(bullet()));
    }
    for (i, row) in rows.into_iter().enumerate() {
        let mut spans = if i == 0 && !bullet_alone {
            bullet()
        } else {
            vec![Span::raw(body_indent.clone())]
        };
        spans.extend(row.spans);
        out.push(Line::from(spans));
    }
    out
}

/// Red left rule + error panel, same object language as a response / code fence.
fn error_block(text: &str, width: u16) -> Vec<Line<'static>> {
    let width = panel_width(width);
    let inner = width.saturating_sub(1);
    let body: Vec<String> = text.lines().map(str::to_string).collect();
    let panel = bordered_block(
        &body,
        inner,
        theme::hairline(),
        Style::default().fg(theme::ERROR()).bg(theme::SURFACE()),
    );
    panel
        .into_iter()
        .map(|line| {
            let mut spans = vec![Span::styled("│", theme::error())];
            spans.extend(line.spans);
            Line::from(spans)
        })
        .collect()
}

fn is_code_fence_line(line: &Line<'_>) -> bool {
    line.spans.first().is_some_and(|s| {
        let t = s.content.as_ref();
        t.starts_with('┌') || t.starts_with('└') || t == "│"
    })
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
    fn error_block_uses_red_rule_and_panel() {
        let lines = error_block("Mock provider error: the model is unavailable.", 60);
        let vis = visible(&lines);
        assert!(vis[0].starts_with('│'), "{vis:?}");
        assert!(vis.iter().any(|r| r.contains('┌')), "{vis:?}");
        assert!(
            vis.iter().any(|r| r.contains("Mock provider error")),
            "{vis:?}"
        );
        assert!(vis.iter().any(|r| r.contains('└')), "{vis:?}");
        assert!(
            vis.iter().all(|r| !r.contains("● error")),
            "no extra error title: {vis:?}"
        );
    }

    #[test]
    fn assistant_card_starts_reply_beside_bullet() {
        let lines = assistant_card(vec![Line::from("hello"), Line::from("world")], false, 40);
        let vis = visible(&lines);
        assert_eq!(vis[0], " ● hello", "{vis:?}");
        assert_eq!(vis[1], "   world", "rest aligns under the text: {vis:?}");
        assert!(
            vis.iter().all(|row| !row.contains('│')),
            "no left rule: {vis:?}"
        );
    }

    #[test]
    fn assistant_card_puts_fence_below_bullet() {
        let lines = assistant_card(vec![Line::from("┌──┐")], false, 40);
        let vis = visible(&lines);
        assert_eq!(vis[0].trim_end(), " ●", "{vis:?}");
        assert!(vis[1].starts_with("   ┌"), "{vis:?}");
    }

    #[test]
    fn user_block_is_a_full_width_band() {
        let lines = user_block("ask\nmore", 30);
        let vis = visible(&lines);
        assert_eq!(lines.len(), 4, "pad, two rows, pad: {vis:?}");
        assert!(vis[1].starts_with(" ● ask"), "{vis:?}");
        assert!(vis[2].starts_with("   more"), "{vis:?}");
        assert!(lines.iter().all(|l| l.width() == 30), "{vis:?}");
        assert!(lines.iter().all(|l| l.style.bg == Some(theme::SURFACE())));
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
    fn assistant_card_has_no_fill_while_streaming() {
        let lines = assistant_card(vec![Line::from("hello")], true, 40);
        assert!(
            lines.iter().all(|l| l.style.bg.is_none()),
            "streaming reply stays on the terminal background"
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
        assert!(
            t.contains("TALARIA") || t.contains('█'),
            "wordmark missing: {t}"
        );
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
        assert!(
            t.contains("TALARIA") || t.contains('█'),
            "wordmark missing: {t}"
        );
        assert!(t.contains("auth refactor"), "{t}");
        assert!(t.contains("notes"), "{t}");
        assert!(t.contains("1–3"), "{t}");
        assert!(!t.contains("Talaria is ready."), "{t}");
        assert!(!t.contains("Welcome to Hermes"), "{t}");
        assert!(!t.contains("Available Tools"), "{t}");
    }

    #[test]
    fn splash_two_panes_wordmark_and_mascot() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut chat = super::super::Chat::new();
        chat.model = "ox-alpha".into();
        chat.cwd = "/tmp".into();
        let mut term = Terminal::new(TestBackend::new(100, 32)).unwrap();
        term.draw(|f| draw_splash(&mut chat, f, f.area())).unwrap();
        let buf = term.backend().buffer();
        let mut s = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        assert!(
            s.contains("TALARIA") || s.contains('█'),
            "header wordmark missing:\n{s}"
        );
        assert!(
            s.chars().any(|c| ('\u{2800}'..='\u{28FF}').contains(&c)),
            "left pane should be the braille mascot:\n{s}"
        );
        assert!(s.contains("Native TUI host for Hermes Agent"), "{s}");
        assert!(s.contains("Talaria is ready."), "{s}");
        let mut mascot_on_right = false;
        let mut copy_on_left = false;
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let ch = buf[(x, y)].symbol().chars().next().unwrap_or('\0');
                if ('\u{2800}'..='\u{28FF}').contains(&ch) && x >= 40 {
                    mascot_on_right = true;
                }
                if buf[(x, y)].symbol() == "N" && x < 40 {
                    // tagline starts with "Native"
                    copy_on_left = true;
                }
            }
        }
        assert!(!mascot_on_right, "mascot should stay in the 40% left pane");
        assert!(!copy_on_left, "copy should stay in the 60% right pane");
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

#[cfg(test)]
mod rail_layout_tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use crate::ui::widgets::RAIL_WIDTH;

    /// `split_rail` is the seam that keeps the rail out of the transcript, and
    /// therefore out of `plain_rows` / drag-copy. Exercise it directly: the
    /// pref is process-global, so asserting on the real render would make these
    /// order-dependent against other tests.
    #[test]
    fn body_loses_exactly_the_rail_columns() {
        let area = Rect::new(0, 0, 140, 30);
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(RAIL_WIDTH)])
            .split(area);
        assert_eq!(chunks[1].width, RAIL_WIDTH);
        assert_eq!(chunks[0].width, 140 - RAIL_WIDTH);
        // The rail sits flush against the right edge, so the document keeps a
        // contiguous block starting at x=0.
        assert_eq!(chunks[0].x, 0);
        assert_eq!(chunks[1].x, 140 - RAIL_WIDTH);
    }

    /// With the pref off (the default) the rail must not steal columns.
    #[test]
    fn rail_off_by_default_gives_the_document_everything() {
        let (body, rail) = split_rail(Rect::new(0, 0, 140, 30));
        assert!(rail.is_none(), "rail is opt-in");
        assert_eq!(body.width, 140);
    }

    /// The whole screen still paints at an awkward size with the rail region
    /// requested — guards against a panic from a zero-width inner rect.
    #[test]
    fn renders_without_panic_at_tight_sizes() {
        for (w, h) in [(140u16, 24u16), (100, 12), (80, 10), (40, 8)] {
            let mut chat = Chat::new();
            chat.push_user("hello".into());
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| draw(&mut chat, f, f.area())).unwrap();
        }
    }
}
