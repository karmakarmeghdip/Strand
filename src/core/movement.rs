//! Movement operations and imperative buffer actions.
//!
//! Mirrors `helix-core/src/movement.rs`.

use gtk::prelude::{IsA, TextBufferExt};
use gtk::TextBuffer;

use super::chars::{char_is_line_ending, is_word_boundary};
use super::selection::{offset_at_insert, set_cursor, Range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordMotionTarget {
    NextWordStart,
    NextWordEnd,
    PrevWordStart,
    PrevWordEnd,
}

pub fn reached_target(target: WordMotionTarget, prev_ch: char, next_ch: char) -> bool {
    match target {
        WordMotionTarget::NextWordStart | WordMotionTarget::PrevWordEnd => {
            is_word_boundary(prev_ch, next_ch)
                && (char_is_line_ending(next_ch) || !next_ch.is_whitespace())
        }
        WordMotionTarget::NextWordEnd | WordMotionTarget::PrevWordStart => {
            is_word_boundary(prev_ch, next_ch)
                && (!prev_ch.is_whitespace() || char_is_line_ending(next_ch))
        }
    }
}

pub struct BufferChars {
    iter: gtk::TextIter,
    reversed: bool,
}

impl BufferChars {
    pub fn new(buffer: &gtk::TextBuffer, pos: usize) -> Self {
        let iter = buffer.iter_at_offset(pos as i32);
        Self {
            iter,
            reversed: false,
        }
    }

    pub fn reverse(&mut self) {
        self.reversed = !self.reversed;
    }

    pub fn prev(&mut self) -> Option<char> {
        if self.reversed {
            if self.iter.is_end() {
                None
            } else {
                let ch = self.iter.char();
                self.iter.forward_char();
                Some(ch)
            }
        } else {
            if self.iter.is_start() {
                None
            } else {
                self.iter.backward_char();
                Some(self.iter.char())
            }
        }
    }
}

impl Iterator for BufferChars {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        if self.reversed {
            if self.iter.is_start() {
                None
            } else {
                self.iter.backward_char();
                Some(self.iter.char())
            }
        } else {
            if self.iter.is_end() {
                None
            } else {
                let ch = self.iter.char();
                self.iter.forward_char();
                Some(ch)
            }
        }
    }
}

pub fn range_to_target(
    buffer: &gtk::TextBuffer,
    origin: Range,
    target: WordMotionTarget,
    is_prev: bool,
) -> Range {
    let mut my_chars = BufferChars::new(buffer, origin.head);
    if is_prev {
        my_chars.reverse();
    }
    let mut anchor = origin.anchor;
    let mut head = origin.head;
    let mut prev_ch = {
        let ch = my_chars.prev();
        if ch.is_some() {
            my_chars.next();
        }
        ch
    };
    while let Some(ch) = my_chars.next() {
        if char_is_line_ending(ch) {
            prev_ch = Some(ch);
            if is_prev {
                head = head.saturating_sub(1);
            } else {
                head += 1;
            }
        } else {
            my_chars.prev();
            break;
        }
    }
    if prev_ch.map(char_is_line_ending).unwrap_or(false) {
        anchor = head;
    }
    let head_start = head;
    for next_ch in my_chars {
        if prev_ch.is_none() || reached_target(target, prev_ch.unwrap(), next_ch) {
            if head == head_start {
                anchor = head;
            } else {
                break;
            }
        }
        prev_ch = Some(next_ch);
        if is_prev {
            head = head.saturating_sub(1);
        } else {
            head += 1;
        }
    }
    Range::new(anchor, head)
}

pub fn word_move(buffer: &gtk::TextBuffer, range: Range, target: WordMotionTarget) -> Range {
    let is_prev = matches!(
        target,
        WordMotionTarget::PrevWordStart | WordMotionTarget::PrevWordEnd
    );
    let total_len = buffer.char_count() as usize;
    if (is_prev && range.head == 0) || (!is_prev && range.head == total_len) {
        return range;
    }
    let start_range = if is_prev {
        if range.anchor < range.head {
            Range::new(range.head, range.head.saturating_sub(1))
        } else {
            Range::new((range.head + 1).min(total_len), range.head)
        }
    } else {
        if range.anchor < range.head {
            let prev = if range.head > 0 { range.head - 1 } else { 0 };
            Range::new(prev, range.head)
        } else {
            Range::new(range.head, (range.head + 1).min(total_len))
        }
    };
    let cur = start_range;
    let next = range_to_target(buffer, cur, target, is_prev);
    if cur == next {
        cur
    } else {
        next
    }
}

pub fn move_horizontally(buffer: &impl IsA<TextBuffer>, dir: i32, extend: bool) {
    let buffer = buffer.as_ref();
    let mut iter = buffer.iter_at_mark(&buffer.get_insert());
    let steps = dir.abs();
    let mut moved = false;
    for _ in 0..steps {
        let step = if dir < 0 {
            iter.backward_cursor_position()
        } else {
            iter.forward_cursor_position()
        };
        if step {
            moved = true;
        } else {
            break;
        }
    }
    if moved {
        set_cursor(buffer, &iter, extend);
    } else if !extend {
        set_cursor(buffer, &iter, false);
    }
}

pub fn move_vertically(buffer: &impl IsA<TextBuffer>, dir: i32, extend: bool) {
    let buffer = buffer.as_ref();
    let cur = buffer.iter_at_mark(&buffer.get_insert());
    let line = cur.line();
    let line_offset = cur.line_offset();

    let target_line = if dir < 0 {
        let steps = -dir;
        if line < steps {
            0
        } else {
            line - steps
        }
    } else {
        let steps = dir;
        let lc = buffer.line_count();
        if line + steps >= lc {
            (lc - 1).max(0)
        } else {
            line + steps
        }
    };

    let mut target = buffer.iter_at_line(target_line).expect("line exists");
    let mut line_end = target;
    if !line_end.ends_line() {
        line_end.forward_to_line_end();
    }
    let target_end_offset = line_end.offset();
    for _ in 0..line_offset {
        if target.offset() >= target_end_offset {
            break;
        }
        if !target.forward_cursor_position() {
            break;
        }
        if target.offset() > target_end_offset {
            target = line_end;
            break;
        }
    }
    set_cursor(buffer, &target, extend);
}

pub fn move_word_forward(buffer: &impl IsA<TextBuffer>, extend: bool) {
    let buffer = buffer.as_ref();
    let total_chars = buffer.char_count() as usize;
    let pos = offset_at_insert(buffer) as usize;
    if total_chars == 0 || pos >= total_chars {
        return;
    }
    let cur_range = {
        let anchor = buffer.iter_at_mark(&buffer.selection_bound()).offset() as usize;
        Range::new(anchor, pos)
    };
    let new_range = if extend {
        let word = word_move(buffer, Range::new(pos, pos), WordMotionTarget::NextWordStart);
        Range::new(cur_range.anchor, word.head)
    } else {
        word_move(buffer, cur_range, WordMotionTarget::NextWordStart)
    };
    let s = new_range.anchor.min(new_range.head);
    let e = new_range.anchor.max(new_range.head);
    let mut start_iter = buffer.iter_at_offset(s as i32);
    let mut end_iter = buffer.iter_at_offset(e as i32);
    while start_iter.offset() < end_iter.offset() {
        let c = start_iter.char();
        if c == '\n' || c == '\r' {
            start_iter.forward_char();
        } else {
            break;
        }
    }
    while end_iter.offset() > start_iter.offset() {
        let mut prev = end_iter;
        prev.backward_char();
        let c = prev.char();
        if c == '\n' || c == '\r' {
            end_iter = prev;
        } else {
            break;
        }
    }
    if start_iter.offset() >= end_iter.offset() {
        return;
    }
    buffer.select_range(&end_iter, &start_iter);
}

pub fn move_word_backward(buffer: &impl IsA<TextBuffer>, extend: bool) {
    let buffer = buffer.as_ref();
    let total_chars = buffer.char_count() as usize;
    let pos = offset_at_insert(buffer) as usize;
    if total_chars == 0 || pos == 0 {
        return;
    }
    let cur_range = {
        let anchor = buffer.iter_at_mark(&buffer.selection_bound()).offset() as usize;
        Range::new(anchor, pos)
    };
    let new_range = if extend {
        let word = word_move(buffer, Range::new(pos, pos), WordMotionTarget::PrevWordStart);
        Range::new(cur_range.anchor, word.head)
    } else {
        word_move(buffer, cur_range, WordMotionTarget::PrevWordStart)
    };
    let s = new_range.anchor.min(new_range.head);
    let e = new_range.anchor.max(new_range.head);
    let mut start_iter = buffer.iter_at_offset(s as i32);
    let mut end_iter = buffer.iter_at_offset(e as i32);
    while start_iter.offset() < end_iter.offset() {
        let c = start_iter.char();
        if c == '\n' || c == '\r' {
            start_iter.forward_char();
        } else {
            break;
        }
    }
    while end_iter.offset() > start_iter.offset() {
        let mut prev = end_iter;
        prev.backward_char();
        let c = prev.char();
        if c == '\n' || c == '\r' {
            end_iter = prev;
        } else {
            break;
        }
    }
    if start_iter.offset() >= end_iter.offset() {
        return;
    }
    if new_range.anchor > new_range.head {
        buffer.select_range(&start_iter, &end_iter);
    } else {
        buffer.select_range(&end_iter, &start_iter);
    }
}

pub fn move_word_end(buffer: &impl IsA<TextBuffer>, extend: bool) {
    let buffer = buffer.as_ref();
    let total_chars = buffer.char_count() as usize;
    let pos = offset_at_insert(buffer) as usize;
    if total_chars == 0 || pos >= total_chars {
        return;
    }
    let cur_range = {
        let anchor = buffer.iter_at_mark(&buffer.selection_bound()).offset() as usize;
        Range::new(anchor, pos)
    };
    let new_range = if extend {
        let word = word_move(buffer, Range::new(pos, pos), WordMotionTarget::NextWordEnd);
        Range::new(cur_range.anchor, word.head)
    } else {
        word_move(buffer, cur_range, WordMotionTarget::NextWordEnd)
    };
    let s = new_range.anchor.min(new_range.head);
    let e = new_range.anchor.max(new_range.head);
    let mut start_iter = buffer.iter_at_offset(s as i32);
    let mut end_iter = buffer.iter_at_offset(e as i32);
    while start_iter.offset() < end_iter.offset() {
        let c = start_iter.char();
        if c == '\n' || c == '\r' {
            start_iter.forward_char();
        } else {
            break;
        }
    }
    while end_iter.offset() > start_iter.offset() {
        let mut prev = end_iter;
        prev.backward_char();
        let c = prev.char();
        if c == '\n' || c == '\r' {
            end_iter = prev;
        } else {
            break;
        }
    }
    if start_iter.offset() >= end_iter.offset() {
        return;
    }
    buffer.select_range(&end_iter, &start_iter);
}

pub fn move_long_word_forward(buffer: &impl IsA<TextBuffer>, extend: bool) {
    let buffer = buffer.as_ref();
    let mut iter = buffer.iter_at_mark(&buffer.get_insert());
    if iter.is_end() {
        return;
    }
    while !iter.is_end() && !iter.char().is_whitespace() && iter.char() != '\n' && iter.char() != '\r' {
        if !iter.forward_char() {
            break;
        }
    }
    while !iter.is_end() && iter.char().is_whitespace() {
        if !iter.forward_char() {
            break;
        }
    }
    set_cursor(buffer, &iter, extend);
}

pub fn select_line(buffer: &impl IsA<TextBuffer>, _extend: bool) {
    let buffer = buffer.as_ref();
    let anchor_off = buffer.iter_at_mark(&buffer.selection_bound()).offset();
    let head_off = buffer.iter_at_mark(&buffer.get_insert()).offset();
    let s = anchor_off.min(head_off);
    let e = anchor_off.max(head_off);
    let s_line = buffer.iter_at_offset(s).line();
    let e_line = if s == e {
        s_line
    } else {
        let last = (e - 1).max(0);
        buffer.iter_at_offset(last).line()
    };
    let line_count = buffer.line_count();
    let start_char = buffer.iter_at_line(s_line).unwrap().offset();
    let end_char = if e_line + 1 < line_count {
        buffer.iter_at_line(e_line + 1).unwrap().offset()
    } else {
        buffer.end_iter().offset()
    };
    let already = s == start_char && e == end_char;
    if already {
        let new_end_line = (e_line + 1).min(line_count - 1);
        if new_end_line == e_line && e_line + 1 >= line_count {
            return;
        }
        let new_end = if new_end_line + 1 < line_count {
            buffer.iter_at_line(new_end_line + 1).unwrap().offset()
        } else {
            buffer.end_iter().offset()
        };
        let anchor_iter = buffer.iter_at_offset(start_char);
        let head_iter = buffer.iter_at_offset(new_end);
        buffer.select_range(&head_iter, &anchor_iter);
    } else {
        let anchor_iter = buffer.iter_at_offset(start_char);
        let head_iter = buffer.iter_at_offset(end_char);
        if anchor_off > head_off {
            buffer.select_range(&anchor_iter, &head_iter);
        } else {
            buffer.select_range(&head_iter, &anchor_iter);
        }
    }
}

// ---------------------------------------------------------------------------
// Buffer Edits
// ---------------------------------------------------------------------------

pub fn delete_selection(buffer: &impl IsA<TextBuffer>) {
    let buffer = buffer.as_ref();
    if let Some((mut start, mut end)) = buffer.selection_bounds() {
        buffer.delete(&mut start, &mut end);
    } else {
        let mut start = buffer.iter_at_mark(&buffer.get_insert());
        let mut end = start;
        if end.forward_cursor_position() {
            buffer.delete(&mut start, &mut end);
        }
    }
}

pub fn yank_selection(buffer: &impl IsA<TextBuffer>) -> String {
    let buffer = buffer.as_ref();
    if let Some((start, end)) = buffer.selection_bounds() {
        buffer.text(&start, &end, false).to_string()
    } else {
        let start = buffer.iter_at_mark(&buffer.get_insert());
        let mut end = start;
        if end.forward_cursor_position() {
            buffer.text(&start, &end, false).to_string()
        } else {
            String::new()
        }
    }
}

pub fn paste_after(buffer: &impl IsA<TextBuffer>, text: &str) {
    let buffer = buffer.as_ref();
    if text.is_empty() {
        return;
    }
    let mut iter = buffer.iter_at_mark(&buffer.get_insert());
    if let Some((mut s, mut e)) = buffer.selection_bounds() {
        buffer.delete(&mut s, &mut e);
        iter = buffer.iter_at_mark(&buffer.get_insert());
    }
    buffer.insert(&mut iter, text);
}

pub fn insert_at_line_start(buffer: &impl IsA<TextBuffer>) {
    let buffer = buffer.as_ref();
    let cur = buffer.iter_at_mark(&buffer.get_insert());
    let line = cur.line();
    let line_start = buffer.iter_at_line(line).unwrap_or_else(|| buffer.start_iter());
    let mut target = line_start;
    let mut non_ws_found = false;
    while !target.ends_line() {
        let ch = target.char();
        if !ch.is_whitespace() {
            non_ws_found = true;
            break;
        }
        if !target.forward_char() {
            break;
        }
    }
    if non_ws_found {
        buffer.place_cursor(&target);
    } else {
        buffer.place_cursor(&line_start);
    }
}

pub fn insert_at_line_end(buffer: &impl IsA<TextBuffer>) {
    let buffer = buffer.as_ref();
    let cur = buffer.iter_at_mark(&buffer.get_insert());
    let line = cur.line();
    let mut iter = buffer.iter_at_line(line).unwrap_or_else(|| buffer.end_iter());
    if !iter.ends_line() {
        iter.forward_to_line_end();
    }
    buffer.place_cursor(&iter);
}

pub fn undo(buffer: &impl IsA<TextBuffer>) {
    let buffer = buffer.as_ref();
    if buffer.can_undo() {
        buffer.undo();
    }
}

pub fn redo(buffer: &impl IsA<TextBuffer>) {
    let buffer = buffer.as_ref();
    if buffer.can_redo() {
        buffer.redo();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn make_buffer(text: &str) -> TextBuffer {
        let buf = TextBuffer::new(None);
        buf.set_text(text);
        buf
    }

    #[test]
    fn test_horizontal_movement() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("hello");
        buf.place_cursor(&buf.iter_at_offset(0));
        move_horizontally(&buf, 1, false);
        assert_eq!(offset_at_insert(&buf), 1);
        move_horizontally(&buf, -1, false);
        assert_eq!(offset_at_insert(&buf), 0);
    }

    #[test]
    fn test_vertical_movement() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("ab\ncd\nef");
        buf.place_cursor(&buf.iter_at_offset(0));
        move_vertically(&buf, 1, false);
        let iter = buf.iter_at_mark(&buf.get_insert());
        assert_eq!(iter.line(), 1);
        assert_eq!(iter.line_offset(), 0);
    }

    #[test]
    fn test_word_moves() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("hello world  foo");
        buf.place_cursor(&buf.iter_at_offset(0));
        move_word_forward(&buf, false);
        let (s, e) = buf.selection_bounds().unwrap();
        let txt = buf.text(&s, &e, false).to_string();
        assert!(txt.contains("hello"));

        buf.place_cursor(&buf.iter_at_offset(6));
        move_word_backward(&buf, false);
        let (s2, e2) = buf.selection_bounds().unwrap();
        let txt2 = buf.text(&s2, &e2, false).to_string();
        assert!(txt2.contains("hello"));
    }

    #[test]
    fn test_line_selection() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("line1\nline2\nline3");
        buf.place_cursor(&buf.iter_at_line(1).unwrap());
        select_line(&buf, false);
        let (s, e) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&s, &e, false).as_str(), "line2");
    }

    #[test]
    fn test_edits() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("hello world");
        buf.select_range(&buf.iter_at_offset(0), &buf.iter_at_offset(5));
        let yanked = yank_selection(&buf);
        assert_eq!(yanked, "hello");
        delete_selection(&buf);
        assert_eq!(buf.text(&buf.start_iter(), &buf.end_iter(), false).as_str(), " world");
        paste_after(&buf, "big");
        assert_eq!(buf.text(&buf.start_iter(), &buf.end_iter(), false).as_str(), "big world");

        // Point-cursor yank
        buf.place_cursor(&buf.iter_at_offset(0));
        let yanked_char = yank_selection(&buf);
        assert_eq!(yanked_char, "b");
    }

    #[test]
    fn test_undo_redo_basic() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("hello");
        buf.set_enable_undo(true);
        buf.insert_at_cursor(" world");
        assert_eq!(buf.text(&buf.start_iter(), &buf.end_iter(), false), "hello world");
        assert!(buf.can_undo());
        undo(&buf);
        assert_eq!(buf.text(&buf.start_iter(), &buf.end_iter(), false), "hello");
        assert!(buf.can_redo());
        redo(&buf);
        assert_eq!(buf.text(&buf.start_iter(), &buf.end_iter(), false), "hello world");
    }

    #[test]
    fn test_line_start_and_end_motions() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("    fn foo() {\n        let x = 1;\n    }");
        buf.place_cursor(&buf.iter_at_offset(10));
        insert_at_line_start(&buf);
        assert_eq!(offset_at_insert(&buf), 4);
        insert_at_line_end(&buf);
        assert_eq!(offset_at_insert(&buf), 14);

        buf.place_cursor(&buf.iter_at_offset(20));
        insert_at_line_start(&buf);
        assert_eq!(offset_at_insert(&buf), 23);
        insert_at_line_end(&buf);
        assert_eq!(offset_at_insert(&buf), 33);

        let buf2 = make_buffer("hello world");
        buf2.place_cursor(&buf2.iter_at_offset(5));
        insert_at_line_start(&buf2);
        assert_eq!(offset_at_insert(&buf2), 0);
        insert_at_line_end(&buf2);
        assert_eq!(offset_at_insert(&buf2), 11);

        let buf3 = make_buffer("    \nsecond");
        buf3.place_cursor(&buf3.iter_at_offset(2));
        insert_at_line_start(&buf3);
        assert_eq!(offset_at_insert(&buf3), 0);
        insert_at_line_end(&buf3);
        assert_eq!(offset_at_insert(&buf3), 4);
    }

    #[test]
    fn test_extend_keeps_anchor() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("hello world");
        buf.place_cursor(&buf.iter_at_offset(0));
        move_horizontally(&buf, 1, true);
        let anchor = buf.iter_at_mark(&buf.selection_bound()).offset();
        let head = buf.iter_at_mark(&buf.get_insert()).offset();
        assert_eq!(anchor, 0);
        assert_eq!(head, 1);
    }

    #[test]
    fn test_grapheme_forward() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("e\u{0301}abc");
        buf.place_cursor(&buf.iter_at_offset(0));
        move_horizontally(&buf, 1, false);
        let off = offset_at_insert(&buf);
        assert!(off == 1 || off == 2);
    }

    #[test]
    fn test_pure_word_forward_logic() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("hello world  foo");
        let start = word_move(&buf, Range::new(0, 0), WordMotionTarget::NextWordStart);
        assert_eq!(start.head, 6);
        let end = word_move(&buf, Range::new(0, 0), WordMotionTarget::NextWordEnd);
        assert_eq!(end.head, 5);
        let prev = word_move(&buf, Range::new(6, 6), WordMotionTarget::PrevWordStart);
        assert_eq!(prev.head, 0);
    }

    #[test]
    fn test_w_with_symbols_and_newline() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("a/b");
        assert_eq!(word_move(&buf, Range::new(0, 0), WordMotionTarget::NextWordStart).head, 1);
        assert_eq!(word_move(&buf, Range::new(1, 1), WordMotionTarget::NextWordStart).head, 2);

        let buf2 = make_buffer("foo\n    /bar");
        assert_eq!(word_move(&buf2, Range::new(0, 0), WordMotionTarget::NextWordStart).head, 3);
        assert_eq!(word_move(&buf2, Range::new(3, 3), WordMotionTarget::NextWordStart).head, 8);
        assert_eq!(word_move(&buf2, Range::new(8, 8), WordMotionTarget::NextWordStart).head, 9);
        assert_eq!(word_move(&buf2, Range::new(9, 9), WordMotionTarget::PrevWordStart).head, 8);
        assert_eq!(word_move(&buf2, Range::new(8, 8), WordMotionTarget::PrevWordStart).head, 0);
    }
}
