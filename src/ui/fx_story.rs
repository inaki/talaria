//! Gallery story: [tachyonfx](https://ratatui.rs/ecosystem/tachyonfx/) on Talaria chrome.
//!
//! Effects run on home / tool-panel / response cards so we can judge them
//! against espresso / mint / gold — not a generic demo.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use tachyonfx::fx::ExpandDirection;
use tachyonfx::{fx, CellFilter, EffectRenderer, Interpolation, Motion};

pub use tachyonfx::{Duration as FxDuration, Effect};

use crate::theme;

pub const KINDS: &[&str] = &[
    "home enter",
    "mint pulse",
    "agent gold",
    "dissolve",
    "panel",
    "response",
];

pub fn kind_title(kind: usize) -> &'static str {
    KINDS.get(kind).copied().unwrap_or(KINDS[0])
}

pub fn next_kind(kind: usize) -> usize {
    (kind + 1) % KINDS.len()
}

pub fn prev_kind(kind: usize) -> usize {
    if kind == 0 {
        KINDS.len() - 1
    } else {
        kind - 1
    }
}

pub fn effect(kind: usize) -> Effect {
    match kind % KINDS.len() {
        0 => fx::repeating(fx::sequence(&[
            fx::sweep_in(
                Motion::LeftToRight,
                10,
                0,
                theme::BACKGROUND(),
                (900, Interpolation::QuadOut),
            ),
            fx::coalesce((700, Interpolation::SineOut)),
            fx::sleep(1100),
        ])),
        1 => fx::repeating(fx::ping_pong(fx::hsl_shift_fg(
            [0.0, 18.0, 12.0],
            (900, Interpolation::SineInOut),
        ))),
        2 => fx::repeating(fx::ping_pong(fx::fade_from_fg(
            theme::AGENT(),
            (800, Interpolation::SineInOut),
        ))),
        3 => fx::repeating(fx::sequence(&[
            fx::dissolve((700, Interpolation::QuadIn)),
            fx::coalesce((700, Interpolation::SineOut)),
            fx::sleep(500),
        ])),
        4 => fx::repeating(fx::sequence(&[
            fx::expand(
                ExpandDirection::Horizontal,
                Style::default().bg(theme::SURFACE()),
                (550, Interpolation::QuadOut),
            ),
            fx::coalesce((500, Interpolation::SineOut)),
            fx::sleep(900),
        ])),
        _ => {
            let rule = CellFilter::Layout(
                Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Length(1), Constraint::Min(1)]),
                0,
            );
            fx::repeating(fx::ping_pong(
                fx::sweep_in(
                    Motion::UpToDown,
                    3,
                    0,
                    theme::BACKGROUND(),
                    (700, Interpolation::QuadInOut),
                )
                .with_filter(rule),
            ))
        }
    }
}

pub fn paint(f: &mut Frame, area: Rect, kind: usize, fx: &mut Effect, dt: FxDuration) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(6),
            Constraint::Length(2),
        ])
        .split(area);
    paint_kind_row(f, rows[0], kind);
    let card = inset(rows[1], 1, 0);
    paint_subject(f, card, kind);
    f.render_effect(fx, card, dt);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("tachyonfx  ", theme::dim()),
            Span::styled("←→", theme::accent()),
            Span::styled(" cycle  ", theme::dim()),
            Span::styled("Space", theme::accent()),
            Span::styled(" replay", theme::dim()),
        ])),
        rows[2],
    );
}

fn paint_kind_row(f: &mut Frame, area: Rect, kind: usize) {
    let mut spans = Vec::new();
    for (i, name) in KINDS.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", theme::dim()));
        }
        if i == kind {
            spans.push(Span::styled(
                format!("▸ {name}"),
                theme::accent().add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled((*name).to_string(), theme::dim()));
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn paint_subject(f: &mut Frame, area: Rect, kind: usize) {
    match kind % KINDS.len() {
        4 => paint_panel(f, area),
        5 => paint_response(f, area),
        _ => paint_home_card(f, area),
    }
}

fn paint_home_card(f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            "TALARIA",
            theme::agent().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(crate::ui::widgets::tagline(), theme::dim())),
        Line::from(""),
        Line::from(vec![
            Span::styled("⚕ ", theme::agent()),
            Span::styled("Hermes Agent", theme::agent()),
            Span::styled(" · ", theme::dim()),
            Span::styled("ox-alpha", theme::text()),
        ]),
        Line::from(""),
        Line::from(Span::styled("▸ 1.  auth refactor", theme::selected())),
        Line::from(Span::styled("      jwt middleware", theme::dim())),
        Line::from(Span::styled("  2.  notes", theme::text())),
        Line::from(""),
        Line::from(vec![
            Span::styled("1–3", theme::accent()),
            Span::styled(" resume", theme::dim()),
        ]),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

/// Hairline tool panel — same object language as a code fence / tool card.
fn paint_panel(f: &mut Frame, area: Rect) {
    let w = (area.width as usize).max(16);
    let inner = w.saturating_sub(4);
    let fill = Style::default().fg(theme::TEXT()).bg(theme::SURFACE());
    let mut lines = vec![
        panel_edge('╭', "─ ▸ terminal  [done] ", '╮', w),
        panel_row("$ git status", inner, fill),
        panel_row("On branch docs/redesign", inner, fill),
        panel_row("modified: src/ui/fx_story.rs", inner, fill),
        panel_edge('╰', &"─".repeat(w.saturating_sub(2)), '╯', w),
    ];
    if lines.len() as u16 > area.height {
        lines.truncate(area.height as usize);
    }
    f.render_widget(Paragraph::new(lines), area);
}

fn panel_edge(left: char, mid: &str, right: char, width: usize) -> Line<'static> {
    let mut s = format!("{left}{mid}");
    let used = unicode_width::UnicodeWidthStr::width(s.as_str());
    if used + 1 < width {
        s.push_str(&"─".repeat(width - used - 1));
    }
    s.push(right);
    Line::from(Span::styled(s, theme::hairline().bg(theme::SURFACE())))
}

fn panel_row(text: &str, inner: usize, fill: Style) -> Line<'static> {
    let mut body = text.to_string();
    let mut tw = unicode_width::UnicodeWidthStr::width(body.as_str());
    if tw > inner {
        body.truncate(inner);
        tw = unicode_width::UnicodeWidthStr::width(body.as_str());
    }
    if tw < inner {
        body.push_str(&" ".repeat(inner - tw));
    }
    Line::from(vec![
        Span::styled("│", theme::hairline().bg(theme::SURFACE())),
        Span::raw(" "),
        Span::styled(body, fill),
        Span::raw(" "),
        Span::styled("│", theme::hairline().bg(theme::SURFACE())),
    ])
}

/// Mint left rule + gold author — same chrome as a live assistant turn.
fn paint_response(f: &mut Frame, area: Rect) {
    let inner = (area.width as usize).saturating_sub(2).max(8);
    let lines = vec![
        rule_line(
            vec![
                Span::styled("● ", theme::accent()),
                Span::styled("⚕", theme::agent()),
                Span::raw(" "),
                Span::styled("Hermes", theme::agent().add_modifier(Modifier::BOLD)),
            ],
            inner,
        ),
        rule_line(
            vec![Span::styled(
                "session.resume writes the row_id,",
                theme::text(),
            )],
            inner,
        ),
        rule_line(
            vec![Span::styled(
                "then confirm_truncate drops turns",
                theme::text(),
            )],
            inner,
        ),
        rule_line(
            vec![Span::styled("after that point.", theme::text())],
            inner,
        ),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

fn rule_line(content: Vec<Span<'static>>, inner: usize) -> Line<'static> {
    let mut spans = vec![Span::styled("│ ", theme::accent())];
    let used = Line::from(content.clone()).width();
    spans.extend(content);
    if used < inner {
        spans.push(Span::raw(" ".repeat(inner - used)));
    }
    Line::from(spans)
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
    fn kinds_are_unique_and_effects_build() {
        let mut seen = std::collections::BTreeSet::new();
        for (i, name) in KINDS.iter().enumerate() {
            assert!(seen.insert(*name), "duplicate {name}");
            let fx = effect(i);
            assert!(fx.running() || !fx.done(), "{name}");
        }
        assert!(KINDS.contains(&"panel"));
        assert!(KINDS.contains(&"response"));
        assert_eq!(next_kind(KINDS.len() - 1), 0);
        assert_eq!(prev_kind(0), KINDS.len() - 1);
    }
}
