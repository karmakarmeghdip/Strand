use serde::{Deserialize, Serialize};

/// GUI-specific configuration for Strand's GTK4 / Libadwaita frontend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct GuiConfig {
    /// Font family to use in the code editor, e.g. "JetBrains Mono", "Fira Code", "Monospace".
    pub font_family: String,
    /// Font size in points.
    pub font_size: f32,
    /// Additional line spacing in pixels.
    pub line_spacing: u32,
    /// Default window width in pixels.
    pub window_width: i32,
    /// Default window height in pixels.
    pub window_height: i32,
    /// Whether the window starts maximized.
    pub maximized: bool,
    /// Whether kinetic/smooth scrolling is enabled in scrolled windows.
    pub smooth_scrolling: bool,
    /// Preferred theme variant ("dark", "light", or "system").
    pub theme_variant: Option<String>,
    /// Whether to display open tabs / bufferline.
    pub show_tab_bar: bool,
    /// Whether to display the bottom status line.
    pub show_status_line: bool,
    /// Whether to show line numbers in the gutter.
    pub show_line_numbers: bool,
    /// Whether to highlight the current line.
    pub highlight_current_line: bool,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            font_family: "Monospace".to_string(),
            font_size: 11.0,
            line_spacing: 1,
            window_width: 1024,
            window_height: 768,
            maximized: false,
            smooth_scrolling: true,
            theme_variant: None,
            show_tab_bar: true,
            show_status_line: true,
            show_line_numbers: true,
            highlight_current_line: true,
        }
    }
}
