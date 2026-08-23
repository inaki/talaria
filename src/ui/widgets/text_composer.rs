//! Multi-line composer. Shift+Enter inserts a newline; Enter submits.
//! History persists under `~/.talaria/history` (not `~/.hermes`).

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::paths::{create_private_dir_all, ensure_private_file, TalariaPaths};
use crate::ui::keys::is_newline_key;

const MAX_HISTORY: usize = 200;
const MAX_VISUAL_LINES: u16 = 6;

#[derive(Debug, Default)]
pub struct TextComposer {
    pub draft: String,
    pub cursor_pos: usize,
    history: Vec<String>,
    history_index: Option<usize>,
    stash: String,
}

impl TextComposer {
    pub fn with_persisted_history() -> Self {
        let mut c = Self::default();
        c.load_from(&TalariaPaths::from_env().history_file());
        c
    }

    pub fn load_from(&mut self, path: &Path) {
        let Ok(file) = std::fs::File::open(path) else {
            return;
        };
        let mut lines: Vec<String> = BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter(|l| !l.trim().is_empty())
            .collect();
        if lines.len() > MAX_HISTORY {
            lines.drain(0..lines.len() - MAX_HISTORY);
        }
        self.history = lines;
    }

    pub fn persist(&self) {
        self.persist_to(&TalariaPaths::from_env().history_file());
    }

    pub fn persist_to(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            create_private_dir_all(parent);
        }
        let mut opts = OpenOptions::new();
        opts.create(true).write(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let Ok(mut file) = opts.open(path) else {
            return;
        };
        ensure_private_file(path);
        let start = self.history.len().saturating_sub(MAX_HISTORY);
        for line in &self.history[start..] {
            let _ = writeln!(file, "{line}");
        }
    }

    pub fn clear(&mut self) {
        self.draft.clear();
        self.cursor_pos = 0;
        self.history_index = None;
        self.stash.clear();
    }

    pub fn insert_char(&mut self, c: char) {
        self.draft.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
        self.history_index = None;
    }

    pub fn insert_str(&mut self, s: &str) {
        self.draft.insert_str(self.cursor_pos, s);
        self.cursor_pos += s.len();
        self.history_index = None;
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let prev = self.draft[..self.cursor_pos]
            .chars()
            .next_back()
            .map(|c| c.len_utf8())
            .unwrap_or(0);
        let start = self.cursor_pos - prev;
        self.draft.replace_range(start..self.cursor_pos, "");
        self.cursor_pos = start;
    }

    pub fn delete(&mut self) {
        if self.cursor_pos >= self.draft.len() {
            return;
        }
        let next = self.draft[self.cursor_pos..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(0);
        self.draft
            .replace_range(self.cursor_pos..self.cursor_pos + next, "");
    }

    pub fn move_left(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let prev = self.draft[..self.cursor_pos]
            .chars()
            .next_back()
            .map(|c| c.len_utf8())
            .unwrap_or(0);
        self.cursor_pos -= prev;
    }

    pub fn move_right(&mut self) {
        if self.cursor_pos >= self.draft.len() {
            return;
        }
        let next = self.draft[self.cursor_pos..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(0);
        self.cursor_pos += next;
    }

    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    pub fn offset_to_line_col(&self) -> (usize, usize) {
        offset_to_line_col(&self.draft, self.cursor_pos)
    }

    pub fn move_line(&mut self, delta: i32) {
        self.cursor_pos = move_cursor_line(&self.draft, self.cursor_pos, delta);
    }

    fn on_first_line(&self) -> bool {
        !self.draft[..self.cursor_pos].contains('\n')
    }

    fn on_last_line(&self) -> bool {
        !self.draft[self.cursor_pos..].contains('\n')
    }

    pub fn submit(&mut self) -> Option<String> {
        let text = self.draft.trim().to_string();
        if text.is_empty() {
            return None;
        }
        if !crate::logging::should_skip_history(&text) {
            self.history.push(text.clone());
            if self.history.len() > MAX_HISTORY {
                let extra = self.history.len() - MAX_HISTORY;
                self.history.drain(0..extra);
            }
        }
        self.clear();
        Some(text)
    }

    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        match self.history_index {
            None => {
                self.stash = self.draft.clone();
                self.history_index = Some(self.history.len() - 1);
            }
            Some(0) => return,
            Some(i) => self.history_index = Some(i - 1),
        }
        if let Some(i) = self.history_index {
            self.draft = self.history[i].clone();
            self.cursor_pos = self.draft.len();
        }
    }

    pub fn history_down(&mut self) {
        let Some(i) = self.history_index else { return };
        if i + 1 >= self.history.len() {
            self.history_index = None;
            self.draft = self.stash.clone();
            self.cursor_pos = self.draft.len();
            return;
        }
        self.history_index = Some(i + 1);
        self.draft = self.history[i + 1].clone();
        self.cursor_pos = self.draft.len();
    }

    /// Content rows inside the prompt box, including soft-wrap at `width`.
    /// Hard newlines (Shift+Enter) and overflow both grow up to `MAX_VISUAL_LINES`.
    pub fn visual_lines(&self, width: u16) -> u16 {
        visual_line_count(&self.draft, width).min(MAX_VISUAL_LINES)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ComposerAction {
        if is_newline_key(&key) {
            self.insert_newline();
            return ComposerAction::None;
        }
        match key.code {
            KeyCode::Enter => {
                if let Some(text) = self.submit() {
                    self.persist();
                    ComposerAction::Submit(text)
                } else {
                    ComposerAction::None
                }
            }
            KeyCode::Backspace => {
                self.backspace();
                ComposerAction::None
            }
            KeyCode::Delete => {
                self.delete();
                ComposerAction::None
            }
            KeyCode::Left => {
                self.move_left();
                ComposerAction::None
            }
            KeyCode::Right => {
                self.move_right();
                ComposerAction::None
            }
            KeyCode::Home => {
                let (line, col) = self.offset_to_line_col();
                self.cursor_pos -= col;
                let _ = line;
                ComposerAction::None
            }
            KeyCode::End => {
                let (line, col) = self.offset_to_line_col();
                let line_text = self.draft.split('\n').nth(line).unwrap_or("");
                self.cursor_pos += line_text.len().saturating_sub(col);
                ComposerAction::None
            }
            KeyCode::Up => {
                if self.draft.contains('\n') && !self.on_first_line() {
                    self.move_line(-1);
                } else {
                    self.history_up();
                }
                ComposerAction::None
            }
            KeyCode::Down => {
                if self.draft.contains('\n') && !self.on_last_line() {
                    self.move_line(1);
                } else {
                    self.history_down();
                }
                ComposerAction::None
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.insert_char(c);
                ComposerAction::None
            }
            _ => ComposerAction::None,
        }
    }
}

/// Byte ranges `[start, end)` of `s` split to fit `width` display columns.
pub fn wrap_chunks(s: &str, width: usize) -> Vec<(usize, usize)> {
    if s.is_empty() {
        return vec![(0, 0)];
    }
    let width = width.max(1);
    let mut out = Vec::new();
    let mut start = 0;
    let mut col = 0;
    for (i, ch) in s.char_indices() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if col + cw > width && i > start {
            out.push((start, i));
            start = i;
            col = 0;
        }
        col = col.saturating_add(cw);
    }
    out.push((start, s.len()));
    out
}

pub fn visual_line_count(text: &str, width: u16) -> u16 {
    let w = (width as usize).max(1);
    let mut n = 0u16;
    for line in text.split('\n') {
        n = n.saturating_add(wrap_chunks(line, w).len() as u16);
    }
    n.max(1)
}

pub fn offset_to_line_col(text: &str, offset: usize) -> (usize, usize) {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut rem = offset;
    for (i, line) in lines.iter().enumerate() {
        let len = line.len();
        if rem <= len || i == lines.len() - 1 {
            return (i, rem.min(len));
        }
        rem -= len + 1;
    }
    (0, 0)
}

pub fn move_cursor_line(text: &str, pos: usize, delta: i32) -> usize {
    let lines: Vec<&str> = text.split('\n').collect();
    let (line, col) = offset_to_line_col(text, pos);
    let target = line as i32 + delta;
    if target < 0 || target as usize >= lines.len() {
        return pos;
    }
    let target_col = col.min(lines[target as usize].len());
    lines[..target as usize]
        .iter()
        .map(|l| l.len() + 1)
        .sum::<usize>()
        + target_col
}

#[derive(Debug)]
pub enum ComposerAction {
    None,
    Submit(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newline_key_grows_and_up_down_move_between_lines() {
        use crate::ui::keys::is_newline_key;
        use crossterm::event::KeyEvent;
        let shift_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT);
        assert!(is_newline_key(&shift_enter));
        let mut c = TextComposer::default();
        c.insert_str("ab");
        c.handle_key(shift_enter);
        assert_eq!(c.draft, "ab\n");
        assert_eq!(c.visual_lines(80), 2);
        c.insert_str("cd");
        assert_eq!(offset_to_line_col(&c.draft, c.cursor_pos), (1, 2));
        c.move_line(-1);
        assert_eq!(offset_to_line_col(&c.draft, c.cursor_pos), (0, 2));
        c.move_line(1);
        assert_eq!(offset_to_line_col(&c.draft, c.cursor_pos), (1, 2));
    }

    #[test]
    fn visual_lines_start_at_one_and_grow_with_newlines() {
        let mut c = TextComposer::default();
        assert_eq!(c.visual_lines(80), 1);
        c.insert_str("hello");
        assert_eq!(c.visual_lines(80), 1);
        c.insert_char('\n');
        assert_eq!(c.visual_lines(80), 2);
        for _ in 0..10 {
            c.insert_char('\n');
        }
        assert_eq!(c.visual_lines(80), MAX_VISUAL_LINES);
    }

    #[test]
    fn visual_lines_wrap_a_long_line() {
        let mut c = TextComposer::default();
        c.insert_str(&"x".repeat(25));
        assert_eq!(c.visual_lines(10), 3);
        assert_eq!(wrap_chunks("abcdefghij", 4), vec![(0, 4), (4, 8), (8, 10)]);
    }

    #[test]
    fn insert_and_backspace() {
        let mut c = TextComposer::default();
        c.insert_str("hi");
        assert_eq!(c.draft, "hi");
        c.backspace();
        assert_eq!(c.draft, "h");
        assert_eq!(c.cursor_pos, 1);
    }

    #[test]
    fn history_roundtrip() {
        let mut c = TextComposer::default();
        c.insert_str("one");
        assert_eq!(c.submit().as_deref(), Some("one"));
        c.history_up();
        assert_eq!(c.draft, "one");
        c.history_down();
        assert_eq!(c.draft, "");
    }

    #[test]
    fn persist_roundtrip_file() {
        let dir = std::env::temp_dir().join(format!("talaria-hist-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("history");
        let mut c = TextComposer::default();
        c.history.push("alpha".into());
        c.history.push("beta".into());
        c.persist_to(&path);
        let mut loaded = TextComposer::default();
        loaded.load_from(&path);
        assert_eq!(loaded.history, vec!["alpha", "beta"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn submit_skips_bang_and_secret_history() {
        let mut c = TextComposer::default();
        c.insert_str("! pwd");
        assert_eq!(c.submit().as_deref(), Some("! pwd"));
        c.insert_str("api_key=sk-live-abcdefghijk");
        assert_eq!(c.submit().as_deref(), Some("api_key=sk-live-abcdefghijk"));
        c.insert_str("list the files");
        assert_eq!(c.submit().as_deref(), Some("list the files"));
        assert_eq!(c.history, vec!["list the files"]);
    }
}
