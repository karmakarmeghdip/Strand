//! Core text primitives, movements, and selection algorithms.
//!
//! Mirrors `helix-core`.
//!
//! Non-negotiable invariant: `GtkSourceBuffer` (`GtkTextBuffer`) is the single
//! source of truth. All algorithms operate via `GtkTextIter` or gap-index `Range`.

pub mod chars;
pub mod match_brackets;
pub mod movement;
pub mod selection;
pub mod surround;
pub mod textobject;

pub use chars::{categorize, char_is_line_ending, is_long_word_boundary, is_word_boundary, CharCategory};
pub use match_brackets::{
    find_closest_enclosing_pair, find_enclosing_bracket_pair, find_enclosing_quote_pair,
    find_matching_close_bracket, find_matching_open_bracket, find_matching_quote, get_pair,
    is_close_bracket, is_close_pair, is_open_bracket, is_open_pair, is_valid_bracket, is_valid_pair,
    match_brackets, BRACKETS, PAIRS,
};
pub use movement::{
    collapse_selection, delete_selection, flip_selection, goto_file_end, goto_file_start,
    goto_first_nonwhitespace, goto_line, goto_line_end, goto_line_start, insert_at_line_end,
    insert_at_line_start, is_linewise, move_horizontally, move_long_word_forward, move_vertically,
    move_word_backward, move_word_end, move_word_forward, open_above, open_below, paste_after,
    paste_before, redo, replace_char, select_all, select_line, toggle_case, undo, word_move,
    yank_selection, BufferChars, WordMotionTarget,
};
pub use selection::{
    apply_range, current_range, iter_at_offset, offset_at_insert, offset_at_selection_bound,
    set_cursor, Direction, Range,
};
pub use surround::{surround_add, surround_delete, surround_replace};
pub use textobject::{select_textobject, word_bounds_at};
