//! Multi-line composer. Shift+Enter inserts a newline; Enter submits.
//! History persists under `~/.hermes-rust/history` (not `~/.hermes`).

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::paths::{create_private_dir_all, HermesRustPaths};

const MAX_HISTORY: usize = 200;

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
        c.load_from(&HermesRustPaths::from_env().history_file());
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
        self.persist_to(&HermesRustPaths::from_env().history_file());
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

    pub fn submit(&mut self) -> Option<String> {
        let text = self.draft.trim().to_string();
        if text.is_empty() {
            return None;
        }
        self.history.push(text.clone());
        if self.history.len() > MAX_HISTORY {
            let extra = self.history.len() - MAX_HISTORY;
            self.history.drain(0..extra);
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

    pub fn visual_lines(&self) -> u16 {
        let n = self.draft.split('\n').count().max(1) as u16;
        n.clamp(3, 8)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ComposerAction {
        match key.code {
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.insert_char('\n');
                ComposerAction::None
            }
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
            KeyCode::Up => {
                self.history_up();
                ComposerAction::None
            }
            KeyCode::Down => {
                self.history_down();
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

#[derive(Debug)]
pub enum ComposerAction {
    None,
    Submit(String),
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let dir = std::env::temp_dir().join(format!("hermes-rust-hist-{}", std::process::id()));
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
}
