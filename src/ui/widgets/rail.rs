//! Right-hand ambient rail (`/custom` → rail). A second *layout* for the
//! meter's data, not a new hosted surface — see `docs/REDESIGN.md` decision 6.
//!
//! Opt-in and width-gated: below [`MIN_TOTAL_WIDTH`] the document keeps every
//! column and the inline meter carries the same numbers.
//!
//! Every panel is fed by state the host already holds. Nothing here infers a
//! field the gateway did not send — no LSP row (no such concept in
//! `tui_gateway`) and no todo list (`goal` belongs to subagents, not the
//! session), per `docs/REDESIGN.md`: do not invent gateway events.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::session::{SubagentRow, UsageSnapshot};
use crate::theme;
use crate::ui::screens::chat::status::{ctx_bar, fmt_k, short_cwd};

/// Columns the rail occupies when shown.
pub const RAIL_WIDTH: u16 = 34;

/// Total terminal width below which the rail hides itself. The document needs
/// ~80 columns to stay readable, so the rail only appears when there is slack.
pub const MIN_TOTAL_WIDTH: u16 = 100;

/// Subagent rows listed before the panel summarizes the remainder.
const MAX_AGENTS: usize = 6;

/// A shown rail must never squeeze the document below 60 columns — that is the
/// entire point of the width gate. Checked at compile time so tuning either
/// constant cannot quietly break the guarantee.
const _: () = assert!(MIN_TOTAL_WIDTH - RAIL_WIDTH >= 60);

/// Is the rail on *and* does the terminal have room for it?
pub fn visible(total_width: u16) -> bool {
    crate::prefs::rail() && total_width >= MIN_TOTAL_WIDTH
}

/// Ambient state the rail draws. Borrowed — the rail owns no state.
pub struct RailData<'a> {
    pub title: &'a str,
    pub model: &'a str,
    pub git_branch: &'a str,
    pub usage: &'a UsageSnapshot,
    pub agents: &'a [SubagentRow],
    pub tool_count: usize,
    pub skill_count: usize,
    pub cwd: &'a str,
    pub version: &'a str,
}

pub fn render(f: &mut Frame, area: Rect, data: &RailData<'_>) {
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(theme::dim())
        .style(ratatui::style::Style::default().bg(theme::BACKGROUND()));
    let inner = block.inner(area).inner(ratatui::layout::Margin {
        horizontal: 1,
        vertical: 1,
    });
    f.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let cols = inner.width as usize;
    let bottom = bottom_lines(data);

    // Footer pins to the bottom like the reference; the body is cut to the rows
    // that remain rather than pushing the footer off-screen.
    let foot_h = (bottom.len() as u16).min(inner.height);
    let body_h = inner.height.saturating_sub(foot_h);

    let mut top = top_lines(data, cols);
    top.truncate(body_h as usize);
    let top: Vec<Line> = trim_dangling(top).into_iter().map(|l| l.line).collect();

    if body_h > 0 {
        f.render_widget(
            Paragraph::new(top),
            Rect {
                height: body_h,
                ..inner
            },
        );
    }
    if foot_h > 0 {
        f.render_widget(
            Paragraph::new(bottom),
            Rect {
                y: inner.y + body_h,
                height: foot_h,
                ..inner
            },
        );
    }
}

/// A rail line plus whether it is a panel heading. Headings are dropped when
/// the frame cuts before their content, so the rail never shows a bare label.
struct RailLine {
    heading: bool,
    line: Line<'static>,
}

fn heading(text: &str) -> RailLine {
    RailLine {
        heading: true,
        line: Line::from(Span::styled(
            text.to_string(),
            theme::accent().add_modifier(ratatui::style::Modifier::BOLD),
        )),
    }
}

fn plain(line: Line<'static>) -> RailLine {
    RailLine {
        heading: false,
        line,
    }
}

fn blank() -> RailLine {
    plain(Line::from(""))
}

/// Truncate to `cols` display columns, ellipsizing. Keeps bullet rows on one
/// line so wrapped text cannot align under the bullet glyph.
fn clip(text: &str, cols: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    if cols == 0 {
        return String::new();
    }
    let mut w = 0usize;
    let mut out = String::new();
    for ch in text.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > cols {
            // Room for the ellipsis, popping back if need be.
            while w + 1 > cols {
                if let Some(prev) = out.pop() {
                    w -= UnicodeWidthChar::width(prev).unwrap_or(0);
                } else {
                    break;
                }
            }
            out.push('…');
            return out;
        }
        out.push(ch);
        w += cw;
    }
    out
}

/// Rows the title may occupy before it is ellipsized.
const TITLE_MAX_ROWS: usize = 2;

/// Word-wrap to `cols`, at most `max_rows` rows, last row ellipsized. The rail
/// wraps here rather than letting `Paragraph` do it so that one logical line is
/// always one visual row — [`trim_dangling`] counts rows, and a widget-wrapped
/// line would silently push a panel's content past the frame.
fn wrap_words(text: &str, cols: usize, max_rows: usize) -> Vec<String> {
    use unicode_width::UnicodeWidthStr;
    if cols == 0 || max_rows == 0 {
        return Vec::new();
    }
    let mut rows: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let candidate = if cur.is_empty() {
            word.to_string()
        } else {
            format!("{cur} {word}")
        };
        if candidate.width() <= cols {
            cur = candidate;
            continue;
        }
        if !cur.is_empty() {
            rows.push(std::mem::take(&mut cur));
            if rows.len() == max_rows {
                break;
            }
        }
        // A single word longer than the rail is clipped rather than split.
        if word.width() > cols {
            rows.push(clip(word, cols));
            if rows.len() == max_rows {
                break;
            }
        } else {
            cur = word.to_string();
        }
    }
    if rows.len() < max_rows && !cur.is_empty() {
        rows.push(cur);
    }
    // Anything that did not fit is signalled on the final row.
    let consumed: usize = rows.iter().map(|r| r.split_whitespace().count()).sum();
    if consumed < text.split_whitespace().count() {
        if let Some(last) = rows.last_mut() {
            *last = clip(&format!("{last} …"), cols);
        }
    }
    rows
}

/// Drop trailing blanks, then any heading left with nothing under it.
fn trim_dangling(mut lines: Vec<RailLine>) -> Vec<RailLine> {
    while lines
        .last()
        .is_some_and(|l| l.line.spans.iter().all(|s| s.content.trim().is_empty()))
    {
        lines.pop();
    }
    while lines.last().is_some_and(|l| l.heading) {
        lines.pop();
        while lines
            .last()
            .is_some_and(|l| l.line.spans.iter().all(|s| s.content.trim().is_empty()))
        {
            lines.pop();
        }
    }
    lines
}

fn row(label: &str, value: String) -> RailLine {
    plain(Line::from(vec![
        Span::styled(format!("{label} "), theme::dim()),
        Span::styled(value, theme::text()),
    ]))
}

fn top_lines(data: &RailData<'_>, cols: usize) -> Vec<RailLine> {
    let mut out: Vec<RailLine> = Vec::new();

    for row in wrap_words(data.title, cols, TITLE_MAX_ROWS) {
        out.push(plain(Line::from(Span::styled(
            row,
            theme::text().add_modifier(ratatui::style::Modifier::BOLD),
        ))));
    }
    if !data.title.is_empty() {
        out.push(blank());
    }

    // ---- Context ----
    out.push(heading("Context"));
    let u = data.usage;
    if u.context_max > 0 {
        let pct = u.context_percent.min(100);
        out.push(plain(Line::from(Span::styled(
            ctx_bar(pct, cols.saturating_sub(6).clamp(4, 18)),
            theme::accent(),
        ))));
        out.push(row(
            "  ",
            format!("{} / {}", fmt_k(u.context_used), fmt_k(u.context_max)),
        ));
        out.push(row("  ", format!("{pct}% used")));
    } else {
        out.push(row("  ", "not reported yet".into()));
    }
    if let Some(cost) = u.cost_label() {
        out.push(row("  ", format!("{cost} spent")));
    }
    out.push(blank());

    // ---- Session ----
    // `Chat` seeds the model with "…" until `session.info` lands, so an unknown
    // model is omitted rather than rendered as a lone ellipsis. If that leaves
    // the panel empty, `trim_dangling` drops the heading too.
    out.push(heading("Session"));
    if !matches!(data.model.trim(), "" | "…") {
        out.push(row(
            "  ",
            clip(
                &crate::ui::screens::chat::status::short_model(data.model),
                cols.saturating_sub(3),
            ),
        ));
    }
    if !data.git_branch.is_empty() {
        out.push(row("  ", format!("({})", data.git_branch)));
    }
    if data.tool_count > 0 || data.skill_count > 0 {
        out.push(row(
            "  ",
            format!("{} tools · {} skills", data.tool_count, data.skill_count),
        ));
    }
    out.push(blank());

    // ---- Agents ----
    // The closest honest analogue to the reference's Todo panel: real
    // `delegation.status` rows, each with the goal the agent was given.
    if !data.agents.is_empty() {
        out.push(heading("Agents"));
        for a in data.agents.iter().take(MAX_AGENTS) {
            let goal = if a.goal.is_empty() {
                a.id.clone()
            } else {
                a.goal.clone()
            };
            out.push(plain(Line::from(vec![
                Span::styled("• ", agent_style(&a.status)),
                Span::styled(clip(&goal, cols.saturating_sub(2)), theme::text()),
            ])));
            if !a.status.is_empty() {
                out.push(plain(Line::from(Span::styled(
                    format!("  {}", clip(&a.status, cols.saturating_sub(2))),
                    theme::dim(),
                ))));
            }
        }
        if data.agents.len() > MAX_AGENTS {
            out.push(plain(Line::from(Span::styled(
                format!("  +{} more · /agents", data.agents.len() - MAX_AGENTS),
                theme::dim(),
            ))));
        }
        out.push(blank());
    }

    out
}

/// Running agents read as accent; anything else stays quiet.
fn agent_style(status: &str) -> ratatui::style::Style {
    let s = status.to_ascii_lowercase();
    if s.contains("run") || s.contains("active") || s.contains("work") {
        theme::accent()
    } else {
        theme::dim()
    }
}

fn bottom_lines(data: &RailData<'_>) -> Vec<Line<'static>> {
    let mut out: Vec<Line> = Vec::new();
    if !data.cwd.is_empty() {
        out.push(Line::from(Span::styled(short_cwd(data.cwd), theme::dim())));
    }
    if !data.version.is_empty() {
        out.push(Line::from(vec![
            Span::styled("Talaria ", theme::dim()),
            Span::styled(data.version.to_string(), theme::dim()),
        ]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage() -> UsageSnapshot {
        UsageSnapshot {
            calls: 3,
            input: 100,
            output: 50,
            total: 150,
            context_used: 29_473,
            context_max: 200_000,
            context_percent: 15,
            cost_usd: Some(0.30),
            cost_status: None,
            model: "hermes-4-405b".into(),
            credits_lines: Vec::new(),
        }
    }

    fn data<'a>(agents: &'a [SubagentRow], u: &'a UsageSnapshot) -> RailData<'a> {
        RailData {
            title: "Implementing signup age-validate field",
            model: "hermes-4-405b",
            git_branch: "main",
            usage: u,
            agents,
            tool_count: 14,
            skill_count: 6,
            cwd: "/tmp/pocket",
            version: "0.1.6",
        }
    }

    fn flat(lines: &[RailLine]) -> String {
        lines
            .iter()
            .map(|l| {
                l.line
                    .spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn flat_plain(lines: &[Line<'_>]) -> String {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn width_gate_hides_the_rail_on_narrow_terminals() {
        // Independent of the pref: a narrow terminal never gets the rail.
        assert!(!visible(MIN_TOTAL_WIDTH - 1));
        assert!(!visible(80));
    }

    #[test]
    fn context_panel_reports_real_usage_only() {
        let u = usage();
        let text = flat(&top_lines(&data(&[], &u), 30));
        assert!(text.contains("Context"), "{text}");
        assert!(text.contains("15% used"), "{text}");
        assert!(text.contains("$0.30 spent"), "{text}");
        // Whatever fmt_k's exact casing, both sides of the ratio are present.
        assert!(text.contains("29.5K"), "{text}");
        assert!(text.contains("200K"), "{text}");
        // No panel we cannot source from the gateway.
        assert!(!text.contains("LSP"), "{text}");
        assert!(!text.to_lowercase().contains("todo"), "{text}");
    }

    #[test]
    fn missing_usage_says_so_instead_of_showing_zero_percent() {
        let mut u = usage();
        u.context_max = 0;
        u.context_percent = 0;
        let text = flat(&top_lines(&data(&[], &u), 30));
        assert!(text.contains("not reported yet"), "{text}");
        assert!(!text.contains("0% used"), "{text}");
    }

    #[test]
    fn agents_panel_lists_goals_and_summarizes_overflow() {
        let u = usage();
        let agents: Vec<SubagentRow> = (0..MAX_AGENTS + 2)
            .map(|i| SubagentRow {
                id: format!("a{i}"),
                goal: format!("goal {i}"),
                status: "running".into(),
            })
            .collect();
        let text = flat(&top_lines(&data(&agents, &u), 30));
        assert!(text.contains("Agents"), "{text}");
        assert!(text.contains("goal 0"), "{text}");
        assert!(text.contains("+2 more · /agents"), "{text}");
        assert!(
            !text.contains("goal 7"),
            "overflow must not be listed: {text}"
        );
    }

    #[test]
    fn agents_panel_absent_when_no_delegation() {
        let u = usage();
        let text = flat(&top_lines(&data(&[], &u), 30));
        assert!(!text.contains("Agents"), "{text}");
    }

    #[test]
    fn long_agent_goal_is_clipped_not_wrapped() {
        let u = usage();
        let agents = vec![SubagentRow {
            id: "a1".into(),
            goal: "Update UserFactory with date_of_birth and backfill every row".into(),
            status: "running".into(),
        }];
        let lines = top_lines(&data(&agents, &u), 24);
        let bullet = lines
            .iter()
            .find(|l| flat(std::slice::from_ref(l)).starts_with("• "))
            .expect("bullet row");
        let text = flat(std::slice::from_ref(bullet));
        assert!(text.ends_with('…'), "should ellipsize: {text:?}");
        assert!(
            unicode_width::UnicodeWidthStr::width(text.as_str()) <= 24,
            "row {text:?} overflows the rail"
        );
    }

    #[test]
    fn a_cut_body_does_not_leave_a_bare_heading() {
        let u = usage();
        // Exactly enough rows for the title, blank, and the "Context" heading.
        let cut = trim_dangling(
            top_lines(&data(&[], &u), 30)
                .into_iter()
                .take(4)
                .collect::<Vec<_>>(),
        );
        let text = flat(&cut);
        assert!(!text.trim_end().ends_with("Context"), "{text:?}");
        assert!(text.contains("Implementing"), "title survives: {text:?}");
    }

    #[test]
    fn unknown_model_drops_the_session_panel_instead_of_showing_an_ellipsis() {
        let u = usage();
        let mut d = data(&[], &u);
        d.model = "…";
        d.git_branch = "";
        d.tool_count = 0;
        d.skill_count = 0;
        let text = flat(&trim_dangling(top_lines(&d, 30)));
        assert!(!text.contains("Session"), "bare heading: {text:?}");
        assert!(!text.contains("   …"), "lone ellipsis row: {text:?}");
        assert!(text.contains("Context"), "context still shows: {text:?}");
    }

    #[test]
    fn footer_carries_cwd_and_version() {
        let u = usage();
        let text = flat_plain(&bottom_lines(&data(&[], &u)));
        assert!(text.contains("pocket"), "{text}");
        assert!(text.contains("0.1.6"), "{text}");
    }
}
