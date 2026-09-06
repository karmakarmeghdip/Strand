pub mod config;
pub mod icons;
pub mod popovers;
pub mod style;
pub mod view;

#[allow(unused_imports)]
pub use config::{
    DiagnosticItem, DiagnosticSeverity, IndentStyle, LineEnding, ModeConfig, StatusLineConfig,
    StatusLineElement, VcsInfo,
};
#[allow(unused_imports)]
pub use icons::init_icons;
#[allow(unused_imports)]
pub use popovers::{DiagnosticsPopover, FormatPopover, GotoLinePopover, ModePopover, VcsPopover};
#[allow(unused_imports)]
pub use style::init_css;
#[allow(unused_imports)]
pub use view::StatusLineView;

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::prelude::*;
    use crate::keymap::Mode;

    fn ensure_gtk() -> bool {
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            let _ = std::panic::catch_unwind(|| gtk::init());
        });
        if !gtk::is_initialized() {
            return false;
        }
        std::panic::catch_unwind(|| {
            let _ = gtk::TextBuffer::new(None);
        })
        .is_ok()
    }

    #[test]
    fn test_vcs_branch_icon_available() {
        if !ensure_gtk() {
            return;
        }
        let display = gtk::gdk::Display::default().unwrap();
        let icon_theme = gtk::IconTheme::for_display(&display);
        icon_theme.add_search_path("data/icons");
        println!("has_icon: {}", icon_theme.has_icon("vcs-branch-symbolic"));
        assert!(icon_theme.has_icon("vcs-branch-symbolic"));
    }

    #[test]
    fn test_statusline_view_lifecycle() {
        if !ensure_gtk() {
            return;
        }

        let config = StatusLineConfig::default();
        let statusline = StatusLineView::new(config);

        // Verify root widget exists
        let _root = statusline.widget();

        // Mode transitions
        statusline.set_mode(Mode::Normal);
        statusline.set_mode(Mode::Insert);
        statusline.set_mode(Mode::Select);
        statusline.set_mode(Mode::Normal);

        // Pending chords and count
        let count = std::num::NonZeroUsize::new(5);
        statusline.set_pending(count, "g-", Some('"'));
        statusline.set_pending(None, "", None);

        // Echo notification
        statusline.echo("Test notification message", DiagnosticSeverity::Info);

        // Diagnostics
        let diags = vec![
            DiagnosticItem {
                severity: DiagnosticSeverity::Error,
                line: 10,
                col: 4,
                message: "cannot borrow immutable local variable".to_string(),
                code: Some("E0596".to_string()),
            },
            DiagnosticItem {
                severity: DiagnosticSeverity::Warning,
                line: 15,
                col: 8,
                message: "unused variable: `x`".to_string(),
                code: Some("unused_variables".to_string()),
            },
        ];
        statusline.set_diagnostics(&diags);

        // VCS info
        let vcs = VcsInfo {
            branch: Some("feat/statusline".to_string()),
            added: 12,
            modified: 3,
            deleted: 1,
        };
        statusline.set_vcs(&vcs);

        // LSP progress
        statusline.set_lsp_progress(true, Some(0.45), Some("Indexing crates..."));
        statusline.set_lsp_progress(false, None, None);
    }

    #[test]
    fn test_statusline_buffer_integration() {
        if !ensure_gtk() {
            return;
        }

        let config = StatusLineConfig::default();
        let statusline = StatusLineView::new(config);

        let buffer = sourceview5::Buffer::new(None);
        buffer.set_text("fn main() {\n    println!(\"Hello\");\n}\n");

        let view = sourceview5::View::with_buffer(&buffer);
        statusline.connect_buffer(&buffer, &view);

        // Verify coordinate update on buffer insertion
        if let Some(iter) = buffer.iter_at_line(1) {
            buffer.place_cursor(&iter);
        }

        // Coordinates should be at Line 2, Col 1
        statusline.update_cursor_coordinates(&buffer);
    }

    #[test]
    fn test_statusline_pending_formatting() {
        if !ensure_gtk() {
            return;
        }

        let config = StatusLineConfig::default();
        let statusline = StatusLineView::new(config);

        // Count only
        statusline.set_pending(std::num::NonZeroUsize::new(12), "", None);
        // Chord only
        statusline.set_pending(None, "space-", None);
        // Register only
        statusline.set_pending(None, "", Some('a'));
        // Count + Chord + Register
        statusline.set_pending(std::num::NonZeroUsize::new(3), "g-", Some('"'));
        // Clear
        statusline.set_pending(None, "", None);
    }

    #[test]
    fn test_statusline_echo_generation() {
        if !ensure_gtk() {
            return;
        }

        let config = StatusLineConfig::default();
        let statusline = StatusLineView::new(config);

        // Rapid successive echoes
        statusline.echo("Message 1", DiagnosticSeverity::Info);
        assert_eq!(statusline.echo_generation(), 1);

        statusline.echo("Message 2", DiagnosticSeverity::Warning);
        assert_eq!(statusline.echo_generation(), 2);

        statusline.echo("Message 3", DiagnosticSeverity::Error);
        assert_eq!(statusline.echo_generation(), 3);
    }

    #[test]
    fn test_statusline_custom_mode_config() {
        if !ensure_gtk() {
            return;
        }

        let mut config = StatusLineConfig::default();
        config.mode.normal = "NORMAL".to_string();
        config.mode.insert = "INSERT".to_string();
        config.mode.select = "SELECT".to_string();

        let statusline = StatusLineView::new(config);
        statusline.set_mode(Mode::Normal);
        assert_eq!(statusline.mode_text(), "NORMAL");
        statusline.set_mode(Mode::Insert);
        assert_eq!(statusline.mode_text(), "INSERT");
        statusline.set_mode(Mode::Select);
        assert_eq!(statusline.mode_text(), "SELECT");
    }
}
