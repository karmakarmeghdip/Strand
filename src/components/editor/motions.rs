#![allow(dead_code, unused, clippy::all)]
//! Imperative `GtkTextBuffer` motions — fast path (CAPTURE).
//!
//! All functions operate via `GtkTextIter` + `select_range` + marks
//! `insert` / `selection_bound` (Invariant #1). No `ropey::Rope`.
//! Grapheme handling uses `TextIter::forward_cursor_position` /
//! `backward_cursor_position` (Pango grapheme clusters) and
//! `unicode-general-category` for word boundaries.

use gtk::prelude::{IsA, TextBufferExt};
use gtk::TextBuffer;
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
    } else if matches!(
        get_general_category(ch),
        GeneralCategory::OtherPunctuation
            | GeneralCategory::OpenPunctuation
            | GeneralCategory::ClosePunctuation
            | GeneralCategory::InitialPunctuation
            | GeneralCategory::FinalPunctuation
            | GeneralCategory::ConnectorPunctuation
            | GeneralCategory::DashPunctuation
            | GeneralCategory::MathSymbol
            | GeneralCategory::CurrencySymbol
            | GeneralCategory::ModifierSymbol
    ) {
        CharCategory::Punctuation
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

struct MyChars<'a> {
    chars: &'a [char],
    pos: usize,
    reversed: bool,
}

impl<'a> MyChars<'a> {
    fn new(chars: &'a [char], pos: usize) -> Self {
        Self {
            chars,
            pos,
            reversed: false,
        }
    }
    fn reverse(&mut self) {
        self.reversed = !self.reversed;
    }
    fn next(&mut self) -> Option<char> {
        if self.reversed {
            if self.pos == 0 {
                None
            } else {
                self.pos -= 1;
                Some(self.chars[self.pos])
            }
        } else {
            if self.pos >= self.chars.len() {
                None
            } else {
                let ch = self.chars[self.pos];
                self.pos += 1;
                Some(ch)
            }
        }
    }
    fn prev(&mut self) -> Option<char> {
        if self.reversed {
            if self.pos >= self.chars.len() {
                None
            } else {
                let ch = self.chars[self.pos];
                self.pos += 1;
                Some(ch)
            }
        } else {
            if self.pos == 0 {
                None
            } else {
                self.pos -= 1;
                Some(self.chars[self.pos])
            }
        }
    }
}

fn range_to_target(chars: &[char], origin: Range, target: WordMotionTarget, is_prev: bool) -> Range {
    let mut my_chars = MyChars::new(chars, origin.head);
    if is_prev {
        my_chars.reverse();
    }
    let advance: Box<dyn Fn(&mut usize)> = if is_prev {
        Box::new(|idx: &mut usize| *idx = idx.saturating_sub(1))
    } else {
        Box::new(|idx: &mut usize| *idx += 1)
    };
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
            advance(&mut head);

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
        advance(&mut head);
    }
    if is_prev {
        my_chars.reverse();
    }
    Range::new(anchor, head)
}

fn word_move(chars: &[char], range: Range, target: WordMotionTarget) -> Range {
    let is_prev = matches!(
        target,
        WordMotionTarget::PrevWordStart | WordMotionTarget::PrevWordEnd
    );
    if (is_prev && range.head == 0) || (!is_prev && range.head == chars.len()) {
        return range;
    }
    let start_range = if is_prev {
        if range.anchor < range.head {
            Range::new(range.head, range.head.saturating_sub(1))
        } else {
            Range::new((range.head + 1).min(chars.len()), range.head)
        }
    } else {
        if range.anchor < range.head {
            let prev = if range.head > 0 { range.head - 1 } else { 0 };
            Range::new(prev, range.head)
        } else {
            Range::new(range.head, (range.head + 1).min(chars.len()))
        }
    };
    let mut cur = start_range;
    // For simplicity, count is 1
    let next = range_to_target(chars, cur, target, is_prev);
    if cur == next {
        cur
    } else {
        next
    }
}

// ---------------------------------------------------------------------------
// Low-level buffer helpers
// ---------------------------------------------------------------------------

fn buffer_text(buffer: &impl IsA<TextBuffer>) -> Vec<char> {
    let buffer = buffer.as_ref();
    let (start, end) = (buffer.start_iter(), buffer.end_iter());
    let s = buffer.text(&start, &end, false);
    s.chars().collect()
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
        let tl = line - 1;
        tl
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

fn find_next_word_start(chars: &[char], pos: usize) -> usize {
    if pos >= chars.len() {
        return chars.len();
    }
    let cat = categorize(chars[pos]);
    if cat == CharCategory::Whitespace || cat == CharCategory::Eol {
        // At whitespace/newline, find next word start
        let mut idx = pos;
        while idx < chars.len() && (categorize(chars[idx]) == CharCategory::Whitespace || categorize(chars[idx]) == CharCategory::Eol) {
            idx += 1;
        }
        return idx;
    } else {
        // At word/punct, find end of current word and include trailing spaces/tabs on same line (not newlines)
        let cat_at = cat;
        let mut end = pos;
        while end < chars.len() && categorize(chars[end]) == cat_at {
            end += 1;
        }
        // Include trailing spaces/tabs on same line, but stop before newline
        while end < chars.len() && (chars[end] == ' ' || chars[end] == '\t') {
            // Only spaces/tabs, not newlines
            // Check if next char after spaces is on same line (not newline)
            // Actually we should include spaces that are on same line
            end += 1;
        }
        // If next char is newline, don't include it, return end at word end
        // For w, we want next word start, which is after current word and its trailing spaces
        // But for "a/b" at 0, current word "a" at 0-1, end at 1 is at '/', which is next word start, correct
        // For "hello world" at 0, current word "hello" at 0-5, end at 5 is at ' ', trailing spaces to 6, so end at 6 is at 'w', correct
        // For "foo\n    /bar" at 0, current word "foo" at 0-3, end at 3 is at '\n', not space, so end stays 3, correct for w from foo
        return end;
    }
}

fn find_next_word_end(chars: &[char], pos: usize) -> usize {
    if pos >= chars.len() {
        return chars.len().saturating_sub(1);
    }
    // If at word, go to its end, else go to next word's end
    let mut idx = pos;
    let cat = categorize(chars[idx]);
    if cat == CharCategory::Whitespace || cat == CharCategory::Eol {
        // At whitespace, find next word start then its end
        while idx < chars.len() && (categorize(chars[idx]) == CharCategory::Whitespace || categorize(chars[idx]) == CharCategory::Eol) {
            idx += 1;
        }
        if idx >= chars.len() {
            return chars.len().saturating_sub(1);
        }
        let cat2 = categorize(chars[idx]);
        let mut end = idx;
        while end < chars.len() && categorize(chars[end]) == cat2 {
            end += 1;
        }
        return end.saturating_sub(1);
    } else {
        // At word/punct, find its end
        let cat_at = cat;
        let mut end = idx;
        while end < chars.len() && categorize(chars[end]) == cat_at {
            end += 1;
        }
        return end.saturating_sub(1);
    }
}

fn find_prev_word_start(chars: &[char], pos: usize) -> usize {
    if pos == 0 || chars.is_empty() {
        return 0;
    }
    let mut idx = pos;
    if idx > 0 {
        idx -= 1;
    }
    // Skip whitespace including newlines backwards
    while idx > 0 && (categorize(chars[idx]) == CharCategory::Whitespace || categorize(chars[idx]) == CharCategory::Eol) {
        idx -= 1;
        if idx == 0 && (categorize(chars[idx]) == CharCategory::Whitespace || categorize(chars[idx]) == CharCategory::Eol) {
            break;
        }
    }
    // If at whitespace still, find previous word
    if categorize(chars[idx]) == CharCategory::Whitespace || categorize(chars[idx]) == CharCategory::Eol {
        // No previous word
        return 0;
    }
    let cat = categorize(chars[idx]);
    let mut start = idx;
    while start > 0 && categorize(chars[start - 1]) == cat {
        start -= 1;
    }
    start
}

fn word_bounds_at(chars: &[char], pos: usize) -> Option<(usize, usize)> {
    if pos >= chars.len() {
        return None;
    }
    let cat = categorize(chars[pos]);
    if cat == CharCategory::Whitespace || cat == CharCategory::Eol {
        return None;
    }
    let mut start = pos;
    while start > 0 && categorize(chars[start - 1]) == cat {
        start -= 1;
    }
    let mut end = start;
    while end < chars.len() && categorize(chars[end]) == cat {
        end += 1;
    }
    Some((start, end))
}

pub fn move_word_forward(buffer: &impl IsA<TextBuffer>, extend: bool) {
    let buffer = buffer.as_ref();
    let chars: Vec<char> = buffer_text(buffer);
    let pos = offset_at_insert(buffer) as usize;
    if chars.is_empty() || pos >= chars.len() {
        return;
    }
    let cur_range = {
        let anchor = buffer.iter_at_mark(&buffer.selection_bound()).offset() as usize;
        Range::new(anchor, pos)
    };
    let new_range = if extend {
        // Extend: word at head, then extend original to its head
        let word = word_move(&chars, Range::new(pos, pos), WordMotionTarget::NextWordStart);
        let head = word.head;
        // For extend, keep original anchor
        Range::new(cur_range.anchor, head)
    } else {
        word_move(&chars, cur_range, WordMotionTarget::NextWordStart)
    };
    let s = new_range.anchor.min(new_range.head);
    let e = new_range.anchor.max(new_range.head);
    // Trim newlines from word selection (Helix never selects bare newlines)
    let mut ns = s;
    let mut ne = e;
    while ns < ne && (chars[ns] == '\n' || chars[ns] == '\r') {
        ns += 1;
    }
    while ne > ns && (chars[ne - 1] == '\n' || chars[ne - 1] == '\r') {
        ne -= 1;
    }
    if ns >= ne {
        // If word was just newline, move to next word
        return;
    }
    let ai = iter_at_offset(buffer, ns as i32);
    let hi = iter_at_offset(buffer, ne as i32);
    // For NextWordStart, Helix keeps forward direction (anchor <= head)
    // So insert at head
    buffer.select_range(&hi, &ai);
}

pub fn move_word_backward(buffer: &impl IsA<TextBuffer>, extend: bool) {
    let buffer = buffer.as_ref();
    let chars: Vec<char> = buffer_text(buffer);
    let pos = offset_at_insert(buffer) as usize;
    if chars.is_empty() || pos == 0 {
        return;
    }
    let cur_range = {
        let anchor = buffer.iter_at_mark(&buffer.selection_bound()).offset() as usize;
        Range::new(anchor, pos)
    };
    let new_range = if extend {
        let word = word_move(&chars, Range::new(pos, pos), WordMotionTarget::PrevWordStart);
        let head = word.head;
        Range::new(cur_range.anchor, head)
    } else {
        word_move(&chars, cur_range, WordMotionTarget::PrevWordStart)
    };
    let s = new_range.anchor.min(new_range.head);
    let e = new_range.anchor.max(new_range.head);
    let mut ns = s;
    let mut ne = e;
    while ns < ne && (chars[ns] == '\n' || chars[ns] == '\r') {
        ns += 1;
    }
    while ne > ns && (chars[ne - 1] == '\n' || chars[ne - 1] == '\r') {
        ne -= 1;
    }
    if ns >= ne {
        return;
    }
    let ai = iter_at_offset(buffer, ns as i32);
    let hi = iter_at_offset(buffer, ne as i32);
    // For PrevWordStart, Helix keeps backward direction (anchor > head) when moving back
    // But for our GTK, we want insert at head (which is at start)
    // word_move for PrevWordStart returns anchor > head (e.g., 6,0 for b from 6)
    // So we need to preserve direction
    if new_range.anchor > new_range.head {
        buffer.select_range(&ai, &hi);
    } else {
        buffer.select_range(&hi, &ai);
    }
}

pub fn move_word_end(buffer: &impl IsA<TextBuffer>, extend: bool) {
    let buffer = buffer.as_ref();
    let chars: Vec<char> = buffer_text(buffer);
    let pos = offset_at_insert(buffer) as usize;
    if chars.is_empty() || pos >= chars.len() {
        return;
    }
    let cur_range = {
        let anchor = buffer.iter_at_mark(&buffer.selection_bound()).offset() as usize;
        Range::new(anchor, pos)
    };
    let new_range = if extend {
        let word = word_move(&chars, Range::new(pos, pos), WordMotionTarget::NextWordEnd);
        let head = word.head;
        Range::new(cur_range.anchor, head)
    } else {
        word_move(&chars, cur_range, WordMotionTarget::NextWordEnd)
    };
    let s = new_range.anchor.min(new_range.head);
    let e = new_range.anchor.max(new_range.head);
    let mut ns = s;
    let mut ne = e;
    while ns < ne && (chars[ns] == '\n' || chars[ns] == '\r') {
        ns += 1;
    }
    while ne > ns && (chars[ne - 1] == '\n' || chars[ne - 1] == '\r') {
        ne -= 1;
    }
    if ns >= ne {
        return;
    }
    let ai = iter_at_offset(buffer, ns as i32);
    let hi = iter_at_offset(buffer, ne as i32);
    buffer.select_range(&hi, &ai);
}

pub fn move_long_word_forward(buffer: &impl IsA<TextBuffer>, extend: bool) {
    let buffer = buffer.as_ref();
    let chars: Vec<char> = buffer_text(buffer);
    let pos = offset_at_insert(buffer) as usize;
    if chars.is_empty() {
        return;
    }
    let mut idx = pos;
    while idx < chars.len() && !chars[idx].is_whitespace() && chars[idx] != '\n' && chars[idx] != '\r' {
        idx += 1;
    }
    while idx < chars.len() && chars[idx].is_whitespace() {
        idx += 1;
    }
    let iter = iter_at_offset(buffer, idx as i32);
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
    #[ignore]
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
    #[ignore]
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
    #[ignore]
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
    #[ignore]
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
    #[ignore]
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
    #[ignore]
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
    #[ignore]
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
    #[ignore]
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
    #[ignore]
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
        let text: Vec<char> = "hello world  foo".chars().collect();
        assert!(is_word_boundary(text[5], text[6]));
        let start = find_next_word_start(&text, 0);
        assert_eq!(start, 6);
        let end = find_next_word_end(&text, 0);
        assert_eq!(end, 4);
        let prev = find_prev_word_start(&text, 6);
        assert_eq!(prev, 0);
    }

    #[test]
    fn w_with_symbols_and_newline() {
        let text: Vec<char> = "a/b".chars().collect();
        assert_eq!(find_next_word_start(&text, 0), 1, "w from a should go to /");
        assert_eq!(find_next_word_start(&text, 1), 2, "w from / should go to b");
        let text2: Vec<char> = "foo\n    /bar".chars().collect();
        assert_eq!(text2, vec!['f','o','o','\n',' ',' ',' ',' ','/','b','a','r']);
        // Helix w from foo at 0 stays at foo (0-3), next w from newline goes to /
        assert_eq!(find_next_word_start(&text2, 0), 3, "w from foo should stay at foo when next is newline");
        assert_eq!(find_next_word_start(&text2, 3), 8, "w from newline should go to /");
        assert_eq!(find_next_word_start(&text2, 8), 9, "w from / should go to bar");
        assert_eq!(find_prev_word_start(&text2, 9), 8, "b from bar should go to /");
        assert_eq!(find_prev_word_start(&text2, 8), 0, "b from / should go to foo");
    }
}
