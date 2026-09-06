use gtk::gdk;

const STATUSLINE_CSS: &str = r#"
/* Catppuccin Mocha palette constants for Status Bar */
/* Base: #1e1e2e, Mantle: #181825, Crust: #11111b */
/* Surface0: #313244, Surface1: #45475a, Surface2: #585b70 */
/* Text: #cdd6f4, Subtext1: #bac2de, Subtext0: #a6adc8 */
/* Overlay1: #7f849c, Overlay0: #6c7086 */
/* Blue: #89b4fa, Green: #a6e3a1, Peach: #fab387, Red: #f38ba8, Yellow: #f9e2af, Lavender: #b4befe */

/* Catppuccin Mocha theme consistency across the window and chrome */
window.strand-main-window,
window.strand-main-window.background {
    background-color: #1e1e2e;
    color: #cdd6f4;
}

/* Ensure all popup surfaces remain completely transparent */
window.popup,
window.popup.background,
window.background.popup {
    background: transparent;
    background-color: transparent;
    box-shadow: none;
    border: none;
}

headerbar {
    background-color: #181825;
    color: #cdd6f4;
    border-bottom: 1px solid #313244;
}

headerbar .title {
    color: #cdd6f4;
    font-weight: 600;
}

headerbar .subtitle {
    color: #a6adc8;
}

/* Status line root container */
.status-line-root {
    background-color: #181825;
}

/* Sleek, low-profile status bar container matching Catppuccin Mocha */
.status-bar {
    background-color: #181825;
    color: #cdd6f4;
    border-top: 1px solid #313244;
    padding: 2px 8px;
    font-size: 11px;
    min-height: 28px;
}

/* Zero out outer menubutton container to eliminate nested box highlights */
.status-bar menubutton,
.status-bar menubutton:hover,
.status-bar menubutton:focus,
.status-bar menubutton:focus-visible,
.status-bar menubutton:focus-within,
.status-bar menubutton:active,
.status-bar menubutton:checked {
    background: transparent;
    background-color: transparent;
    border: none;
    box-shadow: none;
    outline: none;
    padding: 0;
    margin: 0;
    min-height: 0;
    min-width: 0;
}

/* Zero out internal container boxes so only the button renders background */
.status-bar menubutton > button > box,
.status-bar button > box {
    background: transparent;
    background-color: transparent;
    border: none;
    box-shadow: none;
    padding: 0;
    margin: 0;
}

/* Base pill styling on the actual clickable button */
.status-bar menubutton > button,
.status-bar button {
    border: none;
    box-shadow: none;
    outline: none;
    border-radius: 6px;
    padding: 2px 7px;
    min-height: 20px;
    margin: 1px 2px;
    background-color: transparent;
    color: #bac2de;
    font-size: 11px;
    transition: background-color 150ms ease, color 150ms ease;
}

/* Clean single-box hover */
.status-bar menubutton > button:hover,
.status-bar button:hover {
    background-color: #313244;
    color: #cdd6f4;
}

.status-bar menubutton > button:active,
.status-bar menubutton > button:checked {
    background-color: #45475a;
    color: #cdd6f4;
}

/* Mode pill styling with smooth color-blend transitions */
.mode-pill {
    font-weight: 700;
    letter-spacing: 0.5px;
    border-radius: 6px;
    padding: 2px 8px;
    min-height: 20px;
    margin: 1px 2px;
    border: none;
    box-shadow: none;
    transition: background-color 150ms ease, color 150ms ease;
}

.mode-pill.normal {
    background-color: rgba(137, 180, 250, 0.18);
    color: #89b4fa;
}

.mode-pill.insert {
    background-color: rgba(166, 227, 161, 0.18);
    color: #a6e3a1;
}

.mode-pill.select {
    background-color: rgba(250, 179, 135, 0.18);
    color: #fab387;
}


/* Tabular figures for coordinates */
.coords-label {
    font-family: monospace;
    font-feature-settings: "tnum";
    font-weight: 600;
    font-size: 11px;
    color: #cdd6f4;
}

/* Pending chords & count */
.pending-pill {
    background-color: rgba(249, 226, 175, 0.18);
    color: #f9e2af;
    font-weight: 700;
    font-family: monospace;
    border-radius: 6px;
    padding: 2px 6px;
    margin: 1px 2px;
    font-size: 11px;
}

/* File info pill */
.file-info-pill {
    padding: 2px 6px;
    margin: 1px 2px;
    border-radius: 6px;
    background: transparent;
    transition: background-color 150ms ease;
}
.file-info-pill:hover {
    background-color: #313244;
}
.file-name-label {
    font-weight: 600;
    font-size: 11px;
    color: #cdd6f4;
}
.file-modified-badge {
    color: #f9e2af;
    font-weight: 700;
    margin-left: 2px;
}
.file-readonly-badge {
    color: #f38ba8;
    font-weight: 600;
    font-size: 10px;
    margin-left: 4px;
}

/* Delicate 2px background progress bar for indexing */
.lsp-progress-bar {
    background: transparent;
}
.lsp-progress-bar trough {
    min-height: 2px;
    background-color: transparent;
    border: none;
    border-radius: 0;
}
.lsp-progress-bar progress {
    min-height: 2px;
    background-color: #89b4fa;
    border-radius: 0;
}

/* Diagnostics */
.diag-error-label {
    color: #f38ba8;
    font-weight: 600;
    font-size: 11px;
    margin-left: 3px;
    margin-right: 4px;
}
.diag-warning-label {
    color: #f9e2af;
    font-weight: 600;
    font-size: 11px;
    margin-left: 3px;
    margin-right: 4px;
}
.diag-ok-label {
    color: #a6e3a1;
    font-weight: 600;
    font-size: 11px;
    margin-left: 3px;
}

/* Health indicator dot */
.health-dot {
    min-width: 6px;
    min-height: 6px;
    border-radius: 3px;
    margin: 0 4px;
}
.health-dot.ok {
    background-color: #a6e3a1;
}
.health-dot.warning {
    background-color: #f9e2af;
}
.health-dot.error {
    background-color: #f38ba8;
}

/* Breadcrumbs & Echo */
.breadcrumbs-label {
    color: #7f849c;
    font-size: 11px;
}
.echo-label {
    font-weight: 600;
    font-size: 11px;
}
.echo-label.info {
    color: #cdd6f4;
}
.echo-label.error {
    color: #f38ba8;
}
.echo-label.warning {
    color: #f9e2af;
}

/* Popover styling consistent with Catppuccin Mocha */
popover,
popover.status-popover {
    background: transparent;
    background-color: transparent;
    border: none;
    box-shadow: none;
    padding: 0;
}

popover.status-popover > contents {
    background-color: #181825;
    color: #cdd6f4;
    border: 1px solid #313244;
    border-radius: 10px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
    padding: 0;
}

popover.status-popover > arrow {
    background: #181825;
    border-color: #313244;
}

.status-popover-content {
    background: transparent;
    background-color: transparent;
    border: none;
    box-shadow: none;
}

.status-popover-title {
    font-weight: 700;
    font-size: 12px;
    letter-spacing: 0.3px;
    color: #cdd6f4;
}

.status-popover .dim-label {
    color: #a6adc8;
}

.status-popover button {
    background-color: #313244;
    color: #cdd6f4;
    border: 1px solid #45475a;
    border-radius: 6px;
    padding: 4px 10px;
    font-size: 12px;
    transition: background-color 150ms ease;
}

.status-popover button:hover {
    background-color: #45475a;
    color: #cdd6f4;
}

.status-popover button.suggested-action {
    background-color: #89b4fa;
    color: #11111b;
    font-weight: 700;
    border: none;
}

.status-popover button.suggested-action:hover {
    background-color: #b4befe;
    color: #11111b;
}

.goto-entry {
    font-family: monospace;
    font-size: 13px;
    background-color: #11111b;
    color: #cdd6f4;
    border: 1px solid #45475a;
    border-radius: 6px;
    padding: 4px 8px;
}

.goto-entry:focus-within,
.goto-entry:focus {
    border-color: #89b4fa;
    outline: none;
    box-shadow: 0 0 0 1px #89b4fa;
}

.status-popover list,
.status-popover listview {
    background-color: #181825;
    border: 1px solid #313244;
    border-radius: 8px;
}

.status-popover list > row,
.status-popover listview > row {
    background: transparent;
    padding: 4px 6px;
    border-radius: 6px;
}

.status-popover list > row:hover,
.status-popover listview > row:hover {
    background-color: #313244;
}

.status-bar .dim-label {
    color: #a6adc8;
}

.separator-label {
    color: #45475a;
    margin: 0 2px;
    font-size: 10px;
}
"#;

/// Install the CSS styles for the Helix status bar on the default display.
pub fn init_css() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if let Some(display) = gdk::Display::default() {
            let provider = gtk::CssProvider::new();
            provider.load_from_data(STATUSLINE_CSS);
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}
