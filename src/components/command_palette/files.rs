use std::path::PathBuf;
use gtk::prelude::IsA;
use crate::config::editor::FilePickerConfig;
use crate::editor::workspace::Workspace;
use super::{CommandPaletteDialog, PaletteItem, PaletteMode};

/// Open the file picker dialog for the current workspace.
pub fn show_file_picker<F>(
    parent: &impl IsA<gtk::Window>,
    workspace: &Workspace,
    on_open_file: F,
) -> adw::Window
where
    F: Fn(PathBuf) + 'static,
{
    let files = workspace.walk_files(&FilePickerConfig::default());
    let root = workspace.root();

    let items: Vec<PaletteItem> = files
        .into_iter()
        .map(|rel_path| {
            let path_str = rel_path.to_string_lossy().to_string();
            let file_name = rel_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path_str.clone());
            let parent_dir = rel_path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map(|p| p.to_string_lossy().to_string());

            let mut item = PaletteItem::new(&path_str, file_name, &path_str)
                .with_is_path(true);
            if let Some(dir) = parent_dir {
                item = item.with_subtitle(dir);
            }
            item
        })
        .collect();

    let root_buf = root.to_path_buf();
    CommandPaletteDialog::show(
        parent,
        super::PaletteConfig {
            title: "Open File",
            placeholder: "Type to filter files...",
            mode: PaletteMode::FilePicker,
            initial_input: "",
            initial_cursor: 0,
        },
        items,
        move |item, _raw| {
            let full_path = root_buf.join(&item.id);
            on_open_file(full_path);
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::prelude::GtkWindowExt;

    #[test]
    fn test_file_palette_items_construction() {
        let ws = Workspace::from_path_or_cwd(None);
        let files = ws.walk_files(&FilePickerConfig::default());
        assert!(!files.is_empty(), "Workspace should have files");

        let has_cargo_toml = files.iter().any(|f| f.file_name().unwrap_or_default() == "Cargo.toml");
        assert!(has_cargo_toml, "Workspace should contain Cargo.toml");
    }

    #[gtk::test]
    fn test_show_file_picker_dialog() {
        let ws = Workspace::from_path_or_cwd(None);
        let parent = gtk::Window::new();
        let dialog = show_file_picker(&parent, &ws, |_path| {});
        assert_eq!(dialog.title().as_deref(), Some("Open File"));
        dialog.close();
    }
}
