#![allow(dead_code, unused, clippy::all, non_upper_case_globals)]
use std::collections::HashMap;
use std::fmt;

use bitflags::bitflags;
use gtk::gdk;
use gtk::glib;
#[allow(unused_imports)]
use glib::translate::IntoGlib;

use super::actions::{EditorAction, Mode};

// ---------------------------------------------------------------------------
// KeyModifiers / KeyCode / KeyEvent — simplified port of helix-view/input.rs
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

impl std::str::FromStr for KeyEvent {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Handle empty
        if s.is_empty() {
            return Err("empty key".to_string());
        }

        // Split on '-' for modifiers. Last token is code.
        let mut tokens: Vec<&str> = s.split('-').collect();
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

        // Normalize S- handling for char keys: S-w => W
        let mut code = code;
        if let KeyCode::Char(ch) = code {
            if ch.is_ascii_lowercase() && modifiers.contains(KeyModifiers::SHIFT) {
                code = KeyCode::Char(ch.to_ascii_uppercase());
                modifiers.remove(KeyModifiers::SHIFT);
            }
        }

        Ok(KeyEvent { code, modifiers })
    }
}

// ---------------------------------------------------------------------------
// Trie
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyTrie {
    Leaf(EditorAction),
    Node(KeyTrieNode),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeyTrieNode {
    pub name: String,
    pub map: HashMap<KeyEvent, KeyTrie>,
    pub is_sticky: bool,
}

impl KeyTrieNode {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            map: HashMap::new(),
            is_sticky: false,
        }
    }

    pub fn insert(&mut self, key: KeyEvent, trie: KeyTrie) {
        self.map.insert(key, trie);
    }
}

impl KeyTrie {
    pub fn node(&self) -> Option<&KeyTrieNode> {
        match self {
            KeyTrie::Node(n) => Some(n),
            KeyTrie::Leaf(_) => None,
        }
    }

    pub fn search(&self, keys: &[KeyEvent]) -> Option<&KeyTrie> {
        let mut cur = self;
        for k in keys {
            cur = match cur {
                KeyTrie::Node(node) => node.map.get(k)?,
                KeyTrie::Leaf(_) => return None,
            };
        }
        Some(cur)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeymapResult {
    Pending(KeyTrieNode),
    Matched(EditorAction),
    NotFound,
    Cancelled(Vec<KeyEvent>),
}

pub struct KeyTrieRoot {
    /// One root trie per mode.
    pub map: HashMap<Mode, KeyTrie>,
    pending: Vec<KeyEvent>,
    pub sticky: Option<KeyTrieNode>,
}

impl KeyTrieRoot {
    pub fn new(map: HashMap<Mode, KeyTrie>) -> Self {
        Self {
            map,
            pending: Vec::new(),
            sticky: None,
        }
    }

    pub fn pending(&self) -> &[KeyEvent] {
        &self.pending
    }

    pub fn get(&mut self, mode: Mode, key: KeyEvent) -> KeymapResult {
        // Esc always cancels pending and clears sticky.
        if key.code == KeyCode::Esc {
            if !self.pending.is_empty() {
                let cancelled = self.pending.drain(..).collect();
                return KeymapResult::Cancelled(cancelled);
            }
            if self.sticky.is_some() {
                self.sticky = None;
            }
            // Still return Matched ExitToNormal for caller to handle.
            return KeymapResult::Matched(EditorAction::ExitToNormal);
        }

        let root = match self.map.get(&mode) {
            Some(r) => r,
            None => return KeymapResult::NotFound,
        };

        // If sticky is active, search within it first.
        let trie_node: Option<KeyTrieNode> = self.sticky.clone();
        let search_root: KeyTrie = if let Some(node) = trie_node {
            KeyTrie::Node(node)
        } else {
            root.clone()
        };

        // If we have pending, we attempt to extend.
        if !self.pending.is_empty() {
            self.pending.push(key);
            match search_root.search(&self.pending) {
                Some(KeyTrie::Node(n)) => {
                    if n.is_sticky {
                        self.pending.clear();
                        self.sticky = Some(n.clone());
                    }
                    KeymapResult::Pending(n.clone())
                }
                Some(KeyTrie::Leaf(act)) => {
                    self.pending.clear();
                    KeymapResult::Matched(act.clone())
                }
                None => KeymapResult::Cancelled(self.pending.drain(..).collect()),
            }
        } else {
            // No pending — try single key.
            match search_root.search(&[key]) {
                Some(KeyTrie::Leaf(act)) => KeymapResult::Matched(act.clone()),
                Some(KeyTrie::Node(n)) => {
                    if n.is_sticky {
                        self.sticky = Some(n.clone());
                        KeymapResult::Pending(n.clone())
                    } else {
                        self.pending.push(key);
                        KeymapResult::Pending(n.clone())
                    }
                }
                None => KeymapResult::NotFound,
            }
        }
    }

    pub fn clear_pending(&mut self) {
        self.pending.clear();
    }
}

/// Build the default Phase-2 keymap.
///
/// Normal:
///   h/j/k/l, w/b/e, x, i, d/c/y/p, v, Esc
///   g -> goto node (g g = file start, g e = file end for demo; extensible)
///   Space -> space node (placeholder for which-key phase 3)
/// Insert:
///   Esc -> normal
/// Select:
///   same motions as normal but they extend; Esc -> normal
pub fn default_keymap() -> HashMap<Mode, KeyTrie> {
    let mut m = HashMap::new();

    // Helper to build a leaf.
    let leaf = |a| KeyTrie::Leaf(a);

    // --- Normal ---
    let mut normal = KeyTrieNode::new("Normal mode");
    normal.insert(KeyEvent::char('h'), leaf(EditorAction::MoveLeft));
    normal.insert(KeyEvent::char('j'), leaf(EditorAction::MoveDown));
    normal.insert(KeyEvent::char('k'), leaf(EditorAction::MoveUp));
    normal.insert(KeyEvent::char('l'), leaf(EditorAction::MoveRight));
    normal.insert(KeyEvent::char('w'), leaf(EditorAction::MoveWordForward));
    normal.insert(KeyEvent::char('b'), leaf(EditorAction::MoveWordBackward));
    normal.insert(KeyEvent::char('e'), leaf(EditorAction::MoveWordEnd));
    normal.insert(KeyEvent::char('x'), leaf(EditorAction::SelectLine));
    normal.insert(KeyEvent::char('i'), leaf(EditorAction::EnterInsert));
    normal.insert(KeyEvent::char('a'), leaf(EditorAction::EnterInsertAfter));
    normal.insert(KeyEvent::char('v'), leaf(EditorAction::EnterSelect));
    normal.insert(KeyEvent::char('d'), leaf(EditorAction::DeleteSelection));
    normal.insert(KeyEvent::char('c'), leaf(EditorAction::ChangeSelection));
    normal.insert(KeyEvent::char('y'), leaf(EditorAction::YankSelection));
    normal.insert(KeyEvent::char('p'), leaf(EditorAction::PasteAfter));
    // Esc handled specially in get().

    // g prefix
    let mut goto_node = KeyTrieNode::new("Goto");
    goto_node.insert(KeyEvent::char('g'), leaf(EditorAction::MoveLeft)); // placeholder: gg
    goto_node.insert(KeyEvent::char('e'), leaf(EditorAction::MoveWordEnd)); // ge
    normal.insert(KeyEvent::char('g'), KeyTrie::Node(goto_node));

    // Space prefix (which-key placeholder)
    let mut space_node = KeyTrieNode::new("Space");
    // Space f / Space b etc will be added in Phase 3/4.
    // For now leave empty node to test pending.
    let _ = space_node; // keep empty to produce Pending on Space
    normal.insert(
        KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::empty(),
        },
        KeyTrie::Node(space_node),
    );

    // Arrow keys also work
    normal.insert(
        KeyEvent::new(KeyCode::Left, KeyModifiers::empty()),
        leaf(EditorAction::MoveLeft),
    );
    normal.insert(
        KeyEvent::new(KeyCode::Right, KeyModifiers::empty()),
        leaf(EditorAction::MoveRight),
    );
    normal.insert(
        KeyEvent::new(KeyCode::Up, KeyModifiers::empty()),
        leaf(EditorAction::MoveUp),
    );
    normal.insert(
        KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
        leaf(EditorAction::MoveDown),
    );

    m.insert(Mode::Normal, KeyTrie::Node(normal));

    // --- Insert ---
    let insert = KeyTrieNode::new("Insert mode");
    // Insert has no leaves except Esc (handled globally)
    m.insert(Mode::Insert, KeyTrie::Node(insert));

    // --- Select ---
    let mut select = KeyTrieNode::new("Select mode");
    select.insert(KeyEvent::char('h'), leaf(EditorAction::MoveLeft));
    select.insert(KeyEvent::char('j'), leaf(EditorAction::MoveDown));
    select.insert(KeyEvent::char('k'), leaf(EditorAction::MoveUp));
    select.insert(KeyEvent::char('l'), leaf(EditorAction::MoveRight));
    select.insert(KeyEvent::char('w'), leaf(EditorAction::MoveWordForward));
    select.insert(KeyEvent::char('b'), leaf(EditorAction::MoveWordBackward));
    select.insert(KeyEvent::char('e'), leaf(EditorAction::MoveWordEnd));
    select.insert(KeyEvent::char('x'), leaf(EditorAction::SelectLine));
    // d/y/c still meaningful in select
    select.insert(KeyEvent::char('d'), leaf(EditorAction::DeleteSelection));
    select.insert(KeyEvent::char('c'), leaf(EditorAction::ChangeSelection));
    select.insert(KeyEvent::char('y'), leaf(EditorAction::YankSelection));
    select.insert(
        KeyEvent::new(KeyCode::Left, KeyModifiers::empty()),
        leaf(EditorAction::MoveLeft),
    );
    select.insert(
        KeyEvent::new(KeyCode::Right, KeyModifiers::empty()),
        leaf(EditorAction::MoveRight),
    );
    select.insert(
        KeyEvent::new(KeyCode::Up, KeyModifiers::empty()),
        leaf(EditorAction::MoveUp),
    );
    select.insert(
        KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
        leaf(EditorAction::MoveDown),
    );
    m.insert(Mode::Select, KeyTrie::Node(select));

    m
}

// ---------------------------------------------------------------------------
// Underlying GDK → KeyEvent conversion helper (used by editor controller)
// ---------------------------------------------------------------------------

/// Convert a raw GDK keyval + modifier bits into a `KeyEvent`.
pub fn gdk_to_key_event(keyval: u32, state: u32) -> Option<KeyEvent> {
    let keyval = keyval;
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
    const GDK_KEY_Escape: u32 = 0xff1b;
    const GDK_KEY_Return: u32 = 0xff0d;
    const GDK_KEY_KP_Enter: u32 = 0xff8d;
    const GDK_KEY_BackSpace: u32 = 0xff08;
    const GDK_KEY_Tab: u32 = 0xff09;
    const GDK_KEY_Delete: u32 = 0xffff;
    const GDK_KEY_Left: u32 = 0xff51;
    const GDK_KEY_Right: u32 = 0xff53;
    const GDK_KEY_Up: u32 = 0xff52;
    const GDK_KEY_Down: u32 = 0xff54;
    const GDK_KEY_Home: u32 = 0xff50;
    const GDK_KEY_End: u32 = 0xff57;
    const GDK_KEY_Page_Up: u32 = 0xff55;
    const GDK_KEY_Page_Down: u32 = 0xff56;

    let code = match keyval {
        GDK_KEY_Escape => KeyCode::Esc,
        GDK_KEY_Return | GDK_KEY_KP_Enter => KeyCode::Enter,
        GDK_KEY_BackSpace => KeyCode::Backspace,
        GDK_KEY_Tab => KeyCode::Tab,
        GDK_KEY_Delete => KeyCode::Delete,
        GDK_KEY_Left => KeyCode::Left,
        GDK_KEY_Right => KeyCode::Right,
        GDK_KEY_Up => KeyCode::Up,
        GDK_KEY_Down => KeyCode::Down,
        GDK_KEY_Home => KeyCode::Home,
        GDK_KEY_End => KeyCode::End,
        GDK_KEY_Page_Up => KeyCode::PageUp,
        GDK_KEY_Page_Down => KeyCode::PageDown,
        _ => {
            // Assume unicode char. GDK keyval for ' ' is 0x20 etc.
            if let Some(ch) = char::from_u32(keyval) {
                // Filter control chars; treat remaining as Char.
                if ch.is_control() {
                    return None;
                }
                KeyCode::Char(ch)
            } else {
                return None;
            }
        }
    };

    // Normalize: S handling for Char as in FromStr
    let mut code = code;
    let mut mods_norm = mods;
    if let KeyCode::Char(ch) = code {
        if ch.is_ascii_lowercase() && mods.contains(KeyModifiers::SHIFT) {
            code = KeyCode::Char(ch.to_ascii_uppercase());
            mods_norm.remove(KeyModifiers::SHIFT);
        }
        // Space etc should not retain SHIFT.
        if ch == ' ' {
            mods_norm.remove(KeyModifiers::SHIFT);
        }
    }

    Some(KeyEvent {
        code,
        modifiers: mods_norm,
    })
}

/// Wrapper that accepts `gdk::Key` + `gdk::ModifierType` (used in `app.rs`).
pub fn gdk_key_to_event(key: gdk::Key, state: gdk::ModifierType) -> Option<KeyEvent> {
    use glib::translate::IntoGlib;
    gdk_to_key_event(key.into_glib(), state.bits())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

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
            KeyEvent::from_str("C-w").unwrap(),
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
        assert!(KeyEvent::from_str("invalid-key-foo").is_err());
    }

    #[test]
    fn trie_search_single() {
        let map = default_keymap();
        let root = map.get(&Mode::Normal).unwrap();
        assert_eq!(
            root.search(&[KeyEvent::char('h')]),
            Some(&KeyTrie::Leaf(EditorAction::MoveLeft))
        );
        assert_eq!(
            root.search(&[KeyEvent::char('x')]),
            Some(&KeyTrie::Leaf(EditorAction::SelectLine))
        );
    }

    #[test]
    fn trie_pending_g_prefix() {
        let map = default_keymap();
        let mut trie = KeyTrieRoot::new(map);
        let res = trie.get(Mode::Normal, KeyEvent::char('g'));
        assert!(matches!(res, KeymapResult::Pending(_)));
        // g g should match
        let res2 = trie.get(Mode::Normal, KeyEvent::char('g'));
        assert_eq!(res2, KeymapResult::Matched(EditorAction::MoveLeft));
        // Reset pending is cleared
        assert!(trie.pending().is_empty());
    }

    #[test]
    fn trie_cancelled_on_invalid_chord() {
        let map = default_keymap();
        let mut trie = KeyTrieRoot::new(map);
        let _ = trie.get(Mode::Normal, KeyEvent::char('g'));
        let res = trie.get(Mode::Normal, KeyEvent::char('z')); // invalid
        assert!(matches!(res, KeymapResult::Cancelled(_)));
        assert!(trie.pending().is_empty());
    }

    #[test]
    fn esc_cancels_pending() {
        let map = default_keymap();
        let mut trie = KeyTrieRoot::new(map);
        let _ = trie.get(Mode::Normal, KeyEvent::char('g'));
        let res = trie.get(
            Mode::Normal,
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::empty(),
            },
        );
        assert!(matches!(res, KeymapResult::Cancelled(_)));
    }

    #[test]
    fn not_found_on_unbound_key() {
        let map = default_keymap();
        let mut trie = KeyTrieRoot::new(map);
        let res = trie.get(Mode::Normal, KeyEvent::char('q'));
        assert_eq!(res, KeymapResult::NotFound);
    }

    #[test]
    fn space_pending() {
        let map = default_keymap();
        let mut trie = KeyTrieRoot::new(map);
        let res = trie.get(
            Mode::Normal,
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::empty(),
            },
        );
        assert!(matches!(res, KeymapResult::Pending(_)));
    }

    #[test]
    fn insert_mode_esc_matched() {
        let map = default_keymap();
        let mut trie = KeyTrieRoot::new(map);
        let res = trie.get(
            Mode::Insert,
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::empty(),
            },
        );
        assert_eq!(res, KeymapResult::Matched(EditorAction::ExitToNormal));
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
    fn select_mode_motions() {
        let map = default_keymap();
        let root = map.get(&Mode::Select).unwrap();
        assert_eq!(
            root.search(&[KeyEvent::char('w')]),
            Some(&KeyTrie::Leaf(EditorAction::MoveWordForward))
        );
    }
}
