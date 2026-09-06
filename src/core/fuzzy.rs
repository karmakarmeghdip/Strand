use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};

/// Result of a fuzzy match on an item, containing its score and matched character indices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyMatchResult<T> {
    pub item: T,
    pub score: u32,
    pub indices: Vec<u32>,
}

/// Fast fuzzy matcher backed by `nucleo`.
pub struct FuzzyMatcher {
    matcher: Matcher,
    char_buf: Vec<char>,
    indices_buf: Vec<u32>,
}

impl Default for FuzzyMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzyMatcher {
    pub fn new() -> Self {
        Self {
            matcher: Matcher::default(),
            char_buf: Vec::new(),
            indices_buf: Vec::new(),
        }
    }

    /// Reset configuration for matching file paths or generic strings.
    fn configure(&mut self, is_path: bool) {
        self.matcher.config = Config::DEFAULT;
        if is_path {
            self.matcher.config.set_match_paths();
        }
    }

    /// Fuzzy match a single string candidate against a query pattern.
    /// Returns `Some((score, indices))` if matched, `None` otherwise.
    pub fn fuzzy_match(
        &mut self,
        query: &str,
        candidate: &str,
        is_path: bool,
    ) -> Option<(u32, Vec<u32>)> {
        if query.is_empty() {
            return Some((0, Vec::new()));
        }

        self.configure(is_path);
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let haystack = Utf32Str::new(candidate, &mut self.char_buf);

        let score = pattern.score(haystack, &mut self.matcher)?;
        self.indices_buf.clear();
        pattern.indices(haystack, &mut self.matcher, &mut self.indices_buf);
        self.indices_buf.sort_unstable();
        self.indices_buf.dedup();

        Some((score, self.indices_buf.clone()))
    }

    /// Filter and sort a collection of items according to fuzzy match score (descending).
    /// Items with equal score retain stable order.
    pub fn filter_and_sort<T: Clone>(
        &mut self,
        query: &str,
        items: &[T],
        key_fn: impl Fn(&T) -> &str,
        is_path: bool,
    ) -> Vec<FuzzyMatchResult<T>> {
        if query.is_empty() {
            return items
                .iter()
                .cloned()
                .map(|item| FuzzyMatchResult {
                    item,
                    score: 0,
                    indices: Vec::new(),
                })
                .collect();
        }

        self.configure(is_path);
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);

        let mut results = Vec::new();

        for item in items {
            let key = key_fn(item);
            let haystack = Utf32Str::new(key, &mut self.char_buf);
            if let Some(score) = pattern.score(haystack, &mut self.matcher) {
                self.indices_buf.clear();
                pattern.indices(haystack, &mut self.matcher, &mut self.indices_buf);
                self.indices_buf.sort_unstable();
                self.indices_buf.dedup();

                results.push(FuzzyMatchResult {
                    item: item.clone(),
                    score,
                    indices: self.indices_buf.clone(),
                });
            }
        }

        // Higher scores first
        results.sort_by_key(|b| std::cmp::Reverse(b.score));
        results
    }
}

/// Format `text` with Pango markup, wrapping matched character indices with `<b>` and `</b>`.
/// Escapes XML/Pango entities (`&`, `<`, `>`, `'`, `"`).
pub fn highlight_pango_markup(text: &str, indices: &[u32]) -> String {
    highlight_pango_markup_with_tags(text, indices, "<b>", "</b>")
}

/// Format `text` with custom opening and closing tags around matched character indices.
pub fn highlight_pango_markup_with_tags(
    text: &str,
    indices: &[u32],
    tag_open: &str,
    tag_close: &str,
) -> String {
    if indices.is_empty() {
        return escape_pango(text);
    }

    let mut result = String::with_capacity(
        text.len() + indices.len() * (tag_open.len() + tag_close.len()),
    );
    let mut indices_iter = indices.iter().copied();
    let mut next_idx = indices_iter.next();

    for (char_idx, ch) in text.chars().enumerate() {
        let is_highlight = next_idx == Some(char_idx as u32);
        if is_highlight {
            next_idx = indices_iter.next();
            result.push_str(tag_open);
        }
        match ch {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '\'' => result.push_str("&apos;"),
            '"' => result.push_str("&quot;"),
            other => result.push(other),
        }
        if is_highlight {
            result.push_str(tag_close);
        }
    }

    result
}

/// Escape text for Pango markup.
pub fn escape_pango(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '\'' => result.push_str("&apos;"),
            '"' => result.push_str("&quot;"),
            other => result.push(other),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match_basic() {
        let mut matcher = FuzzyMatcher::new();
        let res = matcher.fuzzy_match("ct", "Cargo.toml", true);
        assert!(res.is_some());
        let (score, indices) = res.unwrap();
        assert!(score > 0);
        assert_eq!(indices, vec![0, 6]); // 'C' and 't'
    }

    #[test]
    fn test_fuzzy_match_empty_query() {
        let mut matcher = FuzzyMatcher::new();
        let res = matcher.fuzzy_match("", "Cargo.toml", true);
        assert_eq!(res, Some((0, Vec::new())));
    }

    #[test]
    fn test_filter_and_sort() {
        let items = vec![
            "src/main.rs".to_string(),
            "src/app.rs".to_string(),
            "src/components/editor/controller.rs".to_string(),
            "Cargo.toml".to_string(),
        ];

        let mut matcher = FuzzyMatcher::new();
        let results = matcher.filter_and_sort("app", &items, |s| s.as_str(), true);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item, "src/app.rs");
        assert!(!results[0].indices.is_empty());

        let results_rs = matcher.filter_and_sort("rs", &items, |s| s.as_str(), true);
        assert_eq!(results_rs.len(), 3);
    }

    #[test]
    fn test_pango_markup_highlighting() {
        let text = "Cargo.toml";
        let indices = vec![0, 6];
        let markup = highlight_pango_markup(text, &indices);
        assert_eq!(markup, "<b>C</b>argo.<b>t</b>oml");

        // Test escaping
        let text_with_symbols = "foo <bar> & baz";
        let indices = vec![0, 5]; // 'f' and 'b'
        let markup = highlight_pango_markup(text_with_symbols, &indices);
        assert_eq!(markup, "<b>f</b>oo &lt;<b>b</b>ar&gt; &amp; baz");
    }

    #[test]
    fn test_benchmarking_10000_strings() {
        let mut items = Vec::with_capacity(10_000);
        for i in 0..10_000 {
            items.push(format!("src/module_{}/component_{}/subfile_{}.rs", i % 50, i % 100, i));
        }

        let mut matcher = FuzzyMatcher::new();
        let start = std::time::Instant::now();
        let results = matcher.filter_and_sort("comp25sub425", &items, |s| s.as_str(), true);
        let duration = start.elapsed();

        assert!(!results.is_empty());
        println!("Fuzzy matched 10,000 items in {:?}", duration);
        // Ensure high performance
        assert!(duration.as_millis() < 50, "Matching took too long: {:?}", duration);
    }
}
