pub mod controller;
pub mod motions;
pub mod state;
pub mod theme;

#[allow(unused_imports)]
pub use controller::{handle_key, KeyHandleResult};
#[allow(unused_imports)]
pub use state::EditorState;
#[allow(unused_imports)]
pub use theme::{apply_theme, CATPPUCCIN_FRAPPE, CATPPUCCIN_LATTE, CATPPUCCIN_MACCHIATO, CATPPUCCIN_MOCHA};
