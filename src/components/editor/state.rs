#![allow(dead_code)]
use crate::keymap::{default_keymap, KeyTrieRoot, Mode};

pub struct EditorState {
    pub mode: Mode,
    pub clipboard: String,
    pub keymap: KeyTrieRoot,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            clipboard: String::new(),
            keymap: KeyTrieRoot::new(default_keymap()),
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.keymap.clear_pending();
        self.keymap.sticky = None;
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}
