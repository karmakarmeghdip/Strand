use std::num::NonZeroUsize;

use crate::components::editor::controller::KeyHandleResult;
use crate::editor::register::Registers;
use crate::keymap::{default_keymap, KeyEvent, KeyTrieRoot, Mode};

pub type OnKeyCallback =
    Box<dyn FnOnce(&mut EditorState, &gtk::TextBuffer, KeyEvent) -> KeyHandleResult>;

pub struct EditorState {
    pub mode: Mode,
    pub registers: Registers,
    pub selected_register: Option<char>,
    pub keymap: KeyTrieRoot,
    pub last_matched_bracket: Option<i32>,
    pub count: Option<NonZeroUsize>,
    pub on_next_key: Option<OnKeyCallback>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            registers: Registers::new(),
            selected_register: None,
            keymap: KeyTrieRoot::new(default_keymap()),
            last_matched_bracket: None,
            count: None,
            on_next_key: None,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.keymap.clear_pending();
        self.keymap.sticky = None;
        self.last_matched_bracket = None;
        self.count = None;
        self.on_next_key = None;
        self.selected_register = None;
    }

    pub fn on_next_key<F>(&mut self, f: F)
    where
        F: FnOnce(&mut EditorState, &gtk::TextBuffer, KeyEvent) -> KeyHandleResult + 'static,
    {
        self.on_next_key = Some(Box::new(f));
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}
