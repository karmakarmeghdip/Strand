#![allow(dead_code)]
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
    GotoNextBuffer,         // gn
    GotoPreviousBuffer,     // gp

    // Primitives
    OpenBelow,         // o
    OpenAbove,         // O
    Replace,           // r<char>
    SelectAll,         // %
    CollapseSelection, // ;
    FlipSelection,     // Alt-;
    ToggleCase,        // ~

    // Buffers & Files
    BufferClose,
    FileSave,
    FileSaveAs,
    FileOpen,
    OpenFilePicker,
    OpenBufferPicker,
    CommandPalette,

    // Structural / palette
    Noop,
}

impl EditorAction {
    pub fn name(&self) -> &'static str {
        match self {
            Self::MoveLeft => "move_left",
            Self::MoveRight => "move_right",
            Self::MoveUp => "move_up",
            Self::MoveDown => "move_down",
            Self::MoveWordForward => "move_word_forward",
            Self::MoveWordBackward => "move_word_backward",
            Self::MoveWordEnd => "move_word_end",
            Self::SelectLine => "select_line",
            Self::EnterInsert => "enter_insert",
            Self::EnterInsertAfter => "enter_insert_after",
            Self::InsertAtLineStart => "insert_at_line_start",
            Self::InsertAtLineEnd => "insert_at_line_end",
            Self::EnterSelect => "enter_select",
            Self::ExitToNormal => "exit_to_normal",
            Self::DeleteSelection => "delete_selection",
            Self::ChangeSelection => "change_selection",
            Self::YankSelection => "yank_selection",
            Self::PasteAfter => "paste_after",
            Self::PasteBefore => "paste_before",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::GotoFileStart => "goto_file_start",
            Self::GotoFileEnd => "goto_file_end",
            Self::GotoLineStart => "goto_line_start",
            Self::GotoLineEnd => "goto_line_end",
            Self::GotoFirstNonWhitespace => "goto_first_nonwhitespace",
            Self::GotoNextBuffer => "goto_next_buffer",
            Self::GotoPreviousBuffer => "goto_previous_buffer",
            Self::OpenBelow => "open_below",
            Self::OpenAbove => "open_above",
            Self::Replace => "replace",
            Self::SelectAll => "select_all",
            Self::CollapseSelection => "collapse_selection",
            Self::FlipSelection => "flip_selection",
            Self::ToggleCase => "toggle_case",
            Self::MatchBrackets => "match_brackets",
            Self::SurroundAdd => "surround_add",
            Self::SurroundDelete => "surround_delete",
            Self::SurroundReplace => "surround_replace",
            Self::SelectTextObjectAround => "select_textobject_around",
            Self::SelectTextObjectInner => "select_textobject_inner",
            Self::SelectRegister => "select_register",
            Self::YankToClipboard => "yank_to_clipboard",
            Self::PasteClipboardAfter => "paste_clipboard_after",
            Self::PasteClipboardBefore => "paste_clipboard_before",
            Self::BufferClose => "buffer_close",
            Self::FileSave => "file_save",
            Self::FileSaveAs => "file_save_as",
            Self::FileOpen => "file_open",
            Self::OpenFilePicker => "file_picker",
            Self::OpenBufferPicker => "buffer_picker",
            Self::CommandPalette => "command_palette",
            Self::Noop => "no_op",
        }
    }

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
            Self::GotoNextBuffer => "goto next buffer",
            Self::GotoPreviousBuffer => "goto previous buffer",
            Self::BufferClose => "close current buffer",
            Self::FileSave => "save current file",
            Self::FileSaveAs => "save file as",
            Self::FileOpen => "open file",
            Self::OpenFilePicker => "open file picker",
            Self::OpenBufferPicker => "open buffer picker",
            Self::CommandPalette => "command palette / prompt",
            Self::Noop => "no operation",
        }
    }
}

impl fmt::Display for EditorAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for EditorAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_lowercase();
        match normalized.as_str() {
            // Strand actions & Helix command aliases
            "move_left" | "move_char_left" => Ok(Self::MoveLeft),
            "move_right" | "move_char_right" => Ok(Self::MoveRight),
            "move_up" | "move_line_up" | "move_visual_line_up" => Ok(Self::MoveUp),
            "move_down" | "move_line_down" | "move_visual_line_down" => Ok(Self::MoveDown),
            "move_word_forward" | "move_next_word_start" => Ok(Self::MoveWordForward),
            "move_word_backward" | "move_prev_word_start" => Ok(Self::MoveWordBackward),
            "move_word_end" | "move_next_word_end" => Ok(Self::MoveWordEnd),
            "select_line" | "extend_line_below" => Ok(Self::SelectLine),
            "enter_insert" | "insert_mode" => Ok(Self::EnterInsert),
            "enter_insert_after" | "append_mode" => Ok(Self::EnterInsertAfter),
            "insert_at_line_start" => Ok(Self::InsertAtLineStart),
            "insert_at_line_end" => Ok(Self::InsertAtLineEnd),
            "enter_select" | "select_mode" => Ok(Self::EnterSelect),
            "exit_to_normal" | "normal_mode" => Ok(Self::ExitToNormal),
            "delete_selection" | "delete" => Ok(Self::DeleteSelection),
            "change_selection" | "change" => Ok(Self::ChangeSelection),
            "yank_selection" | "yank" => Ok(Self::YankSelection),
            "paste_after" => Ok(Self::PasteAfter),
            "paste_before" => Ok(Self::PasteBefore),
            "undo" => Ok(Self::Undo),
            "redo" => Ok(Self::Redo),
            "goto_file_start" => Ok(Self::GotoFileStart),
            "goto_file_end" | "goto_last_line" => Ok(Self::GotoFileEnd),
            "goto_line_start" => Ok(Self::GotoLineStart),
            "goto_line_end" => Ok(Self::GotoLineEnd),
            "goto_first_nonwhitespace" => Ok(Self::GotoFirstNonWhitespace),
            "open_below" => Ok(Self::OpenBelow),
            "open_above" => Ok(Self::OpenAbove),
            "replace" => Ok(Self::Replace),
            "select_all" => Ok(Self::SelectAll),
            "collapse_selection" => Ok(Self::CollapseSelection),
            "flip_selection" | "flip_selections" => Ok(Self::FlipSelection),
            "toggle_case" | "switch_case" => Ok(Self::ToggleCase),
            "match_brackets" => Ok(Self::MatchBrackets),
            "surround_add" => Ok(Self::SurroundAdd),
            "surround_delete" => Ok(Self::SurroundDelete),
            "surround_replace" => Ok(Self::SurroundReplace),
            "select_textobject_around" => Ok(Self::SelectTextObjectAround),
            "select_textobject_inner" => Ok(Self::SelectTextObjectInner),
            "select_register" => Ok(Self::SelectRegister),
            "yank_to_clipboard" | "yank_main_selection_to_clipboard" => Ok(Self::YankToClipboard),
            "paste_clipboard_after" => Ok(Self::PasteClipboardAfter),
            "paste_clipboard_before" => Ok(Self::PasteClipboardBefore),
            "goto_next_buffer" | "buffer_next" | "bnext" => Ok(Self::GotoNextBuffer),
            "goto_previous_buffer" | "buffer_prev" | "buffer_previous" | "bprev" => Ok(Self::GotoPreviousBuffer),
            "buffer_close" | "bclose" => Ok(Self::BufferClose),
            "file_save" | "save" | "write" => Ok(Self::FileSave),
            "file_save_as" | "save_as" | "write_as" => Ok(Self::FileSaveAs),
            "file_open" | "open" => Ok(Self::FileOpen),
            "file_picker" | "open_file_picker" => Ok(Self::OpenFilePicker),
            "buffer_picker" | "open_buffer_picker" => Ok(Self::OpenBufferPicker),
            "command_palette" | "command_mode" => Ok(Self::CommandPalette),
            "no_op" | "noop" => Ok(Self::Noop),
            unknown => Err(format!("unknown editor action or command '{unknown}'")),
        }
    }
}

impl Serialize for EditorAction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for EditorAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_action_parsing_and_serde() {
        assert_eq!("move_left".parse::<EditorAction>().unwrap(), EditorAction::MoveLeft);
        assert_eq!("move_char_left".parse::<EditorAction>().unwrap(), EditorAction::MoveLeft);
        assert_eq!("normal_mode".parse::<EditorAction>().unwrap(), EditorAction::ExitToNormal);
        assert_eq!("select_all".parse::<EditorAction>().unwrap(), EditorAction::SelectAll);

        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct ActionWrap {
            action: EditorAction,
        }

        let wrap = ActionWrap {
            action: EditorAction::SelectAll,
        };
        let toml_str = toml::to_string(&wrap).unwrap();
        assert!(toml_str.contains("action = \"select_all\""));

        let deserialized: ActionWrap = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized, wrap);
    }
}
