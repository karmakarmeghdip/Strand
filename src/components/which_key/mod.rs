pub mod view;

pub use view::WhichKeyView;

use std::collections::BTreeSet;

use crate::editor::register::Registers;
use crate::keymap::actions::EditorAction;
use crate::keymap::trie::{KeyCode, KeyEvent, KeyModifiers, KeyTrie, KeyTrieNode};

/// Represents a single shortcut item shown inside the Which-Key overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhichKeyEntry {
    /// Human-friendly key badge text, e.g. `"y"`, `"p, P"`, `"Esc"`.
    pub key_label: String,
    /// Action description or submenu title, e.g. `"yank to clipboard"`, `"Match"`.
    pub description: String,
    /// Whether this entry leads to a child trie node (submenu).
    pub is_submenu: bool,
}

/// The complete model for a Which-Key overlay presentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhichKeyData {
    /// Overlay title, e.g. `"SPACE"`, `"MATCH"`, `"GOTO"`, `"REGISTERS"`.
    pub title: String,
    /// Shortcut entries to display.
    pub entries: Vec<WhichKeyEntry>,
}

/// Format a `KeyEvent` into a clean, human-readable badge label.
pub fn format_key(key: &KeyEvent) -> String {
    let mut prefix = String::new();
    if key.modifiers.contains(KeyModifiers::SUPER) {
        prefix.push_str("Super-");
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        prefix.push_str("C-");
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        prefix.push_str("A-");
    }
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        prefix.push_str("S-");
    }

    let code_str = match key.code {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Delete => "Del".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::F(n) => format!("F{n}"),
        KeyCode::Null => "Null".to_string(),
    };

    format!("{prefix}{code_str}")
}

impl WhichKeyData {
    /// Build `WhichKeyData` from an active `KeyTrieNode`, mirroring Helix's `KeyTrieNode::infobox()`.
    pub fn from_trie_node(node: &KeyTrieNode) -> Self {
        let mut grouped: Vec<(BTreeSet<KeyEvent>, String, bool)> = Vec::new();

        for (&key, trie) in &node.map {
            let (desc, is_sub) = match trie {
                KeyTrie::Leaf(EditorAction::Noop) => continue,
                KeyTrie::Leaf(action) => (action.doc().to_string(), false),
                KeyTrie::Node(sub_node) => (sub_node.name.clone(), true),
            };

            if let Some((keys, _, _)) = grouped
                .iter_mut()
                .find(|(_, d, s)| *d == desc && *s == is_sub)
            {
                keys.insert(key);
            } else {
                let mut set = BTreeSet::new();
                set.insert(key);
                grouped.push((set, desc, is_sub));
            }
        }

        // Sort grouped entries by the first key in each group for predictable presentation
        grouped.sort_by(|(keys_a, _, _), (keys_b, _, _)| {
            keys_a.iter().next().cmp(&keys_b.iter().next())
        });

        let entries = grouped
            .into_iter()
            .map(|(keys, description, is_submenu)| {
                let key_label = keys
                    .iter()
                    .map(format_key)
                    .collect::<Vec<_>>()
                    .join(", ");
                WhichKeyEntry {
                    key_label,
                    description,
                    is_submenu,
                }
            })
            .collect();

        Self {
            title: node.name.clone(),
            entries,
        }
    }

    /// Build `WhichKeyData` for register inspection (`"`), mirroring Helix's `Info::from_registers`.
    pub fn from_registers(registers: &Registers) -> Self {
        let preview_items = registers.iter_preview();
        let entries = preview_items
            .into_iter()
            .map(|(ch, preview)| WhichKeyEntry {
                key_label: ch.to_string(),
                description: preview,
                is_submenu: false,
            })
            .collect();

        Self {
            title: "Registers".to_string(),
            entries,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::{build_match_node, build_space_node};

    #[test]
    fn test_format_key() {
        assert_eq!(format_key(&KeyEvent::char('a')), "a");
        assert_eq!(format_key(&KeyEvent::char(' ')), "Space");
        assert_eq!(
            format_key(&KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())),
            "Esc"
        );
        assert_eq!(
            format_key(&KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL)),
            "C-g"
        );
        assert_eq!(
            format_key(&KeyEvent::new(KeyCode::Left, KeyModifiers::empty())),
            "Left"
        );
    }

    #[test]
    fn test_from_space_node() {
        let node = build_space_node();
        let data = WhichKeyData::from_trie_node(&node);
        assert_eq!(data.title, "Space");
        assert!(!data.entries.is_empty());

        let labels: Vec<&str> = data.entries.iter().map(|e| e.key_label.as_str()).collect();
        assert!(labels.contains(&"y"));
        assert!(labels.contains(&"p"));
        assert!(labels.contains(&"P"));

        let y_entry = data.entries.iter().find(|e| e.key_label == "y").unwrap();
        assert_eq!(y_entry.description, "yank to clipboard");
        assert!(!y_entry.is_submenu);
    }

    #[test]
    fn test_from_match_node() {
        let node = build_match_node();
        let data = WhichKeyData::from_trie_node(&node);
        assert_eq!(data.title, "Match");

        let m_entry = data.entries.iter().find(|e| e.key_label == "m").unwrap();
        assert_eq!(m_entry.description, "goto matching bracket");
        assert!(!m_entry.is_submenu);

        let s_entry = data.entries.iter().find(|e| e.key_label == "s").unwrap();
        assert_eq!(s_entry.description, "surround selection");
    }

    #[test]
    fn test_from_registers() {
        let mut reg = Registers::new();
        reg.write('"', "yanked text");
        reg.write('a', "custom register content");

        let data = WhichKeyData::from_registers(&reg);
        assert_eq!(data.title, "Registers");

        let def_entry = data.entries.iter().find(|e| e.key_label == "\"").unwrap();
        assert!(def_entry.description.contains("yanked text"));

        let a_entry = data.entries.iter().find(|e| e.key_label == "a").unwrap();
        assert_eq!(a_entry.description, "custom register content");
    }
}
