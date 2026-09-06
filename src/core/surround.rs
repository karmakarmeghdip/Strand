//! Surround pair operations (add, delete, replace).
//!
//! Mirrors `helix-core/src/surround.rs`.

use gtk::prelude::{IsA, TextBufferExt};
use gtk::TextBuffer;

use super::match_brackets::{
    find_closest_enclosing_pair, find_enclosing_bracket_pair, find_enclosing_quote_pair, get_pair,
};

pub fn surround_add(buffer: &impl IsA<TextBuffer>, ch: char) {
    let buffer = buffer.as_ref();
    let (open, close) = get_pair(ch);

    let (start_off, end_off) = if let Some((s, e)) = buffer.selection_bounds() {
        (s.offset(), e.offset())
    } else {
        let cur = buffer.iter_at_mark(&buffer.get_insert()).offset();
        let end_buf = buffer.end_iter().offset();
        if cur < end_buf {
            (cur, cur + 1)
        } else {
            (cur, cur)
        }
    };

    let mut end_iter = buffer.iter_at_offset(end_off);
    buffer.insert(&mut end_iter, &close.to_string());

    let mut start_iter = buffer.iter_at_offset(start_off);
    buffer.insert(&mut start_iter, &open.to_string());

    let ai = buffer.iter_at_offset(start_off);
    let hi = buffer.iter_at_offset(end_off + 2);
    buffer.select_range(&hi, &ai);
}

pub fn surround_delete(buffer: &impl IsA<TextBuffer>, ch: char) {
    let buffer = buffer.as_ref();
    let cur_iter = buffer.iter_at_mark(&buffer.get_insert());

    let pair = if ch == 'm' {
        find_closest_enclosing_pair(buffer, &cur_iter)
    } else {
        let (open, close) = get_pair(ch);
        if open == close {
            find_enclosing_quote_pair(&cur_iter, open)
        } else {
            find_enclosing_bracket_pair(buffer, &cur_iter, open, close)
        }
    };

    if let Some((open_iter, close_iter)) = pair {
        let open_off = open_iter.offset();
        let close_off = close_iter.offset();
        if open_off < close_off {
            let mut close_start = buffer.iter_at_offset(close_off);
            let mut close_end = close_start;
            if close_end.forward_cursor_position() {
                buffer.delete(&mut close_start, &mut close_end);
            }
            let mut open_start = buffer.iter_at_offset(open_off);
            let mut open_end = open_start;
            if open_end.forward_cursor_position() {
                buffer.delete(&mut open_start, &mut open_end);
            }
            let new_cur = buffer.iter_at_offset(open_off);
            buffer.place_cursor(&new_cur);
        }
    }
}

pub fn surround_replace(buffer: &impl IsA<TextBuffer>, from: char, to: char) {
    let buffer = buffer.as_ref();
    let cur_iter = buffer.iter_at_mark(&buffer.get_insert());

    let pair = if from == 'm' {
        find_closest_enclosing_pair(buffer, &cur_iter)
    } else {
        let (open, close) = get_pair(from);
        if open == close {
            find_enclosing_quote_pair(&cur_iter, open)
        } else {
            find_enclosing_bracket_pair(buffer, &cur_iter, open, close)
        }
    };

    if let Some((open_iter, close_iter)) = pair {
        let open_off = open_iter.offset();
        let close_off = close_iter.offset();
        if open_off < close_off {
            let (new_open, new_close) = get_pair(to);
            let mut close_start = buffer.iter_at_offset(close_off);
            let mut close_end = close_start;
            if close_end.forward_cursor_position() {
                buffer.delete(&mut close_start, &mut close_end);
                let mut ins = buffer.iter_at_offset(close_off);
                buffer.insert(&mut ins, &new_close.to_string());
            }
            let mut open_start = buffer.iter_at_offset(open_off);
            let mut open_end = open_start;
            if open_end.forward_cursor_position() {
                buffer.delete(&mut open_start, &mut open_end);
                let mut ins = buffer.iter_at_offset(open_off);
                buffer.insert(&mut ins, &new_open.to_string());
            }
            let new_cur = buffer.iter_at_offset(open_off);
            buffer.place_cursor(&new_cur);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_buffer(text: &str) -> TextBuffer {
        let buf = TextBuffer::new(None);
        buf.set_text(text);
        buf
    }

    #[gtk::test]
    fn test_surround_lifecycle() {
        let buf = make_buffer("hello world");
        buf.select_range(&buf.iter_at_offset(5), &buf.iter_at_offset(0));
        surround_add(&buf, '(');
        assert_eq!(buf.text(&buf.start_iter(), &buf.end_iter(), false), "(hello) world");

        buf.place_cursor(&buf.iter_at_offset(3));
        surround_replace(&buf, 'm', '[');
        assert_eq!(buf.text(&buf.start_iter(), &buf.end_iter(), false), "[hello] world");

        buf.place_cursor(&buf.iter_at_offset(3));
        surround_delete(&buf, 'm');
        assert_eq!(buf.text(&buf.start_iter(), &buf.end_iter(), false), "hello world");
    }
}
