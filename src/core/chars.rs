//! Character categorization and boundary detection.
//!
//! Mirrors `helix-core/src/chars.rs`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharCategory {
    Whitespace,
    Eol,
    Word,
    Punctuation,
}

pub fn categorize(ch: char) -> CharCategory {
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

pub fn is_word_boundary(a: char, b: char) -> bool {
    categorize(a) != categorize(b)
}

pub fn is_long_word_boundary(a: char, b: char) -> bool {
    match (categorize(a), categorize(b)) {
        (CharCategory::Word, CharCategory::Punctuation)
        | (CharCategory::Punctuation, CharCategory::Word) => false,
        (x, y) if x != y => true,
        _ => false,
    }
}

pub fn char_is_line_ending(ch: char) -> bool {
    ch == '\n' || ch == '\r'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_word() {
        assert_eq!(categorize('a'), CharCategory::Word);
        assert_eq!(categorize('9'), CharCategory::Word);
        assert_eq!(categorize('_'), CharCategory::Word);
        assert_eq!(categorize(' '), CharCategory::Whitespace);
        assert_eq!(categorize('\t'), CharCategory::Whitespace);
        assert_eq!(categorize('\n'), CharCategory::Eol);
        assert_eq!(categorize('\r'), CharCategory::Eol);
        assert_eq!(categorize('!'), CharCategory::Punctuation);
        assert_eq!(categorize(';'), CharCategory::Punctuation);
    }

    #[test]
    fn test_word_boundary_detection() {
        assert!(is_word_boundary('a', ' '));
        assert!(!is_word_boundary('a', 'b'));
        assert!(is_word_boundary(' ', '!'));
        assert!(is_word_boundary('a', '.'));
    }

    #[test]
    fn test_long_word_boundary() {
        assert!(!is_long_word_boundary('a', '.'));
        assert!(!is_long_word_boundary('.', 'a'));
        assert!(is_long_word_boundary('a', ' '));
        assert!(is_long_word_boundary(' ', 'a'));
    }

    #[test]
    fn test_line_ending() {
        assert!(char_is_line_ending('\n'));
        assert!(char_is_line_ending('\r'));
        assert!(!char_is_line_ending(' '));
        assert!(!char_is_line_ending('a'));
    }
}
