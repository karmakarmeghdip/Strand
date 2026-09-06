pub mod buffers;
pub mod commands;
pub mod files;
pub mod view;

pub use buffers::show_buffer_picker;
pub use commands::show_command_palette;
pub use files::show_file_picker;
pub use view::CommandPaletteDialog;

/// Represents an item displayed and filtered inside the command palette dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteItem {
    /// Unique identifier or path or command text.
    pub id: String,
    /// Primary title shown in the list row, e.g. "main.rs" or ":w".
    pub title: String,
    /// Optional subtitle/directory/hint, e.g. "src" or "Write buffer to disk".
    pub subtitle: Option<String>,
    /// Optional right-aligned status badge, e.g. "[+]" or "[ro]".
    pub badge: Option<String>,
    /// Search key used for fuzzy matching.
    pub search_key: String,
    /// Whether this item represents a filesystem path (enables path-aware scoring in nucleo).
    pub is_path: bool,
}

impl PaletteItem {
    pub fn new(id: impl Into<String>, title: impl Into<String>, search_key: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            badge: None,
            search_key: search_key.into(),
            is_path: false,
        }
    }

    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn with_badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn with_is_path(mut self, is_path: bool) -> Self {
        self.is_path = is_path;
        self
    }
}

/// The mode/purpose of the command palette dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteMode {
    FilePicker,
    BufferPicker,
    Command,
}
