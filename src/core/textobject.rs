//! Text object selections (words, paragraphs, brackets, quotes).
//!
//! Mirrors `helix-core/src/textobject.rs`.

use gtk::prelude::{IsA, TextBufferExt};
use gtk::TextBuffer;

use super::chars::{categorize, CharCategory};
use super::match_brackets::{
    find_closest_enclosing_pair, find_enclosing_bracket_pair, find_enclosing_quote_pair, get_pair,
};

pub fn word_bounds_at(buffer: &gtk::TextBuffer, pos: usize) -> Option<(usize, usize)> {
    let iter = buffer.iter_at_offset(pos as i32);
    if iter.is_end() {
        return None;
    }
    let cat = categorize(iter.char());
    if cat == CharCategory::Whitespace || cat == CharCategory::Eol {
        return None;
    }
    let mut start = iter;
    while !start.is_start() {
        let mut prev = start;
        prev.backward_char();
        if categorize(prev.char()) == cat {
            start = prev;
        } else {
            break;
        }
    }
    let mut end = iter;
    while !end.is_end() && categorize(end.char()) == cat {
        end.forward_char();
    }
    Some((start.offset() as usize, end.offset() as usize))
}

pub fn select_textobject(buffer: &impl IsA<TextBuffer>, obj: char, inside: bool) {
    let buffer = buffer.as_ref();
    if buffer.char_count() == 0 {
        return;
    }
    let cur_iter = if buffer.has_selection() {
        buffer.selection_bounds().unwrap().0
    } else {
        buffer.iter_at_mark(&buffer.get_insert())
    };
    let pos = cur_iter.offset() as usize;

    let bounds: Option<(i32, i32)> = match obj {
        'w' => word_bounds_at(buffer, pos).map(|(start, end)| {
            if inside {
                (start as i32, end as i32)
            } else {
                let mut sel_end = buffer.iter_at_offset(end as i32);
                while !sel_end.is_end() && (sel_end.char() == ' ' || sel_end.char() == '\t') {
                    sel_end.forward_char();
                }
                let mut sel_start = buffer.iter_at_offset(start as i32);
                if sel_end.offset() == end as i32 {
                    while !sel_start.is_start() {
                        let mut prev = sel_start;
                        prev.backward_char();
                        if prev.char() == ' ' || prev.char() == '\t' {
                            sel_start = prev;
                        } else {
                            break;
                        }
                    }
                }
                (sel_start.offset(), sel_end.offset())
            }
        }),
        'W' => {
            let iter = buffer.iter_at_offset(pos as i32);
            if !iter.is_end() && !iter.char().is_whitespace() && iter.char() != '\n' && iter.char() != '\r' {
                let mut start = iter;
                while !start.is_start() {
                    let mut prev = start;
                    prev.backward_char();
                    if !prev.char().is_whitespace() && prev.char() != '\n' && prev.char() != '\r' {
                        start = prev;
                    } else {
                        break;
                    }
                }
                let mut end = iter;
                while !end.is_end() && !end.char().is_whitespace() && end.char() != '\n' && end.char() != '\r' {
                    end.forward_char();
                }
                if inside {
                    Some((start.offset(), end.offset()))
                } else {
                    let mut sel_end = end;
                    while !sel_end.is_end() && (sel_end.char() == ' ' || sel_end.char() == '\t') {
                        sel_end.forward_char();
                    }
                    let mut sel_start = start;
                    if sel_end.offset() == end.offset() {
                        while !sel_start.is_start() {
                            let mut prev = sel_start;
                            prev.backward_char();
                            if prev.char() == ' ' || prev.char() == '\t' {
                                sel_start = prev;
                            } else {
                                break;
                            }
                        }
                    }
                    Some((sel_start.offset(), sel_end.offset()))
                }
            } else {
                None
            }
        }
        'p' => {
            let cur_line = cur_iter.line();
            let line_count = buffer.line_count();

            let is_line_blank = |l: i32| -> bool {
                let start = buffer.iter_at_line(l).unwrap_or_else(|| buffer.end_iter());
                let mut iter = start;
                while !iter.ends_line() {
                    let c = iter.char();
                    if !c.is_whitespace() {
                        return false;
                    }
                    if !iter.forward_char() {
                        break;
                    }
                }
                true
            };

            let mut start_line = cur_line;
            while start_line > 0 && !is_line_blank(start_line - 1) {
                start_line -= 1;
            }
            let mut end_line = cur_line;
            while end_line + 1 < line_count && !is_line_blank(end_line + 1) {
                end_line += 1;
            }

            let start_off = buffer.iter_at_line(start_line).unwrap().offset();
            let mut end_iter = buffer.iter_at_line(end_line).unwrap();
            if !end_iter.ends_line() {
                end_iter.forward_to_line_end();
            }
            let mut end_off = end_iter.offset();

            if !inside {
                let mut blank_line = end_line + 1;
                while blank_line < line_count && is_line_blank(blank_line) {
                    let mut iter = buffer.iter_at_line(blank_line).unwrap();
                    if !iter.ends_line() {
                        iter.forward_to_line_end();
                    }
                    if !iter.is_end() {
                        iter.forward_char();
                    }
                    end_off = iter.offset();
                    blank_line += 1;
                }
            }
            Some((start_off, end_off))
        }
        'm' => find_closest_enclosing_pair(buffer, &cur_iter).map(|(open, close)| {
            if inside {
                (open.offset() + 1, close.offset())
            } else {
                (open.offset(), close.offset() + 1)
            }
        }),
        pair_ch => {
            let (open, close) = get_pair(pair_ch);
            let pair = if open == close {
                find_enclosing_quote_pair(&cur_iter, open)
            } else {
                find_enclosing_bracket_pair(buffer, &cur_iter, open, close)
            };
            pair.map(|(open_iter, close_iter)| {
                if inside {
                    (open_iter.offset() + 1, close_iter.offset())
                } else {
                    (open_iter.offset(), close_iter.offset() + 1)
                }
            })
        }
    };

    if let Some((start, end)) = bounds.filter(|(s, e)| s <= e) {
        let ai = buffer.iter_at_offset(start);
        let hi = buffer.iter_at_offset(end);
        buffer.select_range(&hi, &ai);
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
    fn test_textobjects_suite() {
        let buf = make_buffer("let x = (hello world);");
        buf.place_cursor(&buf.iter_at_offset(10));

        select_textobject(&buf, '(', true);
        let (mut s, mut e) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&mut s, &mut e, false), "hello world");

        select_textobject(&buf, '(', false);
        let (mut s2, mut e2) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&mut s2, &mut e2, false), "(hello world)");

        select_textobject(&buf, 'm', true);
        let (mut s3, mut e3) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&mut s3, &mut e3, false), "hello world");

        select_textobject(&buf, 'w', true);
        let (mut s4, mut e4) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&mut s4, &mut e4, false), "hello");

        select_textobject(&buf, 'w', false);
        let (mut s5, mut e5) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&mut s5, &mut e5, false), "hello ");
    }
}
