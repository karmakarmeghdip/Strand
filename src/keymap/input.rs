use std::fmt;
use std::str::FromStr;

use bitflags::bitflags;
use gtk::gdk;
use gtk::glib;
use glib::translate::IntoGlib;

// ---------------------------------------------------------------------------
// KeyModifiers / KeyCode / KeyEvent — mirrors helix-view/input.rs
// ---------------------------------------------------------------------------

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct KeyModifiers: u8 {
        const SHIFT   = 0b0000_0001;
        const CONTROL = 0b0000_0010;
        const ALT     = 0b0000_0100;
        const SUPER   = 0b0000_1000;
        const NONE    = 0b0000_0000;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KeyCode {
    Char(char),
    Esc,
    Enter,
    Backspace,
    Tab,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    F(u8),
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyEvent {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    pub fn char(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::empty(),
        }
    }

    pub fn ctrl(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::CONTROL,
        }
    }

    pub fn alt(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::ALT,
        }
    }

    pub fn shift(c: char) -> Self {
        let mut ev = Self {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::SHIFT,
        };
        canonicalize_key(&mut ev);
        ev
    }

    /// If this key event corresponds to a character, return it.
    pub fn as_char(&self) -> Option<char> {
        match self.code {
            KeyCode::Char(ch) => Some(ch),
            _ => None,
        }
    }

    /// Format the key in such a way that concatenated chords are readable (mirroring Helix).
    pub fn key_sequence_format(&self) -> String {
        let s = self.to_string();
        if s.chars().count() > 1 {
            format!("<{s}>")
        } else {
            s
        }
    }
}

impl fmt::Display for KeyEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.contains(KeyModifiers::SUPER) {
            f.write_str("Meta-")?;
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            f.write_str("S-")?;
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            f.write_str("A-")?;
        }
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            f.write_str("C-")?;
        }
        match self.code {
            KeyCode::Char(' ') => f.write_str("space"),
            KeyCode::Char('-') => f.write_str("minus"),
            KeyCode::Char(c) => f.write_fmt(format_args!("{c}")),
            KeyCode::Esc => f.write_str("esc"),
            KeyCode::Enter => f.write_str("ret"),
            KeyCode::Backspace => f.write_str("backspace"),
            KeyCode::Tab => f.write_str("tab"),
            KeyCode::Delete => f.write_str("del"),
            KeyCode::Left => f.write_str("left"),
            KeyCode::Right => f.write_str("right"),
            KeyCode::Up => f.write_str("up"),
            KeyCode::Down => f.write_str("down"),
            KeyCode::Home => f.write_str("home"),
            KeyCode::End => f.write_str("end"),
            KeyCode::PageUp => f.write_str("pageup"),
            KeyCode::PageDown => f.write_str("pagedown"),
            KeyCode::F(n) => f.write_fmt(format_args!("F{n}")),
            KeyCode::Null => f.write_str("null"),
        }
    }
}

impl FromStr for KeyEvent {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err("empty key".to_string());
        }

        // Allow angle-bracketed formats like <C-w> or <space>
        let raw = if s.len() >= 3 && s.starts_with('<') && s.ends_with('>') {
            &s[1..s.len() - 1]
        } else {
            s
        };

        if raw == "-" {
            return Ok(KeyEvent {
                code: KeyCode::Char('-'),
                modifiers: KeyModifiers::empty(),
            });
        }

        // Split on '-' for modifiers. Last token is code.
        let mut tokens: Vec<&str> = raw.split('-').collect();
        let code_str = tokens.pop().ok_or("missing key code")?;

        let code = match code_str {
            "esc" => KeyCode::Esc,
            "ret" | "enter" => KeyCode::Enter,
            "backspace" | "bs" => KeyCode::Backspace,
            "tab" => KeyCode::Tab,
            "del" | "delete" => KeyCode::Delete,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "space" => KeyCode::Char(' '),
            "minus" => KeyCode::Char('-'),
            "null" => KeyCode::Null,
            _ if code_str.len() == 1 => {
                KeyCode::Char(code_str.chars().next().unwrap())
            }
            _ if code_str.starts_with('F')
                && code_str.len() > 1
                && code_str[1..].chars().all(|c| c.is_ascii_digit()) =>
            {
                let n: u8 = code_str[1..].parse().map_err(|_| format!("invalid F key {s}"))?;
                if !(1..=24).contains(&n) {
                    return Err(format!("invalid F key {s}"));
                }
                KeyCode::F(n)
            }
            _ => return Err(format!("invalid key code '{code_str}'")),
        };

        let mut modifiers = KeyModifiers::empty();
        for tok in tokens {
            match tok {
                "S" => modifiers.insert(KeyModifiers::SHIFT),
                "C" => modifiers.insert(KeyModifiers::CONTROL),
                "A" => modifiers.insert(KeyModifiers::ALT),
                "Meta" | "Cmd" | "Win" | "Super" => modifiers.insert(KeyModifiers::SUPER),
                _ => return Err(format!("invalid modifier '{tok}'")),
            }
        }

        let mut event = KeyEvent { code, modifiers };
        canonicalize_key(&mut event);
        Ok(event)
    }
}

impl serde::Serialize for KeyEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for KeyEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Canonicalize key event according to Helix rules:
/// Character keys have SHIFT modifier stripped because the character itself already reflects
/// whether shift was pressed (e.g. '"', 'A', ':', '+'). If a lowercase char has SHIFT,
/// it is converted to uppercase.
pub fn canonicalize_key(key: &mut KeyEvent) {
    if let KeyCode::Char(ch) = key.code {
        if ch.is_ascii_lowercase() && key.modifiers.contains(KeyModifiers::SHIFT) {
            key.code = KeyCode::Char(ch.to_ascii_uppercase());
        }
        key.modifiers.remove(KeyModifiers::SHIFT);
    }
}

// ---------------------------------------------------------------------------
// Underlying GDK → KeyEvent conversion helper (used by editor controller)
// ---------------------------------------------------------------------------

/// Convert a raw GDK keyval + modifier bits into a `KeyEvent`.
pub fn gdk_to_key_event(keyval: u32, state: u32) -> Option<KeyEvent> {
    // GDK modifier masks (from gdk4/gdkkeysyms). Values match GDK docs.
    const SHIFT_MASK: u32 = 1;
    const CONTROL_MASK: u32 = 1 << 2;
    const ALT_MASK: u32 = 1 << 3;
    // SUPER = GDK_SUPER_MASK (1<<26) or META; we treat both as SUPER.
    const SUPER_MASK: u32 = 1 << 26;

    let mut mods = KeyModifiers::empty();
    if state & SHIFT_MASK != 0 {
        mods.insert(KeyModifiers::SHIFT);
    }
    if state & CONTROL_MASK != 0 {
        mods.insert(KeyModifiers::CONTROL);
    }
    if state & ALT_MASK != 0 {
        mods.insert(KeyModifiers::ALT);
    }
    if state & SUPER_MASK != 0 {
        mods.insert(KeyModifiers::SUPER);
    }

    // GDK keyvals for non-char specials
    const GDK_KEY_ESCAPE: u32 = 0xff1b;
    const GDK_KEY_RETURN: u32 = 0xff0d;
    const GDK_KEY_KP_ENTER: u32 = 0xff8d;
    const GDK_KEY_BACK_SPACE: u32 = 0xff08;
    const GDK_KEY_TAB: u32 = 0xff09;
    const GDK_KEY_ISO_LEFT_TAB: u32 = 0xfe20;
    const GDK_KEY_DELETE: u32 = 0xffff;
    const GDK_KEY_LEFT: u32 = 0xff51;
    const GDK_KEY_RIGHT: u32 = 0xff53;
    const GDK_KEY_UP: u32 = 0xff52;
    const GDK_KEY_DOWN: u32 = 0xff54;
    const GDK_KEY_HOME: u32 = 0xff50;
    const GDK_KEY_END: u32 = 0xff57;
    const GDK_KEY_PAGE_UP: u32 = 0xff55;
    const GDK_KEY_PAGE_DOWN: u32 = 0xff56;

    let code = match keyval {
        GDK_KEY_ESCAPE => KeyCode::Esc,
        GDK_KEY_RETURN | GDK_KEY_KP_ENTER => KeyCode::Enter,
        GDK_KEY_BACK_SPACE => KeyCode::Backspace,
        GDK_KEY_TAB | GDK_KEY_ISO_LEFT_TAB => KeyCode::Tab,
        GDK_KEY_DELETE => KeyCode::Delete,
        GDK_KEY_LEFT => KeyCode::Left,
        GDK_KEY_RIGHT => KeyCode::Right,
        GDK_KEY_UP => KeyCode::Up,
        GDK_KEY_DOWN => KeyCode::Down,
        GDK_KEY_HOME => KeyCode::Home,
        GDK_KEY_END => KeyCode::End,
        GDK_KEY_PAGE_UP => KeyCode::PageUp,
        GDK_KEY_PAGE_DOWN => KeyCode::PageDown,
        _ => {
            let ch = char::from_u32(keyval)?;
            // Filter control chars; treat remaining as Char.
            if ch.is_control() {
                return None;
            }
            KeyCode::Char(ch)
        }
    };

    let mut event = KeyEvent {
        code,
        modifiers: mods,
    };
    canonicalize_key(&mut event);
    Some(event)
}

/// Wrapper that accepts `gdk::Key` + `gdk::ModifierType` (used in `app.rs`).
pub fn gdk_key_to_event(key: gdk::Key, state: gdk::ModifierType) -> Option<KeyEvent> {
    gdk_to_key_event(key.into_glib(), state.bits())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_keys() {
        assert_eq!(KeyEvent::from_str("h").unwrap(), KeyEvent::char('h'));
        assert_eq!(KeyEvent::from_str("esc").unwrap().code, KeyCode::Esc);
        assert_eq!(
            KeyEvent::from_str("space").unwrap(),
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::empty()
            }
        );
        assert_eq!(
            KeyEvent::from_str("<space>").unwrap(),
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::empty()
            }
        );
        assert_eq!(
            KeyEvent::from_str("C-w").unwrap(),
            KeyEvent {
                code: KeyCode::Char('w'),
                modifiers: KeyModifiers::CONTROL
            }
        );
        assert_eq!(
            KeyEvent::from_str("<C-w>").unwrap(),
            KeyEvent {
                code: KeyCode::Char('w'),
                modifiers: KeyModifiers::CONTROL
            }
        );
        // S-w normalizes to W
        assert_eq!(
            KeyEvent::from_str("S-w").unwrap(),
            KeyEvent {
                code: KeyCode::Char('W'),
                modifiers: KeyModifiers::empty()
            }
        );
        assert_eq!(
            KeyEvent::from_str("-").unwrap(),
            KeyEvent {
                code: KeyCode::Char('-'),
                modifiers: KeyModifiers::empty()
            }
        );
        assert!(KeyEvent::from_str("invalid-key-foo").is_err());
    }

    #[test]
    fn key_sequence_format_test() {
        assert_eq!(KeyEvent::char('w').key_sequence_format(), "w");
        assert_eq!(KeyEvent::ctrl('w').key_sequence_format(), "<C-w>");
        assert_eq!(KeyEvent::char(' ').key_sequence_format(), "<space>");
    }

    #[test]
    fn gdk_conversion_space_and_chars() {
        // GDK keyval for 'h' is 104
        let ev = gdk_to_key_event('h' as u32, 0).unwrap();
        assert_eq!(ev, KeyEvent::char('h'));
        let esc = gdk_to_key_event(0xff1b, 0).unwrap();
        assert_eq!(esc.code, KeyCode::Esc);
        let sp = gdk_to_key_event(0x20, 0).unwrap();
        assert_eq!(sp.code, KeyCode::Char(' '));
    }

    #[test]
    fn gdk_conversion_with_modifiers() {
        const SHIFT_MASK: u32 = 1;
        let ev_a = gdk_to_key_event('A' as u32, SHIFT_MASK).unwrap();
        assert_eq!(ev_a, KeyEvent::char('A'));
        let ev_i = gdk_to_key_event('I' as u32, SHIFT_MASK).unwrap();
        assert_eq!(ev_i, KeyEvent::char('I'));
        let ev_u = gdk_to_key_event('u' as u32, 0).unwrap();
        assert_eq!(ev_u, KeyEvent::char('u'));
        let ev_cap_u = gdk_to_key_event('U' as u32, SHIFT_MASK).unwrap();
        assert_eq!(ev_cap_u, KeyEvent::char('U'));
        let ev_quote = gdk_to_key_event('"' as u32, SHIFT_MASK).unwrap();
        assert_eq!(ev_quote, KeyEvent::char('"'));
        let ev_colon = gdk_to_key_event(':' as u32, SHIFT_MASK).unwrap();
        assert_eq!(ev_colon, KeyEvent::char(':'));
        let ev_plus = gdk_to_key_event('+' as u32, SHIFT_MASK).unwrap();
        assert_eq!(ev_plus, KeyEvent::char('+'));
    }
}
