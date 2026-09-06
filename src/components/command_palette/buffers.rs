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
    pub is_current: bool,
    pub focused_at: std::time::Instant,
}

/// Open the buffer picker dialog for the currently open documents.
pub fn show_buffer_picker<F>(
    parent: &impl IsA<gtk::Window>,
    mut buffers: Vec<BufferPickerItem>,
    on_switch_buffer: F,
) -> adw::Window
where
    F: Fn(DocumentId) + 'static,
{
    // Sort by MRU order (most recently focused first, matching Helix)
    buffers.sort_unstable_by_key(|b| std::cmp::Reverse(b.focused_at));

    // Helix convention: if more than 1 buffer, initial cursor points to index 1 (previous buffer)
    let initial_cursor = if buffers.len() > 1 { 1 } else { 0 };

    let items: Vec<PaletteItem> = buffers
        .into_iter()
        .enumerate()
        .map(|(idx, b)| {
            let list_num = idx + 1;
            let id_str = b.id.0.to_string();
            let title = format!("{}: {}", list_num, b.display_name);
            let search_key = format!(
                "{} {} {}",
                list_num,
                b.display_name,
                b.relative_path.as_deref().unwrap_or_default()
            );

            // Flags: '+' for modified, '*' for active/current buffer (Helix format)
            let mut flags = String::new();
            if b.is_modified {
                flags.push('+');
            }
            if b.is_current {
                flags.push('*');
            }

            let badge = if !flags.is_empty() {
                Some(format!("[{}]", flags))
            } else if b.is_read_only {
                Some("[ro]".to_string())
            } else {
                None
            };

            let mut item = PaletteItem::new(id_str, title, search_key);
            if let Some(path) = b.relative_path
                && path != b.display_name
            {
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
        super::PaletteConfig {
            title: "Switch Buffer",
            placeholder: "Type to filter open buffers...",
            mode: PaletteMode::BufferPicker,
            initial_input: "",
            initial_cursor,
        },
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
            is_current: true,
            focused_at: std::time::Instant::now(),
        };
        assert_eq!(item.id, DocumentId(1));
        assert!(item.is_modified);
        assert!(item.is_current);
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
            is_current: true,
            focused_at: std::time::Instant::now(),
        }];
        let dialog = show_buffer_picker(&parent, items, |_id| {});
        assert_eq!(dialog.title().as_deref(), Some("Switch Buffer"));
        dialog.close();
    }
}
