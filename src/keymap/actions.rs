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
    SurroundAdd,
    SurroundDelete,
    SurroundReplace,
    SelectTextObjectAround,
    SelectTextObjectInner,
    SelectRegister,       // " — select register
    YankToClipboard,      // <space>y — yank to system clipboard
    PasteClipboardAfter,  // <space>p — paste system clipboard after
    PasteClipboardBefore, // <space>P — paste system clipboard before

    // Goto actions (g prefix)
    GotoFileStart,          // gg (or <n>gg to jump to line n)
    GotoFileEnd,            // ge
    GotoLineStart,          // gh
    GotoLineEnd,            // gl
    GotoFirstNonWhitespace, // gs

    // Primitives
    OpenBelow,         // o
    OpenAbove,         // O
    Replace,           // r<char>
    SelectAll,         // %
    CollapseSelection, // ;
    FlipSelection,     // Alt-;
    ToggleCase,        // ~

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
            Self::GotoFileStart => "goto file start",
            Self::GotoFileEnd => "goto file end",
            Self::GotoLineStart => "goto line start",
            Self::GotoLineEnd => "goto line end",
            Self::GotoFirstNonWhitespace => "goto first non-whitespace character",
            Self::OpenBelow => "open newline below",
            Self::OpenAbove => "open newline above",
            Self::Replace => "replace character under cursor",
            Self::SelectAll => "select all",
            Self::CollapseSelection => "collapse selection to cursor",
            Self::FlipSelection => "flip selection anchor and head",
            Self::ToggleCase => "toggle case of selection",
            Self::MatchBrackets => "goto matching bracket",
            Self::SurroundAdd => "surround selection",
            Self::SurroundDelete => "delete surround pair",
            Self::SurroundReplace => "replace surround pair",
            Self::SelectTextObjectAround => "select around textobject",
            Self::SelectTextObjectInner => "select inside textobject",
            Self::SelectRegister => "select register",
            Self::YankToClipboard => "yank to clipboard",
            Self::PasteClipboardAfter => "paste clipboard after",
            Self::PasteClipboardBefore => "paste clipboard before",
            Self::Noop => "no operation",
        }
    }
}
