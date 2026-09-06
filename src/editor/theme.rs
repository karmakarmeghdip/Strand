use std::path::{Path, PathBuf};
use std::sync::Once;

use gtk::glib;
use sourceview5::prelude::BufferExt;

pub const CATPPUCCIN_MOCHA: &str = "catppuccin-mocha";
pub const CATPPUCCIN_MACCHIATO: &str = "catppuccin-macchiato";
pub const CATPPUCCIN_FRAPPE: &str = "catppuccin-frappe";
pub const CATPPUCCIN_LATTE: &str = "catppuccin-latte";

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatppuccinFlavor {
    Mocha,
    Macchiato,
    Frappe,
    Latte,
}

impl CatppuccinFlavor {
    #[allow(dead_code)]
    pub const fn id(&self) -> &'static str {
        match self {
            Self::Mocha => CATPPUCCIN_MOCHA,
            Self::Macchiato => CATPPUCCIN_MACCHIATO,
            Self::Frappe => CATPPUCCIN_FRAPPE,
            Self::Latte => CATPPUCCIN_LATTE,
        }
    }

    #[allow(dead_code)]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Mocha => "Catppuccin Mocha",
            Self::Macchiato => "Catppuccin Macchiato",
            Self::Frappe => "Catppuccin Frappé",
            Self::Latte => "Catppuccin Latte",
        }
    }

    #[allow(dead_code)]
    pub const fn is_dark(&self) -> bool {
        match self {
            Self::Mocha | Self::Macchiato | Self::Frappe => true,
            Self::Latte => false,
        }
    }
}

// Embedded theme XML files ensuring availability in all environments
const THEME_FILES: &[(&str, &str)] = &[
    ("catppuccin-mocha.xml", include_str!("../../data/styles/catppuccin-mocha.xml")),
    ("catppuccin-macchiato.xml", include_str!("../../data/styles/catppuccin-macchiato.xml")),
    ("catppuccin-frappe.xml", include_str!("../../data/styles/catppuccin-frappe.xml")),
    ("catppuccin-latte.xml", include_str!("../../data/styles/catppuccin-latte.xml")),
];

static INIT_THEMES_ONCE: Once = Once::new();

/// Returns user-specific or system style paths to search.
fn fallback_style_dir() -> PathBuf {
    glib::user_data_dir().join("strand").join("styles")
}

/// Ensures embedded theme XML files exist in the user data directory if not running from source repo.
fn ensure_embedded_themes_installed(target_dir: &Path) {
    if let Err(e) = std::fs::create_dir_all(target_dir) {
        eprintln!("Failed to create style directory {}: {}", target_dir.display(), e);
        return;
    }
    for (filename, content) in THEME_FILES {
        let file_path = target_dir.join(filename);
        if !file_path.exists() {
            let _ = std::fs::write(&file_path, content);
        }
    }
}

fn add_search_path_if_exists(manager: &sourceview5::StyleSchemeManager, path: &Path) {
    if let Some(path_str) = path.to_str().filter(|_| path.is_dir()) {
        manager.prepend_search_path(path_str);
    }
}

/// Configure StyleSchemeManager with Strand's Catppuccin theme search paths.
pub fn init_style_schemes(manager: &sourceview5::StyleSchemeManager) {
    INIT_THEMES_ONCE.call_once(|| {
        let user_styles = fallback_style_dir();
        ensure_embedded_themes_installed(&user_styles);
    });

    // 1. Cargo manifest data directory (development / tests)
    let manifest_styles = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data/styles"));
    add_search_path_if_exists(manager, manifest_styles);

    // 2. Relative working directory data/styles
    let rel_styles = Path::new("data/styles");
    add_search_path_if_exists(manager, rel_styles);

    // 3. User data directory fallback
    let user_styles = fallback_style_dir();
    add_search_path_if_exists(manager, &user_styles);

    // Rescan so GtkSourceView picks up the newly added search paths
    manager.force_rescan();
}

/// Sets the Catppuccin theme (defaults to Mocha) on a GtkSourceBuffer.
pub fn apply_theme(
    buffer: &sourceview5::Buffer,
    scheme_id: &str,
) -> bool {
    let manager = sourceview5::StyleSchemeManager::default();
    init_style_schemes(&manager);

    let scheme = manager
        .scheme(scheme_id)
        .or_else(|| manager.scheme(CATPPUCCIN_MOCHA))
        .or_else(|| manager.scheme("Adwaita-dark"))
        .or_else(|| manager.scheme("adwaita-dark"))
        .or_else(|| manager.scheme("classic"));

    if let Some(scheme) = scheme {
        buffer.set_style_scheme(Some(&scheme));
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_theme_files_valid() {
        assert_eq!(THEME_FILES.len(), 4);

        let flavors = [
            ("catppuccin-mocha.xml", "catppuccin-mocha", "#1e1e2e", "#cdd6f4"),
            ("catppuccin-macchiato.xml", "catppuccin-macchiato", "#24273a", "#cad3f5"),
            ("catppuccin-frappe.xml", "catppuccin-frappe", "#303446", "#c6d0f5"),
            ("catppuccin-latte.xml", "catppuccin-latte", "#eff1f5", "#4c4f69"),
        ];

        for (filename, expected_id, expected_base, expected_text) in flavors {
            let found = THEME_FILES.iter().find(|(name, _)| *name == filename);
            assert!(found.is_some(), "Theme file {} must be embedded", filename);
            let (_, content) = found.unwrap();

            assert!(content.contains(&format!("id=\"{}\"", expected_id)));
            assert!(content.contains(&format!("value=\"{}\"", expected_base)));
            assert!(content.contains(&format!("value=\"{}\"", expected_text)));

            // Verify essential styles are present
            assert!(content.contains("name=\"text\""));
            assert!(content.contains("name=\"selection\""));
            assert!(content.contains("name=\"cursor\""));
            assert!(content.contains("name=\"current-line\""));
            assert!(content.contains("name=\"line-numbers\""));
            assert!(content.contains("name=\"current-line-number\""));
            assert!(content.contains("name=\"bracket-match\""));
            assert!(content.contains("name=\"def:keyword\""));
            assert!(content.contains("name=\"def:string\""));
            assert!(content.contains("name=\"def:function\""));
            assert!(content.contains("name=\"def:type\""));
            assert!(content.contains("name=\"def:comment\""));
        }
    }

    #[test]
    fn test_flavor_properties() {
        assert_eq!(CatppuccinFlavor::Mocha.id(), CATPPUCCIN_MOCHA);
        assert_eq!(CatppuccinFlavor::Macchiato.id(), CATPPUCCIN_MACCHIATO);
        assert_eq!(CatppuccinFlavor::Frappe.id(), CATPPUCCIN_FRAPPE);
        assert_eq!(CatppuccinFlavor::Latte.id(), CATPPUCCIN_LATTE);

        assert!(CatppuccinFlavor::Mocha.is_dark());
        assert!(CatppuccinFlavor::Macchiato.is_dark());
        assert!(CatppuccinFlavor::Frappe.is_dark());
        assert!(!CatppuccinFlavor::Latte.is_dark());
    }

    #[gtk::test]
    fn test_theme_gtk() {
        let manager = sourceview5::StyleSchemeManager::default();
        init_style_schemes(&manager);

        let mocha = manager.scheme(CATPPUCCIN_MOCHA);
        assert!(mocha.is_some(), "catppuccin-mocha scheme must be found");
        let mocha_scheme = mocha.unwrap();
        assert_eq!(mocha_scheme.id(), CATPPUCCIN_MOCHA);

        let macchiato = manager.scheme(CATPPUCCIN_MACCHIATO);
        assert!(macchiato.is_some(), "catppuccin-macchiato scheme must be found");

        let frappe = manager.scheme(CATPPUCCIN_FRAPPE);
        assert!(frappe.is_some(), "catppuccin-frappe scheme must be found");

        let latte = manager.scheme(CATPPUCCIN_LATTE);
        assert!(latte.is_some(), "catppuccin-latte scheme must be found");

        let buffer = sourceview5::Buffer::new(None);
        let ok = apply_theme(&buffer, CATPPUCCIN_MOCHA);
        assert!(ok, "applying catppuccin-mocha should succeed");

        let current = buffer.style_scheme();
        assert!(current.is_some());
        assert_eq!(current.unwrap().id(), CATPPUCCIN_MOCHA);
    }
}
