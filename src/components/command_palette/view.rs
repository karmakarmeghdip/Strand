use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gdk;
use gtk::glib;
use gtk::pango;

use crate::core::fuzzy::{escape_pango, highlight_pango_markup_with_tags, FuzzyMatcher};

use super::{PaletteItem, PaletteMode};

const PALETTE_CSS: &str = r#"
.strand-palette-window {
    background-color: #1e1e2e;
    color: #cdd6f4;
    border: 1px solid #45475a;
    border-radius: 12px;
    box-shadow: 0 12px 36px rgba(0, 0, 0, 0.55);
}

.strand-palette-header {
    padding: 10px 14px 6px 14px;
    border-bottom: 1px solid #313244;
}

.strand-palette-search {
    background: #181825;
    color: #cdd6f4;
    border: 1px solid #313244;
    border-radius: 8px;
    padding: 6px 10px;
    font-size: 1.05em;
}

.strand-palette-search:focus-within {
    border-color: #89b4fa;
}

.strand-palette-count {
    color: #6c7086;
    font-size: 0.85em;
    font-weight: 600;
    min-width: 50px;
}

.strand-palette-list {
    background-color: transparent;
}

.strand-palette-row {
    padding: 6px 12px;
    border-radius: 6px;
    margin: 2px 4px;
}

.strand-palette-row:selected {
    background-color: #313244;
    color: #cdd6f4;
}

.strand-palette-title {
    font-size: 0.95em;
    font-weight: 600;
    color: #cdd6f4;
}

.strand-palette-subtitle {
    font-size: 0.8em;
    color: #6c7086;
}

.strand-palette-badge {
    font-family: monospace;
    font-size: 0.8em;
    font-weight: bold;
    color: #f9e2af;
    padding: 2px 6px;
    border-radius: 4px;
    background: #313244;
}
"#;

/// Reusable Command Palette Dialog supporting File, Buffer, and Ex-Command selection.
pub struct CommandPaletteDialog;

impl CommandPaletteDialog {
    /// Initialize CSS styles for the palette dialog.
    pub fn init_css() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            if let Some(display) = gdk::Display::default() {
                let provider = gtk::CssProvider::new();
                provider.load_from_data(PALETTE_CSS);
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
        });
    }

    /// Open a modal command palette dialog.
    pub fn show<F>(
        parent: &impl IsA<gtk::Window>,
        title: &str,
        placeholder: &str,
        mode: PaletteMode,
        initial_input: &str,
        items: Vec<PaletteItem>,
        on_select: F,
    ) -> adw::Window
    where
        F: Fn(PaletteItem, String) + 'static,
    {
        Self::init_css();

        let window = adw::Window::builder()
            .title(title)
            .modal(true)
            .transient_for(parent)
            .destroy_with_parent(true)
            .default_width(640)
            .default_height(420)
            .build();
        window.add_css_class("strand-palette-window");

        let vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .build();

        // Search header
        let header_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .build();
        header_box.add_css_class("strand-palette-header");

        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text(placeholder)
            .hexpand(true)
            .build();
        search_entry.add_css_class("strand-palette-search");

        let total_count = items.len();
        let counter_label = gtk::Label::builder()
            .label(format!("{} / {}", total_count, total_count))
            .halign(gtk::Align::End)
            .valign(gtk::Align::Center)
            .build();
        counter_label.add_css_class("strand-palette-count");

        header_box.append(&search_entry);
        header_box.append(&counter_label);
        vbox.append(&header_box);

        // Scrolled List View
        let scrolled = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .min_content_height(300)
            .build();

        let list_box = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .build();
        list_box.add_css_class("strand-palette-list");
        scrolled.set_child(Some(&list_box));
        vbox.append(&scrolled);

        window.set_content(Some(&vbox));

        let all_items = Rc::new(items);
        let filtered_items = Rc::new(RefCell::new(Vec::<(PaletteItem, Vec<u32>)>::new()));
        let matcher = Rc::new(RefCell::new(FuzzyMatcher::new()));
        let on_select = Rc::new(on_select);

        // Helper to render filtered items to the ListBox
        let render_items = {
            let list_box = list_box.clone();
            let counter_label = counter_label.clone();
            let all_items = all_items.clone();
            let filtered_items = filtered_items.clone();

            move || {
                // Clear existing children
                while let Some(child) = list_box.first_child() {
                    list_box.remove(&child);
                }

                let items = filtered_items.borrow();
                let shown_count = items.len();
                counter_label.set_text(&format!("{} / {}", shown_count, all_items.len()));

                // Cap displayed rows at 150 for 144Hz smoothness
                for (item, indices) in items.iter().take(150) {
                    let row = gtk::ListBoxRow::new();
                    row.add_css_class("strand-palette-row");

                    let hbox = gtk::Box::builder()
                        .orientation(gtk::Orientation::Horizontal)
                        .spacing(8)
                        .valign(gtk::Align::Center)
                        .build();

                    let text_box = gtk::Box::builder()
                        .orientation(gtk::Orientation::Vertical)
                        .spacing(2)
                        .hexpand(true)
                        .build();

                    let title_label = gtk::Label::builder()
                        .xalign(0.0)
                        .use_markup(true)
                        .ellipsize(pango::EllipsizeMode::End)
                        .build();
                    title_label.add_css_class("strand-palette-title");

                    let highlighted = if !indices.is_empty() {
                        highlight_pango_markup_with_tags(
                            &item.title,
                            indices,
                            "<span foreground=\"#89b4fa\" weight=\"bold\">",
                            "</span>",
                        )
                    } else {
                        escape_pango(&item.title)
                    };
                    title_label.set_markup(&highlighted);
                    text_box.append(&title_label);

                    if let Some(ref subtitle) = item.subtitle {
                        let sub_label = gtk::Label::builder()
                            .label(subtitle)
                            .xalign(0.0)
                            .ellipsize(pango::EllipsizeMode::Start)
                            .build();
                        sub_label.add_css_class("strand-palette-subtitle");
                        text_box.append(&sub_label);
                    }

                    hbox.append(&text_box);

                    if let Some(ref badge) = item.badge {
                        let badge_label = gtk::Label::builder()
                            .label(badge)
                            .valign(gtk::Align::Center)
                            .build();
                        badge_label.add_css_class("strand-palette-badge");
                        hbox.append(&badge_label);
                    }

                    row.set_child(Some(&hbox));
                    list_box.append(&row);
                }

                if shown_count > 0
                    && let Some(first_row) = list_box.row_at_index(0)
                {
                    list_box.select_row(Some(&first_row));
                }
            }
        };

        // Helper to filter items based on query
        let do_filter = {
            let all_items = all_items.clone();
            let filtered_items = filtered_items.clone();
            let matcher = matcher.clone();
            let render_items = render_items.clone();

            move |query: &str| {
                let query = query.trim();
                let is_path = mode == PaletteMode::FilePicker;

                let mut list = Vec::new();
                if query.is_empty() {
                    for item in all_items.iter() {
                        list.push((item.clone(), Vec::new()));
                    }
                } else {
                    let mut m = matcher.borrow_mut();
                    let results = m.filter_and_sort(query, &all_items, |it| &it.search_key, is_path);
                    for r in results {
                        list.push((r.item, r.indices));
                    }
                }

                *filtered_items.borrow_mut() = list;
                render_items();
            }
        };

        // Initial filter run
        do_filter(initial_input);
        if !initial_input.is_empty() {
            search_entry.set_text(initial_input);
            search_entry.set_position(-1);
        }

        // Live filter as user types
        {
            let do_filter = do_filter.clone();
            search_entry.connect_search_changed(move |entry| {
                do_filter(&entry.text());
            });
        }

        // Row activation helper (Enter or Click)
        let activate_selection = {
            let window = window.clone();
            let filtered_items = filtered_items.clone();
            let list_box = list_box.clone();
            let search_entry = search_entry.clone();
            let on_select = on_select.clone();

            move || {
                let current_text = search_entry.text().to_string();
                let selected_idx = list_box.selected_row().map(|r| r.index());

                if let Some(idx) = selected_idx
                    && idx >= 0
                {
                    let items = filtered_items.borrow();
                    if let Some((item, _)) = items.get(idx as usize) {
                        let item = item.clone();
                        drop(items);
                        window.close();
                        on_select(item, current_text);
                        return;
                    }
                }

                // If in command mode with manual text (e.g. :w new_name.rs or :theme latte)
                if mode == PaletteMode::Command && !current_text.trim().is_empty() {
                    window.close();
                    let raw = current_text.trim().to_string();
                    let item = PaletteItem::new(&raw, &raw, &raw);
                    on_select(item, current_text);
                }
            }
        };

        // ListBox row activated on mouse click / double click
        {
            let activate_selection = activate_selection.clone();
            list_box.connect_row_activated(move |_, _| {
                activate_selection();
            });
        }

        // Keyboard Controller at CAPTURE phase for arrow keys, Ctrl-n/Ctrl-p, Enter, Esc
        let key_controller = gtk::EventControllerKey::new();
        key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        {
            let window = window.clone();
            let list_box = list_box.clone();
            let scrolled = scrolled.clone();
            let filtered_items = filtered_items.clone();
            let activate_selection = activate_selection.clone();

            key_controller.connect_key_pressed(move |_, keyval, _keycode, state| {
                let is_ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);

                // Esc: dismiss dialog
                if keyval == gdk::Key::Escape {
                    window.close();
                    return glib::Propagation::Stop;
                }

                // Down or Ctrl-n: next item
                if keyval == gdk::Key::Down
                    || (is_ctrl && (keyval == gdk::Key::n || keyval == gdk::Key::N))
                {
                    let count = filtered_items.borrow().len() as i32;
                    let current = list_box.selected_row().map(|r| r.index()).unwrap_or(-1);
                    let next = (current + 1).min(count - 1);
                    if next >= 0
                        && let Some(row) = list_box.row_at_index(next)
                    {
                        list_box.select_row(Some(&row));
                        let adj = scrolled.vadjustment();
                        let alloc = row.allocation();
                        let y = alloc.y() as f64;
                        let h = alloc.height() as f64;
                        let page = adj.page_size();
                        let val = adj.value();
                        if y + h > val + page {
                            adj.set_value(y + h - page);
                        }
                    }
                    return glib::Propagation::Stop;
                }

                // Up or Ctrl-p: previous item
                if keyval == gdk::Key::Up
                    || (is_ctrl && (keyval == gdk::Key::p || keyval == gdk::Key::P))
                {
                    let current = list_box.selected_row().map(|r| r.index()).unwrap_or(0);
                    let prev = (current - 1).max(0);
                    if let Some(row) = list_box.row_at_index(prev) {
                        list_box.select_row(Some(&row));
                        let adj = scrolled.vadjustment();
                        let alloc = row.allocation();
                        let y = alloc.y() as f64;
                        let val = adj.value();
                        if y < val {
                            adj.set_value(y);
                        }
                    }
                    return glib::Propagation::Stop;
                }

                // Page Down: jump 5 items
                if keyval == gdk::Key::Page_Down {
                    let count = filtered_items.borrow().len() as i32;
                    let current = list_box.selected_row().map(|r| r.index()).unwrap_or(-1);
                    let next = (current + 5).min(count - 1);
                    if next >= 0
                        && let Some(row) = list_box.row_at_index(next)
                    {
                        list_box.select_row(Some(&row));
                    }
                    return glib::Propagation::Stop;
                }

                // Page Up: jump 5 items up
                if keyval == gdk::Key::Page_Up {
                    let current = list_box.selected_row().map(|r| r.index()).unwrap_or(0);
                    let prev = (current - 5).max(0);
                    if let Some(row) = list_box.row_at_index(prev) {
                        list_box.select_row(Some(&row));
                    }
                    return glib::Propagation::Stop;
                }

                // Enter / Return: select item
                if keyval == gdk::Key::Return || keyval == gdk::Key::KP_Enter {
                    activate_selection();
                    return glib::Propagation::Stop;
                }

                glib::Propagation::Proceed
            });
        }

        search_entry.add_controller(key_controller);
        window.present();
        search_entry.grab_focus();

        window
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gtk::test]
    fn test_dialog_creation_and_filtering() {
        let parent = gtk::Window::new();
        let items = vec![
            PaletteItem::new("item1", "Item One", "item one").with_subtitle("Sub 1"),
            PaletteItem::new("item2", "Item Two", "item two").with_badge("[+]"),
            PaletteItem::new("item3", "Another Item", "another item"),
        ];

        let selected = Rc::new(RefCell::new(None));
        let sel_clone = selected.clone();
        let dialog = CommandPaletteDialog::show(
            &parent,
            "Test Palette",
            "Search...",
            PaletteMode::FilePicker,
            "",
            items,
            move |item, _raw| {
                *sel_clone.borrow_mut() = Some(item.id);
            },
        );

        assert_eq!(dialog.title().as_deref(), Some("Test Palette"));
        dialog.close();
    }
}
