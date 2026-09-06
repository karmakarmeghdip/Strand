mod app;
pub mod commands;
pub mod config;
mod components;
pub mod core;
pub mod editor;
mod keymap;

use relm4::RelmApp;

use app::{App, AppInit};

fn main() {
    // libadwaita must be initialized before constructing Adw widgets.
    // RelmApp creates a gtk::Application internally; adw::init() ensures
    // Adwaita types are registered.
    relm4::adw::init().expect("failed to init libadwaita");
    relm4::adw::StyleManager::default().set_color_scheme(relm4::adw::ColorScheme::ForceDark);

    // Filter known upstream GTK4 toolkit warning (GNOME/gtk#1819, #5923) where GtkScrolledWindow
    // snapshots newly activated GtkScrollbar gizmos during initial GtkTextView line validation.
    gtk::glib::log_set_writer_func(|level, fields| {
        if level == gtk::glib::LogLevel::Warning {
            let is_snapshot_gizmo_warning = fields.iter().any(|f| {
                f.key() == "MESSAGE"
                    && f.value_str().is_some_and(|msg| {
                        msg.contains("Trying to snapshot GtkGizmo")
                            && msg.contains("without a current allocation")
                    })
            });
            if is_snapshot_gizmo_warning {
                return gtk::glib::LogWriterOutput::Handled;
            }
        }
        gtk::glib::log_writer_default(level, fields)
    });

    // Handle --version and --help flags
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-V" | "--version" => {
                println!("strand {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "-h" | "--help" => {
                println!(
                    "Strand {}\nNative GTK4 Helix-style editor for Linux\n\nUSAGE:\n    strand [PATH]\n\nARGS:\n    <PATH>    File or project directory to open",
                    env!("CARGO_PKG_VERSION")
                );
                return;
            }
            _ => {}
        }
    }

    let target_path = std::env::args()
        .skip(1)
        .find(|arg| !arg.starts_with('-'))
        .map(std::path::PathBuf::from);

    let bin_name = std::env::args().next().unwrap_or_else(|| "strand".to_string());

    let app = RelmApp::new("dev.strand.editor").with_args(vec![bin_name]);
    app.allow_multiple_instances(true);
    app.run::<App>(AppInit { target_path });
}
