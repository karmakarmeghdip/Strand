use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use crate::components::editor::controller::KeyHandleResult;
use crate::components::statusline::DiagnosticSeverity;
use crate::components::which_key::WhichKeyData;
use crate::config::Config;
use crate::editor::document::{Document, DocumentError, DocumentId};
use crate::editor::register::Registers;
use crate::editor::workspace::Workspace;
use crate::keymap::{KeyEvent, KeyTrieRoot, Mode};

/// Errors encountered when closing a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseError {
    DoesNotExist,
    Modified,
}

impl std::fmt::Display for CloseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DoesNotExist => write!(f, "document does not exist"),
            Self::Modified => write!(f, "buffer has unsaved changes (use :q! or force to close)"),
        }
    }
}

impl std::error::Error for CloseError {}

pub type OnKeyCallback =
    Box<dyn FnOnce(&mut EditorState, &gtk::TextBuffer, KeyEvent) -> KeyHandleResult>;

/// Central editor state managing editing mode, registers, keymap,
/// workspace project root, and the collection of open documents.
pub struct EditorState {
    pub mode: Mode,
    pub registers: Registers,
    pub selected_register: Option<char>,
    pub keymap: KeyTrieRoot,
    pub last_matched_bracket: Option<i32>,
    pub count: Option<NonZeroUsize>,
    pub on_next_key: Option<OnKeyCallback>,
    pub which_key: Option<WhichKeyData>,
    pub status_msg: Option<(String, DiagnosticSeverity)>,
    pub config: Config,

    // Project & Documents
    pub workspace: Workspace,
    pub documents: BTreeMap<DocumentId, Document>,
    pub current_document_id: DocumentId,
    pub access_history: Vec<DocumentId>,
    next_document_id: usize,
}

impl EditorState {
    pub fn new() -> Self {
        Self::with_config(Config::load_default().unwrap_or_default())
    }

    pub fn with_config(config: Config) -> Self {
        Self::with_config_and_workspace(config, Workspace::from_path_or_cwd(None))
    }

    pub fn with_config_and_workspace(config: Config, workspace: Workspace) -> Self {
        let mut documents = BTreeMap::new();
        let initial_id = DocumentId(1);
        let initial_doc = Document::new_scratch(
            initial_id,
            workspace.root().to_path_buf(),
            config.theme.as_deref(),
        );
        documents.insert(initial_id, initial_doc);

        Self {
            mode: Mode::Normal,
            registers: Registers::new(),
            selected_register: None,
            keymap: KeyTrieRoot::new(config.keys.clone()),
            last_matched_bracket: None,
            count: None,
            on_next_key: None,
            which_key: None,
            status_msg: None,
            config,
            workspace,
            documents,
            current_document_id: initial_id,
            access_history: vec![initial_id],
            next_document_id: 2,
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
        self.which_key = None;
    }

    pub fn on_next_key<F>(&mut self, f: F)
    where
        F: FnOnce(&mut EditorState, &gtk::TextBuffer, KeyEvent) -> KeyHandleResult + 'static,
    {
        self.on_next_key = Some(Box::new(f));
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_msg = Some((msg.into(), DiagnosticSeverity::Info));
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.status_msg = Some((msg.into(), DiagnosticSeverity::Error));
    }

    pub fn set_warning(&mut self, msg: impl Into<String>) {
        self.status_msg = Some((msg.into(), DiagnosticSeverity::Warning));
    }

    // --- Document & Project Workspace Methods ---

    /// Active document reference.
    pub fn current_document(&self) -> &Document {
        self.documents
            .get(&self.current_document_id)
            .expect("active document must exist")
    }

    /// Active document mutable reference.
    pub fn current_document_mut(&mut self) -> &mut Document {
        self.documents
            .get_mut(&self.current_document_id)
            .expect("active document must exist")
    }

    /// Active document's backing `sourceview5::Buffer`.
    pub fn current_buffer(&self) -> sourceview5::Buffer {
        self.current_document().buffer().clone()
    }

    /// Retrieve document by ID.
    pub fn document(&self, id: DocumentId) -> Option<&Document> {
        self.documents.get(&id)
    }

    /// Retrieve document mutably by ID.
    pub fn document_mut(&mut self, id: DocumentId) -> Option<&mut Document> {
        self.documents.get_mut(&id)
    }

    /// Find document by file path (canonical comparison).
    pub fn document_by_path(&self, path: &Path) -> Option<DocumentId> {
        let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        for (&id, doc) in &self.documents {
            if let Some(doc_path) = doc.path()
                && doc_path == canon
            {
                return Some(id);
            }
        }
        None
    }

    /// Open a file at `path`. If already open, switches to that document.
    /// If opening into an unmodified empty scratch document, replaces it.
    pub fn open(&mut self, path: &Path) -> Result<DocumentId, DocumentError> {
        let resolved = if path.is_relative() {
            self.workspace.root().join(path)
        } else {
            path.to_path_buf()
        };

        if let Some(existing_id) = self.document_by_path(&resolved) {
            self.switch_document(existing_id);
            return Ok(existing_id);
        }

        // If the only document is a pristine, empty scratch buffer, replace it (matching Helix behavior)
        let is_clean_scratch = self.documents.len() == 1
            && self.current_document().is_scratch()
            && !self.current_document().is_modified()
            && self.current_document().is_empty();

        let new_id = if is_clean_scratch {
            let old_id = self.current_document_id;
            self.documents.remove(&old_id);
            self.access_history.retain(|&x| x != old_id);
            old_id
        } else {
            let id = DocumentId(self.next_document_id);
            self.next_document_id += 1;
            id
        };

        let doc = Document::open(
            new_id,
            &resolved,
            self.workspace.root().to_path_buf(),
            self.config.theme.as_deref(),
        )?;

        self.documents.insert(new_id, doc);
        self.current_document_id = new_id;
        self.access_history.retain(|&x| x != new_id);
        self.access_history.push(new_id);
        Ok(new_id)
    }

    /// Create a new scratch document and switch to it.
    pub fn new_scratch_document(&mut self) -> DocumentId {
        let new_id = DocumentId(self.next_document_id);
        self.next_document_id += 1;

        let doc = Document::new_scratch(
            new_id,
            self.workspace.root().to_path_buf(),
            self.config.theme.as_deref(),
        );
        self.documents.insert(new_id, doc);
        self.current_document_id = new_id;
        self.access_history.retain(|&x| x != new_id);
        self.access_history.push(new_id);
        new_id
    }

    /// Close a document. Fails if buffer is modified and `force` is false.
    /// If closing active document, switches to the most recently used remaining document.
    /// Returns `Ok(Some(new_id))` if documents remain, or `Ok(None)` if all documents are closed (matching Helix).
    pub fn close_document(&mut self, id: DocumentId, force: bool) -> Result<Option<DocumentId>, CloseError> {
        let doc = match self.documents.get(&id) {
            Some(d) => d,
            None => return Err(CloseError::DoesNotExist),
        };

        if doc.is_modified() && !force {
            return Err(CloseError::Modified);
        }

        self.documents.remove(&id);
        self.access_history.retain(|&x| x != id);

        if self.documents.is_empty() {
            return Ok(None);
        }

        if self.current_document_id == id {
            let next_id = if let Some(&prev_id) = self.access_history.last() {
                prev_id
            } else {
                *self.documents.keys().next().unwrap()
            };
            self.switch_document(next_id);
            Ok(Some(next_id))
        } else {
            Ok(Some(self.current_document_id))
        }
    }

    /// Switch active document to `id`. Returns true if `id` existed.
    pub fn switch_document(&mut self, id: DocumentId) -> bool {
        if let Some(doc) = self.documents.get_mut(&id) {
            doc.touch_focused();
            self.current_document_id = id;
            self.access_history.retain(|&x| x != id);
            self.access_history.push(id);
            true
        } else {
            false
        }
    }

    /// Return open documents sorted in MRU order (most recently focused first, matching Helix).
    pub fn documents_in_mru_order(&self) -> Vec<&Document> {
        let mut list: Vec<&Document> = self.documents.values().collect();
        list.sort_unstable_by_key(|d| std::cmp::Reverse(d.focused_at()));
        list
    }

    /// Switch to next open document (wrapping around).
    pub fn next_document(&mut self) -> DocumentId {
        let keys: Vec<DocumentId> = self.documents.keys().copied().collect();
        if let Some(pos) = keys.iter().position(|&k| k == self.current_document_id) {
            let next_pos = (pos + 1) % keys.len();
            let next_id = keys[next_pos];
            self.switch_document(next_id);
        }
        self.current_document_id
    }

    /// Switch to previous open document (wrapping around).
    pub fn prev_document(&mut self) -> DocumentId {
        let keys: Vec<DocumentId> = self.documents.keys().copied().collect();
        if let Some(pos) = keys.iter().position(|&k| k == self.current_document_id) {
            let prev_pos = if pos == 0 {
                keys.len() - 1
            } else {
                pos - 1
            };
            let prev_id = keys[prev_pos];
            self.switch_document(prev_id);
        }
        self.current_document_id
    }

    /// List all open document IDs.
    pub fn document_ids(&self) -> Vec<DocumentId> {
        self.documents.keys().copied().collect()
    }

    /// Active project workspace reference.
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    /// Active project workspace mutable reference.
    pub fn workspace_mut(&mut self) -> &mut Workspace {
        &mut self.workspace
    }

    /// Workspace root directory.
    pub fn workspace_root(&self) -> &Path {
        self.workspace.root()
    }

    /// Change workspace root and update all open documents.
    pub fn set_workspace_root(&mut self, root: PathBuf) {
        self.workspace = Workspace::new(root);
        let ws_root = self.workspace.root().to_path_buf();
        for doc in self.documents.values_mut() {
            doc.set_workspace_root(ws_root.clone());
        }
    }

    /// Walk project files using workspace walker.
    pub fn list_project_files(&self) -> Vec<PathBuf> {
        self.workspace.walk_files(&self.config.editor.file_picker)
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gtk::test]
    fn test_editor_state_initial_scratch() {
        let state = EditorState::new();
        assert_eq!(state.documents.len(), 1);
        assert!(state.current_document().is_scratch());
        assert_eq!(state.current_document().display_name(), "[scratch]");
    }

    #[gtk::test]
    fn test_editor_state_open_and_switch() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut state = EditorState::with_config_and_workspace(
            Config::default(),
            Workspace::new(root.clone()),
        );

        let cargo_toml = root.join("Cargo.toml");
        let id1 = state.open(&cargo_toml).expect("Should open Cargo.toml");
        assert_eq!(state.current_document_id, id1);
        assert_eq!(state.current_document().display_name(), "Cargo.toml");

        // Single clean scratch should have been replaced
        assert_eq!(state.documents.len(), 1);

        // Open another file
        let main_rs = root.join("src/main.rs");
        let id2 = state.open(&main_rs).expect("Should open src/main.rs");
        assert_eq!(state.current_document_id, id2);
        assert_eq!(state.documents.len(), 2);

        // Re-opening Cargo.toml switches to id1 without creating a 3rd document
        let id1_again = state.open(&cargo_toml).expect("Should switch to existing");
        assert_eq!(id1_again, id1);
        assert_eq!(state.current_document_id, id1);
        assert_eq!(state.documents.len(), 2);

        // Cycling next and prev
        assert_eq!(state.next_document(), id2);
        assert_eq!(state.next_document(), id1);
        assert_eq!(state.prev_document(), id2);
    }

    #[gtk::test]
    fn test_editor_state_close_document() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut state = EditorState::with_config_and_workspace(
            Config::default(),
            Workspace::new(root.clone()),
        );

        let cargo_toml = root.join("Cargo.toml");
        let id1 = state.open(&cargo_toml).unwrap();
        let main_rs = root.join("src/main.rs");
        let id2 = state.open(&main_rs).unwrap();

        assert_eq!(state.documents.len(), 2);
        assert_eq!(state.current_document_id, id2);

        // Close id2 (active)
        let remaining = state.close_document(id2, false).unwrap();
        assert_eq!(remaining, Some(id1));
        assert_eq!(state.documents.len(), 1);
        assert_eq!(state.current_document_id, id1);

        // Close last document: returns None (editor will quit)
        let remaining = state.close_document(id1, false).unwrap();
        assert_eq!(remaining, None);
        assert_eq!(state.documents.len(), 0);
    }
}
