//! Syntax-aware bracket and pair matching.
//!
//! Mirrors `helix-core/src/match_brackets.rs`.

use gtk::prelude::{IsA, TextBufferExt};
use gtk::TextBuffer;
use sourceview5::prelude::{BufferExt as SourceBufferExt, Cast};

use super::selection::set_cursor;

pub const BRACKETS: [(char, char); 9] = [
    ('(', ')'),
    ('{', '}'),
    ('[', ']'),
    ('<', '>'),
    ('‘', '’'),
    ('“', '”'),
    ('«', '»'),
    ('「', '」'),
    ('（', '）'),
];

pub const PAIRS: [(char, char); BRACKETS.len() + 4] = [
    ('(', ')'),
    ('{', '}'),
    ('[', ']'),
    ('<', '>'),
    ('‘', '’'),
    ('“', '”'),
    ('«', '»'),
    ('「', '」'),
    ('（', '）'),
    ('"', '"'),
    ('\'', '\''),
    ('`', '`'),
    ('|', '|'),
];

pub fn get_pair(ch: char) -> (char, char) {
    PAIRS
        .iter()
        .find(|(open, close)| *open == ch || *close == ch)
        .copied()
        .unwrap_or((ch, ch))
}

pub fn is_open_bracket(ch: char) -> bool {
    BRACKETS.iter().any(|(l, _)| *l == ch)
}

pub fn is_close_bracket(ch: char) -> bool {
    BRACKETS.iter().any(|(_, r)| *r == ch)
}

pub fn is_valid_bracket(ch: char) -> bool {
    BRACKETS.iter().any(|(l, r)| *l == ch || *r == ch)
}

pub fn is_open_pair(ch: char) -> bool {
    PAIRS.iter().any(|(l, _)| *l == ch)
}

pub fn is_close_pair(ch: char) -> bool {
    PAIRS.iter().any(|(_, r)| *r == ch)
}

pub fn is_valid_pair(ch: char) -> bool {
    PAIRS.iter().any(|(l, r)| *l == ch || *r == ch)
}

pub fn is_in_comment_or_string(buffer: &gtk::TextBuffer, iter: &gtk::TextIter) -> bool {
    if let Some(source_buf) = buffer.downcast_ref::<sourceview5::Buffer>() {
        source_buf.iter_has_context_class(iter, "comment")
            || source_buf.iter_has_context_class(iter, "string")
    } else {
        false
    }
}

pub fn find_matching_close_bracket(
    buffer: &gtk::TextBuffer,
    start_iter: &gtk::TextIter,
) -> Option<gtk::TextIter> {
    let open_ch = start_iter.char();
    let (_, close_ch) = get_pair(open_ch);
    if open_ch == close_ch {
        return None;
    }
    let mut iter = *start_iter;
    let mut depth = 1;
    while iter.forward_char() {
        if is_in_comment_or_string(buffer, &iter) {
            continue;
        }
        let c = iter.char();
        if c == open_ch {
            depth += 1;
        } else if c == close_ch {
            depth -= 1;
            if depth == 0 {
                return Some(iter);
            }
        }
    }
    None
}

pub fn find_matching_open_bracket(
    buffer: &gtk::TextBuffer,
    start_iter: &gtk::TextIter,
) -> Option<gtk::TextIter> {
    let close_ch = start_iter.char();
    let (open_ch, _) = get_pair(close_ch);
    if open_ch == close_ch {
        return None;
    }
    let mut iter = *start_iter;
    let mut depth = 1;
    while iter.backward_char() {
        if is_in_comment_or_string(buffer, &iter) {
            continue;
        }
        let c = iter.char();
        if c == close_ch {
            depth += 1;
        } else if c == open_ch {
            depth -= 1;
            if depth == 0 {
                return Some(iter);
            }
        }
    }
    None
}

pub fn find_matching_quote(start_iter: &gtk::TextIter) -> Option<gtk::TextIter> {
    let q = start_iter.char();
    // Search forward on same line
    let mut iter = *start_iter;
    while !iter.ends_line() && iter.forward_char() {
        if iter.char() == q {
            let mut prev = iter;
            if !prev.backward_char() || prev.char() != '\\' {
                return Some(iter);
            }
        }
    }
    // Search backward on same line
    let mut iter = *start_iter;
    while !iter.starts_line() && iter.backward_char() {
        if iter.char() == q {
            let mut prev = iter;
            if !prev.backward_char() || prev.char() != '\\' {
                return Some(iter);
            }
        }
    }
    None
}

pub fn find_enclosing_bracket_pair(
    buffer: &gtk::TextBuffer,
    pos_iter: &gtk::TextIter,
    open_ch: char,
    close_ch: char,
) -> Option<(gtk::TextIter, gtk::TextIter)> {
    let pos_ch = pos_iter.char();
    if pos_ch == open_ch && !is_in_comment_or_string(buffer, pos_iter) {
        let close_iter = find_matching_close_bracket(buffer, pos_iter)?;
        return Some((*pos_iter, close_iter));
    }
    if pos_ch == close_ch && !is_in_comment_or_string(buffer, pos_iter) {
        let open_iter = find_matching_open_bracket(buffer, pos_iter)?;
        return Some((open_iter, *pos_iter));
    }

    let pos_off = pos_iter.offset();

    // Scan forward from pos_iter for close_ch
    let mut fwd = *pos_iter;
    let mut depth = 0;
    while fwd.forward_char() {
        if is_in_comment_or_string(buffer, &fwd) {
            continue;
        }
        let c = fwd.char();
        if c == open_ch {
            depth += 1;
        } else if c == close_ch {
            if depth == 0 {
                if let Some(open_iter) = find_matching_open_bracket(buffer, &fwd)
                    .filter(|open_iter| open_iter.offset() <= pos_off)
                {
                    return Some((open_iter, fwd));
                }
            } else {
                depth -= 1;
            }
        }
    }
    None
}

pub fn find_enclosing_quote_pair(
    pos_iter: &gtk::TextIter,
    q: char,
) -> Option<(gtk::TextIter, gtk::TextIter)> {
    let pos_ch = pos_iter.char();
    if let (true, Some(matched)) = (pos_ch == q, find_matching_quote(pos_iter)) {
        return if pos_iter.offset() < matched.offset() {
            Some((*pos_iter, matched))
        } else {
            Some((matched, *pos_iter))
        };
    }

    // Search backward on same line
    let mut bwd = *pos_iter;
    let mut open_iter = None;
    while !bwd.starts_line() && bwd.backward_char() {
        if bwd.char() == q {
            let mut prev = bwd;
            if !prev.backward_char() || prev.char() != '\\' {
                open_iter = Some(bwd);
                break;
            }
        }
    }
    let open_iter = open_iter?;

    // Search forward on same line from pos_iter
    let mut fwd = *pos_iter;
    while !fwd.ends_line() && fwd.forward_char() {
        if fwd.char() == q {
            let mut prev = fwd;
            if !prev.backward_char() || prev.char() != '\\' {
                return Some((open_iter, fwd));
            }
        }
    }
    None
}

pub fn find_closest_enclosing_pair(
    buffer: &gtk::TextBuffer,
    pos_iter: &gtk::TextIter,
) -> Option<(gtk::TextIter, gtk::TextIter)> {
    let mut closest: Option<(gtk::TextIter, gtk::TextIter)> = None;

    for &(open, close) in &BRACKETS {
        if let Some((o, c)) = find_enclosing_bracket_pair(buffer, pos_iter, open, close) {
            let span = c.offset() - o.offset();
            match closest {
                Some((co, cc)) if (cc.offset() - co.offset()) <= span => {}
                _ => closest = Some((o, c)),
            }
        }
    }

    for q in ['"', '\'', '`', '|'] {
        if let Some((o, c)) = find_enclosing_quote_pair(pos_iter, q) {
            let span = c.offset() - o.offset();
            match closest {
                Some((co, cc)) if (cc.offset() - co.offset()) <= span => {}
                _ => closest = Some((o, c)),
            }
        }
    }

    closest
}

pub fn match_brackets(
    buffer: &impl IsA<TextBuffer>,
    last_matched_bracket: Option<i32>,
    extend: bool,
) {
    let buffer = buffer.as_ref();
    let cur_iter = buffer.iter_at_mark(&buffer.get_insert());
    let cur_ch = cur_iter.char();

    // 1. If GtkSourceBuffer already found a match for current bracket, use it!
    if let (true, Some(target_offset)) = (is_valid_bracket(cur_ch), last_matched_bracket) {
        let target_iter = buffer.iter_at_offset(target_offset);
        set_cursor(buffer, &target_iter, extend);
        return;
    }

    // 2. Otherwise use our syntax-aware bracket search:
    let target_iter = if is_open_bracket(cur_ch) {
        find_matching_close_bracket(buffer, &cur_iter)
    } else if is_close_bracket(cur_ch) {
        find_matching_open_bracket(buffer, &cur_iter)
    } else if cur_ch == '"' || cur_ch == '\'' || cur_ch == '`' || cur_ch == '|' {
        find_matching_quote(&cur_iter)
    } else if let Some((open_iter, close_iter)) = find_closest_enclosing_pair(buffer, &cur_iter) {
        if cur_iter.offset() == close_iter.offset() {
            Some(open_iter)
        } else {
            Some(close_iter)
        }
    } else {
        None
    };

    if let Some(target) = target_iter {
        set_cursor(buffer, &target, extend);
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
    fn test_bracket_logic() {
        let buf = make_buffer("fn foo() {\n    let x = 1;\n}");
        buf.place_cursor(&buf.iter_at_offset(6)); // on '('
        match_brackets(&buf, None, false);
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).offset(), 7); // ')'

        match_brackets(&buf, None, false);
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).offset(), 6); // '('
    }

    #[gtk::test]
    fn test_bracket_extend() {
        let buf = make_buffer("fn foo(bar, baz) {}");
        buf.place_cursor(&buf.iter_at_offset(6));
        match_brackets(&buf, None, true);
        assert_eq!(buf.iter_at_mark(&buf.get_insert()).offset(), 15);
        assert_eq!(buf.iter_at_mark(&buf.selection_bound()).offset(), 6);
    }
}
