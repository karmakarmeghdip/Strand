use std::collections::HashMap;
use gtk::gdk;
use gtk::gdk::prelude::*;

/// Key-value store for editor registers, mirroring `helix-view::register`.
///
/// Each register corresponds to a `char`. Most characters (like `'a'..='z'`)
/// store arbitrary text. Special registers:
/// - `'"'`: Default unnamed register (used when no register is specified)
/// - `'0'`: Yank register (stores last yanked text; not overwritten by delete/change)
/// - `'_'`: Black hole register (all writes are discarded, all reads return `""`)
/// - `'+'`: System clipboard (backed by `gdk::Display::default().clipboard()`)
/// - `'*'`: Primary selection clipboard (backed by `gdk::Display::default().primary_clipboard()`)
#[derive(Debug, Clone)]
pub struct Registers {
    inner: HashMap<char, String>,
}

impl Default for Registers {
    fn default() -> Self {
        Self::new()
    }
}

impl Registers {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    /// Read the text content of register `name`.
    ///
    /// Returns an empty string if the register has not been written to or if `name` is `'_'`.
    pub fn read(&self, name: char) -> &str {
        match name {
            '_' => "",
            _ => self.inner.get(&name).map(|s| s.as_str()).unwrap_or(""),
        }
    }

    /// Write text to register `name`.
    ///
    /// Special registers:
    /// - `'_'`: Discarded.
    /// - `'+'`: Written to GDK system clipboard and cached in memory.
    /// - `'*'`: Written to GDK primary clipboard (if available) and cached in memory.
    /// - Other chars: Stored in memory.
    pub fn write(&mut self, name: char, value: impl Into<String>) {
        let value = value.into();
        match name {
            '_' => {
                // Black hole register: discarded
            }
            '+' => {
                if let Some(display) = gdk::Display::default() {
                    display.clipboard().set_text(&value);
                }
                self.inner.insert('+', value);
            }
            '*' => {
                if let Some(display) = gdk::Display::default() {
                    display.primary_clipboard().set_text(&value);
                }
                self.inner.insert('*', value);
            }
            _ => {
                self.inner.insert(name, value);
            }
        }
    }

    /// Manually update clipboard contents (e.g. from async clipboard notification).
    pub fn set_clipboard(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.inner.insert('+', text.clone());
        self.inner.insert('*', text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_register() {
        let mut reg = Registers::new();
        assert_eq!(reg.read('"'), "");
        reg.write('"', "sample text");
        assert_eq!(reg.read('"'), "sample text");
    }

    #[test]
    fn test_yank_register() {
        let mut reg = Registers::new();
        assert_eq!(reg.read('0'), "");
        reg.write('0', "yanked text");
        assert_eq!(reg.read('0'), "yanked text");
        // default register should not be affected by direct write to '0'
        assert_eq!(reg.read('"'), "");
    }

    #[test]
    fn test_black_hole_register() {
        let mut reg = Registers::new();
        assert_eq!(reg.read('_'), "");
        reg.write('_', "never saved");
        assert_eq!(reg.read('_'), "");
    }

    #[test]
    fn test_named_registers() {
        let mut reg = Registers::new();
        reg.write('a', "alpha");
        reg.write('b', "beta");
        assert_eq!(reg.read('a'), "alpha");
        assert_eq!(reg.read('b'), "beta");
        assert_eq!(reg.read('c'), "");
    }

    #[test]
    fn test_clipboard_registers() {
        let mut reg = Registers::new();
        reg.write('+', "system clip");
        assert_eq!(reg.read('+'), "system clip");

        reg.write('*', "primary clip");
        assert_eq!(reg.read('*'), "primary clip");

        reg.set_clipboard("external clip");
        assert_eq!(reg.read('+'), "external clip");
        assert_eq!(reg.read('*'), "external clip");
    }

    #[test]
    fn test_overwrite_register() {
        let mut reg = Registers::new();
        reg.write('x', "first");
        assert_eq!(reg.read('x'), "first");
        reg.write('x', "second");
        assert_eq!(reg.read('x'), "second");
    }
}
