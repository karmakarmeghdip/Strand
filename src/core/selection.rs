//! Selection ranges and buffer cursor positioning.
//!
//! Mirrors `helix-core/src/selection.rs`.
//!
//! In Helix and Strand, cursors are selections. Even a single point cursor
//! is represented as a selection range with block-cursor semantics inward from
//! the head.

use gtk::prelude::{IsA, TextBufferExt};
use gtk::TextBuffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Forward,
    Backward,
}

/// A single selection range between an `anchor` and a `head`.
///
/// In block cursor mode:
/// - When `head > anchor` (forward selection), the cursor visually covers the
///   character immediately preceding `head`.
/// - When `head <= anchor` (backward or point selection), the cursor covers
///   the character at `head`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Range {
    pub anchor: usize,
    pub head: usize,
}

impl Range {
    pub fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    pub fn point(head: usize) -> Self {
        Self::new(head, head)
    }

    #[inline]
    pub fn cursor(&self) -> usize {
        if self.head > self.anchor {
            self.head.saturating_sub(1)
        } else {
            self.head
        }
    }

    #[inline]
    pub fn from(&self) -> usize {
        self.anchor.min(self.head)
    }

    #[inline]
    pub fn to(&self) -> usize {
        self.anchor.max(self.head)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.to() - self.from()
    }

    #[inline]
    pub fn direction(&self) -> Direction {
        if self.anchor <= self.head {
            Direction::Forward
        } else {
            Direction::Backward
        }
    }
}

// ---------------------------------------------------------------------------
// Low-level buffer helpers
// ---------------------------------------------------------------------------

pub fn offset_at_insert(buffer: &impl IsA<TextBuffer>) -> i32 {
    let buffer = buffer.as_ref();
    buffer.iter_at_mark(&buffer.get_insert()).offset()
}

pub fn offset_at_selection_bound(buffer: &impl IsA<TextBuffer>) -> i32 {
    let buffer = buffer.as_ref();
    buffer.iter_at_mark(&buffer.selection_bound()).offset()
}

pub fn iter_at_offset(buffer: &impl IsA<TextBuffer>, offset: i32) -> gtk::TextIter {
    let buffer = buffer.as_ref();
    buffer.iter_at_offset(offset)
}

pub fn set_cursor(buffer: &impl IsA<TextBuffer>, iter: &gtk::TextIter, extend: bool) {
    let buffer = buffer.as_ref();
    if extend {
        let anchor = buffer.iter_at_mark(&buffer.selection_bound());
        buffer.select_range(iter, &anchor);
    } else {
        buffer.place_cursor(iter);
    }
}

pub fn current_range(buffer: &impl IsA<TextBuffer>) -> Range {
    let buffer = buffer.as_ref();
    let anchor = buffer.iter_at_mark(&buffer.selection_bound()).offset() as usize;
    let head = buffer.iter_at_mark(&buffer.get_insert()).offset() as usize;
    Range::new(anchor, head)
}

pub fn apply_range(buffer: &impl IsA<TextBuffer>, range: Range) {
    let buffer = buffer.as_ref();
    let head_iter = buffer.iter_at_offset(range.head as i32);
    let anchor_iter = buffer.iter_at_offset(range.anchor as i32);
    buffer.select_range(&head_iter, &anchor_iter);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_properties() {
        let forward = Range::new(2, 6);
        assert_eq!(forward.from(), 2);
        assert_eq!(forward.to(), 6);
        assert_eq!(forward.len(), 4);
        assert_eq!(forward.cursor(), 5);
        assert_eq!(forward.direction(), Direction::Forward);

        let backward = Range::new(6, 2);
        assert_eq!(backward.from(), 2);
        assert_eq!(backward.to(), 6);
        assert_eq!(backward.len(), 4);
        assert_eq!(backward.cursor(), 2);
        assert_eq!(backward.direction(), Direction::Backward);

        let point = Range::point(5);
        assert!(point.is_empty());
        assert_eq!(point.cursor(), 5);
    }
}
