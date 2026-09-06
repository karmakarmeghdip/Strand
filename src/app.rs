use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::AdwApplicationWindowExt;
use gtk::gio::prelude::ApplicationExt;
use gtk::glib;
use gtk::prelude::{BoxExt, Cast, EventControllerExt, TextViewExt, WidgetExt};
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use sourceview5::prelude::{BufferExt, ViewExt};

use crate::components::editor::state::EditorState;
use crate::components::editor::{apply_theme, handle_key, KeyHandleResult, CATPPUCCIN_MOCHA};
use crate::components::statusline::{StatusLineConfig, StatusLineView};
use crate::components::which_key::{WhichKeyData, WhichKeyView};
use crate::config::Config;
use crate::editor::workspace::Workspace;
use crate::editor::DocumentId;
use crate::keymap::trie::gdk_key_to_event;
use crate::keymap::Mode;

#[derive(Debug, Clone, Default)]
pub struct AppInit {
    pub target_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteOpenRequest {
    File,
    Buffer,
    Command,
}

pub struct App {
    mode: Mode,
    editor_state: Rc<RefCell<EditorState>>,
    which_key: Option<WhichKeyData>,
    pending_palette: Rc<RefCell<Option<PaletteOpenRequest>>>,
}

#[derive(Debug)]
pub enum AppMsg {
    ModeChanged(Mode),
    DocumentChanged(DocumentId),
    OpenFilePicker,
    OpenBufferPicker,
    OpenCommandPalette,
    OpenFile(PathBuf),
    ExecuteCommand(String),
    ShowWhichKey(WhichKeyData),
    HideWhichKey,
    Quit,
    #[allow(dead_code)]
    Noop,
}

pub struct AppWidgets {
    window: adw::ApplicationWindow,
    source_view: sourceview5::View,
    title: adw::WindowTitle,
    which_key_view: WhichKeyView,
    status_line: StatusLineView,
    #[allow(dead_code)]
    editor_state: Rc<RefCell<EditorState>>,
}

fn connect_bracket_matched_signal(
    buf: &sourceview5::Buffer,
    state: Rc<RefCell<EditorState>>,
) {
    buf.connect_bracket_matched(move |_buf, iter, match_type| {
        if let (sourceview5::BracketMatchType::Found, Some(iter)) = (match_type, iter) {
            state.borrow_mut().last_matched_bracket = Some(iter.offset());
        } else {
            state.borrow_mut().last_matched_bracket = None;
        }
    });
}

impl SimpleComponent for App {
    type Init = AppInit;
    type Input = AppMsg;
    type Output = ();
    type Root = adw::ApplicationWindow;
    type Widgets = AppWidgets;

    fn init_root() -> Self::Root {
        let window = adw::ApplicationWindow::builder()
            .title("Strand")
            .default_width(900)
            .default_height(600)
            .build();
        window.add_css_class("strand-main-window");
        window
    }

    fn init(
        init: Self::Init,
        window: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let workspace = Workspace::from_path_or_cwd(init.target_path.as_deref());
        let config = Config::load_default().unwrap_or_default();
        let mut state = EditorState::with_config_and_workspace(config.clone(), workspace);

        // If target path is a file or new file, open it directly
        if let Some(ref path) = init.target_path
            && (path.is_file() || !path.exists())
        {
            let _ = state.open(path);
        }

        let editor_state = Rc::new(RefCell::new(state));

        use gtk::prelude::GtkWindowExt;
        window.set_default_size(config.gui.window_width, config.gui.window_height);
        if config.gui.maximized {
            window.maximize();
        }

        if let Some(ref variant) = config.gui.theme_variant {
            let style_manager = adw::StyleManager::default();
            match variant.to_lowercase().as_str() {
                "dark" => style_manager.set_color_scheme(adw::ColorScheme::ForceDark),
                "light" => style_manager.set_color_scheme(adw::ColorScheme::ForceLight),
                "system" | "default" => style_manager.set_color_scheme(adw::ColorScheme::Default),
                _ => {}
            }
        }

        let font_css = format!(
            "textview {{ font-family: '{}'; font-size: {}pt; }}",
            config.gui.font_family, config.gui.font_size
        );
        let css_provider = gtk::CssProvider::new();
        css_provider.load_from_data(&font_css);
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &css_provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }

        let model = App {
            mode: Mode::Normal,
            editor_state: editor_state.clone(),
            which_key: None,
            pending_palette: Rc::new(RefCell::new(None)),
        };

        // Active document buffer (Invariant #1: Single source of truth)
        let (buffer, display_name, rel_path, is_mod, is_ro) = {
            let s = editor_state.borrow();
            let doc = s.current_document();
            (
                doc.buffer().clone(),
                doc.display_name(),
                doc.relative_path(),
                doc.is_modified(),
                doc.is_read_only(),
            )
        };

        let theme_name = config.theme.as_deref().unwrap_or(CATPPUCCIN_MOCHA);
        apply_theme(&buffer, theme_name);
        connect_bracket_matched_signal(&buffer, editor_state.clone());

        // ----- GtkSourceView -----
        let source_view = sourceview5::View::with_buffer(&buffer);
        source_view.set_show_line_numbers(config.gui.show_line_numbers);
        source_view.set_highlight_current_line(
            config.gui.highlight_current_line || config.editor.cursorline,
        );
        if config.gui.line_spacing > 0 {
            let half = (config.gui.line_spacing / 2) as i32;
            source_view.set_pixels_above_lines(half);
            source_view.set_pixels_below_lines(
                (config.gui.line_spacing - config.gui.line_spacing / 2) as i32,
            );
        }
        source_view.set_auto_indent(true);
        source_view.set_tab_width(4);
        source_view.set_indent_width(4);
        source_view.set_insert_spaces_instead_of_tabs(false);
        source_view.set_monospace(true);
        source_view.set_wrap_mode(gtk::WrapMode::None);
        source_view.set_hexpand(true);
        source_view.set_vexpand(true);
        source_view.set_focusable(true);

        // ----- GtkScrolledWindow (kinetic scrolling) -----
        let scrolled = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .has_frame(false)
            .kinetic_scrolling(config.gui.smooth_scrolling)
            .overlay_scrolling(true)
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .build();
        scrolled.set_child(Some(&source_view));

        // ----- Which-Key Floating HUD Overlay -----
        let which_key_view = WhichKeyView::new();
        let overlay = gtk::Overlay::builder()
            .hexpand(true)
            .vexpand(true)
            .build();
        overlay.set_child(Some(&scrolled));
        overlay.add_overlay(which_key_view.widget());

        // ----- Status Bar Component (Phase 5 Helix UI Alignment) -----
        let status_line = StatusLineView::new(StatusLineConfig::default());
        status_line.connect_buffer(&buffer, &source_view);
        status_line.set_file_info(rel_path.as_deref(), is_mod, is_ro);
        status_line.set_breadcrumbs(&display_name);
        status_line.set_vcs(&editor_state.borrow().workspace.vcs_info());

        // ----- AdwHeaderBar -----
        let header = adw::HeaderBar::new();
        let title = adw::WindowTitle::new(
            &display_name,
            &editor_state.borrow().workspace.root().display().to_string(),
        );
        header.set_title_widget(Some(&title));

        // ----- Root layout: vertical Box -----
        let vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();
        vbox.append(&header);
        vbox.append(&overlay);
        status_line.widget().set_visible(config.gui.show_status_line);
        vbox.append(status_line.widget());

        window.set_content(Some(&vbox));

        // Normal mode starts with block cursor via native overwrite mode
        source_view.set_overwrite(true);

        // ----- Modal engine: GtkEventControllerKey at CAPTURE (fast path) -----
        // Invariant #2: per-keystroke motions are imperative on GtkTextBuffer;
        // Relm4 messages only for structural transitions (mode switches, which-key, buffer switch).
        let key_controller = gtk::EventControllerKey::new();
        key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let state_for_key = editor_state.clone();
        let source_view_for_key = source_view.clone();
        let which_key_view_for_key = which_key_view.clone();
        let status_line_for_key = status_line.clone();
        let title_for_key = title.clone();
        let sender_for_key = sender.clone();

        key_controller.connect_key_pressed(move |_, keyval, _keycode, state| {
            let (mode, active_buffer, old_doc_id) = {
                let s = state_for_key.borrow();
                (s.mode, s.current_buffer(), s.current_document_id)
            };
            let key_opt = gdk_key_to_event(keyval, state);

            let Some(key) = key_opt else {
                if mode == Mode::Insert {
                    return glib::Propagation::Proceed;
                }
                return glib::Propagation::Stop;
            };

            let old_which_key = state_for_key.borrow().which_key.clone();
            let result = handle_key(&mut state_for_key.borrow_mut(), &active_buffer, key);

            // Sync Which-Key presentation
            let new_which_key = state_for_key.borrow().which_key.clone();
            if new_which_key != old_which_key {
                if let Some(ref data) = new_which_key {
                    which_key_view_for_key.show(data);
                    sender_for_key.input(AppMsg::ShowWhichKey(data.clone()));
                } else {
                    which_key_view_for_key.hide();
                    sender_for_key.input(AppMsg::HideWhichKey);
                }
            }

            // Sync Status messages (echo area)
            let status_msg = state_for_key.borrow_mut().status_msg.take();
            if let Some((msg, sev)) = status_msg {
                status_line_for_key.echo(&msg, sev);
            }

            // Sync pending chord / numerical count / register indicator
            let (count, pending_chord, selected_reg) = {
                let s = state_for_key.borrow();
                (s.count, s.keymap.pending_prefix_string(), s.selected_register)
            };
            status_line_for_key.set_pending(count, &pending_chord, selected_reg);

            // Check if document / buffer switched
            let new_doc_id = state_for_key.borrow().current_document_id;
            if new_doc_id != old_doc_id {
                let (new_buffer, display_name, rel_path, is_mod, is_ro, theme_opt) = {
                    let s = state_for_key.borrow();
                    let doc = s.current_document();
                    (
                        doc.buffer().clone(),
                        doc.display_name(),
                        doc.relative_path(),
                        doc.is_modified(),
                        doc.is_read_only(),
                        s.config.theme.clone(),
                    )
                };
                apply_theme(&new_buffer, theme_opt.as_deref().unwrap_or(CATPPUCCIN_MOCHA));
                connect_bracket_matched_signal(&new_buffer, state_for_key.clone());
                source_view_for_key.set_buffer(Some(&new_buffer));
                title_for_key.set_title(&display_name);
                status_line_for_key.connect_buffer(&new_buffer, &source_view_for_key);
                status_line_for_key.set_file_info(rel_path.as_deref(), is_mod, is_ro);
                status_line_for_key.set_breadcrumbs(&display_name);
                sender_for_key.input(AppMsg::DocumentChanged(new_doc_id));
            }

            match result {
                KeyHandleResult::Propagate => glib::Propagation::Proceed,
                KeyHandleResult::Stop => glib::Propagation::Stop,
                KeyHandleResult::DocumentChanged(_) => glib::Propagation::Stop,
                KeyHandleResult::OpenFilePicker => {
                    sender_for_key.input(AppMsg::OpenFilePicker);
                    glib::Propagation::Stop
                }
                KeyHandleResult::OpenBufferPicker => {
                    sender_for_key.input(AppMsg::OpenBufferPicker);
                    glib::Propagation::Stop
                }
                KeyHandleResult::OpenCommandPalette => {
                    sender_for_key.input(AppMsg::OpenCommandPalette);
                    glib::Propagation::Stop
                }
                KeyHandleResult::Quit => {
                    sender_for_key.input(AppMsg::Quit);
                    glib::Propagation::Stop
                }
                KeyHandleResult::ModeChanged(new_mode) => {
                    source_view_for_key.set_overwrite(new_mode == Mode::Normal);
                    status_line_for_key.set_mode(new_mode);
                    sender_for_key.input(AppMsg::ModeChanged(new_mode));
                    glib::Propagation::Stop
                }
            }
        });
        source_view.add_controller(key_controller);
        source_view.grab_focus();

        let widgets = AppWidgets {
            window,
            source_view,
            title,
            which_key_view,
            status_line,
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
            AppMsg::DocumentChanged(id) => {
                self.editor_state.borrow_mut().switch_document(id);
            }
            AppMsg::OpenFile(path) => {
                let _ = self.editor_state.borrow_mut().open(&path);
            }
            AppMsg::ExecuteCommand(cmd) => {
                let res = {
                    let buf = self.editor_state.borrow().current_buffer();
                    let mut state = self.editor_state.borrow_mut();
                    let mut cx = crate::commands::Context::new(
                        &mut state,
                        buf.upcast_ref(),
                        1,
                        '"',
                    );
                    crate::commands::file::execute_command(&mut cx, &cmd)
                };
                if res == KeyHandleResult::Quit {
                    relm4::main_application().quit();
                }
            }
            AppMsg::ShowWhichKey(data) => {
                self.which_key = Some(data);
            }
            AppMsg::HideWhichKey => {
                self.which_key = None;
            }
            AppMsg::OpenFilePicker => {
                *self.pending_palette.borrow_mut() = Some(PaletteOpenRequest::File);
            }
            AppMsg::OpenBufferPicker => {
                *self.pending_palette.borrow_mut() = Some(PaletteOpenRequest::Buffer);
            }
            AppMsg::OpenCommandPalette => {
                *self.pending_palette.borrow_mut() = Some(PaletteOpenRequest::Command);
            }
            AppMsg::Quit => {
                relm4::main_application().quit();
            }
            AppMsg::Noop => {}
        }
    }

    fn update_view(&self, widgets: &mut Self::Widgets, sender: ComponentSender<Self>) {
        let s = self.editor_state.borrow();
        if s.documents.is_empty() {
            return;
        }
        let doc = s.current_document();
        let current_theme = s.config.theme.as_deref().unwrap_or(CATPPUCCIN_MOCHA);
        apply_theme(doc.buffer(), current_theme);
        connect_bracket_matched_signal(doc.buffer(), self.editor_state.clone());

        widgets.source_view.set_overwrite(self.mode == Mode::Normal);
        widgets.status_line.set_mode(self.mode);
        widgets.source_view.set_buffer(Some(doc.buffer()));
        widgets.title.set_title(&doc.display_name());
        widgets.title.set_subtitle(&s.workspace.root().display().to_string());
        widgets.status_line.connect_buffer(doc.buffer(), &widgets.source_view);
        widgets.status_line.set_file_info(
            doc.relative_path().as_deref(),
            doc.is_modified(),
            doc.is_read_only(),
        );
        widgets.status_line.set_breadcrumbs(&doc.display_name());
        widgets.status_line.set_vcs(&s.workspace.vcs_info());
        if let Some(ref data) = self.which_key {
            widgets.which_key_view.show(data);
        } else {
            widgets.which_key_view.hide();
        }

        // Show pending palette dialog request
        if let Some(req) = self.pending_palette.borrow_mut().take() {
            match req {
                PaletteOpenRequest::File => {
                    let sender = sender.clone();
                    crate::components::command_palette::show_file_picker(
                        &widgets.window,
                        &s.workspace,
                        move |path| sender.input(AppMsg::OpenFile(path)),
                    );
                }
                PaletteOpenRequest::Buffer => {
                    let sender = sender.clone();
                    let current_id = s.current_document_id;
                    let items: Vec<crate::components::command_palette::buffers::BufferPickerItem> = s
                        .documents_in_mru_order()
                        .into_iter()
                        .map(|d| {
                            crate::components::command_palette::buffers::BufferPickerItem {
                                id: d.id(),
                                display_name: d.display_name(),
                                relative_path: d.relative_path().map(|p| p.to_string_lossy().to_string()),
                                is_modified: d.is_modified(),
                                is_read_only: d.is_read_only(),
                                is_current: d.id() == current_id,
                                focused_at: d.focused_at(),
                            }
                        })
                        .collect();
                    crate::components::command_palette::show_buffer_picker(
                        &widgets.window,
                        items,
                        move |id| sender.input(AppMsg::DocumentChanged(id)),
                    );
                }
                PaletteOpenRequest::Command => {
                    let sender = sender.clone();
                    crate::components::command_palette::show_command_palette(
                        &widgets.window,
                        move |cmd| sender.input(AppMsg::ExecuteCommand(cmd)),
                    );
                }
            }
        }

        // Sync echo message if command execution left a status message
        drop(s);
        let status_msg = self.editor_state.borrow_mut().status_msg.take();
        if let Some((msg, sev)) = status_msg {
            widgets.status_line.echo(&msg, sev);
        }
    }
}
