use gtk::prelude::IsA;
use crate::editor::DocumentId;
use super::{CommandPaletteDialog, PaletteItem, PaletteMode};

/// Information about an open document presented in the buffer picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferPickerItem {
    pub id: DocumentId,
    pub display_name: String,
    pub relative_path: Option<String>,
    pub is_modified: bool,
    pub is_read_only: bool,
}

/// Open the buffer picker dialog for the currently open documents.
pub fn show_buffer_picker<F>(
    parent: &impl IsA<gtk::Window>,
    buffers: Vec<BufferPickerItem>,
    on_switch_buffer: F,
) -> adw::Window
where
    F: Fn(DocumentId) + 'static,
{
    let items: Vec<PaletteItem> = buffers
        .into_iter()
        .map(|b| {
            let id_str = b.id.0.to_string();
            let title = format!("{}: {}", b.id.0, b.display_name);
            let search_key = format!(
                "{} {} {}",
                b.id.0,
                b.display_name,
                b.relative_path.as_deref().unwrap_or_default()
            );

            let badge = if b.is_modified {
                Some("[+]".to_string())
            } else if b.is_read_only {
                Some("[ro]".to_string())
            } else {
                None
            };

            let mut item = PaletteItem::new(id_str, title, search_key);
            if let Some(path) = b.relative_path {
                item = item.with_subtitle(path);
            }
            if let Some(badge) = badge {
                item = item.with_badge(badge);
            }
            item
        })
        .collect();

    CommandPaletteDialog::show(
        parent,
        "Switch Buffer",
        "Type to filter open buffers...",
        PaletteMode::BufferPicker,
        "",
        items,
        move |item, _raw| {
            if let Ok(id_num) = item.id.parse::<usize>() {
                on_switch_buffer(DocumentId(id_num));
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::prelude::GtkWindowExt;

    #[test]
    fn test_buffer_picker_item_display() {
        let item = BufferPickerItem {
            id: DocumentId(1),
            display_name: "main.rs".to_string(),
            relative_path: Some("src/main.rs".to_string()),
            is_modified: true,
            is_read_only: false,
        };
        assert_eq!(item.id, DocumentId(1));
        assert!(item.is_modified);
    }

    #[gtk::test]
    fn test_show_buffer_picker_dialog() {
        let parent = gtk::Window::new();
        let items = vec![BufferPickerItem {
            id: DocumentId(1),
            display_name: "test.rs".to_string(),
            relative_path: Some("src/test.rs".to_string()),
            is_modified: false,
            is_read_only: false,
        }];
        let dialog = show_buffer_picker(&parent, items, |_id| {});
        assert_eq!(dialog.title().as_deref(), Some("Switch Buffer"));
        dialog.close();
    }
}
