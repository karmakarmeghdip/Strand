use std::path::{Path, PathBuf};

use crate::commands::buffer::{buffer_close, buffer_force_close, goto_next_buffer, goto_previous_buffer};
use crate::commands::Context;
use crate::components::editor::controller::KeyHandleResult;
use crate::editor::document::{DocumentError, DocumentId};

/// Save the current buffer to disk (`:w` / `:write`).
pub fn file_save(cx: &mut Context) -> KeyHandleResult {
    match cx.state.current_document_mut().save() {
        Ok(()) => {
            let display = cx.state.current_document().display_name();
            cx.set_status(format!("\"{}\" written", display));
            KeyHandleResult::Stop
        }
        Err(DocumentError::NoPathSet) => {
            cx.set_error("Cannot save: no file name (use :w <path>)");
            KeyHandleResult::Stop
        }
        Err(e) => {
            cx.set_error(format!("Save failed: {e}"));
            KeyHandleResult::Stop
        }
    }
}

/// Save the current buffer to a specified path (`:w <path>` / `:write <path>`).
pub fn file_save_as(cx: &mut Context, path: &Path) -> KeyHandleResult {
    match cx.state.current_document_mut().save_as(path) {
        Ok(()) => {
            let display = cx.state.current_document().display_name();
            cx.set_status(format!("\"{}\" written", display));
            KeyHandleResult::Stop
        }
        Err(e) => {
            cx.set_error(format!("Save failed: {e}"));
            KeyHandleResult::Stop
        }
    }
}

/// Open a file at `path` into a new or existing buffer (`:e <path>` / `:open <path>`).
pub fn file_open(cx: &mut Context, path: &Path) -> KeyHandleResult {
    match cx.state.open(path) {
        Ok(id) => {
            let display = cx.state.current_document().display_name();
            cx.set_status(format!("Opened \"{}\"", display));
            KeyHandleResult::DocumentChanged(id)
        }
        Err(e) => {
            cx.set_error(format!("Open failed: {e}"));
            KeyHandleResult::Stop
        }
    }
}

/// Quit the editor (`:q` / `:quit`). Fails if any open buffers are modified and `force` is false.
/// If unmodified, or `force` is true, returns `KeyHandleResult::Quit`.
/// Matches Helix's `quit` and `buffers_remaining_impl`.
pub fn quit(cx: &mut Context, force: bool) -> KeyHandleResult {
    if !force {
        let modified_ids: Vec<(DocumentId, String)> = cx
            .state
            .documents
            .values()
            .filter(|doc| doc.is_modified())
            .map(|doc| (doc.id(), doc.display_name()))
            .collect();

        if let Some((first_id, _)) = modified_ids.first() {
            let current_id = cx.state.current_document_id;
            // If current buffer is not modified, switch to the first modified doc
            if current_id != *first_id {
                cx.state.switch_document(*first_id);
            }
            let count = modified_ids.len();
            let s = if count == 1 { "" } else { "s" };
            let names: Vec<String> = modified_ids.iter().map(|(_, name)| name.clone()).collect();
            cx.set_error(format!(
                "{} unsaved buffer{} remaining: {:?} (use :q! to force)",
                count, s, names
            ));
            return if current_id != *first_id {
                KeyHandleResult::DocumentChanged(*first_id)
            } else {
                KeyHandleResult::Stop
            };
        }
    }

    KeyHandleResult::Quit
}

/// Request to open file picker (`<Space>f`).
pub fn open_file_picker(cx: &mut Context) -> KeyHandleResult {
    let files = cx.state.list_project_files();
    cx.set_status(format!(
        "File picker: {} project files in {}",
        files.len(),
        cx.state.workspace.name()
    ));
    KeyHandleResult::OpenFilePicker
}

/// Request to open buffer picker (`<Space>b`).
pub fn open_buffer_picker(cx: &mut Context) -> KeyHandleResult {
    let count = cx.state.documents.len();
    cx.set_status(format!("Buffer picker: {} open buffers", count));
    KeyHandleResult::OpenBufferPicker
}

/// Parse and execute an ex-style editor command string (e.g. `:w`, `:q`, `:e src/main.rs`).
pub fn execute_command(cx: &mut Context, input: &str) -> KeyHandleResult {
    let input = input.trim();
    let input = input.strip_prefix(':').unwrap_or(input).trim();
    if input.is_empty() {
        return KeyHandleResult::Stop;
    }

    let mut parts = input.split_whitespace();
    let cmd = parts.next().unwrap_or("");
    let arg = parts.next();

    match cmd {
        "w" | "write" => {
            if let Some(path_str) = arg {
                file_save_as(cx, Path::new(path_str))
            } else {
                file_save(cx)
            }
        }
        "w!" | "write!" => {
            if let Some(path_str) = arg {
                file_save_as(cx, Path::new(path_str))
            } else {
                file_save(cx)
            }
        }
        "q" | "quit" => quit(cx, false),
        "q!" | "quit!" => quit(cx, true),
        "bc" | "bclose" => buffer_close(cx),
        "bc!" | "bclose!" => buffer_force_close(cx),
        "wq" | "x" => {
            let res = file_save(cx);
            if res == KeyHandleResult::Stop {
                if let Some((_, severity)) = &cx.state.status_msg
                    && *severity == crate::components::statusline::DiagnosticSeverity::Error
                {
                    return KeyHandleResult::Stop;
                }
                quit(cx, false)
            } else {
                res
            }
        }
        "e" | "open" => {
            if let Some(path_str) = arg {
                file_open(cx, Path::new(path_str))
            } else {
                cx.set_error("No path provided for :e / :open");
                KeyHandleResult::Stop
            }
        }
        "bn" | "bnext" | "buffer-next" => goto_next_buffer(cx),
        "bp" | "bprev" | "buffer-previous" => goto_previous_buffer(cx),
        "b" | "buffer" => {
            if let Some(arg) = arg {
                if let Ok(num) = arg.parse::<usize>() {
                    // Try 1-based index (1..n) matching buffer picker list numbering
                    let mru_ids: Vec<DocumentId> = cx
                        .state
                        .documents_in_mru_order()
                        .into_iter()
                        .map(|d| d.id())
                        .collect();
                    if num >= 1 && num <= mru_ids.len() {
                        let doc_id = mru_ids[num - 1];
                        cx.state.switch_document(doc_id);
                        let display = cx.state.current_document().display_name();
                        cx.set_status(format!("Switched to buffer {}: {}", num, display));
                        return KeyHandleResult::DocumentChanged(doc_id);
                    }
                    // Fallback to raw DocumentId
                    let doc_id = DocumentId(num);
                    if cx.state.switch_document(doc_id) {
                        let display = cx.state.current_document().display_name();
                        cx.set_status(format!("Switched to buffer: {}", display));
                        return KeyHandleResult::DocumentChanged(doc_id);
                    }
                }
                // Try finding by path substring or display name
                let found_id = cx.state.documents.iter().find_map(|(&id, doc)| {
                    if doc.display_name().contains(arg) {
                        Some(id)
                    } else {
                        None
                    }
                });
                if let Some(id) = found_id {
                    cx.state.switch_document(id);
                    let display = cx.state.current_document().display_name();
                    cx.set_status(format!("Switched to buffer: {}", display));
                    KeyHandleResult::DocumentChanged(id)
                } else {
                    cx.set_error(format!("Buffer not found: {arg}"));
                    KeyHandleResult::Stop
                }
            } else {
                open_buffer_picker(cx)
            }
        }
        "pwd" => {
            let root = cx.state.workspace.root().display().to_string();
            cx.set_status(format!("Workspace root: {}", root));
            KeyHandleResult::Stop
        }
        "cd" => {
            if let Some(path_str) = arg {
                let new_root = PathBuf::from(path_str);
                cx.state.set_workspace_root(new_root);
                let root = cx.state.workspace.root().display().to_string();
                cx.set_status(format!("Workspace root set to: {}", root));
            } else {
                cx.set_error("No path provided for :cd");
            }
            KeyHandleResult::Stop
        }
        "theme" => {
            if let Some(theme_name) = arg {
                let full_theme = if !theme_name.starts_with("catppuccin-") {
                    format!("catppuccin-{}", theme_name)
                } else {
                    theme_name.to_string()
                };
                let active_buf = cx.state.current_buffer();
                if crate::components::editor::theme::apply_theme(&active_buf, &full_theme) {
                    cx.state.config.theme = Some(full_theme.clone());
                    cx.set_status(format!("Applied theme: {}", full_theme));
                } else {
                    cx.set_error(format!("Theme not found: {}", theme_name));
                }
            } else {
                cx.set_error("No theme name provided (try mocha, latte, frappe, macchiato)");
            }
            KeyHandleResult::Stop
        }
        unknown => {
            cx.set_error(format!("Unknown command: :{}", unknown));
            KeyHandleResult::Stop
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::prelude::{Cast, TextBufferExt};
    use crate::editor::state::EditorState;

    #[gtk::test]
    fn test_execute_commands() {
        let mut state = EditorState::new();
        let buf = state.current_buffer();
        let mut cx = Context::new(&mut state, buf.upcast_ref(), 1, '"');

        // Test :pwd
        let res = execute_command(&mut cx, ":pwd");
        assert_eq!(res, KeyHandleResult::Stop);
        assert!(cx.state.status_msg.is_some());

        // Test :e with Cargo.toml
        let res_e = execute_command(&mut cx, ":e Cargo.toml");
        assert!(matches!(res_e, KeyHandleResult::DocumentChanged(_)));
        assert_eq!(cx.state.current_document().display_name(), "Cargo.toml");

        // Test :e with src/main.rs
        let res_e2 = execute_command(&mut cx, ":e src/main.rs");
        assert!(matches!(res_e2, KeyHandleResult::DocumentChanged(_)));
        assert_eq!(cx.state.current_document().display_name(), "src/main.rs");

        // Test :bp (back to Cargo.toml)
        let res_bp = execute_command(&mut cx, ":bp");
        assert!(matches!(res_bp, KeyHandleResult::DocumentChanged(_)));
        assert_eq!(cx.state.current_document().display_name(), "Cargo.toml");

        // Test :bn (forward to src/main.rs)
        let res_bn = execute_command(&mut cx, ":bn");
        assert!(matches!(res_bn, KeyHandleResult::DocumentChanged(_)));
        assert_eq!(cx.state.current_document().display_name(), "src/main.rs");

        // Test :b Cargo
        let res_b = execute_command(&mut cx, ":b Cargo");
        assert!(matches!(res_b, KeyHandleResult::DocumentChanged(_)));
        assert_eq!(cx.state.current_document().display_name(), "Cargo.toml");

        // Test :theme
        let res_th = execute_command(&mut cx, ":theme catppuccin-latte");
        assert_eq!(res_th, KeyHandleResult::Stop);

        // Test :q when clean: should return Quit
        let res_q = execute_command(&mut cx, ":q");
        assert_eq!(res_q, KeyHandleResult::Quit);

        // If a document is modified, :q should refuse and return Stop or switch
        cx.state.current_document().buffer().set_text("unsaved edits");
        assert!(cx.state.current_document().is_modified());
        let res_q_unsaved = execute_command(&mut cx, ":q");
        assert_eq!(res_q_unsaved, KeyHandleResult::Stop);
        assert!(cx.state.status_msg.is_some());

        // :q! should force quit
        let res_q_force = execute_command(&mut cx, ":q!");
        assert_eq!(res_q_force, KeyHandleResult::Quit);
    }
}
