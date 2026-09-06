use gtk::prelude::*;
use std::rc::Rc;

use super::config::{DiagnosticItem, DiagnosticSeverity, IndentStyle, LineEnding, VcsInfo};

/// Parses a string like "42", "42:15", "42, 15", or ":15" into (line, col).
pub fn parse_goto_input(input: &str) -> Option<(u32, Option<u32>)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    // e.g. ":15" -> line 1, col 15
    if let Some(col_str) = trimmed.strip_prefix(':') {
        let col = col_str.trim().parse::<u32>().ok()?;
        return Some((1, Some(col.max(1))));
    }

    let cleaned = trimmed
        .trim_start_matches(|c: char| c.is_alphabetic() || c.is_whitespace());

    if cleaned.is_empty() {
        return None;
    }

    let (line_str, col_str) = if let Some((l, c)) = cleaned.split_once(':') {
        (l, Some(c))
    } else if let Some((l, c)) = cleaned.split_once(',') {
        (l, Some(c))
    } else {
        (cleaned, None)
    };

    let line = line_str.trim().parse::<u32>().ok()?;
    let col = match col_str {
        Some(c) => c.trim().parse::<u32>().ok(),
        None => None,
    };

    Some((line.max(1), col.map(|c| c.max(1))))
}

/// Popover for jumping to a specific line and column.
pub struct GotoLinePopover {
    popover: gtk::Popover,
    #[allow(dead_code)]
    entry: gtk::Entry,
    #[allow(dead_code)]
    jump_button: gtk::Button,
}

impl GotoLinePopover {
    pub fn new<F>(on_jump: F) -> Self
    where
        F: Fn(u32, Option<u32>) + 'static,
    {
        let popover = gtk::Popover::builder()
            .autohide(true)
            .has_arrow(true)
            .position(gtk::PositionType::Top)
            .build();
        popover.add_css_class("status-popover");

        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(10)
            .margin_end(10)
            .width_request(240)
            .build();
        root.add_css_class("status-popover-content");

        let title = gtk::Label::builder()
            .label("Go to Line & Column")
            .halign(gtk::Align::Start)
            .build();
        title.add_css_class("status-popover-title");

        let input_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .build();

        let entry = gtk::Entry::builder()
            .placeholder_text("Line[:Col] (e.g. 42 or 42:15)")
            .hexpand(true)
            .build();
        entry.add_css_class("goto-entry");

        let jump_button = gtk::Button::builder().label("Jump").build();
        jump_button.add_css_class("suggested-action");

        input_box.append(&entry);
        input_box.append(&jump_button);

        root.append(&title);
        root.append(&input_box);
        popover.set_child(Some(&root));

        let on_jump = Rc::new(on_jump);

        let jump_action = {
            let entry = entry.clone();
            let popover = popover.clone();
            let on_jump = on_jump.clone();
            move || {
                let text = entry.text();
                if let Some((line, col)) = parse_goto_input(&text) {
                    popover.popdown();
                    on_jump(line, col);
                }
            }
        };

        let ja1 = jump_action.clone();
        entry.connect_activate(move |_| ja1());

        let ja2 = jump_action;
        jump_button.connect_clicked(move |_| ja2());

        // When popover opens, focus and select entry text
        let entry_for_show = entry.clone();
        popover.connect_show(move |_| {
            entry_for_show.grab_focus();
            entry_for_show.select_region(0, -1);
        });

        Self {
            popover,
            entry,
            jump_button,
        }
    }

    pub fn popover(&self) -> &gtk::Popover {
        &self.popover
    }

    #[allow(dead_code)]
    pub fn set_text(&self, text: &str) {
        self.entry.set_text(text);
    }
}

/// Popover presenting the list of diagnostics and allowing navigation.
pub struct DiagnosticsPopover {
    popover: gtk::Popover,
    #[allow(dead_code)]
    title_label: gtk::Label,
    #[allow(dead_code)]
    list_box: gtk::ListBox,
    #[allow(dead_code)]
    on_jump: Rc<dyn Fn(u32, Option<u32>)>,
}

impl DiagnosticsPopover {
    pub fn new<F>(on_jump: F) -> Self
    where
        F: Fn(u32, Option<u32>) + 'static,
    {
        let popover = gtk::Popover::builder()
            .autohide(true)
            .has_arrow(true)
            .position(gtk::PositionType::Top)
            .build();
        popover.add_css_class("status-popover");

        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(10)
            .margin_end(10)
            .width_request(340)
            .build();
        root.add_css_class("status-popover-content");

        let title_label = gtk::Label::builder()
            .label("Diagnostics")
            .halign(gtk::Align::Start)
            .build();
        title_label.add_css_class("status-popover-title");

        let scrolled = gtk::ScrolledWindow::builder()
            .min_content_height(140)
            .max_content_height(260)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .build();

        let list_box = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .build();
        list_box.add_css_class("boxed-list");

        scrolled.set_child(Some(&list_box));

        root.append(&title_label);
        root.append(&scrolled);
        popover.set_child(Some(&root));

        Self {
            popover,
            title_label,
            list_box,
            on_jump: Rc::new(on_jump),
        }
    }

    pub fn popover(&self) -> &gtk::Popover {
        &self.popover
    }

    #[allow(dead_code)]
    pub fn update_diagnostics(&self, diagnostics: &[DiagnosticItem]) {
        // Clear previous items
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        let mut errors = 0;
        let mut warnings = 0;
        let mut infos = 0;

        for d in diagnostics {
            match d.severity {
                DiagnosticSeverity::Error => errors += 1,
                DiagnosticSeverity::Warning => warnings += 1,
                DiagnosticSeverity::Info | DiagnosticSeverity::Hint => infos += 1,
            }
        }

        if diagnostics.is_empty() {
            self.title_label.set_text("Diagnostics (0)");
            let empty_row = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(8)
                .margin_top(12)
                .margin_bottom(12)
                .halign(gtk::Align::Center)
                .build();
            let check_icon = gtk::Image::from_icon_name("object-select-symbolic");
            check_icon.add_css_class("diag-ok-label");
            let label = gtk::Label::new(Some("No problems in document"));
            label.add_css_class("dim-label");
            empty_row.append(&check_icon);
            empty_row.append(&label);
            self.list_box.append(&empty_row);
            return;
        }

        let mut title_parts = Vec::new();
        if errors > 0 {
            title_parts.push(format!("{errors} error{}", if errors == 1 { "" } else { "s" }));
        }
        if warnings > 0 {
            title_parts.push(format!("{warnings} warning{}", if warnings == 1 { "" } else { "s" }));
        }
        if infos > 0 {
            title_parts.push(format!("{infos} info"));
        }
        self.title_label
            .set_text(&format!("Diagnostics ({})", title_parts.join(", ")));

        for item in diagnostics {
            let row = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(8)
                .margin_top(6)
                .margin_bottom(6)
                .margin_start(8)
                .margin_end(8)
                .build();

            let icon_name = match item.severity {
                DiagnosticSeverity::Error => "dialog-error-symbolic",
                DiagnosticSeverity::Warning => "dialog-warning-symbolic",
                DiagnosticSeverity::Info | DiagnosticSeverity::Hint => "dialog-information-symbolic",
            };
            let icon = gtk::Image::from_icon_name(icon_name);
            match item.severity {
                DiagnosticSeverity::Error => icon.add_css_class("diag-error-label"),
                DiagnosticSeverity::Warning => icon.add_css_class("diag-warning-label"),
                DiagnosticSeverity::Info | DiagnosticSeverity::Hint => icon.add_css_class("dim-label"),
            }

            let coord_label = gtk::Label::new(Some(&format!("{}:{}", item.line, item.col)));
            coord_label.add_css_class("coords-label");
            coord_label.add_css_class("dim-label");

            let msg_label = gtk::Label::builder()
                .label(&item.message)
                .halign(gtk::Align::Start)
                .hexpand(true)
                .wrap(true)
                .xalign(0.0)
                .build();

            row.append(&icon);
            row.append(&coord_label);
            row.append(&msg_label);

            let click_btn = gtk::Button::builder().child(&row).has_frame(false).build();

            let on_jump = self.on_jump.clone();
            let popover = self.popover.clone();
            let line = item.line;
            let col = item.col;
            click_btn.connect_clicked(move |_| {
                on_jump(line, Some(col));
                popover.popdown();
            });

            self.list_box.append(&click_btn);
        }
    }
}

/// Popover for switching encoding, line ending, and indentation.
pub struct FormatPopover {
    popover: gtk::Popover,
}

impl FormatPopover {
    pub fn new<FEnd, FIndent>(on_line_ending: FEnd, on_indent: FIndent) -> Self
    where
        FEnd: Fn(LineEnding) + 'static,
        FIndent: Fn(IndentStyle) + 'static,
    {
        let popover = gtk::Popover::builder()
            .autohide(true)
            .has_arrow(true)
            .position(gtk::PositionType::Top)
            .build();
        popover.add_css_class("status-popover");

        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(10)
            .margin_end(10)
            .width_request(220)
            .build();
        root.add_css_class("status-popover-content");

        // Line Ending section
        let le_title = gtk::Label::builder()
            .label("Line Ending")
            .halign(gtk::Align::Start)
            .build();
        le_title.add_css_class("status-popover-title");

        let le_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .homogeneous(true)
            .build();

        let lf_btn = gtk::Button::with_label("LF (Unix)");
        let crlf_btn = gtk::Button::with_label("CRLF (Win)");

        le_box.append(&lf_btn);
        le_box.append(&crlf_btn);

        // Indentation section
        let indent_title = gtk::Label::builder()
            .label("Indentation")
            .halign(gtk::Align::Start)
            .build();
        indent_title.add_css_class("status-popover-title");

        let indent_grid = gtk::Grid::builder()
            .row_spacing(6)
            .column_spacing(6)
            .column_homogeneous(true)
            .build();

        let space2_btn = gtk::Button::with_label("2 Spaces");
        let space4_btn = gtk::Button::with_label("4 Spaces");
        let space8_btn = gtk::Button::with_label("8 Spaces");
        let tabs_btn = gtk::Button::with_label("Tabs");

        indent_grid.attach(&space2_btn, 0, 0, 1, 1);
        indent_grid.attach(&space4_btn, 1, 0, 1, 1);
        indent_grid.attach(&space8_btn, 0, 1, 1, 1);
        indent_grid.attach(&tabs_btn, 1, 1, 1, 1);

        // Encoding section
        let enc_title = gtk::Label::builder()
            .label("Encoding")
            .halign(gtk::Align::Start)
            .build();
        enc_title.add_css_class("status-popover-title");

        let enc_label = gtk::Label::builder()
            .label("UTF-8 (Default)")
            .halign(gtk::Align::Start)
            .build();
        enc_label.add_css_class("dim-label");

        root.append(&le_title);
        root.append(&le_box);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        root.append(&indent_title);
        root.append(&indent_grid);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        root.append(&enc_title);
        root.append(&enc_label);

        popover.set_child(Some(&root));

        let on_le = Rc::new(on_line_ending);
        let on_ind = Rc::new(on_indent);

        {
            let on_le = on_le.clone();
            let pop = popover.clone();
            lf_btn.connect_clicked(move |_| {
                on_le(LineEnding::Lf);
                pop.popdown();
            });
        }
        {
            let on_le = on_le;
            let pop = popover.clone();
            crlf_btn.connect_clicked(move |_| {
                on_le(LineEnding::Crlf);
                pop.popdown();
            });
        }
        {
            let on_ind = on_ind.clone();
            let pop = popover.clone();
            space2_btn.connect_clicked(move |_| {
                on_ind(IndentStyle::Spaces(2));
                pop.popdown();
            });
        }
        {
            let on_ind = on_ind.clone();
            let pop = popover.clone();
            space4_btn.connect_clicked(move |_| {
                on_ind(IndentStyle::Spaces(4));
                pop.popdown();
            });
        }
        {
            let on_ind = on_ind.clone();
            let pop = popover.clone();
            space8_btn.connect_clicked(move |_| {
                on_ind(IndentStyle::Spaces(8));
                pop.popdown();
            });
        }
        {
            let on_ind = on_ind;
            let pop = popover.clone();
            tabs_btn.connect_clicked(move |_| {
                on_ind(IndentStyle::Tabs);
                pop.popdown();
            });
        }

        Self { popover }
    }

    pub fn popover(&self) -> &gtk::Popover {
        &self.popover
    }
}

/// Popover presenting Git repository status and branch details.
pub struct VcsPopover {
    popover: gtk::Popover,
    branch_label: gtk::Label,
    diff_label: gtk::Label,
}

impl VcsPopover {
    pub fn new() -> Self {
        let popover = gtk::Popover::builder()
            .autohide(true)
            .has_arrow(true)
            .position(gtk::PositionType::Top)
            .build();
        popover.add_css_class("status-popover");

        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(10)
            .margin_end(10)
            .width_request(200)
            .build();
        root.add_css_class("status-popover-content");

        let header = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .build();
        let icon = gtk::Image::from_icon_name("vcs-branch-symbolic");
        let branch_label = gtk::Label::builder()
            .label("Branch: main")
            .halign(gtk::Align::Start)
            .build();
        branch_label.add_css_class("status-popover-title");

        header.append(&icon);
        header.append(&branch_label);

        let diff_label = gtk::Label::builder()
            .label("Working tree clean")
            .halign(gtk::Align::Start)
            .build();
        diff_label.add_css_class("dim-label");

        root.append(&header);
        root.append(&diff_label);
        popover.set_child(Some(&root));

        Self {
            popover,
            branch_label,
            diff_label,
        }
    }

    pub fn popover(&self) -> &gtk::Popover {
        &self.popover
    }

    pub fn update_vcs(&self, vcs: &VcsInfo) {
        let branch = vcs.branch.as_deref().unwrap_or("main");
        self.branch_label.set_text(&format!("Branch: {branch}"));

        if vcs.added == 0 && vcs.modified == 0 && vcs.deleted == 0 {
            self.diff_label.set_text("Working tree clean");
        } else {
            self.diff_label.set_text(&format!(
                "+{} additions, ~{} modifications, -{} deletions",
                vcs.added, vcs.modified, vcs.deleted
            ));
        }
    }
}

/// Popover presenting modal editor overview and keymap reference.
#[allow(dead_code)]
pub struct ModePopover {
    popover: gtk::Popover,
    title_label: gtk::Label,
    desc_label: gtk::Label,
}

#[allow(dead_code)]
impl ModePopover {
    pub fn new<F>(on_switch_mode: F) -> Self
    where
        F: Fn(crate::keymap::Mode) + 'static,
    {
        let popover = gtk::Popover::builder()
            .autohide(true)
            .has_arrow(true)
            .position(gtk::PositionType::Top)
            .build();
        popover.add_css_class("status-popover");

        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(10)
            .margin_end(10)
            .width_request(240)
            .build();
        root.add_css_class("status-popover-content");

        let title_label = gtk::Label::builder()
            .label("NORMAL Mode")
            .halign(gtk::Align::Start)
            .build();
        title_label.add_css_class("status-popover-title");

        let desc_label = gtk::Label::builder()
            .label("Object-first movement, motions, and selection commands.")
            .halign(gtk::Align::Start)
            .wrap(true)
            .xalign(0.0)
            .build();
        desc_label.add_css_class("dim-label");

        let switch_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .homogeneous(true)
            .build();

        let norm_btn = gtk::Button::with_label("Normal (Esc)");
        let ins_btn = gtk::Button::with_label("Insert (i)");
        let sel_btn = gtk::Button::with_label("Select (v)");

        switch_box.append(&norm_btn);
        switch_box.append(&ins_btn);
        switch_box.append(&sel_btn);

        let on_switch = Rc::new(on_switch_mode);

        {
            let on_switch = on_switch.clone();
            let pop = popover.clone();
            norm_btn.connect_clicked(move |_| {
                on_switch(crate::keymap::Mode::Normal);
                pop.popdown();
            });
        }
        {
            let on_switch = on_switch.clone();
            let pop = popover.clone();
            ins_btn.connect_clicked(move |_| {
                on_switch(crate::keymap::Mode::Insert);
                pop.popdown();
            });
        }
        {
            let on_switch = on_switch;
            let pop = popover.clone();
            sel_btn.connect_clicked(move |_| {
                on_switch(crate::keymap::Mode::Select);
                pop.popdown();
            });
        }

        root.append(&title_label);
        root.append(&desc_label);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        root.append(&switch_box);

        popover.set_child(Some(&root));

        Self {
            popover,
            title_label,
            desc_label,
        }
    }

    pub fn popover(&self) -> &gtk::Popover {
        &self.popover
    }

    pub fn update_mode(&self, mode: crate::keymap::Mode) {
        match mode {
            crate::keymap::Mode::Normal => {
                self.title_label.set_text("NORMAL Mode");
                self.desc_label
                    .set_text("Object-first movement, motions, and selection commands.");
            }
            crate::keymap::Mode::Insert => {
                self.title_label.set_text("INSERT Mode");
                self.desc_label
                    .set_text("Direct text insertion. Press Esc to return to Normal mode.");
            }
            crate::keymap::Mode::Select => {
                self.title_label.set_text("SELECT Mode");
                self.desc_label
                    .set_text("Selection extension mode. Motions extend the active range.");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_goto_input() {
        assert_eq!(parse_goto_input("42"), Some((42, None)));
        assert_eq!(parse_goto_input("42:15"), Some((42, Some(15))));
        assert_eq!(parse_goto_input("42, 15"), Some((42, Some(15))));
        assert_eq!(parse_goto_input("line 42:5"), Some((42, Some(5))));
        assert_eq!(parse_goto_input(":15"), Some((1, Some(15))));
        assert_eq!(parse_goto_input("0"), Some((1, None)));
        assert_eq!(parse_goto_input(""), None);
        assert_eq!(parse_goto_input("   "), None);
        assert_eq!(parse_goto_input("abc"), None);
    }
}
