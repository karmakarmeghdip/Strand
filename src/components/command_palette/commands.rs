use gtk::prelude::IsA;
use super::{CommandPaletteDialog, PaletteItem, PaletteMode};

/// Built-in editor command metadata.
#[derive(Debug, Clone)]
pub struct CommandDescriptor {
    pub name: &'static str,
    pub alias: Option<&'static str>,
    pub args: Option<&'static str>,
    pub description: &'static str,
}

pub const BUILTIN_COMMANDS: &[CommandDescriptor] = &[
    CommandDescriptor {
        name: ":w",
        alias: Some(":write"),
        args: Some("[path]"),
        description: "Write/save the current buffer to disk",
    },
    CommandDescriptor {
        name: ":q",
        alias: Some(":quit"),
        args: None,
        description: "Close the current buffer (exits if last)",
    },
    CommandDescriptor {
        name: ":q!",
        alias: Some(":quit!"),
        args: None,
        description: "Force close buffer, discarding unsaved changes",
    },
    CommandDescriptor {
        name: ":wq",
        alias: Some(":x"),
        args: None,
        description: "Write changes and close the buffer",
    },
    CommandDescriptor {
        name: ":e",
        alias: Some(":open"),
        args: Some("<path>"),
        description: "Open file at path into a buffer",
    },
    CommandDescriptor {
        name: ":b",
        alias: Some(":buffer"),
        args: Some("<id|name>"),
        description: "Switch to buffer by index or file name",
    },
    CommandDescriptor {
        name: ":bn",
        alias: Some(":bnext"),
        args: None,
        description: "Switch to the next buffer",
    },
    CommandDescriptor {
        name: ":bp",
        alias: Some(":bprev"),
        args: None,
        description: "Switch to the previous buffer",
    },
    CommandDescriptor {
        name: ":bc",
        alias: Some(":bclose"),
        args: None,
        description: "Close current buffer",
    },
    CommandDescriptor {
        name: ":pwd",
        alias: None,
        args: None,
        description: "Print current workspace root directory",
    },
    CommandDescriptor {
        name: ":cd",
        alias: None,
        args: Some("<path>"),
        description: "Change current workspace directory",
    },
    CommandDescriptor {
        name: ":theme",
        alias: None,
        args: Some("<name>"),
        description: "Switch Catppuccin theme (latte, frappe, macchiato, mocha)",
    },
];

/// Open the command palette dialog pre-populated with built-in commands.
pub fn show_command_palette<F>(
    parent: &impl IsA<gtk::Window>,
    on_execute: F,
) -> adw::Window
where
    F: Fn(String) + 'static,
{
    let items: Vec<PaletteItem> = BUILTIN_COMMANDS
        .iter()
        .map(|cmd| {
            let title = if let Some(args) = cmd.args {
                format!("{} {}", cmd.name, args)
            } else {
                cmd.name.to_string()
            };

            let search_key = if let Some(alias) = cmd.alias {
                format!("{} {} {}", cmd.name, alias, cmd.description)
            } else {
                format!("{} {}", cmd.name, cmd.description)
            };

            PaletteItem::new(cmd.name, title, search_key)
                .with_subtitle(cmd.description)
        })
        .collect();

    CommandPaletteDialog::show(
        parent,
        super::PaletteConfig {
            title: "Command Palette",
            placeholder: "Type command (:w, :q, :e <path>, :theme <name>)...",
            mode: PaletteMode::Command,
            initial_input: ":",
            initial_cursor: 0,
        },
        items,
        move |item, raw_input| {
            let cmd_to_run = if !raw_input.trim().is_empty() && raw_input.trim() != ":" {
                raw_input
            } else {
                item.id
            };
            on_execute(cmd_to_run);
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::prelude::GtkWindowExt;

    #[test]
    fn test_builtin_commands_catalog() {
        assert!(BUILTIN_COMMANDS.iter().any(|c| c.name == ":w"));
        assert!(BUILTIN_COMMANDS.iter().any(|c| c.name == ":q"));
        assert!(BUILTIN_COMMANDS.iter().any(|c| c.name == ":theme"));
        assert!(BUILTIN_COMMANDS.iter().any(|c| c.name == ":e"));
        assert!(BUILTIN_COMMANDS.iter().any(|c| c.name == ":b"));
    }

    #[gtk::test]
    fn test_show_command_palette_dialog() {
        let parent = gtk::Window::new();
        let dialog = show_command_palette(&parent, |_cmd| {});
        assert_eq!(dialog.title().as_deref(), Some("Command Palette"));
        dialog.close();
    }
}
