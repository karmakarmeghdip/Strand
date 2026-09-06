use std::fmt;

/// Elements that can appear in the status line, mirroring `helix_view::editor::StatusLineElement`.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusLineElement {
    /// The editor mode (Normal, Insert, Visual/Selection).
    Mode,
    /// The LSP activity spinner.
    Spinner,
    /// The file basename (the leaf of the open file's path).
    FileBaseName,
    /// The relative file path.
    FileName,
    /// The file absolute path.
    FileAbsolutePath,
    /// The file modification indicator (`[+]`).
    FileModificationIndicator,
    /// An indicator that shows `"[readonly]"` when a file cannot be written.
    ReadOnlyIndicator,
    /// The file encoding (e.g. UTF-8).
    FileEncoding,
    /// The file line endings (CRLF or LF).
    FileLineEnding,
    /// The file indentation style (e.g. 4 Spaces, Tabs).
    FileIndentStyle,
    /// The file type (language ID or "text").
    FileType,
    /// A summary of the number of errors and warnings in the document.
    Diagnostics,
    /// A summary of the number of errors and warnings in the workspace.
    WorkspaceDiagnostics,
    /// The number of selections (cursors).
    Selections,
    /// The number of characters currently in primary selection.
    PrimarySelectionLength,
    /// The cursor position (`line:col`).
    Position,
    /// The cursor position as a percent of the total file (`42%`).
    PositionPercentage,
    /// The total line numbers of the current file.
    TotalLineNumbers,
    /// The separator string.
    Separator,
    /// A single space or spacer widget.
    Spacer,
    /// Current version control information (branch and diff stats).
    VersionControl,
    /// Indicator for selected register, pending chord, and numerical count.
    Register,
    /// The base of current working directory.
    CurrentWorkingDirectory,
    /// Indicator for when code actions are available.
    CodeActionHint,
}

/// Mode labels configuration, mirroring `helix_view::editor::ModeConfig`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeConfig {
    pub normal: String,
    pub insert: String,
    pub select: String,
}

impl Default for ModeConfig {
    fn default() -> Self {
        Self {
            normal: String::from("NOR"),
            insert: String::from("INS"),
            select: String::from("SEL"),
        }
    }
}

/// Diagnostic severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "Error"),
            Self::Warning => write!(f, "Warning"),
            Self::Info => write!(f, "Info"),
            Self::Hint => write!(f, "Hint"),
        }
    }
}

/// A single diagnostic item for display in the status bar and diagnostics popover.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticItem {
    pub severity: DiagnosticSeverity,
    pub line: u32,
    pub col: u32,
    pub message: String,
    pub code: Option<String>,
}

/// Version control status information for the current buffer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VcsInfo {
    pub branch: Option<String>,
    pub added: u32,
    pub modified: u32,
    pub deleted: u32,
}

/// Supported line endings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    Crlf,
}

impl fmt::Display for LineEnding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lf => write!(f, "LF"),
            Self::Crlf => write!(f, "CRLF"),
        }
    }
}

/// Supported indentation styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    Spaces(u32),
    Tabs,
}

impl fmt::Display for IndentStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spaces(n) => write!(f, "{n} Spaces"),
            Self::Tabs => write!(f, "Tabs"),
        }
    }
}

/// Status line layout configuration, mirroring `helix_view::editor::StatusLineConfig`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusLineConfig {
    pub left: Vec<StatusLineElement>,
    pub center: Vec<StatusLineElement>,
    pub right: Vec<StatusLineElement>,
    pub separator: String,
    pub mode: ModeConfig,
    pub default_encoding: String,
}

impl Default for StatusLineConfig {
    fn default() -> Self {
        use StatusLineElement as E;
        Self {
            left: vec![
                E::Mode,
                E::Register,
                E::FileBaseName,
                E::FileModificationIndicator,
                E::ReadOnlyIndicator,
                E::Diagnostics,
            ],
            center: vec![E::Spacer],
            right: vec![
                E::Spinner,
                E::VersionControl,
                E::FileIndentStyle,
                E::FileEncoding,
                E::Selections,
                E::Position,
            ],
            separator: String::from("│"),
            mode: ModeConfig::default(),
            default_encoding: String::from("UTF-8"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = StatusLineConfig::default();
        assert_eq!(config.mode.normal, "NOR");
        assert_eq!(config.mode.insert, "INS");
        assert_eq!(config.mode.select, "SEL");
        assert_eq!(config.separator, "│");
        assert!(config.left.contains(&StatusLineElement::Mode));
        assert!(config.right.contains(&StatusLineElement::Position));
    }

    #[test]
    fn test_indent_style_display() {
        assert_eq!(IndentStyle::Spaces(4).to_string(), "4 Spaces");
        assert_eq!(IndentStyle::Spaces(2).to_string(), "2 Spaces");
        assert_eq!(IndentStyle::Tabs.to_string(), "Tabs");
    }

    #[test]
    fn test_line_ending_display() {
        assert_eq!(LineEnding::Lf.to_string(), "LF");
        assert_eq!(LineEnding::Crlf.to_string(), "CRLF");
    }
}
