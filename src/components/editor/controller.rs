#![allow(dead_code)]
use gtk::prelude::TextBufferExt;
use gtk::glib;

use crate::keymap::{KeyEvent, KeymapResult, Mode};

use super::state::EditorState;

/// Result of handling a key in CAPTURE phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyHandleResult {
    /// Event consumed — `GDK_EVENT_STOP`.
    Stop,
    /// Event should propagate — `GDK_EVENT_PROPAGATE`.
    Propagate,
    /// Mode changed — caller must update UI via Relm4.
    ModeChanged(Mode),
}

/// Imperative handler for a single GDK key event in CAPTURE phase.
///
/// `extend` is derived from `state.mode == Mode::Select`.
/// Mutates `buffer` directly via `GtkTextIter` (fast path) and
/// `state` for mode/clipboard/pending trie.
/// Never sends via Relm4 for per-keystroke motions.
pub fn handle_key<B>(state: &mut EditorState, buffer: &B, mut key: KeyEvent) -> KeyHandleResult
where
    B: glib::object::IsA<gtk::TextBuffer>,
{
    crate::keymap::canonicalize_key(&mut key);
    let buffer = buffer.as_ref();

    // Esc: cancel any pending on_next_key, count, pending chords, selected register, and return to Normal mode.
    if key.code == crate::keymap::trie::KeyCode::Esc {
        state.on_next_key = None;
        state.count = None;
        state.selected_register = None;
        if !state.keymap.pending().is_empty() {
            state.keymap.clear_pending();
            return KeyHandleResult::Stop;
        }
        if state.mode == Mode::Insert || state.mode == Mode::Select {
            state.set_mode(Mode::Normal);
            let iter = buffer.iter_at_mark(&buffer.get_insert());
            buffer.place_cursor(&iter);
            return KeyHandleResult::ModeChanged(Mode::Normal);
        }
        return KeyHandleResult::Stop;
    }

    // Dynamic on_next_key callback (Helix style)
    if let Some(cb) = state.on_next_key.take() {
        return cb(state, buffer, key);
    }

    // Insert mode: passthrough everything
    if state.mode == Mode::Insert {
        return KeyHandleResult::Propagate;
    }

    // Numerical counts in Normal / Select mode
    match (key, state.count) {
        (KeyEvent { code: crate::keymap::trie::KeyCode::Char(c @ '0'..='9'), modifiers }, Some(cur_count))
            if modifiers.is_empty() =>
        {
            let digit = c.to_digit(10).unwrap() as usize;
            let new_count = cur_count.get().saturating_mul(10).saturating_add(digit);
            state.count = std::num::NonZeroUsize::new(new_count.min(100_000_000));
            return KeyHandleResult::Stop;
        }
        (KeyEvent { code: crate::keymap::trie::KeyCode::Char(c @ '1'..='9'), modifiers }, None)
            if modifiers.is_empty() && !state.keymap.contains_key(state.mode, key) =>
        {
            let digit = c.to_digit(10).unwrap() as usize;
            state.count = std::num::NonZeroUsize::new(digit);
            return KeyHandleResult::Stop;
        }
        _ => {}
    }

    let result = state.keymap.get(state.mode, key);

    match result {
        KeymapResult::Pending(_) => {
            // Chord pending — consume, keep count active
            KeyHandleResult::Stop
        }
        KeymapResult::Cancelled(_) => {
            // Invalid chord — reset count and selected register
            state.count = None;
            state.selected_register = None;
            KeyHandleResult::Stop
        }
        KeymapResult::NotFound => {
            // In Normal/Select, unknown keys are swallowed to prevent insertion
            state.count = None;
            state.selected_register = None;
            KeyHandleResult::Stop
        }
        KeymapResult::Matched(action) => {
            let count = state.count.take().map_or(1, |c| c.get());
            let register = state.selected_register.take().unwrap_or('"');
            let mut cx = crate::commands::Context::new(state, buffer, count, register);
            crate::commands::dispatch(action, &mut cx)
        }
    }
}

#[cfg(test)]
mod tests {
    fn ensure_gtk() -> bool {
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            let _ = std::panic::catch_unwind(|| gtk::init());
        });
        if !gtk::is_initialized() {
            return false;
        }
        std::panic::catch_unwind(|| {
            let _ = gtk::TextBuffer::new(None);
        })
        .is_ok()
    }
    use super::*;
    use crate::keymap::{trie::KeyCode, KeyModifiers};

    fn buf_with(text: &str) -> gtk::TextBuffer {
        let b = gtk::TextBuffer::new(None);
        b.set_text(text);
        b
    }

    #[test]
    fn insert_passthrough_and_esc() {
        if !ensure_gtk() { return; }
        let mut state = EditorState::new();
        state.set_mode(Mode::Insert);
        let buf = buf_with("hi");
        buf.place_cursor(&buf.iter_at_offset(2));
        // 'a' in insert should propagate
        let res = handle_key(&mut state, &buf, KeyEvent::char('a'));
        assert_eq!(res, KeyHandleResult::Propagate);
        assert_eq!(state.mode, Mode::Insert);
        // Esc should go normal
        let res2 = handle_key(
            &mut state,
            &buf,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        );
        assert_eq!(res2, KeyHandleResult::ModeChanged(Mode::Normal));
        assert_eq!(state.mode, Mode::Normal);
    }

    #[test]
    fn normal_hjkl() {
        if !ensure_gtk() { return; }
        let mut state = EditorState::new();
        let buf = buf_with("hello\nworld");
        buf.place_cursor(&buf.iter_at_offset(0));
        handle_key(&mut state, &buf, KeyEvent::char('l'));
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).offset(), 1);
        handle_key(&mut state, &buf, KeyEvent::char('j'));
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).line(), 1);
        handle_key(&mut state, &buf, KeyEvent::char('h'));
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).offset(), 6);
        handle_key(&mut state, &buf, KeyEvent::char('k'));
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).line(), 0);
    }

    #[test]
    fn normal_i_enters_insert() {
        if !ensure_gtk() { return; }
        let mut state = EditorState::new();
        let buf = buf_with("hi");
        let res = handle_key(&mut state, &buf, KeyEvent::char('i'));
        assert_eq!(res, KeyHandleResult::ModeChanged(Mode::Insert));
        assert_eq!(state.mode, Mode::Insert);
    }

    #[test]
    fn normal_v_enters_select_and_extends() {
        if !ensure_gtk() { return; }
        let mut state = EditorState::new();
        let buf = buf_with("hello world");
        buf.place_cursor(&buf.iter_at_offset(0));
        handle_key(&mut state, &buf, KeyEvent::char('v'));
        assert_eq!(state.mode, Mode::Select);
        handle_key(&mut state, &buf, KeyEvent::char('l'));
        let anchor = buf.iter_at_mark(&buf.selection_bound()).offset();
        let head = buf.iter_at_mark(&buf.get_insert()).offset();
        assert!(head != anchor);
    }

    #[test]
    fn esc_from_select_goes_normal() {
        if !ensure_gtk() { return; }
        let mut state = EditorState::new();
        state.set_mode(Mode::Select);
        let buf = buf_with("hi");
        let res = handle_key(
            &mut state,
            &buf,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        );
        assert_eq!(res, KeyHandleResult::ModeChanged(Mode::Normal));
    }

    #[test]
    fn word_motions_via_trie() {
        if !ensure_gtk() { return; }
        let mut state = EditorState::new();
        let buf = buf_with("hello world foo");
        buf.place_cursor(&buf.iter_at_offset(0));
        handle_key(&mut state, &buf, KeyEvent::char('w'));
        assert!(buf.iter_at_mark(&buf.get_insert()).offset() > 0);
        handle_key(&mut state, &buf, KeyEvent::char('b'));
        // After b from word start, should go back towards 0
        // Accept 0 as success
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).offset(), 0);
        handle_key(&mut state, &buf, KeyEvent::char('e'));
        assert!(buf.iter_at_mark(&buf.get_insert()).offset() > 0);
    }

    #[test]
    fn delete_yank_paste() {
        if !ensure_gtk() { return; }
        let mut state = EditorState::new();
        let buf = buf_with("hello world");
        // select hello
        buf.select_range(&buf.iter_at_offset(0), &buf.iter_at_offset(5));
        // Need to be in Select to test y
        state.set_mode(Mode::Select);
        handle_key(&mut state, &buf, KeyEvent::char('y'));
        assert_eq!(state.registers.read('"'), "hello");
        assert_eq!(state.registers.read('0'), "hello");
        assert_eq!(state.mode, Mode::Normal);
        buf.place_cursor(&buf.iter_at_offset(5));
        handle_key(&mut state, &buf, KeyEvent::char('p'));
        assert!(buf.text(&mut buf.start_iter(), &mut buf.end_iter(), false).contains("hello"));
    }

    #[test]
    fn pending_g_and_space() {
        if !ensure_gtk() { return; }
        let mut state = EditorState::new();
        let buf = buf_with("hi");
        let res = handle_key(&mut state, &buf, KeyEvent::char('g'));
        assert_eq!(res, KeyHandleResult::Stop);
        assert!(!state.keymap.pending().is_empty());
        // Esc cancels
        let res2 = handle_key(
            &mut state,
            &buf,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        );
        assert!(state.keymap.pending().is_empty());
        let _ = res2;
    }

    #[test]
    fn x_selects_line() {
        if !ensure_gtk() { return; }
        let mut state = EditorState::new();
        let buf = buf_with("line1\nline2\nline3");
        buf.place_cursor(&buf.iter_at_line(1).unwrap());
        handle_key(&mut state, &buf, KeyEvent::char('x'));
        let (mut s, mut e) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&mut s, &mut e, false), "line2");
    }

    #[test]
    fn x_extends_to_next_line_on_repeat() {
        if !ensure_gtk() { return; }
        let mut state = EditorState::new();
        let buf = buf_with("line1\nline2\nline3\nline4");
        buf.place_cursor(&buf.iter_at_line(0).unwrap());
        handle_key(&mut state, &buf, KeyEvent::char('x'));
        let (mut s, mut e) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&mut s, &mut e, false), "line1\n");
        handle_key(&mut state, &buf, KeyEvent::char('x'));
        let (mut s2, mut e2) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&mut s2, &mut e2, false), "line1\nline2\n");
        handle_key(&mut state, &buf, KeyEvent::char('x'));
        let (mut s3, mut e3) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&mut s3, &mut e3, false), "line1\nline2\nline3\n");
    }

    #[test]
    fn v_then_l_extends_and_keeps_anchor() {
        if !ensure_gtk() { return; }
        let mut state = EditorState::new();
        let buf = buf_with("hello");
        buf.place_cursor(&buf.iter_at_offset(0));
        handle_key(&mut state, &buf, KeyEvent::char('v'));
        assert_eq!(state.mode, Mode::Select);
        handle_key(&mut state, &buf, KeyEvent::char('l'));
        let anchor1 = buf.iter_at_mark(&buf.selection_bound()).offset();
        let head1 = buf.iter_at_mark(&buf.get_insert()).offset();
        assert_eq!(anchor1, 0);
        assert_eq!(head1, 1);
        handle_key(&mut state, &buf, KeyEvent::char('l'));
        let anchor2 = buf.iter_at_mark(&buf.selection_bound()).offset();
        let head2 = buf.iter_at_mark(&buf.get_insert()).offset();
        assert_eq!(anchor2, 0);
        assert_eq!(head2, 2);
        handle_key(&mut state, &buf, KeyEvent::char('l'));
        let anchor3 = buf.iter_at_mark(&buf.selection_bound()).offset();
        let head3 = buf.iter_at_mark(&buf.get_insert()).offset();
        assert_eq!(anchor3, 0);
        assert_eq!(head3, 3);
    }

    #[test]
    fn w_in_normal_creates_selection_and_wc_deletes_word() {
        if !ensure_gtk() { return; }
        let mut state = EditorState::new();
        let buf = buf_with("hello world foo");
        buf.place_cursor(&buf.iter_at_offset(0));
        // w should select "hello " (0-6)
        handle_key(&mut state, &buf, KeyEvent::char('w'));
        let (mut s, mut e) = buf.selection_bounds().unwrap();
        let sel = buf.text(&mut s, &mut e, false).to_string();
        assert!(sel.contains("hello"), "w should select hello, got {:?}", sel);
        // c should delete selection and enter insert
        let res = handle_key(&mut state, &buf, KeyEvent::char('c'));
        assert_eq!(res, KeyHandleResult::ModeChanged(Mode::Insert));
        assert_eq!(state.mode, Mode::Insert);
        let txt = buf.text(&mut buf.start_iter(), &mut buf.end_iter(), false).to_string();
        assert!(!txt.starts_with("hello"), "word should be deleted, got {:?}", txt);
    }

    #[test]
    fn w_in_select_extends_selection() {
        if !ensure_gtk() { return; }
        let mut state = EditorState::new();
        let buf = buf_with("hello world foo bar");
        buf.place_cursor(&buf.iter_at_offset(0));
        handle_key(&mut state, &buf, KeyEvent::char('v'));
        handle_key(&mut state, &buf, KeyEvent::char('w'));
        let (mut s1, mut e1) = buf.selection_bounds().unwrap();
        let sel1 = buf.text(&mut s1, &mut e1, false).to_string();
        assert!(sel1.contains("hello"), "v+w should select hello, got {:?}", sel1);
        handle_key(&mut state, &buf, KeyEvent::char('w'));
        let (mut s2, mut e2) = buf.selection_bounds().unwrap();
        let sel2 = buf.text(&mut s2, &mut e2, false).to_string();
        assert!(sel2.contains("world"), "second w should extend to world, got {:?}", sel2);
        assert!(sel2.len() > sel1.len());
    }

    #[test]
    fn w_across_newline_does_not_select_newline() {
        if !ensure_gtk() { return; }
        let mut state = EditorState::new();
        let buf = buf_with("hello\nworld\nfoo");
        buf.place_cursor(&buf.iter_at_offset(0));
        handle_key(&mut state, &buf, KeyEvent::char('w'));
        if let Some((mut s, mut e)) = buf.selection_bounds() {
            let sel = buf.text(&mut s, &mut e, false).to_string();
            assert!(!sel.contains('\n'), "w should not select newline, got {:?}", sel);
            assert!(sel.contains("hello"), "got {:?}", sel);
        } else {
            panic!("no selection after w");
        }
        buf.place_cursor(&buf.iter_at_offset(6));
        state.set_mode(Mode::Normal);
        handle_key(&mut state, &buf, KeyEvent::char('b'));
        if let Some((mut s2, mut e2)) = buf.selection_bounds() {
            let sel2 = buf.text(&mut s2, &mut e2, false).to_string();
            assert!(!sel2.contains('\n'), "b should not select newline, got {:?}", sel2);
        } else {
            panic!("no selection after b");
        }
    }

    #[test]
    fn k_from_fn_main_goes_up_not_left() {
        if !ensure_gtk() { return; }
        let text = "// Strand — Phase 1: AdwApplicationWindow + GtkSourceView\n// Verify: syntax highlighting, line numbers, kinetic scroll\n\nfn main() {\n    println!(\"Hello, Strand!\");";
        let buf = buf_with(text);
        let offset = text.find("fn main").unwrap();
        let char_offset: usize = text[..offset].chars().count();
        let pos = char_offset + 3;
        buf.place_cursor(&buf.iter_at_offset(pos as i32));
        let mut state = EditorState::new();
        let before_line = buf.iter_at_mark(&buf.get_insert()).line();
        handle_key(&mut state, &buf, KeyEvent::char('k'));
        let after_line = buf.iter_at_mark(&buf.get_insert()).line();
        assert_eq!(after_line, before_line - 1, "k should go up one line");
        buf.place_cursor(&buf.iter_at_offset(pos as i32));
        let mut state2 = EditorState::new();
        handle_key(&mut state2, &buf, KeyEvent { code: crate::keymap::trie::KeyCode::Up, modifiers: crate::keymap::trie::KeyModifiers::empty() });
        let after_up = buf.iter_at_mark(&buf.get_insert()).line();
        assert_eq!(after_up, before_line - 1, "Up should go up");
    }

    #[test]
    fn normal_i_and_a_insert_at_line_start_and_end() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("    let x = 10;");
        buf.place_cursor(&buf.iter_at_offset(8));

        // 'I' enters insert mode at first non-whitespace
        let res = handle_key(&mut state, &buf, KeyEvent::char('I'));
        assert_eq!(res, KeyHandleResult::ModeChanged(Mode::Insert));
        assert_eq!(state.mode, Mode::Insert);
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).offset(), 4);

        // Esc back to normal
        handle_key(
            &mut state,
            &buf,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        );
        assert_eq!(state.mode, Mode::Normal);

        // 'A' enters insert mode at end of line
        let res2 = handle_key(&mut state, &buf, KeyEvent::char('A'));
        assert_eq!(res2, KeyHandleResult::ModeChanged(Mode::Insert));
        assert_eq!(state.mode, Mode::Insert);
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).offset(), 15);
    }

    #[test]
    fn select_mode_i_and_a() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("    fn main() {}");
        state.set_mode(Mode::Select);
        buf.select_range(&buf.iter_at_offset(4), &buf.iter_at_offset(8));

        let res = handle_key(&mut state, &buf, KeyEvent::char('I'));
        assert_eq!(res, KeyHandleResult::ModeChanged(Mode::Insert));
        assert_eq!(state.mode, Mode::Insert);
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).offset(), 4);

        // Esc to normal, enter select again
        handle_key(
            &mut state,
            &buf,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        );
        state.set_mode(Mode::Select);
        buf.select_range(&buf.iter_at_offset(4), &buf.iter_at_offset(8));

        let res2 = handle_key(&mut state, &buf, KeyEvent::char('A'));
        assert_eq!(res2, KeyHandleResult::ModeChanged(Mode::Insert));
        assert_eq!(state.mode, Mode::Insert);
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).offset(), 16);
    }

    #[test]
    fn undo_and_redo_keys() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("initial");
        buf.set_enable_undo(true);
        buf.place_cursor(&buf.iter_at_offset(7));
        buf.insert_at_cursor(" text");
        assert_eq!(
            buf.text(&buf.start_iter(), &buf.end_iter(), false),
            "initial text"
        );

        // 'u' undos
        let res = handle_key(&mut state, &buf, KeyEvent::char('u'));
        assert_eq!(res, KeyHandleResult::Stop);
        assert_eq!(
            buf.text(&buf.start_iter(), &buf.end_iter(), false),
            "initial"
        );

        // 'U' redos
        let res2 = handle_key(&mut state, &buf, KeyEvent::char('U'));
        assert_eq!(res2, KeyHandleResult::Stop);
        assert_eq!(
            buf.text(&buf.start_iter(), &buf.end_iter(), false),
            "initial text"
        );
    }

    #[test]
    fn match_mode_mm_jump() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("fn foo() { let x = 1; }");
        buf.place_cursor(&buf.iter_at_offset(9)); // on '{'
        handle_key(&mut state, &buf, KeyEvent::char('m'));
        let res = handle_key(&mut state, &buf, KeyEvent::char('m'));
        assert_eq!(res, KeyHandleResult::Stop);
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).offset(), 23); // on '}'
    }

    #[test]
    fn match_mode_surround_add_in_select() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("hello world");
        state.set_mode(Mode::Select);
        buf.select_range(&buf.iter_at_offset(0), &buf.iter_at_offset(5)); // select "hello"

        handle_key(&mut state, &buf, KeyEvent::char('m'));
        handle_key(&mut state, &buf, KeyEvent::char('s'));
        let res = handle_key(&mut state, &buf, KeyEvent::char('('));
        assert_eq!(res, KeyHandleResult::ModeChanged(Mode::Normal));
        assert_eq!(state.mode, Mode::Normal);
        let full = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
        assert_eq!(full, "(hello) world");
    }

    #[test]
    fn match_mode_surround_delete_and_replace() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("let a = (foo);");
        buf.place_cursor(&buf.iter_at_offset(10)); // inside "(foo)"

        // Replace '(' with '['
        handle_key(&mut state, &buf, KeyEvent::char('m'));
        handle_key(&mut state, &buf, KeyEvent::char('r'));
        handle_key(&mut state, &buf, KeyEvent::char('('));
        let res = handle_key(&mut state, &buf, KeyEvent::char('['));
        assert_eq!(res, KeyHandleResult::Stop);
        let text1 = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
        assert_eq!(text1, "let a = [foo];");

        // Delete '['
        handle_key(&mut state, &buf, KeyEvent::char('m'));
        handle_key(&mut state, &buf, KeyEvent::char('d'));
        let res2 = handle_key(&mut state, &buf, KeyEvent::char('['));
        assert_eq!(res2, KeyHandleResult::Stop);
        let text2 = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
        assert_eq!(text2, "let a = foo;");
    }

    #[test]
    fn match_mode_textobjects_inner_and_around() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("fn bar() { hello_world }");
        buf.place_cursor(&buf.iter_at_offset(15)); // inside '{ ... }'

        // mi{ selects inside '{' and '}'
        handle_key(&mut state, &buf, KeyEvent::char('m'));
        handle_key(&mut state, &buf, KeyEvent::char('i'));
        let res = handle_key(&mut state, &buf, KeyEvent::char('{'));
        assert_eq!(res, KeyHandleResult::Stop);
        let (s, e) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&s, &e, false).as_str(), " hello_world ");

        // ma{ selects around '{' and '}'
        handle_key(&mut state, &buf, KeyEvent::char('m'));
        handle_key(&mut state, &buf, KeyEvent::char('a'));
        let res2 = handle_key(&mut state, &buf, KeyEvent::char('{'));
        assert_eq!(res2, KeyHandleResult::Stop);
        let (s2, e2) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&s2, &e2, false).as_str(), "{ hello_world }");
    }

    #[test]
    fn numerical_count_hjkl_motions() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("line0\nline1\nline2\nline3\nline4\nline5\nline6\n");
        buf.place_cursor(&buf.iter_at_offset(0));

        // 5j -> down 5 lines (to line5)
        handle_key(&mut state, &buf, KeyEvent::char('5'));
        assert_eq!(state.count.map(|c| c.get()), Some(5));
        let res = handle_key(&mut state, &buf, KeyEvent::char('j'));
        assert_eq!(res, KeyHandleResult::Stop);
        assert_eq!(state.count, None); // reset after execution
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).line(), 5);

        // 3k -> up 3 lines (to line2)
        handle_key(&mut state, &buf, KeyEvent::char('3'));
        handle_key(&mut state, &buf, KeyEvent::char('k'));
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).line(), 2);

        // 4l -> right 4 chars
        handle_key(&mut state, &buf, KeyEvent::char('4'));
        handle_key(&mut state, &buf, KeyEvent::char('l'));
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).line_offset(), 4);

        // 2h -> left 2 chars
        handle_key(&mut state, &buf, KeyEvent::char('2'));
        handle_key(&mut state, &buf, KeyEvent::char('h'));
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).line_offset(), 2);
    }

    #[test]
    fn numerical_count_multi_digit_and_reset() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("abcdefghijklmnopqrstuvwxyz");
        buf.place_cursor(&buf.iter_at_offset(0));

        // 1, 2, l -> moves right 12 chars
        handle_key(&mut state, &buf, KeyEvent::char('1'));
        assert_eq!(state.count.map(|c| c.get()), Some(1));
        handle_key(&mut state, &buf, KeyEvent::char('2'));
        assert_eq!(state.count.map(|c| c.get()), Some(12));

        let res = handle_key(&mut state, &buf, KeyEvent::char('l'));
        assert_eq!(res, KeyHandleResult::Stop);
        assert_eq!(state.count, None);
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).offset(), 12);

        // Subsequent 'l' without count moves only 1 character
        handle_key(&mut state, &buf, KeyEvent::char('l'));
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).offset(), 13);
    }

    #[test]
    fn numerical_count_cancelled_by_esc() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("line0\nline1\nline2\nline3\nline4\nline5\n");
        buf.place_cursor(&buf.iter_at_offset(0));

        // 5, then Esc -> cancels count
        handle_key(&mut state, &buf, KeyEvent::char('5'));
        assert_eq!(state.count.map(|c| c.get()), Some(5));

        handle_key(&mut state, &buf, KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
        assert_eq!(state.count, None);

        // Next j moves only 1 line, not 5
        handle_key(&mut state, &buf, KeyEvent::char('j'));
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).line(), 1);
    }

    #[test]
    fn numerical_count_word_and_line_selection() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("apple banana cherry date elderberry\nsecond line\nthird line\n");
        buf.place_cursor(&buf.iter_at_offset(0));

        // 3w -> advance 3 words (to 'date')
        handle_key(&mut state, &buf, KeyEvent::char('3'));
        handle_key(&mut state, &buf, KeyEvent::char('w'));
        let (s, e) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&s, &e, false).as_str(), "date");

        // 2x -> select 2 lines
        buf.place_cursor(&buf.iter_at_offset(0));
        handle_key(&mut state, &buf, KeyEvent::char('2'));
        handle_key(&mut state, &buf, KeyEvent::char('x'));
        let (s, e) = buf.selection_bounds().unwrap();
        assert_eq!(
            buf.text(&s, &e, false).as_str(),
            "apple banana cherry date elderberry\nsecond line\n"
        );
    }

    #[test]
    fn dynamic_on_next_key_custom_chars() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("target text");
        state.set_mode(Mode::Select);
        buf.select_range(&buf.iter_at_offset(0), &buf.iter_at_offset(6)); // select "target"

        // ms* -> surround with '*'
        handle_key(&mut state, &buf, KeyEvent::char('m'));
        handle_key(&mut state, &buf, KeyEvent::char('s'));
        let res = handle_key(&mut state, &buf, KeyEvent::char('*'));
        assert_eq!(res, KeyHandleResult::ModeChanged(Mode::Normal));
        assert_eq!(
            buf.text(&buf.start_iter(), &buf.end_iter(), false).as_str(),
            "*target* text"
        );

        // mr*# -> replace '*' with '#'
        buf.place_cursor(&buf.iter_at_offset(4)); // inside *target*
        handle_key(&mut state, &buf, KeyEvent::char('m'));
        handle_key(&mut state, &buf, KeyEvent::char('r'));
        handle_key(&mut state, &buf, KeyEvent::char('*'));
        handle_key(&mut state, &buf, KeyEvent::char('#'));
        assert_eq!(
            buf.text(&buf.start_iter(), &buf.end_iter(), false).as_str(),
            "#target# text"
        );

        // md# -> delete '#'
        handle_key(&mut state, &buf, KeyEvent::char('m'));
        handle_key(&mut state, &buf, KeyEvent::char('d'));
        handle_key(&mut state, &buf, KeyEvent::char('#'));
        assert_eq!(
            buf.text(&buf.start_iter(), &buf.end_iter(), false).as_str(),
            "target text"
        );
    }

    #[test]
    fn named_register_yank_and_paste() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("foo bar baz");

        // Select "foo" (0..3)
        state.set_mode(Mode::Select);
        buf.select_range(&buf.iter_at_offset(0), &buf.iter_at_offset(3));

        // " a y -> yank into register 'a'
        handle_key(&mut state, &buf, KeyEvent::char('"'));
        assert!(state.on_next_key.is_some());
        handle_key(&mut state, &buf, KeyEvent::char('a'));
        assert_eq!(state.selected_register, Some('a'));

        handle_key(&mut state, &buf, KeyEvent::char('y'));
        assert_eq!(state.selected_register, None); // consumed
        assert_eq!(state.registers.read('a'), "foo");
        assert_eq!(state.registers.read('"'), ""); // default untouched
        assert_eq!(state.registers.read('0'), ""); // yank register 0 untouched

        // Move to end (offset 11) and paste from 'a' via: " a p
        buf.place_cursor(&buf.iter_at_offset(11));
        handle_key(&mut state, &buf, KeyEvent::char('"'));
        handle_key(&mut state, &buf, KeyEvent::char('a'));
        handle_key(&mut state, &buf, KeyEvent::char('p'));

        assert_eq!(
            buf.text(&buf.start_iter(), &buf.end_iter(), false).as_str(),
            "foo bar bazfoo"
        );
    }

    #[test]
    fn black_hole_register_delete() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("keep_me delete_me");

        // First yank "keep_me" into default register
        state.set_mode(Mode::Select);
        buf.select_range(&buf.iter_at_offset(0), &buf.iter_at_offset(7));
        handle_key(&mut state, &buf, KeyEvent::char('y'));
        assert_eq!(state.registers.read('"'), "keep_me");

        // Now select "delete_me" (8..17)
        state.set_mode(Mode::Select);
        buf.select_range(&buf.iter_at_offset(8), &buf.iter_at_offset(17));

        // " _ d -> delete into black hole
        handle_key(&mut state, &buf, KeyEvent::char('"'));
        handle_key(&mut state, &buf, KeyEvent::char('_'));
        handle_key(&mut state, &buf, KeyEvent::char('d'));

        // Buffer content is deleted
        assert_eq!(
            buf.text(&buf.start_iter(), &buf.end_iter(), false).as_str(),
            "keep_me "
        );
        // But default register STILL holds "keep_me"!
        assert_eq!(state.registers.read('"'), "keep_me");
    }

    #[test]
    fn yank_register_0_persists_across_deletes() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("alpha beta gamma");

        // Yank "alpha" into default
        state.set_mode(Mode::Select);
        buf.select_range(&buf.iter_at_offset(0), &buf.iter_at_offset(5));
        handle_key(&mut state, &buf, KeyEvent::char('y'));
        assert_eq!(state.registers.read('"'), "alpha");
        assert_eq!(state.registers.read('0'), "alpha");

        // Delete "beta" into default
        state.set_mode(Mode::Select);
        buf.select_range(&buf.iter_at_offset(6), &buf.iter_at_offset(10));
        handle_key(&mut state, &buf, KeyEvent::char('d'));

        // Default register is now "beta", but yank register '0' is still "alpha"!
        assert_eq!(state.registers.read('"'), "beta");
        assert_eq!(state.registers.read('0'), "alpha");

        // " 0 p pastes "alpha", not "beta"
        buf.place_cursor(&buf.iter_at_offset(buf.end_iter().offset()));
        handle_key(&mut state, &buf, KeyEvent::char('"'));
        handle_key(&mut state, &buf, KeyEvent::char('0'));
        handle_key(&mut state, &buf, KeyEvent::char('p'));

        assert!(buf.text(&buf.start_iter(), &buf.end_iter(), false).ends_with("alpha"));
    }

    #[test]
    fn shifted_register_yank_and_paste_in_normal_mode() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("hello world");
        assert_eq!(state.mode, Mode::Normal);

        // Place cursor on 'h' at offset 0
        buf.place_cursor(&buf.iter_at_offset(0));

        // Simulate user pressing Shift+' (producing GDK quotedbl with SHIFT modifier)
        let key_quote = crate::keymap::gdk_to_key_event('"' as u32, 1).unwrap();
        let res_quote = handle_key(&mut state, &buf, key_quote);
        assert_eq!(res_quote, KeyHandleResult::Stop);
        assert_eq!(state.mode, Mode::Normal);
        assert!(state.on_next_key.is_some());

        // Press 'a' to select register 'a'
        let res_a = handle_key(&mut state, &buf, KeyEvent::char('a'));
        assert_eq!(res_a, KeyHandleResult::Stop);
        assert_eq!(state.mode, Mode::Normal);
        assert_eq!(state.selected_register, Some('a'));

        // Press 'y' to yank character under cursor ('h') into register 'a'
        let res_y = handle_key(&mut state, &buf, KeyEvent::char('y'));
        assert_eq!(res_y, KeyHandleResult::Stop);
        assert_eq!(state.mode, Mode::Normal);
        assert_eq!(state.selected_register, None); // consumed
        assert_eq!(state.registers.read('a'), "h");
        // Ensure buffer was not modified (e.g. no 'y' inserted)
        assert_eq!(
            buf.text(&buf.start_iter(), &buf.end_iter(), false).as_str(),
            "hello world"
        );

        // Now move cursor to end and paste with " a p (with shifted quote again)
        buf.place_cursor(&buf.iter_at_offset(11));
        let res_paste_quote = handle_key(&mut state, &buf, key_quote);
        assert_eq!(res_paste_quote, KeyHandleResult::Stop);
        let res_paste_a = handle_key(&mut state, &buf, KeyEvent::char('a'));
        assert_eq!(res_paste_a, KeyHandleResult::Stop);
        let res_paste_p = handle_key(&mut state, &buf, KeyEvent::char('p'));
        assert_eq!(res_paste_p, KeyHandleResult::Stop);

        assert_eq!(
            buf.text(&buf.start_iter(), &buf.end_iter(), false).as_str(),
            "hello worldh"
        );
    }

    #[test]
    fn numbered_register_1_yank_and_paste_vs_default_register() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("foo bar");

        // 1. " 1 y on "foo" (0..3) using GDK shifted quote
        let key_quote = crate::keymap::gdk_to_key_event('"' as u32, 1).unwrap();
        state.set_mode(Mode::Select);
        buf.select_range(&buf.iter_at_offset(0), &buf.iter_at_offset(3));

        handle_key(&mut state, &buf, key_quote);
        assert!(state.on_next_key.is_some());
        handle_key(&mut state, &buf, KeyEvent::char('1'));
        assert_eq!(state.selected_register, Some('1'));
        handle_key(&mut state, &buf, KeyEvent::char('y'));
        assert_eq!(state.registers.read('1'), "foo");
        assert_eq!(state.registers.read('"'), ""); // default register untouched!

        // 2. Select "bar" (4..7) and simply yank with normal 'y'
        state.set_mode(Mode::Select);
        buf.select_range(&buf.iter_at_offset(4), &buf.iter_at_offset(7));
        handle_key(&mut state, &buf, KeyEvent::char('y'));
        assert_eq!(state.registers.read('"'), "bar");
        // Register '1' still has "foo"
        assert_eq!(state.registers.read('1'), "foo");

        // 3. Normal 'p' pastes from default register ("bar")
        buf.place_cursor(&buf.iter_at_offset(buf.end_iter().offset()));
        handle_key(&mut state, &buf, KeyEvent::char('p'));
        assert!(buf.text(&buf.start_iter(), &buf.end_iter(), false).ends_with("bar"));

        // 4. " 1 p pastes from register '1' ("foo"), NOT default register ("bar")!
        buf.place_cursor(&buf.iter_at_offset(buf.end_iter().offset()));
        handle_key(&mut state, &buf, key_quote);
        handle_key(&mut state, &buf, KeyEvent::char('1'));
        handle_key(&mut state, &buf, KeyEvent::char('p'));
        assert!(buf.text(&buf.start_iter(), &buf.end_iter(), false).ends_with("foo"));
    }

    #[test]
    fn space_y_and_space_p_clipboard() {
        if !ensure_gtk() {
            return;
        }
        let mut state = EditorState::new();
        let buf = buf_with("helix editor");

        // Select "helix" (0..5)
        state.set_mode(Mode::Select);
        buf.select_range(&buf.iter_at_offset(0), &buf.iter_at_offset(5));

        // Space y -> yank to system clipboard '+'
        handle_key(&mut state, &buf, KeyEvent::char(' '));
        let res = handle_key(&mut state, &buf, KeyEvent::char('y'));
        assert_eq!(res, KeyHandleResult::ModeChanged(Mode::Normal));
        assert_eq!(state.mode, Mode::Normal);
        assert_eq!(state.registers.read('+'), "helix");

        // Move to end and Space p -> paste system clipboard
        buf.place_cursor(&buf.iter_at_offset(12));
        handle_key(&mut state, &buf, KeyEvent::char(' '));
        handle_key(&mut state, &buf, KeyEvent::char('p'));

        assert_eq!(
            buf.text(&buf.start_iter(), &buf.end_iter(), false).as_str(),
            "helix editorhelix"
        );
    }
}



