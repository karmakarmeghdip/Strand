use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::AdwApplicationWindowExt;
use gtk::prelude::{BoxExt, EventControllerExt, TextBufferExt, TextViewExt, WidgetExt};
use gtk::glib;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use sourceview5::prelude::{BufferExt, ViewExt};

use crate::components::editor::state::EditorState;
use crate::components::editor::{apply_theme, handle_key, KeyHandleResult, CATPPUCCIN_MOCHA};
use crate::keymap::trie::gdk_key_to_event;
use crate::keymap::Mode;

pub struct App {
    mode: Mode,
    editor_state: Rc<RefCell<EditorState>>,
}

#[derive(Debug)]
pub enum AppMsg {
    ModeChanged(Mode),
    #[allow(dead_code)]
    Noop,
}

pub struct AppWidgets {
    #[allow(dead_code)]
    source_view: sourceview5::View,
    #[allow(dead_code)]
    buffer: sourceview5::Buffer,
    title: adw::WindowTitle,
    #[allow(dead_code)]
    editor_state: Rc<RefCell<EditorState>>,
}

fn mode_title(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
        Mode::Select => "SELECT",
    }
}

impl SimpleComponent for App {
    type Init = ();
    type Input = AppMsg;
    type Output = ();
    type Root = adw::ApplicationWindow;
    type Widgets = AppWidgets;

    fn init_root() -> Self::Root {
        adw::ApplicationWindow::builder()
            .title("Strand")
            .default_width(900)
            .default_height(600)
            .build()
    }

    fn init(
        _init: Self::Init,
        window: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let editor_state = Rc::new(RefCell::new(EditorState::new()));
        let model = App {
            mode: Mode::Normal,
            editor_state: editor_state.clone(),
        };

        // ----- GtkSourceBuffer: single source of truth (Invariant #1) -----
        let buffer = sourceview5::Buffer::new(None);
        buffer.set_enable_undo(true);

        // Language: Rust for demo highlighting. Falls back to plain if unavailable.
        let lang_manager = sourceview5::LanguageManager::default();
        if let Some(language) = lang_manager.language("rust") {
            buffer.set_language(Some(&language));
        }
        buffer.set_highlight_syntax(true);
        buffer.set_highlight_matching_brackets(true);

        let state_for_bracket = editor_state.clone();
        buffer.connect_bracket_matched(move |_buf, iter, match_type| {
            if let (sourceview5::BracketMatchType::Found, Some(iter)) = (match_type, iter) {
                state_for_bracket.borrow_mut().last_matched_bracket = Some(iter.offset());
            } else {
                state_for_bracket.borrow_mut().last_matched_bracket = None;
            }
        });

        // Style scheme: Catppuccin Mocha (from Helix theme)
        apply_theme(&buffer, CATPPUCCIN_MOCHA);

        // Initial content – proves highlighting/line numbers/kinetic scroll.
        let initial_text = r#"// Strand — Phase 1: AdwApplicationWindow + GtkSourceView
// Verify: syntax highlighting, line numbers, kinetic scroll

fn main() {
    println!("Hello, Strand!");
    // Scroll me… kinetic scrolling should be smooth at 120/144Hz
    let mut buf = String::new();
    for i in 0..200 {
        buf.push_str(&format!("// line {}\n", i));
    }
    println!("{}", buf);
}
"#;
        buffer.set_text(initial_text);

        // ----- GtkSourceView -----
        let source_view = sourceview5::View::with_buffer(&buffer);
        source_view.set_show_line_numbers(true);
        source_view.set_highlight_current_line(true);
        source_view.set_auto_indent(true);
        source_view.set_tab_width(4);
        source_view.set_indent_width(4);
        source_view.set_insert_spaces_instead_of_tabs(false);
        source_view.set_monospace(true);
        source_view.set_wrap_mode(gtk::WrapMode::None);
        source_view.set_hexpand(true);
        source_view.set_vexpand(true);
        // Give focus to the view so keystrokes go there (future: modal capture).
        source_view.set_focusable(true);

        // ----- GtkScrolledWindow (kinetic scrolling) -----
        let scrolled = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .has_frame(false)
            .kinetic_scrolling(true)
            .overlay_scrolling(true)
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .build();
        // GTK4 single-child uses set_child(), not pack_start/show_all.
        scrolled.set_child(Some(&source_view));

        // ----- AdwHeaderBar -----
        let header = adw::HeaderBar::new();
        let title = adw::WindowTitle::new("Strand", mode_title(Mode::Normal));
        header.set_title_widget(Some(&title));

        // ----- Root layout: vertical Box -----
        let vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();
        vbox.append(&header);
        vbox.append(&scrolled);

        // AdwApplicationWindow is single-child via set_content().
        window.set_content(Some(&vbox));

        // Normal mode starts with block cursor via native overwrite mode
        source_view.set_overwrite(true);

        // Focus the source view after window is ready.
        // No show_all() in GTK4 — widgets visible by default.

        // ----- Modal engine: GtkEventControllerKey at CAPTURE (fast path) -----
        // Invariant #2: per-keystroke motions are imperative on GtkTextBuffer;
        // Relm4 messages only for structural transitions (mode switches).
        let key_controller = gtk::EventControllerKey::new();
        key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let state_for_key = editor_state.clone();
        let buffer_for_key = buffer.clone();
        let title_for_key = title.clone();
        let source_view_for_key = source_view.clone();
        let sender_for_key = sender.clone();
        key_controller.connect_key_pressed(move |_, keyval, _keycode, state| {
            let mode = state_for_key.borrow().mode;
            let key_opt = gdk_key_to_event(keyval, state);

            let Some(key) = key_opt else {
                // Unrecognized key: Insert → propagate, Normal/Select → stop
                if mode == Mode::Insert {
                    return glib::Propagation::Proceed;
                }
                return glib::Propagation::Stop;
            };

            let result = handle_key(&mut state_for_key.borrow_mut(), &buffer_for_key, key);
            match result {
                KeyHandleResult::Propagate => glib::Propagation::Proceed,
                KeyHandleResult::Stop => glib::Propagation::Stop,
                KeyHandleResult::ModeChanged(new_mode) => {
                    // Update cursor: block in Normal, I-beam in Insert/Select
                    source_view_for_key.set_overwrite(new_mode == Mode::Normal);
                    // Update header subtitle imperatively for immediate feedback
                    title_for_key.set_subtitle(mode_title(new_mode));
                    // Also via Relm4 for structural state sync
                    sender_for_key.input(AppMsg::ModeChanged(new_mode));
                    glib::Propagation::Stop
                }
            }
        });
        source_view.add_controller(key_controller);
        // Ensure view can receive focus for CAPTURE
        source_view.grab_focus();

        let widgets = AppWidgets {
            source_view,
            buffer,
            title,
            editor_state,
        };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            AppMsg::ModeChanged(mode) => {
                self.mode = mode;
                self.editor_state.borrow_mut().mode = mode;
            }
            AppMsg::Noop => {}
        }
    }

    fn update_view(&self, widgets: &mut Self::Widgets, _sender: ComponentSender<Self>) {
        widgets.source_view.set_overwrite(self.mode == Mode::Normal);
        widgets.title.set_subtitle(mode_title(self.mode));
    }
}
