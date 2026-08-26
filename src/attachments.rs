//! Dropped / pasted image paths become `[Image #n]` placeholders in the composer.
//!
//! A terminal drag-and-drop arrives as a bracketed paste of the *shell-escaped*
//! path — `~/Library/Application\ Support/…/CleanShot\ 2026-08-26\ at\ 10.34.26@2x.png`.
//! Escaped spaces are why plain "no spaces allowed" path sniffing misses drops.
//!
//! Numbering is per-turn: the label the user sees (`[Image #1]`) is the same
//! ordinal the gateway receives as `image.attach`'s `label`, so the model can
//! resolve "the second image" without the path ever entering the prompt text.

/// One attachment held for the turn being composed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// 1-based, matches the `[Image #n]` placeholder in the draft.
    pub index: usize,
    /// Unescaped absolute path handed to `image.attach`.
    pub path: String,
}

impl Attachment {
    pub fn label(&self) -> String {
        placeholder(self.index)
    }
}

/// Placeholder text inserted into the composer.
pub fn placeholder(index: usize) -> String {
    format!("[Image #{index}]")
}

/// Per-turn attachment list. Cleared when the prompt is submitted.
#[derive(Debug, Clone, Default)]
pub struct Attachments {
    items: Vec<Attachment>,
}

impl Attachments {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn items(&self) -> &[Attachment] {
        &self.items
    }

    /// Register `path` and return its placeholder. Same path twice still gets a
    /// new ordinal — the user dropped it twice and expects two references.
    pub fn push(&mut self, path: String) -> Attachment {
        let index = self.items.len() + 1;
        let a = Attachment { index, path };
        self.items.push(a.clone());
        a
    }

    /// Path behind `[Image #n]`, for resolving a reference back to a file.
    pub fn path_for(&self, index: usize) -> Option<&str> {
        self.items
            .iter()
            .find(|a| a.index == index)
            .map(|a| a.path.as_str())
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

const IMAGE_EXTS: &[&str] = &[".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp", ".heic"];

fn has_image_ext(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    IMAGE_EXTS.iter().any(|ext| lower.ends_with(ext))
}

/// Image paths in one paste, in drop order. Empty when the paste is ordinary text.
///
/// Every token must be an image path — a paste mixing prose and a filename is
/// prose, and replacing part of it with a placeholder would eat the user's words.
pub fn image_paths_from_paste(text: &str) -> Vec<String> {
    let tokens = split_escaped(text);
    if tokens.is_empty() {
        return Vec::new();
    }
    if !tokens.iter().all(|t| has_image_ext(t)) {
        return Vec::new();
    }
    tokens
}

/// Split a paste on *unescaped* whitespace, honouring `\ `, `'…'`, `"…"` and
/// `file://`. Multi-file drops arrive as several escaped paths on one line.
fn split_escaped(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for ch in text.trim().chars() {
        if escaped {
            cur.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' if quote != Some('\'') => escaped = true,
            '\'' | '"' => match quote {
                Some(q) if q == ch => quote = None,
                Some(_) => cur.push(ch),
                None => quote = Some(ch),
            },
            c if c.is_whitespace() && quote.is_none() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    // A trailing lone backslash is a literal, not a dangling escape.
    if escaped {
        cur.push('\\');
    }
    if !cur.is_empty() {
        out.push(cur);
    }

    out.into_iter().filter_map(normalize_path).collect()
}

/// Strip a `file://` scheme and percent-decode it. Non-paths return `None`.
fn normalize_path(token: String) -> Option<String> {
    let t = token.trim().to_string();
    if t.is_empty() {
        return None;
    }
    if let Some(rest) = t.strip_prefix("file://") {
        // file:///Users/… — the empty authority is the local host.
        let rest = rest.strip_prefix("localhost").unwrap_or(rest);
        if !rest.starts_with('/') {
            return None;
        }
        return Some(percent_decode(rest));
    }
    Some(t)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(b) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragged_finder_path_with_escaped_spaces() {
        let dropped = r"/Users/inaki/Library/Application\ Support/CleanShot/media/media_GZJVilOqsS/CleanShot\ 2026-08-26\ at\ 10.34.26@2x.png";
        assert_eq!(
            image_paths_from_paste(dropped),
            vec![
                "/Users/inaki/Library/Application Support/CleanShot/media/media_GZJVilOqsS/CleanShot 2026-08-26 at 10.34.26@2x.png"
            ]
        );
    }

    #[test]
    fn plain_and_quoted_paths() {
        assert_eq!(image_paths_from_paste("/tmp/a.png"), vec!["/tmp/a.png"]);
        assert_eq!(
            image_paths_from_paste("'/tmp/my shot.png'"),
            vec!["/tmp/my shot.png"]
        );
        assert_eq!(
            image_paths_from_paste("\"/tmp/my shot.jpeg\""),
            vec!["/tmp/my shot.jpeg"]
        );
    }

    #[test]
    fn multi_file_drop_keeps_order() {
        let dropped = r"/tmp/one.png /tmp/two\ shot.jpg";
        assert_eq!(
            image_paths_from_paste(dropped),
            vec!["/tmp/one.png", "/tmp/two shot.jpg"]
        );
    }

    #[test]
    fn file_url_is_decoded() {
        assert_eq!(
            image_paths_from_paste("file:///tmp/my%20shot.png"),
            vec!["/tmp/my shot.png"]
        );
    }

    #[test]
    fn prose_and_mixed_pastes_are_not_attachments() {
        assert!(image_paths_from_paste("look at this").is_empty());
        assert!(image_paths_from_paste("").is_empty());
        assert!(image_paths_from_paste("/tmp/notes.md").is_empty());
        // Prose mentioning a file stays prose.
        assert!(image_paths_from_paste("check /tmp/a.png please").is_empty());
        // Newline-separated prose is not a drop.
        assert!(image_paths_from_paste("hello\nworld").is_empty());
    }

    #[test]
    fn newline_separated_drop_is_accepted() {
        assert_eq!(
            image_paths_from_paste("/tmp/a.png\n/tmp/b.png"),
            vec!["/tmp/a.png", "/tmp/b.png"]
        );
    }

    #[test]
    fn placeholders_number_from_one_and_reset() {
        let mut a = Attachments::default();
        assert_eq!(a.push("/tmp/a.png".into()).label(), "[Image #1]");
        assert_eq!(a.push("/tmp/b.png".into()).label(), "[Image #2]");
        assert_eq!(a.len(), 2);
        assert_eq!(a.path_for(2), Some("/tmp/b.png"));
        assert_eq!(a.path_for(3), None);
        a.clear();
        assert!(a.is_empty());
        assert_eq!(a.push("/tmp/c.png".into()).label(), "[Image #1]");
    }

    #[test]
    fn same_path_twice_gets_two_ordinals() {
        let mut a = Attachments::default();
        assert_eq!(a.push("/tmp/a.png".into()).index, 1);
        assert_eq!(a.push("/tmp/a.png".into()).index, 2);
    }

    #[test]
    fn trailing_backslash_is_literal_so_not_an_image() {
        // A dangling escape keeps the backslash, which no longer ends in an
        // image extension — the paste stays prose rather than becoming a
        // half-parsed attachment.
        assert!(image_paths_from_paste(r"/tmp/a.png\").is_empty());
    }
}
