//! Compact markdown → ratatui `Line`s. Good enough for assistant bubbles.
//!
//! Fenced code is a full rectangle (hairline border, surface fill, mint type),
//! not `┌ │ └` prefixes on a bare canvas.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::theme;

pub fn markdown_to_lines(src: &str) -> Vec<Line<'static>> {
    markdown_to_lines_at(src, 72)
}

pub fn markdown_to_lines_at(src: &str, width: usize) -> Vec<Line<'static>> {
    if src.trim().is_empty() {
        return Vec::new();
    }
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(src, opts);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut style = Style::default().fg(theme::TEXT());
    let mut list_depth: u8 = 0;
    let mut in_code_block = false;
    let mut code_buf: Vec<String> = Vec::new();
    let mut code_acc = String::new();

    for ev in parser {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                flush_line(&mut lines, &mut spans);
                style = heading_style(level);
            }
            Event::End(TagEnd::Heading(_)) => {
                flush_line(&mut lines, &mut spans);
                style = Style::default().fg(theme::TEXT());
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                flush_line(&mut lines, &mut spans);
                lines.push(Line::from(""));
            }
            Event::Start(Tag::List(_)) => {
                list_depth = list_depth.saturating_add(1);
            }
            Event::End(TagEnd::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
                flush_line(&mut lines, &mut spans);
            }
            Event::Start(Tag::Item) => {
                flush_line(&mut lines, &mut spans);
                let indent = "  ".repeat(list_depth.saturating_sub(1) as usize);
                spans.push(Span::styled(
                    format!("{indent}• "),
                    Style::default().fg(theme::PRIMARY()),
                ));
            }
            Event::End(TagEnd::Item) => {
                flush_line(&mut lines, &mut spans);
            }
            Event::Start(Tag::CodeBlock(_)) => {
                flush_line(&mut lines, &mut spans);
                in_code_block = true;
                code_buf.clear();
                code_acc.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                if !code_acc.is_empty() || !code_buf.is_empty() {
                    code_buf.push(std::mem::take(&mut code_acc));
                }
                while code_buf.last().is_some_and(|s| s.is_empty()) {
                    code_buf.pop();
                }
                lines.extend(code_fence(&code_buf, width));
                code_buf.clear();
                in_code_block = false;
            }
            Event::Start(Tag::Emphasis) => {
                style = style.add_modifier(Modifier::ITALIC);
            }
            Event::End(TagEnd::Emphasis) => {
                style = style.remove_modifier(Modifier::ITALIC);
            }
            Event::Start(Tag::Strong) => {
                style = style.add_modifier(Modifier::BOLD);
            }
            Event::End(TagEnd::Strong) => {
                style = style.remove_modifier(Modifier::BOLD);
            }
            Event::Start(Tag::Strikethrough) => {
                style = style.add_modifier(Modifier::CROSSED_OUT);
            }
            Event::End(TagEnd::Strikethrough) => {
                style = style.remove_modifier(Modifier::CROSSED_OUT);
            }
            Event::Code(code) => {
                spans.push(Span::styled(
                    format!(" {code} "),
                    Style::default().fg(theme::PRIMARY()).bg(theme::SURFACE()),
                ));
            }
            Event::Text(text) => {
                if in_code_block {
                    let mut parts = text.split('\n');
                    if let Some(first) = parts.next() {
                        code_acc.push_str(first);
                    }
                    for rest in parts {
                        code_buf.push(std::mem::take(&mut code_acc));
                        code_acc.push_str(rest);
                    }
                } else {
                    spans.push(Span::styled(text.to_string(), style));
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                flush_line(&mut lines, &mut spans);
            }
            Event::Rule => {
                flush_line(&mut lines, &mut spans);
                lines.push(Line::from(Span::styled("  ────────", theme::dim())));
            }
            Event::Start(Tag::Link { .. }) => {
                style = style
                    .fg(theme::PRIMARY())
                    .add_modifier(Modifier::UNDERLINED);
            }
            Event::End(TagEnd::Link) => {
                style = Style::default().fg(theme::TEXT());
            }
            _ => {}
        }
    }
    flush_line(&mut lines, &mut spans);
    while lines.last().is_some_and(|l| l.spans.is_empty()) {
        lines.pop();
    }
    lines
}

/// Hairline rectangle, surface fill, mint type. `width` is the total columns
/// of the box (including the two vertical bars).
fn code_fence(body: &[String], width: usize) -> Vec<Line<'static>> {
    let box_w = width.max(16);
    let inner = box_w.saturating_sub(2).max(8);
    let border = Style::default().fg(theme::SEPARATOR()).bg(theme::SURFACE());
    let code = Style::default().fg(theme::PRIMARY()).bg(theme::SURFACE());
    let mut out = Vec::with_capacity(body.len() + 4);
    out.push(fence_edge('┌', '┐', inner, border));
    out.push(fence_row("", inner, border, code));
    for line in body {
        out.push(fence_row(line, inner, border, code));
    }
    out.push(fence_row("", inner, border, code));
    out.push(fence_edge('└', '┘', inner, border));
    out
}

fn fence_edge(left: char, right: char, inner: usize, style: Style) -> Line<'static> {
    Line::from(Span::styled(
        format!("{left}{}{right}", "─".repeat(inner)),
        style,
    ))
}

fn fence_row(src: &str, inner: usize, border: Style, code: Style) -> Line<'static> {
    let mut body = format!("  {src}");
    let mut w = UnicodeWidthStr::width(body.as_str());
    if w > inner {
        body = ellipsize(&body, inner);
        w = UnicodeWidthStr::width(body.as_str());
    }
    if w < inner {
        body.push_str(&" ".repeat(inner - w));
    }
    Line::from(vec![
        Span::styled("│", border),
        Span::styled(body, code),
        Span::styled("│", border),
    ])
}

fn ellipsize(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if UnicodeWidthStr::width(s) <= max {
        return s.to_string();
    }
    let keep = max.saturating_sub(1);
    let mut out = String::new();
    let mut w = 0usize;
    for ch in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > keep {
            break;
        }
        out.push(ch);
        w += cw;
    }
    out.push('…');
    out
}

fn heading_style(_level: HeadingLevel) -> Style {
    Style::default()
        .fg(theme::TEXT())
        .add_modifier(Modifier::BOLD)
}

fn flush_line(lines: &mut Vec<Line<'static>>, spans: &mut Vec<Span<'static>>) {
    if spans.is_empty() {
        return;
    }
    lines.push(Line::from(std::mem::take(spans)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visible(lines: &[Line]) -> String {
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
    fn renders_heading_and_code() {
        let lines = markdown_to_lines("# Hi\n\nUse `cargo test`.\n");
        let joined = visible(&lines);
        assert!(joined.contains("Hi"));
        assert!(joined.contains("cargo test"));
    }

    #[test]
    fn fenced_code_is_a_rectangle() {
        let src = "```rust\npub async fn verify_token() {}\n\n// comment\n```\n";
        let lines = markdown_to_lines_at(src, 40);
        let t = visible(&lines);
        assert!(t.lines().next().unwrap().starts_with('┌'), "{t}");
        assert!(t.lines().next().unwrap().ends_with('┐'), "{t}");
        assert!(t.contains("pub async fn verify_token() {}"), "{t}");
        assert!(t.contains("// comment"), "{t}");
        assert!(t.lines().last().unwrap().starts_with('└'), "{t}");
        assert!(t.lines().last().unwrap().ends_with('┘'), "{t}");
        assert!(
            lines.iter().all(|l| l.width() == 40),
            "{:?}",
            lines.iter().map(|l| l.width()).collect::<Vec<_>>()
        );
        assert!(
            lines.iter().all(|l| l.style.bg.is_none()
                && l.spans.iter().all(|s| s.style.bg == Some(theme::SURFACE()))),
            "code box sits on surface fill"
        );
    }
}
