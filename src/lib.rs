//! Greedy word wrapping, including a streaming path for input that
//! shouldn't be loaded into memory all at once (piped stdin, a large file
//! opened for reading, a socket).
//!
//! For a `&str` you already hold, use [`wrap`]. For anything implementing
//! `std::io::Read`, use [`stream::LineWrapper`], which reads lazily and
//! never buffers more than one line and one word at a time.

pub mod stream;

pub use stream::LineWrapper;

use std::io::Cursor;

/// Wraps `text` into lines of at most `width` characters, breaking on
/// whitespace. This is a convenience built on top of [`LineWrapper`] for
/// callers who already have the whole string in memory; it offers no
/// advantage over that type for large input.
///
/// # Panics
///
/// Panics if `width` is zero.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    LineWrapper::new(Cursor::new(text.as_bytes()), width)
        .collect::<std::io::Result<Vec<_>>>()
        .expect("wrapping an in-memory &str cannot fail")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_a_short_paragraph() {
        let lines = wrap("the quick brown fox jumps over the lazy dog", 15);
        assert_eq!(
            lines,
            vec!["the quick brown", "fox jumps over", "the lazy dog"]
        );
    }

    #[test]
    fn single_word_within_width_is_one_line() {
        assert_eq!(wrap("hello", 80), vec!["hello"]);
    }

    #[test]
    #[should_panic(expected = "width must be greater than zero")]
    fn zero_width_panics() {
        wrap("hello", 0);
    }
}
