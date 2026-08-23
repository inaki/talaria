use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn is_ctrl_c(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL))
}

pub fn is_ctrl_v(key: &KeyEvent) -> bool {
    matches!(
        key.code,
        KeyCode::Char('v')
            if key.modifiers.contains(KeyModifiers::CONTROL)
                || key.modifiers.contains(KeyModifiers::SUPER)
    )
}

pub fn is_ctrl_o(key: &KeyEvent) -> bool {
    matches!(
        key.code,
        KeyCode::Char('o')
            if key.modifiers.contains(KeyModifiers::CONTROL)
                || key.modifiers.contains(KeyModifiers::SUPER)
    )
}

pub fn is_ctrl_d(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL))
}

pub fn is_ctrl_g(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL))
}

pub fn is_ctrl_u(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL))
}

/// Insert a newline rather than submit.
///
/// Terminals disagree on Shift+Enter:
/// * `Enter` + SHIFT — kitty-protocol (`CSI 13;2u`)
/// * `Enter` + ALT — some macOS terminals
/// * `Ctrl+J` — Ghostty / others send LF for Shift+Enter; raw-mode crossterm
///   surfaces that as `Char('j')` + CONTROL, not `Enter`
pub fn is_newline_key(key: &KeyEvent) -> bool {
    match key.code {
        KeyCode::Enter => {
            key.modifiers.contains(KeyModifiers::SHIFT) || key.modifiers.contains(KeyModifiers::ALT)
        }
        KeyCode::Char('j') => key.modifiers.contains(KeyModifiers::CONTROL),
        KeyCode::Char('\n') => true,
        _ => false,
    }
}

pub fn typed_char(key: &KeyEvent) -> Option<char> {
    match key.code {
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            Some(c)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    #[test]
    fn newline_encodings() {
        assert!(is_newline_key(&KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::SHIFT
        )));
        assert!(is_newline_key(&KeyEvent::new(
            KeyCode::Char('j'),
            KeyModifiers::CONTROL
        )));
        assert!(!is_newline_key(&KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE
        )));
    }
}
