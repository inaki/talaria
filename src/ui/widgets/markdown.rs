//! Compact markdown → ratatui `Line`s. Good enough for assistant bubbles.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme;

pub fn markdown_to_lines(src: &str) -> Vec<Line<'static>> {
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
            Event::Start(Tag::CodeBlock(kind)) => {
                flush_line(&mut lines, &mut spans);
                in_code_block = true;
                let label = match kind {
                    CodeBlockKind::Fenced(info) => info.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                if !label.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("  ┌ {label}"),
                        theme::dim(),
                    )));
                } else {
                    lines.push(Line::from(Span::styled("  ┌", theme::dim())));
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                flush_line(&mut lines, &mut spans);
                in_code_block = false;
                lines.push(Line::from(Span::styled("  └", theme::dim())));
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
                    code.to_string(),
                    Style::default()
                        .fg(theme::TOOL())
                        .add_modifier(Modifier::ITALIC),
                ));
            }
            Event::Text(text) => {
                if in_code_block {
                    for raw in text.split('\n') {
                        flush_line(&mut lines, &mut spans);
                        spans.push(Span::styled(
                            format!("  │ {raw}"),
                            Style::default().fg(theme::TOOL()),
                        ));
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

fn heading_style(level: HeadingLevel) -> Style {
    let base = Style::default()
        .fg(theme::PRIMARY())
        .add_modifier(Modifier::BOLD);
    match level {
        HeadingLevel::H1 | HeadingLevel::H2 => base,
        _ => Style::default()
            .fg(theme::TEXT())
            .add_modifier(Modifier::BOLD),
    }
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

    #[test]
    fn renders_heading_and_code() {
        let lines = markdown_to_lines("# Hi\n\nUse `cargo test`.\n");
        let joined: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(joined.contains("Hi"));
        assert!(joined.contains("cargo test"));
    }
}
