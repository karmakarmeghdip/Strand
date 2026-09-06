use std::path::{Path, PathBuf};
use std::sync::Once;

const VCS_BRANCH_SVG: &str = include_str!("../../../data/icons/hicolor/scalable/actions/vcs-branch-symbolic.svg");

static INIT_ICONS_ONCE: Once = Once::new();

fn fallback_icon_dir() -> PathBuf {
    gtk::glib::user_data_dir().join("strand").join("icons")
}

fn ensure_embedded_icons_installed(target_dir: &Path) {
    let action_dir = target_dir.join("hicolor").join("scalable").join("actions");
    if let Err(e) = std::fs::create_dir_all(&action_dir) {
        eprintln!("Failed to create icon directory {}: {}", action_dir.display(), e);
        return;
    }
    let file_path = action_dir.join("vcs-branch-symbolic.svg");
    if !file_path.exists() {
        let _ = std::fs::write(&file_path, VCS_BRANCH_SVG);
    }
}

/// Configure GTK IconTheme with Strand's embedded and bundled icons.
pub fn init_icons() {
    INIT_ICONS_ONCE.call_once(|| {
        let user_icons = fallback_icon_dir();
        ensure_embedded_icons_installed(&user_icons);
    });

    if let Some(display) = gtk::gdk::Display::default() {
        let icon_theme = gtk::IconTheme::for_display(&display);

        // 1. Cargo manifest data directory
        let manifest_icons = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data/icons"));
        if manifest_icons.is_dir() {
            icon_theme.add_search_path(manifest_icons);
        }

        // 2. Relative working directory
        let rel_icons = Path::new("data/icons");
        if rel_icons.is_dir() {
            icon_theme.add_search_path(rel_icons);
        }

        // 3. User data directory fallback
        let user_icons = fallback_icon_dir();
        if user_icons.is_dir() {
            icon_theme.add_search_path(&user_icons);
        }
    }
}
