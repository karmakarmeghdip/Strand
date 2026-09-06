use std::str::FromStr;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LineNumber {
    #[default]
    Absolute,
    Relative,
}

impl FromStr for LineNumber {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "absolute" | "abs" => Ok(Self::Absolute),
            "relative" | "rel" => Ok(Self::Relative),
            _ => Err(format!("invalid line-number mode '{s}', expected 'absolute' or 'relative'")),
        }
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CursorShape {
    #[default]
    Block,
    Bar,
    Underline,
}

impl FromStr for CursorShape {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "block" => Ok(Self::Block),
            "bar" => Ok(Self::Bar),
            "underline" => Ok(Self::Underline),
            _ => Err(format!("invalid cursor-shape '{s}', expected 'block', 'bar', or 'underline'")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct CursorShapeConfig {
    pub normal: CursorShape,
    pub insert: CursorShape,
    pub select: CursorShape,
}

impl Default for CursorShapeConfig {
    fn default() -> Self {
        Self {
            normal: CursorShape::Block,
            insert: CursorShape::Block,
            select: CursorShape::Block,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GutterType {
    Diagnostics,
    LineNumbers,
    Spacer,
    Diff,
    CodeActionHint,
}

impl FromStr for GutterType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "diagnostics" => Ok(Self::Diagnostics),
            "line-numbers" => Ok(Self::LineNumbers),
            "spacer" => Ok(Self::Spacer),
            "diff" => Ok(Self::Diff),
            "code-action-hint" => Ok(Self::CodeActionHint),
            _ => Err(format!("invalid gutter-type '{s}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct GutterLineNumbersConfig {
    pub min_width: usize,
}

impl Default for GutterLineNumbersConfig {
    fn default() -> Self {
        Self { min_width: 3 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct GutterConfig {
    pub layout: Vec<GutterType>,
    pub line_numbers: GutterLineNumbersConfig,
}

impl Default for GutterConfig {
    fn default() -> Self {
        Self {
            layout: vec![
                GutterType::Diagnostics,
                GutterType::Spacer,
                GutterType::LineNumbers,
                GutterType::Spacer,
                GutterType::Diff,
            ],
            line_numbers: GutterLineNumbersConfig::default(),
        }
    }
}

impl<'de> Deserialize<'de> for GutterConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct GutterVisitor;

        impl<'de> serde::de::Visitor<'de> for GutterVisitor {
            type Value = GutterConfig;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "an array of gutter names or a detailed gutter table")
            }

            fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
            where
                S: serde::de::SeqAccess<'de>,
            {
                let mut gutters = Vec::new();
                while let Some(gutter) = seq.next_element::<String>()? {
                    gutters.push(gutter.parse::<GutterType>().map_err(serde::de::Error::custom)?);
                }
                Ok(GutterConfig {
                    layout: gutters,
                    line_numbers: GutterLineNumbersConfig::default(),
                })
            }

            fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                #[derive(Deserialize)]
                #[serde(rename_all = "kebab-case", default)]
                struct GutterDetailed {
                    layout: Vec<GutterType>,
                    line_numbers: GutterLineNumbersConfig,
                }
                impl Default for GutterDetailed {
                    fn default() -> Self {
                        let def = GutterConfig::default();
                        Self {
                            layout: def.layout,
                            line_numbers: def.line_numbers,
                        }
                    }
                }
                let detailed = GutterDetailed::deserialize(
                    serde::de::value::MapAccessDeserializer::new(map),
                )?;
                Ok(GutterConfig {
                    layout: detailed.layout,
                    line_numbers: detailed.line_numbers,
                })
            }
        }

        deserializer.deserialize_any(GutterVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct FilePickerConfig {
    pub hidden: bool,
    pub follow_symlinks: bool,
    pub deduplicate_links: bool,
    pub parents: bool,
    pub ignore: bool,
    pub git_ignore: bool,
    pub git_global: bool,
    pub git_exclude: bool,
    pub max_depth: Option<usize>,
}

impl Default for FilePickerConfig {
    fn default() -> Self {
        Self {
            hidden: true,
            follow_symlinks: true,
            deduplicate_links: true,
            parents: true,
            ignore: true,
            git_ignore: true,
            git_global: true,
            git_exclude: true,
            max_depth: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct FileExplorerConfig {
    pub hidden: bool,
    pub follow_symlinks: bool,
    pub parents: bool,
    pub ignore: bool,
    pub git_ignore: bool,
    pub git_global: bool,
    pub git_exclude: bool,
    pub flatten_dirs: bool,
}

impl Default for FileExplorerConfig {
    fn default() -> Self {
        Self {
            hidden: false,
            follow_symlinks: false,
            parents: false,
            ignore: false,
            git_ignore: false,
            git_global: false,
            git_exclude: false,
            flatten_dirs: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct StatusLineConfig {
    pub left: Vec<String>,
    pub center: Vec<String>,
    pub right: Vec<String>,
    pub separator: String,
}

impl Default for StatusLineConfig {
    fn default() -> Self {
        Self {
            left: vec![
                "mode".to_string(),
                "spinner".to_string(),
                "file-name".to_string(),
                "read-only-indicator".to_string(),
                "file-modification-indicator".to_string(),
            ],
            center: Vec::new(),
            right: vec![
                "diagnostics".to_string(),
                "selections".to_string(),
                "position".to_string(),
                "file-encoding".to_string(),
                "file-line-ending".to_string(),
                "file-type".to_string(),
            ],
            separator: "│".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct IndentGuidesConfig {
    pub render: bool,
    pub character: char,
    pub skip_levels: u32,
}

impl Default for IndentGuidesConfig {
    fn default() -> Self {
        Self {
            render: false,
            character: '│',
            skip_levels: 0,
        }
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WhitespaceRender {
    #[default]
    None,
    All,
    Selected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct WhitespaceCharacters {
    pub space: char,
    pub nbsp: char,
    pub nnbsp: char,
    pub tab: char,
    pub newline: char,
}

impl Default for WhitespaceCharacters {
    fn default() -> Self {
        Self {
            space: '·',
            nbsp: '⍽',
            nnbsp: '␣',
            tab: '→',
            newline: '⏎',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case", default)]
pub struct WhitespaceConfig {
    pub render: WhitespaceRender,
    pub characters: WhitespaceCharacters,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct LspConfig {
    pub display_messages: bool,
    pub auto_signature_help: bool,
    pub display_inlay_hints: bool,
    pub display_color_swatches: bool,
    pub snippets: bool,
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            display_messages: false,
            auto_signature_help: true,
            display_inlay_hints: false,
            display_color_swatches: true,
            snippets: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct SearchConfig {
    pub smart_case: bool,
    pub wrap_around: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            smart_case: true,
            wrap_around: true,
        }
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BufferLine {
    #[default]
    Never,
    Always,
    Multiple,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct SoftWrapConfig {
    pub enable: bool,
    pub max_wrap: usize,
    pub max_indent_retain: usize,
    pub wrap_indicator: String,
    pub wrap_at_text_width: bool,
}

impl Default for SoftWrapConfig {
    fn default() -> Self {
        Self {
            enable: false,
            max_wrap: 20,
            max_indent_retain: 40,
            wrap_indicator: "↪ ".to_string(),
            wrap_at_text_width: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AutoPairsConfig {
    Enable(bool),
    Custom(std::collections::HashMap<String, String>),
}

impl Default for AutoPairsConfig {
    fn default() -> Self {
        Self::Enable(true)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AutoSaveConfig {
    Enable(bool),
    Custom(std::collections::HashMap<String, toml::Value>),
}

impl Default for AutoSaveConfig {
    fn default() -> Self {
        Self::Enable(false)
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LineEndingConfig {
    #[default]
    Native,
    Lf,
    Crlf,
}

/// Comprehensive Helix-compatible editor configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct EditorConfig {
    pub scrolloff: usize,
    pub scroll_lines: isize,
    pub mouse: bool,
    pub line_number: LineNumber,
    pub cursorline: bool,
    pub cursorcolumn: bool,
    pub gutters: GutterConfig,
    pub auto_pairs: AutoPairsConfig,
    pub auto_completion: bool,
    pub path_completion: bool,
    pub auto_format: bool,
    pub auto_save: AutoSaveConfig,
    pub text_width: usize,
    pub idle_timeout: u64,
    pub completion_timeout: u64,
    pub preview_completion_insert: bool,
    pub completion_trigger_len: u8,
    pub auto_info: bool,
    pub file_picker: FilePickerConfig,
    pub file_explorer: FileExplorerConfig,
    pub statusline: StatusLineConfig,
    pub cursor_shape: CursorShapeConfig,
    pub true_color: bool,
    pub undercurl: bool,
    pub search: SearchConfig,
    pub lsp: LspConfig,
    pub rulers: Vec<u16>,
    pub whitespace: WhitespaceConfig,
    pub bufferline: BufferLine,
    pub indent_guides: IndentGuidesConfig,
    pub color_modes: bool,
    pub soft_wrap: SoftWrapConfig,
    pub default_line_ending: LineEndingConfig,
    pub insert_final_newline: bool,
    pub atomic_save: bool,
    pub trim_final_newlines: bool,
    pub trim_trailing_whitespace: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            scrolloff: 5,
            scroll_lines: 3,
            mouse: true,
            line_number: LineNumber::Absolute,
            cursorline: false,
            cursorcolumn: false,
            gutters: GutterConfig::default(),
            auto_pairs: AutoPairsConfig::default(),
            auto_completion: true,
            path_completion: true,
            auto_format: true,
            auto_save: AutoSaveConfig::default(),
            text_width: 80,
            idle_timeout: 250,
            completion_timeout: 250,
            preview_completion_insert: true,
            completion_trigger_len: 2,
            auto_info: true,
            file_picker: FilePickerConfig::default(),
            file_explorer: FileExplorerConfig::default(),
            statusline: StatusLineConfig::default(),
            cursor_shape: CursorShapeConfig::default(),
            true_color: true,
            undercurl: false,
            search: SearchConfig::default(),
            lsp: LspConfig::default(),
            rulers: Vec::new(),
            whitespace: WhitespaceConfig::default(),
            bufferline: BufferLine::Never,
            indent_guides: IndentGuidesConfig::default(),
            color_modes: false,
            soft_wrap: SoftWrapConfig::default(),
            default_line_ending: LineEndingConfig::Native,
            insert_final_newline: true,
            atomic_save: true,
            trim_final_newlines: false,
            trim_trailing_whitespace: false,
        }
    }
}
