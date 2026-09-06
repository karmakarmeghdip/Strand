#![allow(dead_code)]
use std::fmt;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Mode {
    Normal = 0,
    Select = 1,
    Insert = 2,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mode::Normal => f.write_str("normal"),
            Mode::Select => f.write_str("select"),
            Mode::Insert => f.write_str("insert"),
        }
    }
}

/// Helix-style editor actions for Phase 2 fast-path.
///
/// These are executed imperatively on `GtkSourceBuffer` via `GtkTextIter`
/// in the `GtkEventControllerKey` CAPTURE handler — never via Relm4 async queue.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EditorAction {
    // Movement (fast-path, handled via GtkTextIter)
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveWordForward,  // w
    MoveWordBackward, // b
    MoveWordEnd,      // e
    SelectLine,       // x — extend/select line

    // Mode switches (structural — via Relm4 message)
    EnterInsert,
    EnterInsertAfter, // a (optional)
    InsertAtLineStart, // I
    InsertAtLineEnd,   // A
    EnterSelect,      // v
    ExitToNormal,     // Esc

    // Edit (fast-path on buffer)
    DeleteSelection, // d
    ChangeSelection, // c (delete + insert)
    YankSelection,   // y
    PasteAfter,      // p
    PasteBefore,     // P
    Undo,            // u
    Redo,            // U

    // Match actions (m prefix)
    MatchBrackets,
    SurroundAdd(char),
    SurroundDelete(char),
    SurroundReplace(char, char),
    SelectTextObjectAround(char),
    SelectTextObjectInner(char),

    // Structural / palette
    Noop,
}

impl EditorAction {
    pub fn doc(&self) -> &'static str {
        match self {
            Self::MoveLeft => "move left",
            Self::MoveRight => "move right",
            Self::MoveUp => "move up",
            Self::MoveDown => "move down",
            Self::MoveWordForward => "move to next word start",
            Self::MoveWordBackward => "move to previous word start",
            Self::MoveWordEnd => "move to next word end",
            Self::SelectLine => "select line",
            Self::EnterInsert => "enter insert mode",
            Self::EnterInsertAfter => "append after cursor",
            Self::InsertAtLineStart => "insert at line start",
            Self::InsertAtLineEnd => "insert at line end",
            Self::EnterSelect => "enter select mode",
            Self::ExitToNormal => "enter normal mode",
            Self::DeleteSelection => "delete selection",
            Self::ChangeSelection => "change selection",
            Self::YankSelection => "yank selection",
            Self::PasteAfter => "paste after",
            Self::PasteBefore => "paste before",
            Self::Undo => "undo change",
            Self::Redo => "redo change",
            Self::MatchBrackets => "goto matching bracket",
            Self::SurroundAdd(_) => "surround selection",
            Self::SurroundDelete(_) => "delete surround pair",
            Self::SurroundReplace(_, _) => "replace surround pair",
            Self::SelectTextObjectAround(_) => "select around textobject",
            Self::SelectTextObjectInner(_) => "select inside textobject",
            Self::Noop => "no operation",
        }
    }
}
