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

        let mut event = KeyEvent { code, modifiers };
        canonicalize_key(&mut event);
        Ok(event)
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

    /// Generate Which-Key presentation data, mirroring Helix's `KeyTrieNode::infobox()`.
    pub fn which_key_data(&self) -> crate::components::which_key::WhichKeyData {
        crate::components::which_key::WhichKeyData::from_trie_node(self)
    }

    /// Alias for `which_key_data()` matching Helix naming.
    pub fn infobox(&self) -> crate::components::which_key::WhichKeyData {
        self.which_key_data()
    }
}

impl KeyTrie {
    pub fn node(&self) -> Option<&KeyTrieNode> {
        match self {
            KeyTrie::Node(n) => Some(n),
            KeyTrie::Leaf(_) => None,
        }
    }

    pub fn search(&self, keys: &[KeyEvent]) -> Option<KeyTrie> {
        let mut cur = self.clone();
        for k in keys {
            cur = match cur {
                KeyTrie::Node(node) => node.map.get(k)?.clone(),
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
        // Esc or Ctrl-g always cancels pending and clears sticky.
        let is_cancel = key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('g') && key.modifiers.contains(KeyModifiers::CONTROL));

        if is_cancel {
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
                    KeymapResult::Pending(n)
                }
                Some(KeyTrie::Leaf(act)) => {
                    self.pending.clear();
                    KeymapResult::Matched(act)
                }
                None => KeymapResult::Cancelled(self.pending.drain(..).collect()),
            }
        } else {
            // No pending — try single key.
            match search_root.search(&[key]) {
                Some(KeyTrie::Leaf(act)) => KeymapResult::Matched(act),
                Some(KeyTrie::Node(n)) => {
                    if n.is_sticky {
                        self.sticky = Some(n.clone());
                        KeymapResult::Pending(n)
                    } else {
                        self.pending.push(key);
                        KeymapResult::Pending(n)
                    }
                }
                None => KeymapResult::NotFound,
            }
        }
    }

    pub fn clear_pending(&mut self) {
        self.pending.clear();
    }

    pub fn pending_prefix_string(&self) -> String {
        if !self.pending.is_empty() {
            let keys: Vec<String> = self.pending.iter().map(|k| k.to_string()).collect();
            format!("{}-", keys.join(""))
        } else if let Some(ref sticky) = self.sticky {
            format!("{}-", sticky.name.to_lowercase())
        } else {
            String::new()
        }
    }

    pub fn contains_key(&self, mode: Mode, key: KeyEvent) -> bool {
        if let Some(ref sticky_node) = self.sticky {
            if sticky_node.map.contains_key(&key) {
                return true;
            }
        }
        if let Some(KeyTrie::Node(node)) = self.map.get(&mode) {
            node.map.contains_key(&key)
        } else {
            false
        }
    }
}

/// Build the Helix-style Match (`m`) submap:
/// - `mm` -> goto matching bracket
/// - `ms` -> surround selection (waits for char via on_next_key)
/// - `mr` -> replace surround pair (waits for from/to via on_next_key)
/// - `md` -> delete surround pair (waits for char via on_next_key)
/// - `ma` -> select around textobject (waits for obj via on_next_key)
/// - `mi` -> select inside textobject (waits for obj via on_next_key)
pub fn build_match_node() -> KeyTrieNode {
    let mut match_node = KeyTrieNode::new("Match");
    let leaf = |a| KeyTrie::Leaf(a);

    // mm -> match_brackets
    match_node.insert(KeyEvent::char('m'), leaf(EditorAction::MatchBrackets));

    // ms -> surround_add
    match_node.insert(KeyEvent::char('s'), leaf(EditorAction::SurroundAdd));

    // md -> surround_delete
    match_node.insert(KeyEvent::char('d'), leaf(EditorAction::SurroundDelete));

    // mr -> surround_replace
    match_node.insert(KeyEvent::char('r'), leaf(EditorAction::SurroundReplace));

    // ma -> select_textobject_around
    match_node.insert(KeyEvent::char('a'), leaf(EditorAction::SelectTextObjectAround));

    // mi -> select_textobject_inner
    match_node.insert(KeyEvent::char('i'), leaf(EditorAction::SelectTextObjectInner));

    match_node
}

/// Helper to construct Helix space mode node:
/// - `y` -> yank to system clipboard
/// - `p` -> paste system clipboard after
/// - `P` -> paste system clipboard before
pub fn build_space_node() -> KeyTrieNode {
    let mut space_node = KeyTrieNode::new("Space");
    let leaf = |a| KeyTrie::Leaf(a);
    space_node.insert(KeyEvent::char('y'), leaf(EditorAction::YankToClipboard));
    space_node.insert(KeyEvent::char('p'), leaf(EditorAction::PasteClipboardAfter));
    space_node.insert(KeyEvent::char('P'), leaf(EditorAction::PasteClipboardBefore));
    space_node
}

/// Build the default Phase-2 keymap.
///
/// Normal:
///   h/j/k/l, w/b/e, x, i, d/c/y/p, v, Esc
///   g -> goto node (g g = file start, g e = file end for demo; extensible)
///   m -> match node (mm = match brackets, ms = surround add, mr = replace, md = delete, ma/mi = textobjects)
///   Space -> space node (y = yank to clipboard, p/P = paste from clipboard)
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
    normal.insert(KeyEvent::char('I'), leaf(EditorAction::InsertAtLineStart));
    normal.insert(KeyEvent::char('a'), leaf(EditorAction::EnterInsertAfter));
    normal.insert(KeyEvent::char('A'), leaf(EditorAction::InsertAtLineEnd));
    normal.insert(KeyEvent::char('v'), leaf(EditorAction::EnterSelect));
    normal.insert(KeyEvent::char('d'), leaf(EditorAction::DeleteSelection));
    normal.insert(KeyEvent::char('c'), leaf(EditorAction::ChangeSelection));
    normal.insert(KeyEvent::char('y'), leaf(EditorAction::YankSelection));
    normal.insert(KeyEvent::char('p'), leaf(EditorAction::PasteAfter));
    normal.insert(KeyEvent::char('u'), leaf(EditorAction::Undo));
    normal.insert(KeyEvent::char('U'), leaf(EditorAction::Redo));
    normal.insert(KeyEvent::char('"'), leaf(EditorAction::SelectRegister));
    // Esc handled specially in get().

    // g prefix
    let mut goto_node = KeyTrieNode::new("Goto");
    goto_node.insert(KeyEvent::char('g'), leaf(EditorAction::MoveLeft)); // placeholder: gg
    goto_node.insert(KeyEvent::char('e'), leaf(EditorAction::MoveWordEnd)); // ge
    normal.insert(KeyEvent::char('g'), KeyTrie::Node(goto_node));

    // m prefix (Helix match mode)
    normal.insert(KeyEvent::char('m'), KeyTrie::Node(build_match_node()));

    // Space prefix (Helix clipboard and which-key)
    normal.insert(
        KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::empty(),
        },
        KeyTrie::Node(build_space_node()),
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
    select.insert(KeyEvent::char('I'), leaf(EditorAction::InsertAtLineStart));
    select.insert(KeyEvent::char('A'), leaf(EditorAction::InsertAtLineEnd));
    // d/y/c still meaningful in select
    select.insert(KeyEvent::char('d'), leaf(EditorAction::DeleteSelection));
    select.insert(KeyEvent::char('c'), leaf(EditorAction::ChangeSelection));
    select.insert(KeyEvent::char('y'), leaf(EditorAction::YankSelection));
    select.insert(KeyEvent::char('"'), leaf(EditorAction::SelectRegister));
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
    // m prefix also available in Select mode
    select.insert(KeyEvent::char('m'), KeyTrie::Node(build_match_node()));
    // Space prefix also available in Select mode
    select.insert(
        KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::empty(),
        },
        KeyTrie::Node(build_space_node()),
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
    const GDK_KEY_ISO_Left_Tab: u32 = 0xfe20;
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
        GDK_KEY_Tab | GDK_KEY_ISO_Left_Tab => KeyCode::Tab,
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

    let mut event = KeyEvent {
        code,
        modifiers: mods,
    };
    canonicalize_key(&mut event);
    Some(event)
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
            Some(KeyTrie::Leaf(EditorAction::MoveLeft))
        );
        assert_eq!(
            root.search(&[KeyEvent::char('x')]),
            Some(KeyTrie::Leaf(EditorAction::SelectLine))
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
            Some(KeyTrie::Leaf(EditorAction::MoveWordForward))
        );
        assert_eq!(
            root.search(&[KeyEvent::char('I')]),
            Some(KeyTrie::Leaf(EditorAction::InsertAtLineStart))
        );
        assert_eq!(
            root.search(&[KeyEvent::char('A')]),
            Some(KeyTrie::Leaf(EditorAction::InsertAtLineEnd))
        );
    }

    #[test]
    fn helix_a_i_u_redo_keybinds() {
        let map = default_keymap();
        let root = map.get(&Mode::Normal).unwrap();
        assert_eq!(
            root.search(&[KeyEvent::char('I')]),
            Some(KeyTrie::Leaf(EditorAction::InsertAtLineStart))
        );
        assert_eq!(
            root.search(&[KeyEvent::char('A')]),
            Some(KeyTrie::Leaf(EditorAction::InsertAtLineEnd))
        );
        assert_eq!(
            root.search(&[KeyEvent::char('u')]),
            Some(KeyTrie::Leaf(EditorAction::Undo))
        );
        assert_eq!(
            root.search(&[KeyEvent::char('U')]),
            Some(KeyTrie::Leaf(EditorAction::Redo))
        );

        // Test GDK conversion with SHIFT mask
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

    #[test]
    fn match_mode_keybinds() {
        let map = default_keymap();
        let mut trie = KeyTrieRoot::new(map);

        // mm -> MatchBrackets
        let res = trie.get(Mode::Normal, KeyEvent::char('m'));
        assert!(matches!(res, KeymapResult::Pending(_)));
        let res2 = trie.get(Mode::Normal, KeyEvent::char('m'));
        assert_eq!(res2, KeymapResult::Matched(EditorAction::MatchBrackets));

        // ms -> SurroundAdd
        let _ = trie.get(Mode::Normal, KeyEvent::char('m'));
        let res_ms = trie.get(Mode::Normal, KeyEvent::char('s'));
        assert_eq!(res_ms, KeymapResult::Matched(EditorAction::SurroundAdd));

        // md -> SurroundDelete
        let _ = trie.get(Mode::Normal, KeyEvent::char('m'));
        let res_md = trie.get(Mode::Normal, KeyEvent::char('d'));
        assert_eq!(res_md, KeymapResult::Matched(EditorAction::SurroundDelete));

        // mr -> SurroundReplace
        let _ = trie.get(Mode::Normal, KeyEvent::char('m'));
        let res_mr = trie.get(Mode::Normal, KeyEvent::char('r'));
        assert_eq!(res_mr, KeymapResult::Matched(EditorAction::SurroundReplace));

        // ma -> SelectTextObjectAround
        let _ = trie.get(Mode::Normal, KeyEvent::char('m'));
        let res_ma = trie.get(Mode::Normal, KeyEvent::char('a'));
        assert_eq!(res_ma, KeymapResult::Matched(EditorAction::SelectTextObjectAround));

        // mi -> SelectTextObjectInner
        let _ = trie.get(Mode::Normal, KeyEvent::char('m'));
        let res_mi = trie.get(Mode::Normal, KeyEvent::char('i'));
        assert_eq!(res_mi, KeymapResult::Matched(EditorAction::SelectTextObjectInner));

        // Works in Select mode too
        let _ = trie.get(Mode::Select, KeyEvent::char('m'));
        let res_sel_mm = trie.get(Mode::Select, KeyEvent::char('m'));
        assert_eq!(res_sel_mm, KeymapResult::Matched(EditorAction::MatchBrackets));

        let _ = trie.get(Mode::Select, KeyEvent::char('m'));
        let res_sel_ms = trie.get(Mode::Select, KeyEvent::char('s'));
        assert_eq!(res_sel_ms, KeymapResult::Matched(EditorAction::SurroundAdd));
    }
}
