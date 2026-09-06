use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use gtk::prelude::TextBufferExt;
use sourceview5::prelude::BufferExt;

use crate::components::editor::theme::apply_theme;
use crate::components::statusline::{IndentStyle, LineEnding};

/// Unique identifier for an open document buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentId(pub usize);

impl std::fmt::Display for DocumentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Errors that can occur when opening, reading, or saving a document.
#[derive(Debug)]
pub enum DocumentError {
    Io(io::Error),
    IsADirectory(PathBuf),
    NoPathSet,
    InvalidEncoding,
}

impl std::fmt::Display for DocumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::IsADirectory(p) => write!(f, "'{}' is a directory", p.display()),
            Self::NoPathSet => write!(f, "no path set for document"),
            Self::InvalidEncoding => write!(f, "file contains invalid UTF-8 data"),
        }
    }
}

impl std::error::Error for DocumentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for DocumentError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// A document managed by the editor, backed by a `sourceview5::Buffer`.
///
/// Following AGENTS.md Invariant #1: `GtkSourceBuffer` is the single source of truth.
/// Selection and editing logic operates on this buffer.
pub struct Document {
    id: DocumentId,
    buffer: sourceview5::Buffer,
    path: Option<PathBuf>,
    workspace_root: PathBuf,
    encoding: String,
    last_saved_time: Option<SystemTime>,
    indent_style: IndentStyle,
    line_ending: LineEnding,
    read_only: bool,
    pub focused_at: std::time::Instant,
}

impl Document {
    /// Create a new scratch document without a backing file.
    pub fn new_scratch(id: DocumentId, workspace_root: PathBuf, theme_name: Option<&str>) -> Self {
        gtk::init().ok();
        let buffer = sourceview5::Buffer::new(None);
        buffer.set_enable_undo(true);
        buffer.set_highlight_syntax(true);
        buffer.set_highlight_matching_brackets(true);

        if let Some(theme) = theme_name {
            apply_theme(&buffer, theme);
        }

        Self {
            id,
            buffer,
            path: None,
            workspace_root,
            encoding: "UTF-8".to_string(),
            last_saved_time: None,
            indent_style: IndentStyle::Spaces(4),
            line_ending: LineEnding::Lf,
            read_only: false,
            focused_at: std::time::Instant::now(),
        }
    }

    /// Open a file from disk into a new `Document`.
    /// If the file does not exist, an empty buffer with `path` set is created.
    pub fn open(
        id: DocumentId,
        path: &Path,
        workspace_root: PathBuf,
        theme_name: Option<&str>,
    ) -> Result<Self, DocumentError> {
        gtk::init().ok();
        if path.is_dir() {
            return Err(DocumentError::IsADirectory(path.to_path_buf()));
        }

        let canonical_path = if path.exists() {
            path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
        } else if let Some(parent) = path.parent() {
            if let Ok(canon_parent) = parent.canonicalize() {
                if let Some(name) = path.file_name() {
                    canon_parent.join(name)
                } else {
                    path.to_path_buf()
                }
            } else {
                path.to_path_buf()
            }
        } else {
            path.to_path_buf()
        };

        let (content, read_only, last_saved_time, line_ending, indent_style) =
            if canonical_path.exists() {
                let bytes = fs::read(&canonical_path)?;
                let text = String::from_utf8(bytes).map_err(|_| DocumentError::InvalidEncoding)?;
                let metadata = fs::metadata(&canonical_path).ok();
                let ro = metadata
                    .as_ref()
                    .map(|m| m.permissions().readonly())
                    .unwrap_or(false);
                let mtime = metadata.and_then(|m| m.modified().ok());
                let le = detect_line_ending(&text);
                let ind = detect_indent_style(&text);
                (text, ro, mtime, le, ind)
            } else {
                (
                    String::new(),
                    false,
                    None,
                    LineEnding::Lf,
                    IndentStyle::Spaces(4),
                )
            };

        let buffer = sourceview5::Buffer::new(None);
        // Disable undo during initial content load so buffer starts clean
        buffer.set_enable_undo(false);
        buffer.set_text(&content);
        buffer.set_enable_undo(true);
        buffer.set_modified(false);
        buffer.set_highlight_syntax(true);
        buffer.set_highlight_matching_brackets(true);

        // Auto-detect syntax language via GtkSourceView LanguageManager
        let lang_mgr = sourceview5::LanguageManager::default();
        let filename = canonical_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if let Some(lang) = lang_mgr.guess_language(Some(filename), None) {
            buffer.set_language(Some(&lang));
        }

        if let Some(theme) = theme_name {
            apply_theme(&buffer, theme);
        }

        Ok(Self {
            id,
            buffer,
            path: Some(canonical_path),
            workspace_root,
            encoding: "UTF-8".to_string(),
            last_saved_time,
            indent_style,
            line_ending,
            read_only,
            focused_at: std::time::Instant::now(),
        })
    }

    #[inline]
    pub fn focused_at(&self) -> std::time::Instant {
        self.focused_at
    }

    #[inline]
    pub fn touch_focused(&mut self) {
        self.focused_at = std::time::Instant::now();
    }

    /// Save buffer contents to its backing file path.
    pub fn save(&mut self) -> Result<(), DocumentError> {
        let path = match self.path.as_ref() {
            Some(p) => p.clone(),
            None => return Err(DocumentError::NoPathSet),
        };

        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent)?;
        }

        let (start, end) = self.buffer.bounds();
        let text = self.buffer.text(&start, &end, false).to_string();

        // Write file atomically via temporary sibling file
        let temp_file = path.with_extension(format!("tmp_{}", std::process::id()));
        fs::write(&temp_file, &text)?;
        if fs::rename(&temp_file, &path).is_err() {
            // Fallback to direct write if atomic rename across filesystems fails
            let _ = fs::remove_file(&temp_file);
            fs::write(&path, &text)?;
        }

        self.buffer.set_modified(false);
        self.last_saved_time = Some(SystemTime::now());
        Ok(())
    }

    /// Save buffer contents to a new file path (`:w <path>`).
    pub fn save_as(&mut self, new_path: &Path) -> Result<(), DocumentError> {
        let resolved = if new_path.is_relative() {
            self.workspace_root.join(new_path)
        } else {
            new_path.to_path_buf()
        };

        self.path = Some(resolved);

        // Update syntax language for the new filename/extension
        let lang_mgr = sourceview5::LanguageManager::default();
        if let Some(filename) = self.path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str())
            && let Some(lang) = lang_mgr.guess_language(Some(filename), None)
        {
            self.buffer.set_language(Some(&lang));
        }

        self.save()
    }

    /// Reload buffer contents from disk.
    pub fn reload(&mut self) -> Result<(), DocumentError> {
        let path = match self.path.as_ref() {
            Some(p) => p.clone(),
            None => return Err(DocumentError::NoPathSet),
        };

        let bytes = fs::read(&path)?;
        let text = String::from_utf8(bytes).map_err(|_| DocumentError::InvalidEncoding)?;

        self.buffer.set_enable_undo(false);
        self.buffer.set_text(&text);
        self.buffer.set_enable_undo(true);
        self.buffer.set_modified(false);
        self.last_saved_time = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
        Ok(())
    }

    /// Document unique ID.
    pub fn id(&self) -> DocumentId {
        self.id
    }

    /// Backing `sourceview5::Buffer`.
    pub fn buffer(&self) -> &sourceview5::Buffer {
        &self.buffer
    }

    /// File path if this document is backed by a file on disk.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Workspace root for this document.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Update workspace root.
    pub fn set_workspace_root(&mut self, root: PathBuf) {
        self.workspace_root = root;
    }

    /// Human-friendly display name (relative to workspace root if inside workspace, or filename, or `[scratch]`).
    pub fn display_name(&self) -> String {
        match self.path.as_deref() {
            Some(path) => {
                if let Ok(rel) = path.strip_prefix(&self.workspace_root) {
                    rel.display().to_string()
                } else {
                    path.display().to_string()
                }
            }
            None => "[scratch]".to_string(),
        }
    }

    /// Relative path within workspace root, if path is inside workspace.
    pub fn relative_path(&self) -> Option<PathBuf> {
        self.path.as_ref().map(|p| {
            if let Ok(rel) = p.strip_prefix(&self.workspace_root) {
                rel.to_path_buf()
            } else {
                p.clone()
            }
        })
    }

    /// Filename without parent directory.
    pub fn file_name(&self) -> Option<String> {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
    }

    /// Whether this document is an unsaved scratch buffer.
    pub fn is_scratch(&self) -> bool {
        self.path.is_none()
    }

    /// Whether the buffer has unsaved changes.
    pub fn is_modified(&self) -> bool {
        self.buffer.is_modified()
    }

    /// Whether the backing file is read-only.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Character count in buffer is 0.
    pub fn is_empty(&self) -> bool {
        self.buffer.char_count() == 0
    }

    /// File text encoding (UTF-8).
    pub fn encoding(&self) -> &str {
        &self.encoding
    }

    /// Indentation style.
    pub fn indent_style(&self) -> IndentStyle {
        self.indent_style
    }

    /// Line ending style.
    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    /// Language name from GtkSourceView syntax highlighting (e.g. "Rust", "TOML").
    pub fn language_name(&self) -> Option<String> {
        self.buffer.language().map(|l| l.name().to_string())
    }

    /// Language ID from GtkSourceView syntax highlighting (e.g. "rust", "toml").
    pub fn language_id(&self) -> Option<String> {
        self.buffer.language().map(|l| l.id().to_string())
    }

    /// Complete text of the document.
    pub fn text(&self) -> String {
        let (start, end) = self.buffer.bounds();
        self.buffer.text(&start, &end, false).to_string()
    }
}

fn detect_line_ending(text: &str) -> LineEnding {
    if text.contains("\r\n") {
        LineEnding::Crlf
    } else {
        LineEnding::Lf
    }
}

fn detect_indent_style(text: &str) -> IndentStyle {
    let mut tab_count = 0;
    let mut space_2 = 0;
    let mut space_4 = 0;
    for line in text.lines().take(100) {
        if line.starts_with('\t') {
            tab_count += 1;
        } else if line.starts_with("    ") {
            space_4 += 1;
        } else if line.starts_with("  ") {
            space_2 += 1;
        }
    }
    if tab_count > space_4 && tab_count > space_2 {
        IndentStyle::Tabs
    } else if space_2 > space_4 {
        IndentStyle::Spaces(2)
    } else {
        IndentStyle::Spaces(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gtk::test]
    fn test_scratch_document_lifecycle() {
        let root = PathBuf::from("/tmp");
        let doc = Document::new_scratch(DocumentId(1), root, None);
        assert_eq!(doc.id(), DocumentId(1));
        assert!(doc.is_scratch());
        assert_eq!(doc.display_name(), "[scratch]");
        assert!(!doc.is_modified());
        assert!(doc.is_empty());
    }

    #[gtk::test]
    fn test_open_existing_file() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let cargo_path = root.join("Cargo.toml");
        let doc = Document::open(DocumentId(2), &cargo_path, root, None)
            .expect("Cargo.toml must open cleanly");

        assert_eq!(doc.id(), DocumentId(2));
        assert!(!doc.is_scratch());
        assert_eq!(doc.display_name(), "Cargo.toml");
        assert!(!doc.is_modified());
        assert!(!doc.is_empty());
        assert_eq!(doc.file_name(), Some("Cargo.toml".to_string()));
        assert_eq!(doc.relative_path(), Some(PathBuf::from("Cargo.toml")));
        // Syntax language should be detected as TOML
        if let Some(lang_id) = doc.language_id() {
            assert_eq!(lang_id, "toml");
        }
    }

    #[gtk::test]
    fn test_save_and_reload_document() {
        let temp_dir = std::env::temp_dir().join(format!("strand_test_{}", std::process::id()));
        fs::create_dir_all(&temp_dir).unwrap();
        let file_path = temp_dir.join("test_save.txt");

        let mut doc = Document::open(DocumentId(3), &file_path, temp_dir.clone(), None)
            .expect("Should open non-existent file as empty buffer");

        assert!(doc.is_empty());
        assert!(!doc.is_modified());

        // Modify buffer
        doc.buffer.set_text("Hello from Strand document test!");
        assert!(doc.is_modified());

        // Save
        doc.save().expect("Save must succeed");
        assert!(!doc.is_modified());
        assert!(file_path.exists());

        let read_back = fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_back, "Hello from Strand document test!");

        // Modify and reload
        doc.buffer.set_text("Discard me");
        assert!(doc.is_modified());
        doc.reload().expect("Reload must succeed");
        assert_eq!(doc.text(), "Hello from Strand document test!");
        assert!(!doc.is_modified());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
