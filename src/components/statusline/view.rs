use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use sourceview5::prelude::*;

use crate::keymap::Mode;

use super::config::{
    DiagnosticItem, DiagnosticSeverity, IndentStyle, LineEnding, StatusLineConfig, VcsInfo,
};
use super::icons::init_icons;
use super::popovers::{DiagnosticsPopover, FormatPopover, GotoLinePopover, VcsPopover};
use super::style::init_css;

/// The GTK4 reactive status bar widget mirroring the Helix editor statusline.
#[derive(Clone)]
pub struct StatusLineView {
    root_box: gtk::Box,
    #[allow(dead_code)]
    progress_bar: gtk::ProgressBar,
    #[allow(dead_code)]
    bar_box: gtk::CenterBox,

    // Left elements
    mode_box: gtk::Box,
    mode_label: gtk::Label,
    pending_box: gtk::Box,
    pending_label: gtk::Label,
    #[allow(dead_code)]
    file_box: gtk::Box,
    file_name_label: gtk::Label,
    file_modified_label: gtk::Label,
    file_readonly_label: gtk::Label,
    diag_btn: gtk::MenuButton,
    diag_error_box: gtk::Box,
    diag_error_label: gtk::Label,
    diag_warn_box: gtk::Box,
    diag_warn_label: gtk::Label,
    diag_ok_box: gtk::Box,

    // Center elements
    stack: gtk::Stack,
    breadcrumbs_label: gtk::Label,
    echo_label: gtk::Label,
    echo_generation: Rc<Cell<u64>>,

    // Right elements
    spinner: gtk::Spinner,
    health_dot: gtk::Box,
    #[allow(dead_code)]
    git_btn: gtk::MenuButton,
    git_label: gtk::Label,
    format_btn: gtk::MenuButton,
    format_label: gtk::Label,
    #[allow(dead_code)]
    selections_btn: gtk::Button,
    selections_label: gtk::Label,
    position_btn: gtk::MenuButton,
    position_label: gtk::Label,

    // Popover controllers
    diag_popover: Rc<DiagnosticsPopover>,
    #[allow(dead_code)]
    goto_popover: Rc<GotoLinePopover>,
    #[allow(dead_code)]
    format_popover: Rc<FormatPopover>,
    git_popover: Rc<VcsPopover>,

    // Reactive statusline settings
    config: StatusLineConfig,
    active_line_ending: Rc<Cell<LineEnding>>,
    active_indent: Rc<Cell<IndentStyle>>,
    active_encoding: Rc<std::cell::RefCell<String>>,
}

impl StatusLineView {
    /// Construct a new `StatusLineView` with the provided configuration.
    pub fn new(config: StatusLineConfig) -> Self {
        init_css();
        init_icons();

        let root_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .hexpand(true)
            .build();
        root_box.add_css_class("status-line-root");

        // 1. Delicate 2px background progress bar for indexing
        let progress_bar = gtk::ProgressBar::builder()
            .fraction(0.0)
            .visible(false)
            .hexpand(true)
            .build();
        progress_bar.add_css_class("lsp-progress-bar");

        // 2. Horizontal status bar container (CenterBox guarantees true window centering)
        let bar_box = gtk::CenterBox::builder()
            .hexpand(true)
            .build();
        bar_box.add_css_class("status-bar");

        // ===== LEFT SECTION =====
        let left_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .halign(gtk::Align::Start)
            .build();

        // Mode pill (clean, non-interactive status indicator)
        let mode_label = gtk::Label::builder()
            .label(&config.mode.normal)
            .build();
        let mode_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .valign(gtk::Align::Center)
            .build();
        mode_box.add_css_class("mode-pill");
        mode_box.add_css_class("normal");
        mode_box.append(&mode_label);

        // Pending chords / count pill
        let pending_label = gtk::Label::new(None);
        let pending_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(2)
            .visible(false)
            .build();
        pending_box.add_css_class("pending-pill");
        pending_box.append(&pending_label);

        // File info pill
        let file_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .build();
        file_box.add_css_class("file-info-pill");

        let file_icon = gtk::Image::from_icon_name("text-x-generic-symbolic");
        let file_name_label = gtk::Label::builder()
            .label("[scratch]")
            .build();
        file_name_label.add_css_class("file-name-label");

        let file_modified_label = gtk::Label::new(None);
        file_modified_label.add_css_class("file-modified-badge");

        let file_readonly_label = gtk::Label::new(None);
        file_readonly_label.add_css_class("file-readonly-badge");

        file_box.append(&file_icon);
        file_box.append(&file_name_label);
        file_box.append(&file_modified_label);
        file_box.append(&file_readonly_label);

        // Diagnostics pill
        let diag_popover = Rc::new(DiagnosticsPopover::new(|_, _| {}));

        let diag_btn = gtk::MenuButton::builder()
            .has_frame(false)
            .focusable(false)
            .popover(diag_popover.popover())
            .build();
        diag_btn.add_css_class("diag-pill");

        let diag_content = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .build();

        let diag_error_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(2)
            .visible(false)
            .build();
        let diag_err_icon = gtk::Image::from_icon_name("dialog-error-symbolic");
        diag_err_icon.add_css_class("diag-error-label");
        let diag_error_label = gtk::Label::new(Some("0"));
        diag_error_label.add_css_class("diag-error-label");
        diag_error_box.append(&diag_err_icon);
        diag_error_box.append(&diag_error_label);

        let diag_warn_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(2)
            .visible(false)
            .build();
        let diag_warn_icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
        diag_warn_icon.add_css_class("diag-warning-label");
        let diag_warn_label = gtk::Label::new(Some("0"));
        diag_warn_label.add_css_class("diag-warning-label");
        diag_warn_box.append(&diag_warn_icon);
        diag_warn_box.append(&diag_warn_label);

        let diag_ok_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(2)
            .visible(true)
            .build();
        let diag_ok_icon = gtk::Image::from_icon_name("object-select-symbolic");
        diag_ok_icon.add_css_class("diag-ok-label");
        let diag_ok_label = gtk::Label::new(Some("0"));
        diag_ok_label.add_css_class("diag-ok-label");
        diag_ok_box.append(&diag_ok_icon);
        diag_ok_box.append(&diag_ok_label);

        diag_content.append(&diag_error_box);
        diag_content.append(&diag_warn_box);
        diag_content.append(&diag_ok_box);
        diag_btn.set_child(Some(&diag_content));

        left_box.append(&mode_box);
        left_box.append(&pending_box);
        left_box.append(&file_box);
        left_box.append(&diag_btn);

        // ===== CENTER SECTION =====
        let center_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .halign(gtk::Align::Center)
            .build();

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(150)
            .build();

        let breadcrumbs_label = gtk::Label::builder()
            .label("")
            .build();
        breadcrumbs_label.add_css_class("breadcrumbs-label");

        let echo_label = gtk::Label::builder()
            .label("")
            .build();
        echo_label.add_css_class("echo-label");
        echo_label.add_css_class("info");

        stack.add_named(&breadcrumbs_label, Some("breadcrumbs"));
        stack.add_named(&echo_label, Some("echo"));
        stack.set_visible_child_name("breadcrumbs");
        center_box.append(&stack);

        // ===== RIGHT SECTION =====
        let right_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .halign(gtk::Align::End)
            .build();

        // LSP Spinner
        let spinner = gtk::Spinner::builder()
            .spinning(false)
            .visible(false)
            .build();

        // Document health dot
        let health_dot = gtk::Box::builder()
            .valign(gtk::Align::Center)
            .build();
        health_dot.add_css_class("health-dot");
        health_dot.add_css_class("ok");

        // Git VCS pill
        let git_popover = Rc::new(VcsPopover::new());
        let git_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .build();
        let git_icon = gtk::Image::from_icon_name("vcs-branch-symbolic");
        let git_label = gtk::Label::new(Some("main"));
        git_label.add_css_class("dim-label");
        git_box.append(&git_icon);
        git_box.append(&git_label);

        let git_btn = gtk::MenuButton::builder()
            .child(&git_box)
            .has_frame(false)
            .focusable(false)
            .popover(git_popover.popover())
            .build();
        git_btn.add_css_class("git-pill");

        // Format pill (LF · 4 Spaces · UTF-8)
        let active_line_ending = Rc::new(Cell::new(LineEnding::Lf));
        let active_indent = Rc::new(Cell::new(IndentStyle::Spaces(4)));
        let active_encoding = Rc::new(std::cell::RefCell::new(config.default_encoding.clone()));

        let format_popover = Rc::new(FormatPopover::new(|_| {}, |_| {}));

        let format_label = gtk::Label::builder()
            .label(format!(
                "{} · {} · {}",
                active_encoding.borrow(),
                active_line_ending.get(),
                active_indent.get()
            ))
            .build();
        format_label.add_css_class("dim-label");

        let format_btn = gtk::MenuButton::builder()
            .child(&format_label)
            .has_frame(false)
            .focusable(false)
            .popover(format_popover.popover())
            .build();
        format_btn.add_css_class("format-pill");

        // Selections pill
        let selections_label = gtk::Label::new(Some("1 sel"));
        selections_label.add_css_class("dim-label");
        let selections_btn = gtk::Button::builder()
            .child(&selections_label)
            .has_frame(false)
            .focusable(false)
            .build();
        selections_btn.add_css_class("selections-pill");

        // Position pill
        let goto_popover = Rc::new(GotoLinePopover::new(|_, _| {}));
        let position_label = gtk::Label::builder()
            .label("Ln 1, Col 1 (100%)")
            .build();
        position_label.add_css_class("coords-label");

        let position_btn = gtk::MenuButton::builder()
            .child(&position_label)
            .has_frame(false)
            .focusable(false)
            .popover(goto_popover.popover())
            .build();
        position_btn.add_css_class("position-pill");

        right_box.append(&spinner);
        right_box.append(&health_dot);
        right_box.append(&git_btn);
        right_box.append(&format_btn);
        right_box.append(&selections_btn);
        right_box.append(&position_btn);

        bar_box.set_start_widget(Some(&left_box));
        bar_box.set_center_widget(Some(&center_box));
        bar_box.set_end_widget(Some(&right_box));

        root_box.append(&progress_bar);
        root_box.append(&bar_box);

        Self {
            root_box,
            progress_bar,
            bar_box,
            mode_box,
            mode_label,
            pending_box,
            pending_label,
            file_box,
            file_name_label,
            file_modified_label,
            file_readonly_label,
            diag_btn,
            diag_error_box,
            diag_error_label,
            diag_warn_box,
            diag_warn_label,
            diag_ok_box,
            stack,
            breadcrumbs_label,
            echo_label,
            echo_generation: Rc::new(Cell::new(0)),
            spinner,
            health_dot,
            git_btn,
            git_label,
            format_btn,
            format_label,
            selections_btn,
            selections_label,
            position_btn,
            position_label,
            diag_popover,
            goto_popover,
            format_popover,
            git_popover,
            config,
            active_line_ending,
            active_indent,
            active_encoding,
        }
    }

    /// The root widget to pack into the editor vertical layout.
    pub fn widget(&self) -> &gtk::Widget {
        self.root_box.upcast_ref()
    }

    /// Update the mode indicator pill and smoothly transition its colors.
    pub fn set_mode(&self, mode: Mode) {
        let label = match mode {
            Mode::Normal => &self.config.mode.normal,
            Mode::Insert => &self.config.mode.insert,
            Mode::Select => &self.config.mode.select,
        };
        self.mode_label.set_text(label);

        self.mode_box.remove_css_class("normal");
        self.mode_box.remove_css_class("insert");
        self.mode_box.remove_css_class("select");

        match mode {
            Mode::Normal => self.mode_box.add_css_class("normal"),
            Mode::Insert => self.mode_box.add_css_class("insert"),
            Mode::Select => self.mode_box.add_css_class("select"),
        }
    }

    /// Update the pending chord, count, or register indicator pill.
    pub fn set_pending(
        &self,
        count: Option<std::num::NonZeroUsize>,
        chord: &str,
        register: Option<char>,
    ) {
        let mut parts = Vec::new();
        if let Some(c) = count {
            parts.push(c.to_string());
        }
        if !chord.is_empty() {
            parts.push(chord.to_string());
        }
        if let Some(r) = register {
            parts.push(format!("reg=\"{r}"));
        }

        if parts.is_empty() {
            self.pending_box.set_visible(false);
        } else {
            self.pending_label.set_text(&parts.join(" "));
            self.pending_box.set_visible(true);
        }
    }

    /// Update file information: filename/basename, modified status, readonly status.
    pub fn set_file_info(&self, path: Option<&Path>, modified: bool, readonly: bool) {
        let display = match path {
            Some(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "[scratch]".to_string()),
            None => "[scratch]".to_string(),
        };

        self.file_name_label.set_text(&display);
        self.file_modified_label.set_text(if modified { "[+]" } else { "" });
        self.file_readonly_label
            .set_text(if readonly { " [readonly]" } else { "" });
    }

    /// Update breadcrumbs displayed in the center stack.
    pub fn set_breadcrumbs(&self, breadcrumbs: &str) {
        self.breadcrumbs_label.set_text(breadcrumbs);
    }

    /// Display an echo/notification message for 3 seconds with smooth crossfade.
    pub fn echo(&self, msg: &str, severity: DiagnosticSeverity) {
        self.echo_label.set_text(msg);
        self.echo_label.remove_css_class("info");
        self.echo_label.remove_css_class("warning");
        self.echo_label.remove_css_class("error");

        match severity {
            DiagnosticSeverity::Info | DiagnosticSeverity::Hint => {
                self.echo_label.add_css_class("info")
            }
            DiagnosticSeverity::Warning => self.echo_label.add_css_class("warning"),
            DiagnosticSeverity::Error => self.echo_label.add_css_class("error"),
        }

        self.stack.set_visible_child_name("echo");

        // Bump generation to cancel any previously scheduled dismiss timer
        let gen_id = self.echo_generation.get().wrapping_add(1);
        self.echo_generation.set(gen_id);

        let stack_weak = self.stack.downgrade();
        let gen_cell = self.echo_generation.clone();

        glib::timeout_add_local_once(std::time::Duration::from_secs(3), move || {
            if gen_cell.get() == gen_id
                && let Some(stack) = stack_weak.upgrade()
            {
                stack.set_visible_child_name("breadcrumbs");
            }
        });
    }

    /// Update document diagnostics: error/warning badges, document health dot, and popover list.
    #[allow(dead_code)]
    pub fn set_diagnostics(&self, diagnostics: &[DiagnosticItem]) {
        let mut errors = 0;
        let mut warnings = 0;

        for d in diagnostics {
            match d.severity {
                DiagnosticSeverity::Error => errors += 1,
                DiagnosticSeverity::Warning => warnings += 1,
                _ => {}
            }
        }

        if errors > 0 || warnings > 0 {
            self.diag_error_box.set_visible(errors > 0);
            self.diag_error_label.set_text(&errors.to_string());
            self.diag_warn_box.set_visible(warnings > 0);
            self.diag_warn_label.set_text(&warnings.to_string());
            self.diag_ok_box.set_visible(false);
        } else {
            self.diag_error_box.set_visible(false);
            self.diag_warn_box.set_visible(false);
            self.diag_ok_box.set_visible(true);
        }

        self.health_dot.remove_css_class("ok");
        self.health_dot.remove_css_class("warning");
        self.health_dot.remove_css_class("error");

        if errors > 0 {
            self.health_dot.add_css_class("error");
        } else if warnings > 0 {
            self.health_dot.add_css_class("warning");
        } else {
            self.health_dot.add_css_class("ok");
        }

        self.diag_popover.update_diagnostics(diagnostics);
    }

    /// Update version control information and diff stats.
    pub fn set_vcs(&self, vcs: &VcsInfo) {
        let branch = vcs.branch.as_deref().unwrap_or("main");
        if vcs.added > 0 || vcs.modified > 0 || vcs.deleted > 0 {
            self.git_label.set_text(&format!(
                "{} +{} ~{}",
                branch, vcs.added, vcs.modified
            ));
        } else {
            self.git_label.set_text(branch);
        }
        self.git_popover.update_vcs(vcs);
    }

    /// Update LSP progress and sub-pixel progress bar along the top edge.
    #[allow(dead_code)]
    pub fn set_lsp_progress(&self, active: bool, fraction: Option<f64>, _message: Option<&str>) {
        if active {
            self.spinner.set_visible(true);
            self.spinner.start();

            if let Some(f) = fraction {
                self.progress_bar.set_visible(true);
                self.progress_bar.set_fraction(f.clamp(0.0, 1.0));
            } else {
                self.progress_bar.set_visible(true);
                self.progress_bar.pulse();
            }
        } else {
            self.spinner.stop();
            self.spinner.set_visible(false);
            self.progress_bar.set_visible(false);
        }
    }

    /// Update format pill string.
    fn refresh_format_label(&self) {
        self.format_label.set_text(&format!(
            "{} · {} · {}",
            self.active_encoding.borrow(),
            self.active_line_ending.get(),
            self.active_indent.get()
        ));
    }

    /// Update cursor position and selection coordinates from a text buffer.
    pub fn update_cursor_coordinates<B: IsA<gtk::TextBuffer>>(&self, buffer: &B) {
        let buffer = buffer.as_ref();
        let iter = buffer.iter_at_mark(&buffer.get_insert());
        let line = iter.line() + 1;
        let col = iter.line_offset() + 1;
        let total_lines = buffer.line_count().max(1);
        let pct = ((line as usize) * 100) / (total_lines as usize);

        self.position_label
            .set_text(&format!("Ln {line}, Col {col} ({pct}%)"));

        if buffer.has_selection() {
            if let Some((start, end)) = buffer.selection_bounds() {
                let chars = (end.offset() - start.offset()).abs();
                self.selections_label.set_text(&format!(
                    "1 sel ({} char{})",
                    chars,
                    if chars == 1 { "" } else { "s" }
                ));
            }
        } else {
            self.selections_label.set_text("1 sel");
        }
    }

    /// Connect the status bar imperatively to a `GtkSourceBuffer` and `GtkSourceView`.
    ///
    /// Preserves Invariant #2: Keystrokes and cursor movements update the position
    /// readouts directly on the main thread via GTK signal handlers, bypassing the Relm4
    /// message queue for 120/144Hz fluidity.
    pub fn connect_buffer(&self, buffer: &sourceview5::Buffer, view: &sourceview5::View) {
        // 1. Initial coordinates and modified status
        self.update_cursor_coordinates(buffer);
        self.file_modified_label
            .set_text(if buffer.is_modified() { "[+]" } else { "" });

        // 2. Cursor movements and selection changes
        let sl_weak = self.clone();
        buffer.connect_mark_set(move |buf, _iter, mark| {
            if mark.name().as_deref() == Some("insert") {
                sl_weak.update_cursor_coordinates(buf);
            }
        });

        // 3. Modified state tracking
        let sl_weak_mod = self.clone();
        buffer.connect_notify_local(Some("modified"), move |buf, _| {
            sl_weak_mod
                .file_modified_label
                .set_text(if buf.is_modified() { "[+]" } else { "" });
        });

        // 4. Buffer text changes (e.g. total lines change)
        let sl_weak_ch = self.clone();
        buffer.connect_changed(move |buf| {
            sl_weak_ch.update_cursor_coordinates(buf);
        });

        // 5. Connect Go to Line popover jump action
        let buf_for_jump = buffer.clone();
        let view_for_jump = view.clone();
        let goto_popover = Rc::new(GotoLinePopover::new(move |line, col| {
            let target_line = line.saturating_sub(1) as i32;
            let total_lines = buf_for_jump.line_count();
            let clamped_line = target_line.min(total_lines.saturating_sub(1).max(0));
            if let Some(mut iter) = buf_for_jump.iter_at_line(clamped_line) {
                if let Some(col_num) = col {
                    let chars_in_line = iter.chars_in_line();
                    let target_col =
                        (col_num.saturating_sub(1) as i32).min(chars_in_line.saturating_sub(1).max(0));
                    iter.forward_chars(target_col);
                }
                buf_for_jump.place_cursor(&iter);
                view_for_jump.scroll_to_iter(&mut iter, 0.1, false, 0.0, 0.5);
                let v = view_for_jump.clone();
                glib::idle_add_local_once(move || {
                    v.grab_focus();
                });
            }
        }));
        let v_goto_close = view.clone();
        goto_popover.popover().connect_closed(move |_| {
            let v = v_goto_close.clone();
            glib::idle_add_local_once(move || {
                v.grab_focus();
            });
        });
        self.position_btn
            .set_popover(Some(goto_popover.popover()));

        // 6. Connect Diagnostics popover jump action
        let buf_for_diag = buffer.clone();
        let view_for_diag = view.clone();
        let diag_popover = Rc::new(DiagnosticsPopover::new(move |line, col| {
            let target_line = line.saturating_sub(1) as i32;
            let total_lines = buf_for_diag.line_count();
            let clamped_line = target_line.min(total_lines.saturating_sub(1).max(0));
            if let Some(mut iter) = buf_for_diag.iter_at_line(clamped_line) {
                if let Some(col_num) = col {
                    let chars_in_line = iter.chars_in_line();
                    let target_col =
                        (col_num.saturating_sub(1) as i32).min(chars_in_line.saturating_sub(1).max(0));
                    iter.forward_chars(target_col);
                }
                buf_for_diag.place_cursor(&iter);
                view_for_diag.scroll_to_iter(&mut iter, 0.1, false, 0.0, 0.5);
                let v = view_for_diag.clone();
                glib::idle_add_local_once(move || {
                    v.grab_focus();
                });
            }
        }));
        let v_diag_close = view.clone();
        diag_popover.popover().connect_closed(move |_| {
            let v = v_diag_close.clone();
            glib::idle_add_local_once(move || {
                v.grab_focus();
            });
        });
        self.diag_btn.set_popover(Some(diag_popover.popover()));

        // 7. Connect Format popover actions
        let view_for_indent = view.clone();
        let sl_weak_format = self.clone();
        let format_popover = Rc::new(FormatPopover::new(
            {
                let sl = sl_weak_format.clone();
                move |le| {
                    sl.active_line_ending.set(le);
                    sl.refresh_format_label();
                }
            },
            move |indent| {
                match indent {
                    IndentStyle::Spaces(n) => {
                        view_for_indent.set_insert_spaces_instead_of_tabs(true);
                        view_for_indent.set_tab_width(n);
                        view_for_indent.set_indent_width(n as i32);
                    }
                    IndentStyle::Tabs => {
                        view_for_indent.set_insert_spaces_instead_of_tabs(false);
                    }
                }
                sl_weak_format.active_indent.set(indent);
                sl_weak_format.refresh_format_label();
            },
        ));
        let v_format_close = view.clone();
        format_popover.popover().connect_closed(move |_| {
            let v = v_format_close.clone();
            glib::idle_add_local_once(move || {
                v.grab_focus();
            });
        });
        self.format_btn
            .set_popover(Some(format_popover.popover()));

        // 8. Restore editor focus when VCS popover closes
        let v_git_close = view.clone();
        self.git_popover.popover().connect_closed(move |_| {
            let v = v_git_close.clone();
            glib::idle_add_local_once(move || {
                v.grab_focus();
            });
        });
    }

    #[cfg(test)]
    pub fn echo_generation(&self) -> u64 {
        self.echo_generation.get()
    }

    #[cfg(test)]
    pub fn mode_text(&self) -> String {
        self.mode_label.text().to_string()
    }
}
