use adw::prelude::*;
use gtk::gdk;
use gtk::pango;

use super::WhichKeyData;

const WHICH_KEY_CSS: &str = r#"
.which-key-card {
    background-color: #181825;
    color: #cdd6f4;
    border: 1px solid #313244;
    padding: 10px 16px 14px 16px;
    border-radius: 12px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
}

.which-key-title {
    color: #cdd6f4;
    font-weight: 700;
    font-size: 0.85em;
    letter-spacing: 0.06em;
}

.which-key-keycap {
    font-family: monospace;
    font-weight: bold;
    font-size: 0.85em;
    border-radius: 5px;
    padding: 2px 6px;
    background: #313244;
    border: 1px solid #45475a;
    color: #89b4fa;
    min-width: 18px;
}

.which-key-entry {
    padding: 3px 6px;
    border-radius: 6px;
}

.which-key-entry:hover {
    background: #313244;
}
"#;

/// Floating card overlay widget presenting Which-Key hints using Libadwaita styling.
#[derive(Clone)]
pub struct WhichKeyView {
    revealer: gtk::Revealer,
    title_label: gtk::Label,
    grid: gtk::Grid,
}

impl WhichKeyView {
    /// Initialize CSS styles for Which-Key keycaps and card.
    pub fn init_css() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            if let Some(display) = gdk::Display::default() {
                let provider = gtk::CssProvider::new();
                provider.load_from_data(WHICH_KEY_CSS);
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
        });
    }

    /// Construct a new `WhichKeyView`.
    pub fn new() -> Self {
        Self::init_css();

        let revealer = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::SlideUp)
            .transition_duration(150)
            .reveal_child(false)
            .valign(gtk::Align::End)
            .halign(gtk::Align::Fill)
            .margin_bottom(12)
            .margin_start(16)
            .margin_end(16)
            .build();

        let clamp = adw::Clamp::builder()
            .maximum_size(760)
            .tightening_threshold(540)
            .build();

        let card = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .build();
        card.add_css_class("card");
        card.add_css_class("which-key-card");

        // Header section: title on left, escape dismiss hint on right
        let header = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let title_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();
        title_label.add_css_class("heading");
        title_label.add_css_class("which-key-title");

        let hint_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .halign(gtk::Align::End)
            .build();

        let esc_pill = gtk::Label::new(Some("Esc"));
        esc_pill.add_css_class("which-key-keycap");

        let dismiss_label = gtk::Label::new(Some("dismiss"));
        dismiss_label.add_css_class("dim-label");
        dismiss_label.add_css_class("caption");

        hint_box.append(&esc_pill);
        hint_box.append(&dismiss_label);

        header.append(&title_label);
        header.append(&hint_box);

        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);

        let grid = gtk::Grid::builder()
            .row_spacing(6)
            .column_spacing(16)
            .hexpand(true)
            .build();

        card.append(&header);
        card.append(&separator);
        card.append(&grid);

        clamp.set_child(Some(&card));
        revealer.set_child(Some(&clamp));

        Self {
            revealer,
            title_label,
            grid,
        }
    }

    /// Root widget to add to `gtk::Overlay`.
    pub fn widget(&self) -> &gtk::Revealer {
        &self.revealer
    }

    /// Populate the grid with entries and reveal the overlay.
    pub fn show(&self, data: &WhichKeyData) {
        // Clear previous entries
        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }

        self.title_label.set_text(&data.title.to_uppercase());

        // Responsive columns: adapt based on entry count
        let count = data.entries.len();
        let num_cols = if count <= 4 {
            (count as i32).max(1)
        } else if count <= 9 {
            3
        } else {
            4
        };

        for (i, entry) in data.entries.iter().enumerate() {
            let col = (i as i32) % num_cols;
            let row = (i as i32) / num_cols;

            let item_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(8)
                .hexpand(true)
                .build();
            item_box.add_css_class("which-key-entry");

            let key_badge = gtk::Label::new(Some(&entry.key_label));
            key_badge.add_css_class("which-key-keycap");
            key_badge.set_halign(gtk::Align::Start);
            key_badge.set_valign(gtk::Align::Center);
            key_badge.set_xalign(0.5);

            let desc_label = gtk::Label::new(Some(&entry.description));
            desc_label.set_halign(gtk::Align::Start);
            desc_label.set_valign(gtk::Align::Center);
            desc_label.set_hexpand(true);
            desc_label.set_ellipsize(pango::EllipsizeMode::End);
            desc_label.set_xalign(0.0);

            item_box.append(&key_badge);
            item_box.append(&desc_label);

            if entry.is_submenu {
                let arrow = gtk::Label::new(Some("→"));
                arrow.add_css_class("dim-label");
                arrow.set_halign(gtk::Align::End);
                item_box.append(&arrow);
            }

            self.grid.attach(&item_box, col, row, 1, 1);
        }

        self.revealer.set_reveal_child(true);
    }

    /// Hide the overlay with smooth slide-down animation.
    pub fn hide(&self) {
        self.revealer.set_reveal_child(false);
    }

    /// Check if the overlay is currently revealed or animating open.
    #[allow(dead_code)]
    pub fn is_visible(&self) -> bool {
        self.revealer.reveals_child()
    }
}

impl Default for WhichKeyView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::which_key::WhichKeyEntry;

    #[test]
    fn test_which_key_view_lifecycle() {
        let _ = std::panic::catch_unwind(|| {
            if !gtk::is_initialized() {
                return;
            }

            let view = WhichKeyView::new();
            assert!(!view.is_visible());

            let data = WhichKeyData {
                title: "Test Space".to_string(),
                entries: vec![
                    WhichKeyEntry {
                        key_label: "y".to_string(),
                        description: "yank to clipboard".to_string(),
                        is_submenu: false,
                    },
                    WhichKeyEntry {
                        key_label: "w".to_string(),
                        description: "window".to_string(),
                        is_submenu: true,
                    },
                ],
            };

            view.show(&data);
            assert!(view.is_visible());

            view.hide();
            assert!(!view.is_visible());
        });
    }
}
