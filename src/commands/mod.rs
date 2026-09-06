pub mod edit;
pub mod motion;

use crate::components::editor::controller::KeyHandleResult;
use crate::editor::state::EditorState;
use crate::keymap::{EditorAction, Mode};

/// Unified command context mirroring Helix's `helix-term::commands::Context<'a>`.
pub struct Context<'a> {
    pub state: &'a mut EditorState,
    pub buffer: &'a gtk::TextBuffer,
    pub count: usize,
    pub register: char,
}

impl<'a> Context<'a> {
    pub fn new(
        state: &'a mut EditorState,
        buffer: &'a gtk::TextBuffer,
        count: usize,
        register: char,
    ) -> Self {
        Self {
            state,
            buffer,
            count,
            register,
        }
    }

    #[inline]
    pub fn count(&self) -> usize {
        self.count
    }

    #[inline]
    pub fn register(&self) -> char {
        self.register
    }

    #[inline]
    pub fn extend(&self) -> bool {
        self.state.mode == Mode::Select
    }

    #[inline]
    pub fn on_next_key<F>(&mut self, f: F)
    where
        F: FnOnce(&mut EditorState, &gtk::TextBuffer, crate::keymap::KeyEvent) -> KeyHandleResult
            + 'static,
    {
        self.state.on_next_key(f);
    }
}

/// Central command dispatch function mapping `EditorAction` to its command handler.
pub fn dispatch(action: EditorAction, cx: &mut Context) -> KeyHandleResult {
    match action {
        // Motions
        EditorAction::MoveLeft => motion::move_left(cx),
        EditorAction::MoveRight => motion::move_right(cx),
        EditorAction::MoveUp => motion::move_up(cx),
        EditorAction::MoveDown => motion::move_down(cx),
        EditorAction::MoveWordForward => motion::move_word_forward(cx),
        EditorAction::MoveWordBackward => motion::move_word_backward(cx),
        EditorAction::MoveWordEnd => motion::move_word_end(cx),
        EditorAction::SelectLine => motion::select_line(cx),
        EditorAction::MatchBrackets => motion::match_brackets(cx),

        // Mode switches
        EditorAction::EnterInsert => edit::enter_insert(cx),
        EditorAction::EnterInsertAfter => edit::enter_insert_after(cx),
        EditorAction::InsertAtLineStart => edit::insert_at_line_start(cx),
        EditorAction::InsertAtLineEnd => edit::insert_at_line_end(cx),
        EditorAction::EnterSelect => edit::enter_select(cx),
        EditorAction::ExitToNormal => edit::exit_to_normal(cx),

        // Edits & registers
        EditorAction::DeleteSelection => edit::delete_selection(cx),
        EditorAction::ChangeSelection => edit::change_selection(cx),
        EditorAction::YankSelection => edit::yank_selection(cx),
        EditorAction::PasteAfter => edit::paste_after(cx),
        EditorAction::PasteBefore => edit::paste_before(cx),
        EditorAction::Undo => edit::undo(cx),
        EditorAction::Redo => edit::redo(cx),
        EditorAction::SelectRegister => edit::select_register(cx),
        EditorAction::YankToClipboard => edit::yank_to_clipboard(cx),
        EditorAction::PasteClipboardAfter => edit::paste_clipboard_after(cx),
        EditorAction::PasteClipboardBefore => edit::paste_clipboard_before(cx),

        // Surround & textobjects
        EditorAction::SurroundAdd => edit::surround_add(cx),
        EditorAction::SurroundDelete => edit::surround_delete(cx),
        EditorAction::SurroundReplace => edit::surround_replace(cx),
        EditorAction::SelectTextObjectAround => edit::select_textobject_around(cx),
        EditorAction::SelectTextObjectInner => edit::select_textobject_inner(cx),

        EditorAction::Noop => KeyHandleResult::Stop,
    }
}
