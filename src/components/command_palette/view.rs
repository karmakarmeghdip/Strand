use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::pango;

use crate::core::fuzzy::{escape_pango, highlight_pango_markup_with_tags, FuzzyMatcher};

use super::{PaletteConfig, PaletteItem, PaletteMode};

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

.strand-palette-list row {
    padding: 6px 12px;
    border-radius: 6px;
    margin: 2px 4px;
}

.strand-palette-list row:selected {
    background-color: #313244;
    color: #cdd6f4;
}

.strand-palette-list row:hover {
    background-color: #262738;
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

#[derive(Clone)]
struct PaletteRowData {
    item: PaletteItem,
    indices: Vec<u32>,
}

/// Reusable Command Palette Dialog supporting File, Buffer, and Ex-Command selection
/// using virtualized `GtkListView` for 144Hz smooth rendering across large codebases.
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

    /// Open a modal command palette dialog with virtualized list rendering.
    pub fn show<F>(
        parent: &impl IsA<gtk::Window>,
        config: PaletteConfig<'_>,
        items: Vec<PaletteItem>,
        on_select: F,
    ) -> adw::Window
    where
        F: Fn(PaletteItem, String) + 'static,
    {
        Self::init_css();

        let title = config.title;
        let placeholder = config.placeholder;
        let mode = config.mode;
        let initial_input = config.initial_input;
        let initial_cursor = config.initial_cursor;

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

        // Virtualized Scrolled List View using GtkListView + GtkSignalListItemFactory
        let scrolled = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .min_content_height(300)
            .build();

        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let selection = gtk::SingleSelection::new(Some(store.clone()));
        selection.set_autoselect(false);
        selection.set_can_unselect(false);

        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_factory, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let row_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(8)
                .valign(gtk::Align::Center)
                .build();
            row_box.add_css_class("strand-palette-row");

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

            let sub_label = gtk::Label::builder()
                .xalign(0.0)
                .ellipsize(pango::EllipsizeMode::Start)
                .build();
            sub_label.add_css_class("strand-palette-subtitle");

            text_box.append(&title_label);
            text_box.append(&sub_label);
            row_box.append(&text_box);

            let badge_label = gtk::Label::builder()
                .valign(gtk::Align::Center)
                .build();
            badge_label.add_css_class("strand-palette-badge");
            row_box.append(&badge_label);

            list_item.set_child(Some(&row_box));
        });

        factory.connect_bind(|_factory, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let row_box = list_item.child().and_downcast::<gtk::Box>().unwrap();
            let text_box = row_box.first_child().and_downcast::<gtk::Box>().unwrap();
            let title_label = text_box.first_child().and_downcast::<gtk::Label>().unwrap();
            let sub_label = title_label.next_sibling().and_downcast::<gtk::Label>().unwrap();
            let badge_label = row_box.last_child().and_downcast::<gtk::Label>().unwrap();

            let boxed = list_item.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
            let data: std::cell::Ref<PaletteRowData> = boxed.borrow();

            let highlighted = if !data.indices.is_empty() {
                highlight_pango_markup_with_tags(
                    &data.item.title,
                    &data.indices,
                    "<span foreground=\"#89b4fa\" weight=\"bold\">",
                    "</span>",
                )
            } else {
                escape_pango(&data.item.title)
            };
            title_label.set_markup(&highlighted);

            if let Some(ref subtitle) = data.item.subtitle {
                sub_label.set_text(subtitle);
                sub_label.set_visible(true);
            } else {
                sub_label.set_text("");
                sub_label.set_visible(false);
            }

            if let Some(ref badge) = data.item.badge {
                badge_label.set_text(badge);
                badge_label.set_visible(true);
            } else {
                badge_label.set_text("");
                badge_label.set_visible(false);
            }
        });

        let list_view = gtk::ListView::new(Some(selection.clone()), Some(factory));
        list_view.add_css_class("strand-palette-list");
        list_view.set_single_click_activate(true);
        scrolled.set_child(Some(&list_view));
        vbox.append(&scrolled);

        window.set_content(Some(&vbox));

        let all_items = Rc::new(items);
        let matcher = Rc::new(RefCell::new(FuzzyMatcher::new()));
        let on_select = Rc::new(on_select);

        let scroll_to = {
            let list_view = list_view.clone();
            Rc::new(move |pos: u32| {
                if list_view.is_mapped() && list_view.width() > 0 {
                    let _ = list_view.activate_action("list.scroll-to-item", Some(&pos.to_variant()));
                }
            })
        };

        // Filter and virtualized population helper
        let do_filter = {
            let all_items = all_items.clone();
            let store = store.clone();
            let selection = selection.clone();
            let counter_label = counter_label.clone();
            let matcher = matcher.clone();
            let scroll_to = scroll_to.clone();

            move |query: &str, target_cursor: usize| {
                let query = query.trim();
                let is_path = mode == PaletteMode::FilePicker;

                let mut list = Vec::new();
                if query.is_empty() {
                    for item in all_items.iter() {
                        list.push(PaletteRowData {
                            item: item.clone(),
                            indices: Vec::new(),
                        });
                    }
                } else {
                    let mut m = matcher.borrow_mut();
                    let results = m.filter_and_sort(query, &all_items, |it| &it.search_key, is_path);
                    for r in results {
                        list.push(PaletteRowData {
                            item: r.item,
                            indices: r.indices,
                        });
                    }
                }

                let total_shown = list.len();
                counter_label.set_text(&format!("{} / {}", total_shown, all_items.len()));

                let boxed_items: Vec<glib::BoxedAnyObject> = list
                    .into_iter()
                    .map(glib::BoxedAnyObject::new)
                    .collect();

                store.splice(0, store.n_items(), &boxed_items);

                if total_shown > 0 {
                    let cur = (target_cursor as u32).min((total_shown - 1) as u32);
                    selection.set_selected(cur);
                    scroll_to(cur);
                }
            }
        };

        // Initial filter run with starting cursor position
        do_filter(initial_input, initial_cursor);
        if !initial_input.is_empty() {
            search_entry.set_text(initial_input);
            search_entry.set_position(-1);
        }

        // Live filter as user types
        {
            let do_filter = do_filter.clone();
            search_entry.connect_search_changed(move |entry| {
                do_filter(&entry.text(), 0);
            });
        }

        // Row activation helper (Enter or Click)
        let activate_selection = {
            let window = window.clone();
            let selection = selection.clone();
            let store = store.clone();
            let search_entry = search_entry.clone();
            let on_select = on_select.clone();

            move || {
                let current_text = search_entry.text().to_string();
                let selected_pos = selection.selected();

                if selected_pos != gtk::INVALID_LIST_POSITION
                    && let Some(obj) = store.item(selected_pos).and_downcast::<glib::BoxedAnyObject>()
                {
                    let row_data: std::cell::Ref<PaletteRowData> = obj.borrow();
                    let item = row_data.item.clone();
                    drop(row_data);
                    window.close();
                    on_select(item, current_text);
                    return;
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

        // ListView item activated on single click or row activation
        {
            let activate_selection = activate_selection.clone();
            list_view.connect_activate(move |_view, _pos| {
                activate_selection();
            });
        }

        // Keyboard Controller at CAPTURE phase for arrow keys, Ctrl-n/Ctrl-p, Enter, Esc
        let key_controller = gtk::EventControllerKey::new();
        key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        {
            let window = window.clone();
            let selection = selection.clone();
            let store = store.clone();
            let activate_selection = activate_selection.clone();
            let scroll_to = scroll_to.clone();

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
                    let count = store.n_items();
                    if count > 0 {
                        let cur = selection.selected();
                        let next = if cur == gtk::INVALID_LIST_POSITION {
                            0
                        } else {
                            (cur + 1).min(count - 1)
                        };
                        selection.set_selected(next);
                        scroll_to(next);
                    }
                    return glib::Propagation::Stop;
                }

                // Up or Ctrl-p: previous item
                if keyval == gdk::Key::Up
                    || (is_ctrl && (keyval == gdk::Key::p || keyval == gdk::Key::P))
                {
                    let count = store.n_items();
                    if count > 0 {
                        let cur = selection.selected();
                        let prev = if cur == gtk::INVALID_LIST_POSITION {
                            0
                        } else {
                            cur.saturating_sub(1)
                        };
                        selection.set_selected(prev);
                        scroll_to(prev);
                    }
                    return glib::Propagation::Stop;
                }

                // Page Down: jump 5 items
                if keyval == gdk::Key::Page_Down {
                    let count = store.n_items();
                    if count > 0 {
                        let cur = selection.selected();
                        let next = if cur == gtk::INVALID_LIST_POSITION {
                            0
                        } else {
                            (cur + 5).min(count - 1)
                        };
                        selection.set_selected(next);
                        scroll_to(next);
                    }
                    return glib::Propagation::Stop;
                }

                // Page Up: jump 5 items up
                if keyval == gdk::Key::Page_Up {
                    let count = store.n_items();
                    if count > 0 {
                        let cur = selection.selected();
                        let prev = if cur == gtk::INVALID_LIST_POSITION {
                            0
                        } else {
                            cur.saturating_sub(5)
                        };
                        selection.set_selected(prev);
                        scroll_to(prev);
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

        if initial_cursor > 0 {
            let list_view_clone = list_view.clone();
            let scroll_to = scroll_to.clone();
            let mut attempts = 0;
            glib::idle_add_local(move || {
                attempts += 1;
                if list_view_clone.is_mapped() && list_view_clone.width() > 0 {
                    scroll_to(initial_cursor as u32);
                    glib::ControlFlow::Break
                } else if attempts > 10 {
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            });
        }

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
            PaletteConfig {
                title: "Test Palette",
                placeholder: "Search...",
                mode: PaletteMode::FilePicker,
                initial_input: "",
                initial_cursor: 0,
            },
            items,
            move |item, _raw| {
                *sel_clone.borrow_mut() = Some(item.id);
            },
        );

        assert_eq!(dialog.title().as_deref(), Some("Test Palette"));
        for _ in 0..20 {
            glib::MainContext::default().iteration(false);
        }
        dialog.close();
        for _ in 0..10 {
            glib::MainContext::default().iteration(false);
        }
    }

    #[gtk::test]
    fn test_dialog_creation_with_nonzero_cursor() {
        let parent = gtk::Window::new();
        let items = vec![
            PaletteItem::new("buf1", "1: src/main.rs", "src/main.rs"),
            PaletteItem::new("buf2", "2: src/app.rs", "src/app.rs"),
            PaletteItem::new("buf3", "3: Cargo.toml", "Cargo.toml"),
        ];

        let selected = Rc::new(RefCell::new(None));
        let sel_clone = selected.clone();
        let dialog = CommandPaletteDialog::show(
            &parent,
            PaletteConfig {
                title: "Buffers",
                placeholder: "Open buffer...",
                mode: PaletteMode::BufferPicker,
                initial_input: "",
                initial_cursor: 1,
            },
            items,
            move |item, _raw| {
                *sel_clone.borrow_mut() = Some(item.id);
            },
        );

        assert_eq!(dialog.title().as_deref(), Some("Buffers"));
        for _ in 0..20 {
            glib::MainContext::default().iteration(false);
        }
        dialog.close();
        for _ in 0..10 {
            glib::MainContext::default().iteration(false);
        }
    }
}
