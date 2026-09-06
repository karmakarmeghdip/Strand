use std::collections::HashMap;

use super::actions::{EditorAction, Mode};
use super::input::{KeyCode, KeyEvent, KeyModifiers};
use super::trie::{KeyTrie, KeyTrieNode};

/// Build the Helix-style Match (`m`) submap:
/// - `mm` -> goto matching bracket
/// - `ms` -> surround selection (waits for char via on_next_key)
/// - `mr` -> replace surround pair (waits for from/to via on_next_key)
/// - `md` -> delete surround pair (waits for char via on_next_key)
/// - `ma` -> select around textobject (waits for obj via on_next_key)
/// - `mi` -> select inside textobject (waits for obj via on_next_key)
pub fn build_match_node() -> KeyTrieNode {
    let mut match_node = KeyTrieNode::new("Match");
    let leaf = KeyTrie::Leaf;

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
    let leaf = KeyTrie::Leaf;
    space_node.insert(KeyEvent::char('y'), leaf(EditorAction::YankToClipboard));
    space_node.insert(KeyEvent::char('p'), leaf(EditorAction::PasteClipboardAfter));
    space_node.insert(KeyEvent::char('P'), leaf(EditorAction::PasteClipboardBefore));
    space_node
}

/// Helper to construct Helix goto mode node:
/// - `g` -> goto file start (or line n if count provided)
/// - `e` -> goto file end
/// - `h` -> goto line start
/// - `l` -> goto line end
/// - `s` -> goto first non-whitespace
pub fn build_goto_node() -> KeyTrieNode {
    let mut goto_node = KeyTrieNode::new("Goto");
    let leaf = KeyTrie::Leaf;
    goto_node.insert(KeyEvent::char('g'), leaf(EditorAction::GotoFileStart));
    goto_node.insert(KeyEvent::char('e'), leaf(EditorAction::GotoFileEnd));
    goto_node.insert(KeyEvent::char('h'), leaf(EditorAction::GotoLineStart));
    goto_node.insert(KeyEvent::char('l'), leaf(EditorAction::GotoLineEnd));
    goto_node.insert(KeyEvent::char('s'), leaf(EditorAction::GotoFirstNonWhitespace));
    goto_node
}

/// Build the default keymap for all editor modes.
///
/// Normal:
///   h/j/k/l, w/b/e, x, i, I, a, A, d/c/y/p/P, u/U, o/O, r, %, ;, Alt-;, ~, v, Esc
///   g -> goto node (gg = file start, ge = file end, gh/gl = line start/end, gs = non-ws)
///   m -> match node (mm = match brackets, ms = surround add, mr = replace, md = delete, ma/mi = textobjects)
///   Space -> space node (y = yank to clipboard, p/P = paste from clipboard)
/// Insert:
///   Esc -> normal
/// Select:
///   same motions as normal but they extend selection; Esc -> normal
pub fn default_keymap() -> HashMap<Mode, KeyTrie> {
    let mut m = HashMap::new();
    let leaf = KeyTrie::Leaf;

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
    normal.insert(KeyEvent::char('P'), leaf(EditorAction::PasteBefore));
    normal.insert(KeyEvent::char('u'), leaf(EditorAction::Undo));
    normal.insert(KeyEvent::char('U'), leaf(EditorAction::Redo));
    normal.insert(KeyEvent::char('"'), leaf(EditorAction::SelectRegister));
    normal.insert(KeyEvent::char('o'), leaf(EditorAction::OpenBelow));
    normal.insert(KeyEvent::char('O'), leaf(EditorAction::OpenAbove));
    normal.insert(KeyEvent::char('r'), leaf(EditorAction::Replace));
    normal.insert(KeyEvent::char('%'), leaf(EditorAction::SelectAll));
    normal.insert(KeyEvent::char(';'), leaf(EditorAction::CollapseSelection));
    normal.insert(
        KeyEvent::new(KeyCode::Char(';'), KeyModifiers::ALT),
        leaf(EditorAction::FlipSelection),
    );
    normal.insert(KeyEvent::char('~'), leaf(EditorAction::ToggleCase));

    // g prefix
    normal.insert(KeyEvent::char('g'), KeyTrie::Node(build_goto_node()));

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

    // Arrow keys
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
    select.insert(KeyEvent::char('d'), leaf(EditorAction::DeleteSelection));
    select.insert(KeyEvent::char('c'), leaf(EditorAction::ChangeSelection));
    select.insert(KeyEvent::char('y'), leaf(EditorAction::YankSelection));
    select.insert(KeyEvent::char('p'), leaf(EditorAction::PasteAfter));
    select.insert(KeyEvent::char('P'), leaf(EditorAction::PasteBefore));
    select.insert(KeyEvent::char('"'), leaf(EditorAction::SelectRegister));
    select.insert(KeyEvent::char('o'), leaf(EditorAction::OpenBelow));
    select.insert(KeyEvent::char('O'), leaf(EditorAction::OpenAbove));
    select.insert(KeyEvent::char('r'), leaf(EditorAction::Replace));
    select.insert(KeyEvent::char('%'), leaf(EditorAction::SelectAll));
    select.insert(KeyEvent::char(';'), leaf(EditorAction::CollapseSelection));
    select.insert(
        KeyEvent::new(KeyCode::Char(';'), KeyModifiers::ALT),
        leaf(EditorAction::FlipSelection),
    );
    select.insert(KeyEvent::char('~'), leaf(EditorAction::ToggleCase));
    select.insert(KeyEvent::char('g'), KeyTrie::Node(build_goto_node()));
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
    select.insert(KeyEvent::char('m'), KeyTrie::Node(build_match_node()));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_keymap_structure() {
        let keymap = default_keymap();
        assert!(keymap.contains_key(&Mode::Normal));
        assert!(keymap.contains_key(&Mode::Insert));
        assert!(keymap.contains_key(&Mode::Select));

        let normal_node = keymap.get(&Mode::Normal).unwrap().node().unwrap();
        assert_eq!(normal_node.name, "Normal mode");
        assert!(normal_node.map.contains_key(&KeyEvent::char('h')));
        assert!(normal_node.map.contains_key(&KeyEvent::char('g')));
        assert!(normal_node.map.contains_key(&KeyEvent::char('m')));
        assert!(normal_node.map.contains_key(&KeyEvent::char(' ')));
    }

    #[test]
    fn test_goto_node() {
        let goto = build_goto_node();
        assert_eq!(goto.name, "Goto");
        assert_eq!(
            goto.map.get(&KeyEvent::char('g')),
            Some(&KeyTrie::Leaf(EditorAction::GotoFileStart))
        );
        assert_eq!(
            goto.map.get(&KeyEvent::char('e')),
            Some(&KeyTrie::Leaf(EditorAction::GotoFileEnd))
        );
        assert_eq!(
            goto.map.get(&KeyEvent::char('h')),
            Some(&KeyTrie::Leaf(EditorAction::GotoLineStart))
        );
        assert_eq!(
            goto.map.get(&KeyEvent::char('l')),
            Some(&KeyTrie::Leaf(EditorAction::GotoLineEnd))
        );
        assert_eq!(
            goto.map.get(&KeyEvent::char('s')),
            Some(&KeyTrie::Leaf(EditorAction::GotoFirstNonWhitespace))
        );
    }

    #[test]
    fn test_space_node() {
        let space = build_space_node();
        assert_eq!(space.name, "Space");
        assert_eq!(
            space.map.get(&KeyEvent::char('y')),
            Some(&KeyTrie::Leaf(EditorAction::YankToClipboard))
        );
        assert_eq!(
            space.map.get(&KeyEvent::char('p')),
            Some(&KeyTrie::Leaf(EditorAction::PasteClipboardAfter))
        );
        assert_eq!(
            space.map.get(&KeyEvent::char('P')),
            Some(&KeyTrie::Leaf(EditorAction::PasteClipboardBefore))
        );
    }

    #[test]
    fn test_match_node() {
        let match_node = build_match_node();
        assert_eq!(match_node.name, "Match");
        assert_eq!(
            match_node.map.get(&KeyEvent::char('m')),
            Some(&KeyTrie::Leaf(EditorAction::MatchBrackets))
        );
        assert_eq!(
            match_node.map.get(&KeyEvent::char('s')),
            Some(&KeyTrie::Leaf(EditorAction::SurroundAdd))
        );
    }
}
