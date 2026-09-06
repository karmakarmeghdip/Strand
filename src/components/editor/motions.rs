#![allow(dead_code, unused)]
//! Imperative `GtkTextBuffer` motions — fast path (CAPTURE).
//!
//! All functions operate via `GtkTextIter` + `select_range` + marks
//! `insert` / `selection_bound` (Invariant #1). No `ropey::Rope`.
//! Grapheme handling uses `TextIter::forward_cursor_position` /
//! `backward_cursor_position` (Pango grapheme clusters) and
//! `unicode-general-category` for word boundaries.

use gtk::prelude::{IsA, TextBufferExt};
use gtk::TextBuffer;
use sourceview5::prelude::{BufferExt as SourceBufferExt, Cast};
use unicode_general_category::{get_general_category, GeneralCategory};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharCategory {
    Whitespace,
    Eol,
    Word,
    Punctuation,
}

fn categorize(ch: char) -> CharCategory {
    if ch == '\n' || ch == '\r' {
        CharCategory::Eol
    } else if ch.is_whitespace() {
        CharCategory::Whitespace
    } else if ch.is_alphanumeric() || ch == '_' {
        CharCategory::Word
    } else {
        CharCategory::Punctuation
    }
}

fn is_word_boundary(a: char, b: char) -> bool {
    categorize(a) != categorize(b)
}

fn is_long_word_boundary(a: char, b: char) -> bool {
    match (categorize(a), categorize(b)) {
        (CharCategory::Word, CharCategory::Punctuation)
        | (CharCategory::Punctuation, CharCategory::Word) => false,
        (x, y) if x != y => true,
        _ => false,
    }
}

fn char_is_line_ending(ch: char) -> bool {
    ch == '\n' || ch == '\r'
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Range {
    anchor: usize,
    head: usize,
}

impl Range {
    fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }
    fn cursor(&self, chars: &[char]) -> usize {
        if self.head > self.anchor {
            self.head.saturating_sub(1)
        } else {
            self.head
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordMotionTarget {
    NextWordStart,
    NextWordEnd,
    PrevWordStart,
    PrevWordEnd,
}

fn reached_target(target: WordMotionTarget, prev_ch: char, next_ch: char) -> bool {
    match target {
        WordMotionTarget::NextWordStart | WordMotionTarget::PrevWordEnd => {
            is_word_boundary(prev_ch, next_ch)
                && (char_is_line_ending(next_ch) || !next_ch.is_whitespace())
        }
        WordMotionTarget::NextWordEnd | WordMotionTarget::PrevWordStart => {
            is_word_boundary(prev_ch, next_ch)
                && (!prev_ch.is_whitespace() || char_is_line_ending(next_ch))
        }
        _ => false,
    }
}

struct BufferChars {
    iter: gtk::TextIter,
    reversed: bool,
}

impl BufferChars {
    fn new(buffer: &gtk::TextBuffer, pos: usize) -> Self {
        let iter = buffer.iter_at_offset(pos as i32);
        Self {
            iter,
            reversed: false,
        }
    }
    fn reverse(&mut self) {
        self.reversed = !self.reversed;
    }
    fn next(&mut self) -> Option<char> {
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
    fn prev(&mut self) -> Option<char> {
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

fn range_to_target(
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
    while let Some(next_ch) = my_chars.next() {
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

fn word_move(buffer: &gtk::TextBuffer, range: Range, target: WordMotionTarget) -> Range {
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

fn offset_at_insert(buffer: &impl IsA<TextBuffer>) -> i32 {
    let buffer = buffer.as_ref();
    buffer.iter_at_mark(&buffer.get_insert()).offset()
}

fn iter_at_offset(buffer: &impl IsA<TextBuffer>, offset: i32) -> gtk::TextIter {
    let buffer = buffer.as_ref();
    buffer.iter_at_offset(offset)
}

fn set_cursor(buffer: &impl IsA<TextBuffer>, iter: &gtk::TextIter, extend: bool) {
    let buffer = buffer.as_ref();
    if extend {
        let anchor = buffer.iter_at_mark(&buffer.selection_bound());
        buffer.select_range(iter, &anchor);
    } else {
        buffer.place_cursor(iter);
    }
}

// ---------------------------------------------------------------------------
// Horizontal / vertical
// ---------------------------------------------------------------------------

pub fn move_horizontally(buffer: &impl IsA<TextBuffer>, dir: i32, extend: bool) {
    let buffer = buffer.as_ref();
    let mut iter = buffer.iter_at_mark(&buffer.get_insert());
    let moved = if dir < 0 {
        iter.backward_cursor_position()
    } else {
        iter.forward_cursor_position()
    };
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
        if line == 0 {
            return;
        }
        line - 1
    } else {
        let lc = buffer.line_count();
        if line + 1 >= lc {
            return;
        }
        line + 1
    };

    let mut target = buffer.iter_at_line(target_line).expect("line exists");
    let mut line_end = target;
    if !line_end.ends_line() {
        line_end.forward_to_line_end();
    }
    let target_end_offset = line_end.offset();
    for i in 0..line_offset {
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

// ---------------------------------------------------------------------------
// Word motions — Helix-style via word_move
// ---------------------------------------------------------------------------

fn word_bounds_at(buffer: &gtk::TextBuffer, pos: usize) -> Option<(usize, usize)> {
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

// ---------------------------------------------------------------------------
// Line / selection helpers
// ---------------------------------------------------------------------------

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
// Edits
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
    if let Some((mut start, mut end)) = buffer.selection_bounds() {
        buffer.text(&start, &end, false).to_string()
    } else {
        String::new()
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

// ---------------------------------------------------------------------------
// Match mode: bracket navigation, surround, and textobjects
// ---------------------------------------------------------------------------

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

fn is_in_comment_or_string(buffer: &gtk::TextBuffer, iter: &gtk::TextIter) -> bool {
    if let Some(source_buf) = buffer.downcast_ref::<sourceview5::Buffer>() {
        source_buf.iter_has_context_class(iter, "comment")
            || source_buf.iter_has_context_class(iter, "string")
    } else {
        false
    }
}

pub fn find_matching_close_bracket(buffer: &gtk::TextBuffer, start_iter: &gtk::TextIter) -> Option<gtk::TextIter> {
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

pub fn find_matching_open_bracket(buffer: &gtk::TextBuffer, start_iter: &gtk::TextIter) -> Option<gtk::TextIter> {
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

pub fn match_brackets(buffer: &impl IsA<TextBuffer>, last_matched_bracket: Option<i32>, extend: bool) {
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

pub fn select_textobject(buffer: &impl IsA<TextBuffer>, obj: char, inside: bool) {
    let buffer = buffer.as_ref();
    let cur_iter = buffer.iter_at_mark(&buffer.get_insert());
    let pos = cur_iter.offset() as usize;
    if buffer.char_count() == 0 {
        return;
    }

    let bounds: Option<(i32, i32)> = match obj {
        'w' => {
            word_bounds_at(buffer, pos).map(|(start, end)| {
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
            })
        }
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
        'm' => {
            find_closest_enclosing_pair(buffer, &cur_iter).map(|(open, close)| {
                if inside {
                    (open.offset() + 1, close.offset())
                } else {
                    (open.offset(), close.offset() + 1)
                }
            })
        }
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
    use gtk::prelude::*;

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
    fn horiz_moves() {
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
    fn vert_moves() {
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
    fn word_forward() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("hello world  foo");
        buf.place_cursor(&buf.iter_at_offset(0));
        move_word_forward(&buf, false);
        let (mut s, mut e) = buf.selection_bounds().unwrap();
        let txt = buf.text(&mut s, &mut e, false).to_string();
        assert!(txt.contains("hello"), "got {:?}", txt);
        assert!(!txt.contains('\n'));
    }

    #[test]
    fn word_backward() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("hello world");
        buf.place_cursor(&buf.iter_at_offset(6));
        move_word_backward(&buf, false);
        let (mut s, mut e) = buf.selection_bounds().unwrap();
        let txt = buf.text(&mut s, &mut e, false).to_string();
        assert!(txt.contains("hello"), "got {:?}", txt);
    }

    #[test]
    fn select_line_test() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("line1\nline2\nline3");
        buf.place_cursor(&buf.iter_at_line(1).unwrap());
        select_line(&buf, false);
        let (mut s, mut e) = buf.selection_bounds().unwrap();
        let txt = buf.text(&mut s, &mut e, false).to_string();
        assert!(txt.contains("line2"), "got {:?}", txt);
    }

    #[test]
    fn delete_and_yank() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("hello world");
        buf.select_range(&buf.iter_at_offset(0), &buf.iter_at_offset(5));
        let yanked = yank_selection(&buf);
        assert_eq!(yanked, "hello");
        delete_selection(&buf);
        assert_eq!(buf.text(&buf.start_iter(), &buf.end_iter(), false), " world");
    }

    #[test]
    fn paste() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("hello");
        buf.place_cursor(&buf.iter_at_offset(5));
        paste_after(&buf, " world");
        assert_eq!(buf.text(&buf.start_iter(), &buf.end_iter(), false), "hello world");
    }

    #[test]
    fn undo_redo_basic() {
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
    fn line_start_and_end_motions() {
        if !ensure_gtk() {
            return;
        }
        // Test with indentation
        let buf = make_buffer("    fn foo() {\n        let x = 1;\n    }");
        buf.place_cursor(&buf.iter_at_offset(10));
        insert_at_line_start(&buf);
        assert_eq!(offset_at_insert(&buf), 4); // First non-ws character 'f'
        insert_at_line_end(&buf);
        assert_eq!(offset_at_insert(&buf), 14); // End of line before '\n'

        // Test second line
        buf.place_cursor(&buf.iter_at_offset(20));
        insert_at_line_start(&buf);
        assert_eq!(offset_at_insert(&buf), 23); // 'l' in "        let" (15 + 8)
        insert_at_line_end(&buf);
        assert_eq!(offset_at_insert(&buf), 33); // after ';' before '\n'

        // Test line without indentation
        let buf2 = make_buffer("hello world");
        buf2.place_cursor(&buf2.iter_at_offset(5));
        insert_at_line_start(&buf2);
        assert_eq!(offset_at_insert(&buf2), 0);
        insert_at_line_end(&buf2);
        assert_eq!(offset_at_insert(&buf2), 11);

        // Test line with only whitespace
        let buf3 = make_buffer("    \nsecond");
        buf3.place_cursor(&buf3.iter_at_offset(2));
        insert_at_line_start(&buf3);
        assert_eq!(offset_at_insert(&buf3), 0);
        insert_at_line_end(&buf3);
        assert_eq!(offset_at_insert(&buf3), 4);
    }

    #[test]
    fn extend_keeps_anchor() {
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
    fn grapheme_forward() {
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
    fn categorize_word() {
        assert_eq!(categorize('a'), CharCategory::Word);
        assert_eq!(categorize('_'), CharCategory::Word);
        assert_eq!(categorize(' '), CharCategory::Whitespace);
        assert_eq!(categorize('\n'), CharCategory::Eol);
        assert_eq!(categorize('!'), CharCategory::Punctuation);
    }

    #[test]
    fn word_boundary_detection() {
        assert!(is_word_boundary('a', ' '));
        assert!(!is_word_boundary('a', 'b'));
        assert!(is_word_boundary(' ', '!'));
    }

    #[test]
    fn pure_word_forward_logic() {
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
    fn w_with_symbols_and_newline() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("a/b");
        assert_eq!(word_move(&buf, Range::new(0, 0), WordMotionTarget::NextWordStart).head, 1);
        assert_eq!(word_move(&buf, Range::new(1, 1), WordMotionTarget::NextWordStart).head, 2);

        let buf2 = make_buffer("foo\n    /bar");
        // Helix w from foo at 0 stays at foo (0-3), next w from newline goes to /
        assert_eq!(word_move(&buf2, Range::new(0, 0), WordMotionTarget::NextWordStart).head, 3);
        assert_eq!(word_move(&buf2, Range::new(3, 3), WordMotionTarget::NextWordStart).head, 8);
        assert_eq!(word_move(&buf2, Range::new(8, 8), WordMotionTarget::NextWordStart).head, 9);
        assert_eq!(word_move(&buf2, Range::new(9, 9), WordMotionTarget::PrevWordStart).head, 8);
        assert_eq!(word_move(&buf2, Range::new(8, 8), WordMotionTarget::PrevWordStart).head, 0);
    }

    #[test]
    fn test_match_brackets_logic() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("fn foo() {\n    let x = 1;\n}");
        // Place cursor on '(' at offset 6
        buf.place_cursor(&buf.iter_at_offset(6));
        match_brackets(&buf, None, false);
        assert_eq!(offset_at_insert(&buf), 7); // jumps to ')'

        // Press again on ')' -> jumps to '('
        match_brackets(&buf, None, false);
        assert_eq!(offset_at_insert(&buf), 6);

        // Place cursor inside { ... }, e.g. at line 1 offset 15
        buf.place_cursor(&buf.iter_at_offset(15));
        match_brackets(&buf, None, false);
        // Should jump to closing '}'
        let insert_off = offset_at_insert(&buf);
        let ch = buf.iter_at_offset(insert_off).char();
        assert_eq!(ch, '}');

        // Press again from '}' -> should jump to '{'
        match_brackets(&buf, None, false);
        let open_ch = buf.iter_at_offset(offset_at_insert(&buf)).char();
        assert_eq!(open_ch, '{');
    }

    #[test]
    fn test_match_brackets_extend() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("fn foo(bar, baz) {}");
        // Place cursor on '(' at offset 6
        buf.place_cursor(&buf.iter_at_offset(6));
        // Extend to matching bracket
        match_brackets(&buf, None, true);
        assert_eq!(offset_at_insert(&buf), 16); // at ')'
        let anchor = buf.iter_at_mark(&buf.selection_bound()).offset();
        assert_eq!(anchor, 6);
    }

    #[test]
    fn test_surround_operations() {
        if !ensure_gtk() {
            return;
        }
        // Surround Add
        let buf = make_buffer("hello world");
        buf.select_range(&buf.iter_at_offset(5), &buf.iter_at_offset(0));
        surround_add(&buf, '(');
        assert_eq!(buf.text(&buf.start_iter(), &buf.end_iter(), false), "(hello) world");

        // Surround Replace
        // Cursor inside (hello)
        buf.place_cursor(&buf.iter_at_offset(3));
        surround_replace(&buf, 'm', '[');
        assert_eq!(buf.text(&buf.start_iter(), &buf.end_iter(), false), "[hello] world");

        // Surround Delete
        buf.place_cursor(&buf.iter_at_offset(3));
        surround_delete(&buf, 'm');
        assert_eq!(buf.text(&buf.start_iter(), &buf.end_iter(), false), "hello world");
    }

    #[test]
    fn test_textobjects() {
        if !ensure_gtk() {
            return;
        }
        let buf = make_buffer("let x = (hello world);");
        // Place cursor on 'hello' at offset 10
        buf.place_cursor(&buf.iter_at_offset(10));

        // mi( selects inside 'hello world'
        select_textobject(&buf, '(', true);
        let (mut s, mut e) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&mut s, &mut e, false), "hello world");

        // ma( selects '(hello world)'
        select_textobject(&buf, '(', false);
        let (mut s2, mut e2) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&mut s2, &mut e2, false), "(hello world)");

        // mim selects inside closest pair
        select_textobject(&buf, 'm', true);
        let (mut s3, mut e3) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&mut s3, &mut e3, false), "hello world");

        // miw selects word 'hello'
        select_textobject(&buf, 'w', true);
        let (mut s4, mut e4) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&mut s4, &mut e4, false), "hello");

        // maw selects 'hello '
        select_textobject(&buf, 'w', false);
        let (mut s5, mut e5) = buf.selection_bounds().unwrap();
        assert_eq!(buf.text(&mut s5, &mut e5, false), "hello ");
    }
}
