pub mod editor;
pub mod gui;

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use gtk::glib;
use serde::Deserialize;

use crate::keymap::default_keymap;
use crate::keymap::trie::{merge_keys, KeyTrie};
use crate::keymap::Mode;

pub use editor::EditorConfig;
pub use gui::GuiConfig;

/// Complete application configuration for Strand, mirroring Helix's config architecture
/// with additional GUI-specific properties.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Active theme name, e.g. "catppuccin-mocha", "onedark".
    pub theme: Option<String>,
    /// Keymap trie roots per mode (default keymap merged with user overrides).
    pub keys: HashMap<Mode, KeyTrie>,
    /// Editor configuration (Helix-compatible).
    pub editor: EditorConfig,
    /// GUI-specific configuration (font family, size, window metrics, etc.).
    pub gui: GuiConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: None,
            keys: default_keymap(),
            editor: EditorConfig::default(),
            gui: GuiConfig::default(),
        }
    }
}

/// Raw configuration structure for deserializing from `config.toml`.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(rename_all = "kebab-case", default)]
pub struct ConfigRaw {
    pub theme: Option<String>,
    pub keys: Option<HashMap<Mode, KeyTrie>>,
    pub editor: Option<toml::Value>,
    pub gui: Option<GuiConfig>,
}

#[derive(Debug)]
pub enum ConfigLoadError {
    BadConfig(toml::de::Error),
    Io(std::io::Error),
}

impl fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadConfig(err) => write!(f, "Invalid configuration TOML: {err}"),
            Self::Io(err) => write!(f, "Failed to read configuration file: {err}"),
        }
    }
}

impl std::error::Error for ConfigLoadError {}

impl From<toml::de::Error> for ConfigLoadError {
    fn from(err: toml::de::Error) -> Self {
        Self::BadConfig(err)
    }
}

impl From<std::io::Error> for ConfigLoadError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// Recursively merge two `toml::Value`s with a depth limit (ported from `helix-loader`).
pub fn merge_toml_values(left: toml::Value, right: toml::Value, merge_depth: usize) -> toml::Value {
    use toml::Value;

    fn get_name(v: &Value) -> Option<&str> {
        v.get("name").and_then(Value::as_str)
    }

    match (left, right) {
        (Value::Array(mut left_items), Value::Array(right_items)) => {
            if merge_depth > 0 {
                left_items.reserve(right_items.len());
                for rvalue in right_items {
                    let lvalue = get_name(&rvalue)
                        .and_then(|rname| {
                            left_items.iter().position(|v| get_name(v) == Some(rname))
                        })
                        .map(|lpos| left_items.remove(lpos));
                    let mvalue = match lvalue {
                        Some(lvalue) => merge_toml_values(lvalue, rvalue, merge_depth - 1),
                        None => rvalue,
                    };
                    left_items.push(mvalue);
                }
                Value::Array(left_items)
            } else {
                Value::Array(right_items)
            }
        }
        (Value::Table(mut left_map), Value::Table(right_map)) => {
            if merge_depth > 0 {
                for (rname, rvalue) in right_map {
                    match left_map.remove(&rname) {
                        Some(lvalue) => {
                            let merged_value = merge_toml_values(lvalue, rvalue, merge_depth - 1);
                            left_map.insert(rname, merged_value);
                        }
                        None => {
                            left_map.insert(rname, rvalue);
                        }
                    }
                }
                Value::Table(left_map)
            } else {
                Value::Table(right_map)
            }
        }
        (_, value) => value,
    }
}

/// Returns the path to the primary global `config.toml` file.
pub fn global_config_file() -> PathBuf {
    if let Ok(val) = std::env::var("STRAND_CONFIG") {
        return PathBuf::from(val);
    }
    if let Ok(val) = std::env::var("HELIX_CONFIG") {
        return PathBuf::from(val);
    }
    let config_dir = glib::user_config_dir();
    let strand_file = config_dir.join("strand").join("config.toml");
    if strand_file.exists() {
        return strand_file;
    }
    let helix_file = config_dir.join("helix").join("config.toml");
    if helix_file.exists() {
        return helix_file;
    }
    strand_file
}

/// Returns the workspace-local `.strand/config.toml` or `.helix/config.toml` by searching upwards from CWD.
pub fn workspace_config_file() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    for ancestor in cwd.ancestors() {
        let strand_cfg = ancestor.join(".strand").join("config.toml");
        if strand_cfg.is_file() {
            return Some(strand_cfg);
        }
        let helix_cfg = ancestor.join(".helix").join("config.toml");
        if helix_cfg.is_file() {
            return Some(helix_cfg);
        }
    }
    None
}

impl Config {
    /// Parse a configuration directly from a TOML string.
    pub fn from_toml(s: &str) -> Result<Self, ConfigLoadError> {
        Self::load(Some(s), None)
    }

    /// Load and merge global and local (workspace) configurations (mirroring Helix's `Config::load`).
    pub fn load(
        global: Option<&str>,
        local: Option<&str>,
    ) -> Result<Self, ConfigLoadError> {
        let global_raw: Option<ConfigRaw> = match global {
            Some(s) if !s.trim().is_empty() => Some(toml::from_str(s)?),
            _ => None,
        };

        let local_raw: Option<ConfigRaw> = match local {
            Some(s) if !s.trim().is_empty() => Some(toml::from_str(s)?),
            _ => None,
        };

        let mut keys = default_keymap();
        let mut theme = None;
        let mut editor_val: Option<toml::Value> = None;
        let mut gui = GuiConfig::default();

        if let Some(global) = global_raw {
            if let Some(global_theme) = global.theme {
                theme = Some(global_theme);
            }
            if let Some(global_keys) = global.keys {
                merge_keys(&mut keys, global_keys);
            }
            if let Some(val) = global.editor {
                editor_val = Some(val);
            }
            if let Some(global_gui) = global.gui {
                gui = global_gui;
            }
        }

        if let Some(local) = local_raw {
            if let Some(local_theme) = local.theme {
                theme = Some(local_theme);
            }
            if let Some(local_keys) = local.keys {
                merge_keys(&mut keys, local_keys);
            }
            if let Some(val) = local.editor {
                editor_val = match editor_val {
                    Some(prev) => Some(merge_toml_values(prev, val, 3)),
                    None => Some(val),
                };
            }
            if let Some(local_gui) = local.gui {
                gui = local_gui;
            }
        }

        let editor: EditorConfig = match editor_val {
            Some(val) => val.try_into().map_err(ConfigLoadError::BadConfig)?,
            None => EditorConfig::default(),
        };

        Ok(Config {
            theme,
            keys,
            editor,
            gui,
        })
    }

    /// Load default configuration from disk (global config merged with optional workspace config).
    pub fn load_default() -> Result<Self, ConfigLoadError> {
        let global_path = global_config_file();
        let global_str = if global_path.is_file() {
            Some(fs::read_to_string(&global_path)?)
        } else {
            None
        };

        let local_path = workspace_config_file();
        let local_str = match local_path {
            Some(ref p) if p.is_file() => Some(fs::read_to_string(p)?),
            _ => None,
        };

        Self::load(global_str.as_deref(), local_str.as_deref())
    }

    /// Load configuration from specific file paths.
    pub fn load_from_paths(
        global_path: Option<&Path>,
        local_path: Option<&Path>,
    ) -> Result<Self, ConfigLoadError> {
        let global_str = match global_path {
            Some(p) if p.is_file() => Some(fs::read_to_string(p)?),
            _ => None,
        };
        let local_str = match local_path {
            Some(p) if p.is_file() => Some(fs::read_to_string(p)?),
            _ => None,
        };
        Self::load(global_str.as_deref(), local_str.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::actions::EditorAction;
    use crate::keymap::input::KeyEvent;
    use crate::keymap::trie::KeyTrieRoot;
    use crate::keymap::KeymapResult;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.theme, None);
        assert_eq!(config.gui.font_family, "Monospace");
        assert_eq!(config.gui.font_size, 11.0);
        assert_eq!(config.editor.scrolloff, 5);
        assert_eq!(config.editor.mouse, true);
        assert_eq!(config.editor.line_number, editor::LineNumber::Absolute);
    }

    #[test]
    fn test_parse_complete_config_toml() {
        let toml_str = r#"
            theme = "catppuccin-mocha"

            [editor]
            line-number = "relative"
            scrolloff = 10
            mouse = false
            cursorline = true
            gutters = ["diagnostics", "line-numbers"]
            rulers = [80, 120]

            [editor.cursor-shape]
            normal = "block"
            insert = "bar"
            select = "underline"

            [editor.file-picker]
            hidden = false
            follow-symlinks = false

            [gui]
            font-family = "JetBrains Mono"
            font-size = 13.5
            window-width = 1280
            window-height = 800
            maximized = true
            line-spacing = 2
            smooth-scrolling = true

            [keys.normal]
            "C-s" = "select_all"
            "w" = "move_left"
            "g" = { "x" = "move_word_forward" }
        "#;

        let config = Config::from_toml(toml_str).unwrap();
        assert_eq!(config.theme.as_deref(), Some("catppuccin-mocha"));

        // Editor
        assert_eq!(config.editor.line_number, editor::LineNumber::Relative);
        assert_eq!(config.editor.scrolloff, 10);
        assert_eq!(config.editor.mouse, false);
        assert_eq!(config.editor.cursorline, true);
        assert_eq!(config.editor.rulers, vec![80, 120]);
        assert_eq!(config.editor.cursor_shape.insert, editor::CursorShape::Bar);
        assert_eq!(config.editor.cursor_shape.select, editor::CursorShape::Underline);
        assert_eq!(config.editor.file_picker.hidden, false);

        // GUI
        assert_eq!(config.gui.font_family, "JetBrains Mono");
        assert_eq!(config.gui.font_size, 13.5);
        assert_eq!(config.gui.window_width, 1280);
        assert_eq!(config.gui.window_height, 800);
        assert_eq!(config.gui.maximized, true);
        assert_eq!(config.gui.line_spacing, 2);
        assert_eq!(config.gui.smooth_scrolling, true);

        // Keys
        let mut root = KeyTrieRoot::new(config.keys);
        assert_eq!(
            root.get(Mode::Normal, "C-s".parse::<KeyEvent>().unwrap()),
            KeymapResult::Matched(EditorAction::SelectAll)
        );
        assert_eq!(
            root.get(Mode::Normal, KeyEvent::char('w')),
            KeymapResult::Matched(EditorAction::MoveLeft)
        );
        let _ = root.get(Mode::Normal, KeyEvent::char('g'));
        assert_eq!(
            root.get(Mode::Normal, KeyEvent::char('x')),
            KeymapResult::Matched(EditorAction::MoveWordForward)
        );
    }

    #[test]
    fn test_merge_global_and_local_configs() {
        let global_toml = r#"
            theme = "catppuccin-latte"

            [editor]
            scrolloff = 5
            cursorline = false

            [gui]
            font-family = "Monospace"
            font-size = 12.0

            [keys.normal]
            "C-s" = "select_all"
        "#;

        let local_toml = r#"
            theme = "catppuccin-mocha"

            [editor]
            cursorline = true

            [gui]
            font-size = 14.0

            [keys.normal]
            "w" = "move_left"
        "#;

        let config = Config::load(Some(global_toml), Some(local_toml)).unwrap();

        // Local theme overrides global
        assert_eq!(config.theme.as_deref(), Some("catppuccin-mocha"));

        // Editor settings merge
        assert_eq!(config.editor.scrolloff, 5);
        assert_eq!(config.editor.cursorline, true);

        // GUI settings merge
        assert_eq!(config.gui.font_family, "Monospace");
        assert_eq!(config.gui.font_size, 14.0);

        // Keys merge: both C-s and w exist
        let mut root = KeyTrieRoot::new(config.keys);
        assert_eq!(
            root.get(Mode::Normal, "C-s".parse::<KeyEvent>().unwrap()),
            KeymapResult::Matched(EditorAction::SelectAll)
        );
        assert_eq!(
            root.get(Mode::Normal, KeyEvent::char('w')),
            KeymapResult::Matched(EditorAction::MoveLeft)
        );
    }

    #[test]
    fn test_merge_toml_values() {
        use toml::Value;

        let base: Value = toml::from_str(
            r#"
            [section]
            a = 1
            b = 2
            "#,
        )
        .unwrap();

        let override_val: Value = toml::from_str(
            r#"
            [section]
            b = 42
            c = 3
            "#,
        )
        .unwrap();

        let merged = merge_toml_values(base, override_val, 3);
        assert_eq!(merged["section"]["a"].as_integer(), Some(1));
        assert_eq!(merged["section"]["b"].as_integer(), Some(42));
        assert_eq!(merged["section"]["c"].as_integer(), Some(3));
    }

    #[test]
    fn test_helix_command_names_in_keys() {
        let toml_str = r#"
            [keys.normal]
            "i" = "insert_mode"
            "j" = "move_visual_line_down"
            "y" = "yank"
            "g" = { "e" = "goto_last_line" }
        "#;

        let config = Config::from_toml(toml_str).unwrap();
        let mut root = KeyTrieRoot::new(config.keys);

        assert_eq!(
            root.get(Mode::Normal, KeyEvent::char('i')),
            KeymapResult::Matched(EditorAction::EnterInsert)
        );
        assert_eq!(
            root.get(Mode::Normal, KeyEvent::char('j')),
            KeymapResult::Matched(EditorAction::MoveDown)
        );
        assert_eq!(
            root.get(Mode::Normal, KeyEvent::char('y')),
            KeymapResult::Matched(EditorAction::YankSelection)
        );
        let _ = root.get(Mode::Normal, KeyEvent::char('g'));
        assert_eq!(
            root.get(Mode::Normal, KeyEvent::char('e')),
            KeymapResult::Matched(EditorAction::GotoFileEnd)
        );
    }

    #[test]
    fn test_helix_editor_sections_and_gui() {
        let toml_str = r#"
            [editor.whitespace]
            render = "all"
            characters = { space = "·", newline = "⏎", tab = "→" }

            [editor.indent-guides]
            render = true
            character = "┆"
            skip-levels = 1

            [editor.lsp]
            display-messages = true
            display-inlay-hints = true

            [editor.soft-wrap]
            enable = true
            max-wrap = 30
            wrap-indicator = "↳"

            [gui]
            font-family = "Fira Code"
            font-size = 14.0
            theme-variant = "dark"
            show-tab-bar = false
        "#;

        let config = Config::from_toml(toml_str).unwrap();

        assert_eq!(config.editor.whitespace.render, editor::WhitespaceRender::All);
        assert_eq!(config.editor.whitespace.characters.space, '·');
        assert_eq!(config.editor.indent_guides.render, true);
        assert_eq!(config.editor.indent_guides.character, '┆');
        assert_eq!(config.editor.indent_guides.skip_levels, 1);
        assert_eq!(config.editor.lsp.display_messages, true);
        assert_eq!(config.editor.lsp.display_inlay_hints, true);
        assert_eq!(config.editor.soft_wrap.enable, true);
        assert_eq!(config.editor.soft_wrap.max_wrap, 30);
        assert_eq!(config.editor.soft_wrap.wrap_indicator, "↳");

        assert_eq!(config.gui.font_family, "Fira Code");
        assert_eq!(config.gui.font_size, 14.0);
        assert_eq!(config.gui.theme_variant.as_deref(), Some("dark"));
        assert_eq!(config.gui.show_tab_bar, false);
    }

    #[test]
    fn test_load_from_paths() {
        let temp_dir = std::env::temp_dir().join(format!("strand_config_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let global_path = temp_dir.join("global_config.toml");
        let local_path = temp_dir.join("local_config.toml");

        fs::write(
            &global_path,
            r#"
            theme = "catppuccin-latte"
            [gui]
            font-size = 12.0
            "#,
        )
        .unwrap();

        fs::write(
            &local_path,
            r#"
            [gui]
            font-size = 15.0
            "#,
        )
        .unwrap();

        let config = Config::load_from_paths(Some(&global_path), Some(&local_path)).unwrap();
        assert_eq!(config.theme.as_deref(), Some("catppuccin-latte"));
        assert_eq!(config.gui.font_size, 15.0);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_example_config_file_valid() {
        let example_content = include_str!("../../data/config.example.toml");
        let config = Config::from_toml(example_content).expect("config.example.toml should parse cleanly");
        assert_eq!(config.theme.as_deref(), Some("catppuccin-mocha"));
        assert_eq!(config.gui.font_family, "Monospace");
        assert_eq!(config.gui.theme_variant.as_deref(), Some("dark"));
        assert_eq!(config.editor.cursorline, true);
        assert_eq!(config.editor.rulers, vec![80, 120]);

        let mut root = KeyTrieRoot::new(config.keys);
        assert_eq!(
            root.get(Mode::Normal, "C-s".parse::<KeyEvent>().unwrap()),
            KeymapResult::Matched(EditorAction::SelectAll)
        );
        assert_eq!(
            root.get(Mode::Normal, KeyEvent::char('U')),
            KeymapResult::Matched(EditorAction::Redo)
        );
    }
}
