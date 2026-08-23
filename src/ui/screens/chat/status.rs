//! Composer status strip: model, context, clocks. Progressive by width.

use std::time::Duration;

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::session::UsageSnapshot;
use crate::theme;

pub struct Meter {
    pub model: String,
    pub usage: UsageSnapshot,
    pub session: Duration,
    pub turn: Option<Duration>,
    pub turn_live: bool,
    pub idle: Option<Duration>,
    pub right: String,
    /// Live spinner glyph (thinking / streaming / tools).
    pub spinner: Option<&'static str>,
}

pub fn line(meter: &Meter, width: u16) -> Line<'static> {
    let cols = width as usize;
    let model = short_model(&meter.model);
    let mut left: Vec<Seg> = Vec::new();
    if let Some(g) = meter.spinner {
        left.push(Seg::text(g, theme::accent()));
    }
    left.push(Seg::text(model, theme::accent()));

    if meter.usage.context_max > 0 {
        left.push(Seg::text(
            format!(
                "{} / {}",
                fmt_k(meter.usage.context_used),
                fmt_k(meter.usage.context_max)
            ),
            theme::dim(),
        ));
        let pct = meter.usage.context_percent.min(100);
        let bar = ctx_bar(pct, 10);
        left.push(Seg::spans(vec![
            Span::styled(format!("[{bar}]"), ctx_style(pct)),
            Span::styled(format!(" {pct}%"), ctx_style(pct)),
        ]));
    } else if meter.usage.total > 0 {
        left.push(Seg::text(
            format!("{} tok", fmt_k(meter.usage.total)),
            theme::dim(),
        ));
    }

    if !meter.session.is_zero() || meter.turn.is_some() {
        left.push(Seg::text(
            fmt_duration(meter.session.as_secs()),
            theme::dim(),
        ));
    }
    if let Some(turn) = meter.turn {
        let label = format!("⏱ {}", fmt_duration(turn.as_secs()));
        left.push(Seg::text(
            label,
            if meter.turn_live {
                theme::accent()
            } else {
                theme::dim()
            },
        ));
    }
    if let Some(idle) = meter.idle {
        left.push(Seg::text(
            format!("✓ {}", fmt_duration(idle.as_secs())),
            theme::dim(),
        ));
    }

    let right = if meter.right.is_empty() {
        None
    } else {
        Some(Seg::text(meter.right.clone(), theme::accent()))
    };
    let right_w = right.as_ref().map(|s| s.width).unwrap_or(0);
    let sep_w = if right_w > 0 { 2 } else { 0 };

    while left.len() > 1 {
        let used = joined_width(&left);
        if used + sep_w + right_w <= cols {
            break;
        }
        left.pop();
    }

    let mut spans = join_spans(&left);
    let used = joined_width(&left);
    if let Some(r) = right {
        if used + sep_w + r.width <= cols {
            let pad = cols.saturating_sub(used).saturating_sub(r.width);
            if pad > 0 {
                spans.push(Span::raw(" ".repeat(pad)));
            }
            spans.extend(r.spans);
        }
    }
    Line::from(spans)
}

struct Seg {
    spans: Vec<Span<'static>>,
    width: usize,
}

impl Seg {
    fn text(s: impl Into<String>, style: Style) -> Self {
        let s = s.into();
        let width = s.width();
        Self {
            spans: vec![Span::styled(s, style)],
            width,
        }
    }

    fn spans(spans: Vec<Span<'static>>) -> Self {
        let width = spans.iter().map(|s| s.content.width()).sum();
        Self { spans, width }
    }
}

fn joined_width(segs: &[Seg]) -> usize {
    if segs.is_empty() {
        return 0;
    }
    segs.iter().map(|s| s.width).sum::<usize>() + 2 * segs.len().saturating_sub(1)
}

fn join_spans(segs: &[Seg]) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    for (i, s) in segs.iter().enumerate() {
        if i > 0 {
            out.push(Span::styled("  ", theme::dim()));
        }
        out.extend(s.spans.clone());
    }
    out
}

pub fn fmt_k(n: u64) -> String {
    if n >= 1_000_000 {
        trim_one(n as f64 / 1_000_000.0, "M")
    } else if n >= 1_000 {
        trim_one(n as f64 / 1_000.0, "K")
    } else {
        n.to_string()
    }
}

fn trim_one(v: f64, suffix: &str) -> String {
    let s = format!("{v:.1}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{s}{suffix}")
}

pub fn fmt_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

pub fn ctx_bar(pct: u64, w: usize) -> String {
    if w == 0 {
        return String::new();
    }
    let p = pct.min(100) as usize;
    let filled = (p * w + 50) / 100;
    let filled = filled.min(w);
    format!("{}{}", "█".repeat(filled), "░".repeat(w - filled))
}

fn ctx_style(pct: u64) -> Style {
    if pct >= 80 {
        theme::error()
    } else if pct >= 50 {
        Style::default().fg(theme::WARNING())
    } else {
        Style::default().fg(theme::SUCCESS())
    }
}

pub fn short_model(model: &str) -> String {
    model
        .rsplit(['/', ':'])
        .next()
        .unwrap_or(model)
        .replace('_', "-")
}

pub fn short_cwd(cwd: &str) -> String {
    let home = std::env::var("HOME").ok();
    let s = match home {
        Some(h) if !h.is_empty() && cwd.starts_with(&h) => {
            format!("~{}", &cwd[h.len()..])
        }
        _ => cwd.to_string(),
    };
    let n = s.chars().count();
    if n <= 28 {
        s
    } else {
        let tail: String = s.chars().skip(n - 27).collect();
        format!("…{tail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_counts() {
        assert_eq!(fmt_k(0), "0");
        assert_eq!(fmt_k(999), "999");
        assert_eq!(fmt_k(1_000), "1K");
        assert_eq!(fmt_k(25_700), "25.7K");
        assert_eq!(fmt_k(1_000_000), "1M");
    }

    #[test]
    fn durations() {
        assert_eq!(fmt_duration(0), "0s");
        assert_eq!(fmt_duration(56), "56s");
        assert_eq!(fmt_duration(75), "1m 15s");
        assert_eq!(fmt_duration(9 * 3600 + 15 * 60), "9h 15m");
    }

    #[test]
    fn bar_fills() {
        assert_eq!(ctx_bar(0, 10), "░░░░░░░░░░");
        assert_eq!(ctx_bar(100, 10), "██████████");
        assert_eq!(ctx_bar(2, 10).chars().filter(|c| *c == '█').count(), 0);
        assert_eq!(ctx_bar(50, 10).chars().filter(|c| *c == '█').count(), 5);
    }

    #[test]
    fn model_shortens() {
        assert_eq!(short_model("stealth/ox-alpha"), "ox-alpha");
        assert_eq!(short_model("ox-alpha"), "ox-alpha");
    }

    #[test]
    fn meter_keeps_model_on_narrow() {
        let meter = Meter {
            model: "stealth/ox-alpha".into(),
            usage: UsageSnapshot {
                context_used: 25_700,
                context_max: 1_000_000,
                context_percent: 2,
                ..Default::default()
            },
            session: Duration::from_secs(9 * 3600 + 15 * 60),
            turn: Some(Duration::from_secs(56)),
            turn_live: false,
            idle: Some(Duration::from_secs(8 * 3600 + 32 * 60)),
            right: "Get assistant name".into(),
            spinner: None,
        };
        let wide = line(&meter, 120);
        let text: String = wide.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("ox-alpha"), "{text}");
        assert!(text.contains("25.7K"), "{text}");
        assert!(text.contains("2%"), "{text}");
        assert!(text.contains("9h 15m"), "{text}");
        assert!(text.contains("56s"), "{text}");
        assert!(text.contains("8h 32m"), "{text}");

        let narrow = line(&meter, 20);
        let t: String = narrow.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(t.contains("ox-alpha"), "{t}");
        assert!(!t.contains("8h 32m"), "{t}");
    }

    #[test]
    fn meter_spinner_sits_before_model() {
        let meter = Meter {
            model: "ox-alpha".into(),
            usage: UsageSnapshot::default(),
            session: Duration::from_secs(1),
            turn: None,
            turn_live: true,
            idle: None,
            right: String::new(),
            spinner: Some("◐"),
        };
        let text: String = line(&meter, 40)
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        let spin = text.find('◐').expect(&text);
        let model = text.find("ox-alpha").expect(&text);
        assert!(spin < model, "{text}");
    }
}
