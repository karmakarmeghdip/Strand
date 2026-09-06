use crate::commands::Context;
use crate::components::editor::controller::KeyHandleResult;
use crate::editor::state::CloseError;

/// Switch to next open buffer in document list (`gn`, `:bnext`, `:bn`).
pub fn goto_next_buffer(cx: &mut Context) -> KeyHandleResult {
    let next_id = cx.state.next_document();
    let display = cx.state.current_document().display_name();
    cx.set_status(format!("Switched to buffer {}: {}", next_id, display));
    KeyHandleResult::DocumentChanged(next_id)
}

/// Switch to previous open buffer in document list (`gp`, `:bprev`, `:bp`).
pub fn goto_previous_buffer(cx: &mut Context) -> KeyHandleResult {
    let prev_id = cx.state.prev_document();
    let display = cx.state.current_document().display_name();
    cx.set_status(format!("Switched to buffer {}: {}", prev_id, display));
    KeyHandleResult::DocumentChanged(prev_id)
}

/// Close active buffer (`:bclose`, `:bc`, `:q`). Fails if modified without force.
pub fn buffer_close(cx: &mut Context) -> KeyHandleResult {
    let current_id = cx.state.current_document_id;
    match cx.state.close_document(current_id, false) {
        Ok(()) => {
            let new_id = cx.state.current_document_id;
            let display = cx.state.current_document().display_name();
            cx.set_status(format!("Closed buffer. Now at {}: {}", new_id, display));
            KeyHandleResult::DocumentChanged(new_id)
        }
        Err(CloseError::Modified) => {
            cx.set_error("Buffer has unsaved changes (use :q! or force to close)");
            KeyHandleResult::Stop
        }
        Err(CloseError::DoesNotExist) => {
            cx.set_error("Buffer does not exist");
            KeyHandleResult::Stop
        }
    }
}

/// Force close active buffer ignoring unsaved changes (`:q!`, `:bc!`).
pub fn buffer_force_close(cx: &mut Context) -> KeyHandleResult {
    let current_id = cx.state.current_document_id;
    match cx.state.close_document(current_id, true) {
        Ok(()) => {
            let new_id = cx.state.current_document_id;
            let display = cx.state.current_document().display_name();
            cx.set_status(format!("Closed buffer. Now at {}: {}", new_id, display));
            KeyHandleResult::DocumentChanged(new_id)
        }
        Err(e) => {
            cx.set_error(format!("Error closing buffer: {e}"));
            KeyHandleResult::Stop
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use gtk::prelude::{Cast, TextBufferExt};
    use crate::config::Config;
    use crate::editor::state::EditorState;
    use crate::editor::workspace::Workspace;

    #[gtk::test]
    fn test_buffer_navigation_and_close() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut state = EditorState::with_config_and_workspace(
            Config::default(),
            Workspace::new(root.clone()),
        );

        let cargo_toml = root.join("Cargo.toml");
        let main_rs = root.join("src/main.rs");

        let id1 = state.open(&cargo_toml).unwrap();
        let id2 = state.open(&main_rs).unwrap();

        let buf = state.current_buffer();
        let mut cx = Context::new(&mut state, buf.upcast_ref(), 1, '"');

        // Initial buffer is id2
        assert_eq!(cx.state.current_document_id, id2);

        // Next buffer switches to id1
        let res1 = goto_next_buffer(&mut cx);
        assert_eq!(res1, KeyHandleResult::DocumentChanged(id1));
        assert_eq!(cx.state.current_document_id, id1);

        // Previous buffer switches back to id2
        let res2 = goto_previous_buffer(&mut cx);
        assert_eq!(res2, KeyHandleResult::DocumentChanged(id2));
        assert_eq!(cx.state.current_document_id, id2);

        // Modify buffer id2
        cx.state.current_document().buffer().set_text("modified text");
        assert!(cx.state.current_document().is_modified());

        // buffer_close without force should fail
        let res_close = buffer_close(&mut cx);
        assert_eq!(res_close, KeyHandleResult::Stop);
        assert!(cx.state.documents.contains_key(&id2));

        // buffer_force_close should succeed and switch to id1
        let res_fclose = buffer_force_close(&mut cx);
        assert_eq!(res_fclose, KeyHandleResult::DocumentChanged(id1));
        assert!(!cx.state.documents.contains_key(&id2));
        assert_eq!(cx.state.current_document_id, id1);
    }
}
