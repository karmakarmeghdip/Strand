use std::collections::HashMap;

use super::actions::{EditorAction, Mode};
// Re-exports for backwards compatibility
#[allow(unused_imports)]
pub use super::default::{build_goto_node, build_match_node, build_space_node, default_keymap};
#[allow(unused_imports)]
pub use super::input::{
    canonicalize_key, gdk_key_to_event, gdk_to_key_event, KeyCode, KeyEvent, KeyModifiers,
};

// ---------------------------------------------------------------------------
// KeyTrie & KeyTrieNode — generic trie matching helix-term/keymap.rs
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

    /// Merge another Node in (mirroring Helix's `KeyTrieNode::merge`).
    /// Leaves and subnodes from `other` replace corresponding `KeyEvent` in `self`,
    /// except when both `self` and `other` have subnodes for the same key.
    /// In that case the merge is recursive.
    pub fn merge(&mut self, mut other: Self) {
        for (key, trie) in std::mem::take(&mut other.map) {
            match (self.map.get_mut(&key), trie) {
                (Some(KeyTrie::Node(node)), KeyTrie::Node(other_node)) => {
                    node.merge(other_node);
                }
                (_, trie) => {
                    self.map.insert(key, trie);
                }
            }
        }
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

    pub fn node_mut(&mut self) -> Option<&mut KeyTrieNode> {
        match self {
            KeyTrie::Node(n) => Some(n),
            KeyTrie::Leaf(_) => None,
        }
    }

    /// Merge another KeyTrie in (mirroring Helix's `KeyTrie::merge_nodes`).
    pub fn merge_nodes(&mut self, mut other: Self) {
        if let (Some(self_node), Some(other_node)) = (self.node_mut(), other.node_mut()) {
            let other_node_val = std::mem::take(other_node);
            self_node.merge(other_node_val);
        } else {
            *self = other;
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

/// Merge default keymap with user overwritten keys for custom configuration (mirroring Helix's `merge_keys`).
pub fn merge_keys(dst: &mut HashMap<Mode, KeyTrie>, mut delta: HashMap<Mode, KeyTrie>) {
    for (mode, keys) in dst.iter_mut() {
        if let Some(delta_trie) = delta.remove(mode) {
            keys.merge_nodes(delta_trie);
        }
    }
    for (mode, keys) in delta {
        dst.insert(mode, keys);
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
        if self.sticky.as_ref().is_some_and(|node| node.map.contains_key(&key)) {
            return true;
        }
        if let Some(KeyTrie::Node(node)) = self.map.get(&mode) {
            node.map.contains_key(&key)
        } else {
            false
        }
    }

    /// Merge custom keybinding overrides into this root trie.
    pub fn merge_keys(&mut self, delta: HashMap<Mode, KeyTrie>) {
        merge_keys(&mut self.map, delta);
    }

    /// Remap a key or chord string (e.g. `"C-s"` or `"g x"`) in the given mode to an action.
    pub fn remap(&mut self, mode: Mode, chord_str: &str, action: EditorAction) -> Result<(), String> {
        let keys: Vec<KeyEvent> = chord_str
            .split_whitespace()
            .map(|s| s.parse::<KeyEvent>())
            .collect::<Result<Vec<_>, _>>()?;

        if keys.is_empty() {
            return Err("empty key chord".to_string());
        }

        let mode_root = self.map.entry(mode).or_insert_with(|| {
            KeyTrie::Node(KeyTrieNode::new(&format!("{mode} mode")))
        });

        let mut curr = match mode_root {
            KeyTrie::Node(node) => node,
            KeyTrie::Leaf(_) => {
                *mode_root = KeyTrie::Node(KeyTrieNode::new(&format!("{mode} mode")));
                match mode_root {
                    KeyTrie::Node(node) => node,
                    _ => unreachable!(),
                }
            }
        };

        for (i, &k) in keys.iter().enumerate() {
            if i == keys.len() - 1 {
                curr.insert(k, KeyTrie::Leaf(action.clone()));
                break;
            } else {
                let entry = curr.map.entry(k).or_insert_with(|| {
                    KeyTrie::Node(KeyTrieNode::new(""))
                });
                match entry {
                    KeyTrie::Node(next) => curr = next,
                    KeyTrie::Leaf(_) => {
                        *entry = KeyTrie::Node(KeyTrieNode::new(""));
                        match entry {
                            KeyTrie::Node(next) => curr = next,
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

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
        assert_eq!(res2, KeymapResult::Matched(EditorAction::GotoFileStart));
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

    #[test]
    fn custom_keybinding_override_via_merge_keys() {
        let mut keymap = default_keymap();

        // Override 'w' in Normal mode to MoveLeft instead of MoveWordForward
        // Add a new binding 'C-s' -> SelectAll
        let mut normal_overrides = KeyTrieNode::new("Normal overrides");
        normal_overrides.insert(KeyEvent::char('w'), KeyTrie::Leaf(EditorAction::MoveLeft));
        normal_overrides.insert(
            KeyEvent::from_str("C-s").unwrap(),
            KeyTrie::Leaf(EditorAction::SelectAll),
        );

        // Add subnode override in 'g' prefix: 'g x' -> MoveWordForward
        let mut goto_overrides = KeyTrieNode::new("Goto overrides");
        goto_overrides.insert(KeyEvent::char('x'), KeyTrie::Leaf(EditorAction::MoveWordForward));
        normal_overrides.insert(KeyEvent::char('g'), KeyTrie::Node(goto_overrides));

        let mut delta = HashMap::new();
        delta.insert(Mode::Normal, KeyTrie::Node(normal_overrides));

        merge_keys(&mut keymap, delta);

        let mut root = KeyTrieRoot::new(keymap);

        // 'w' should now be MoveLeft
        assert_eq!(
            root.get(Mode::Normal, KeyEvent::char('w')),
            KeymapResult::Matched(EditorAction::MoveLeft)
        );

        // 'C-s' should be SelectAll
        assert_eq!(
            root.get(Mode::Normal, KeyEvent::from_str("C-s").unwrap()),
            KeymapResult::Matched(EditorAction::SelectAll)
        );

        // 'g g' should still be GotoFileStart (preserved from default keymap!)
        let res_g = root.get(Mode::Normal, KeyEvent::char('g'));
        assert!(matches!(res_g, KeymapResult::Pending(_)));
        assert_eq!(
            root.get(Mode::Normal, KeyEvent::char('g')),
            KeymapResult::Matched(EditorAction::GotoFileStart)
        );

        // 'g x' should be MoveWordForward (new merged subnode entry!)
        let _ = root.get(Mode::Normal, KeyEvent::char('g'));
        assert_eq!(
            root.get(Mode::Normal, KeyEvent::char('x')),
            KeymapResult::Matched(EditorAction::MoveWordForward)
        );
    }

    #[test]
    fn custom_keybinding_via_remap() {
        let mut root = KeyTrieRoot::new(default_keymap());

        // Remap C-s to SelectAll in Normal mode
        root.remap(Mode::Normal, "C-s", EditorAction::SelectAll).unwrap();
        assert_eq!(
            root.get(Mode::Normal, KeyEvent::from_str("C-s").unwrap()),
            KeymapResult::Matched(EditorAction::SelectAll)
        );

        // Remap multi-key chord "g z" to ToggleCase
        root.remap(Mode::Normal, "g z", EditorAction::ToggleCase).unwrap();
        let _ = root.get(Mode::Normal, KeyEvent::char('g'));
        assert_eq!(
            root.get(Mode::Normal, KeyEvent::char('z')),
            KeymapResult::Matched(EditorAction::ToggleCase)
        );

        // Existing "g e" should still work
        let _ = root.get(Mode::Normal, KeyEvent::char('g'));
        assert_eq!(
            root.get(Mode::Normal, KeyEvent::char('e')),
            KeymapResult::Matched(EditorAction::GotoFileEnd)
        );
    }
}
