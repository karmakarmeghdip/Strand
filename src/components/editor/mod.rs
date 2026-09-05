pub mod controller;
pub mod motions;
pub mod state;

#[allow(unused_imports)]
pub use controller::{handle_key, KeyHandleResult};
#[allow(unused_imports)]
pub use state::EditorState;
