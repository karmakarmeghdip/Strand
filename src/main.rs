mod app;
pub mod commands;
mod components;
pub mod core;
pub mod editor;
mod keymap;

use relm4::RelmApp;

use app::App;

fn main() {
    // libadwaita must be initialized before constructing Adw widgets.
    // RelmApp creates a gtk::Application internally; adw::init() ensures
    // Adwaita types are registered.
    relm4::adw::init().expect("failed to init libadwaita");
    relm4::adw::StyleManager::default().set_color_scheme(relm4::adw::ColorScheme::ForceDark);

    let app = RelmApp::new("dev.strand.editor");
    app.run::<App>(());
}
