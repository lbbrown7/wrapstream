use std::collections::VecDeque;
use std::io::{self, BufReader, Read};

/// Wraps a byte stream into lines of at most `width` characters, reading the
/// source lazily and holding at most one line plus one word in memory at a
/// time. This is the reason the crate exists: `wrap()` in `lib.rs` is fine
/// for a string you already have, but it has to hold the whole thing (input
/// and output) in RAM. `LineWrapper` doesn't, so it works the same way on a
/// 40KB config file and a 40GB log dump piped over stdin.
///
/// A blank line in the source (two or more consecutive newlines, possibly
/// with trailing whitespace on the empty line) ends the current paragraph:
/// it is preserved in the output as an empty line, and the words on either
/// side are never joined onto the same wrapped line. Runs of blank lines
/// collapse to a single empty output line, and a blank run at the very
/// start or end of the input produces no empty line at all, since there is
/// no paragraph on that side to separate.
pub struct LineWrapper<R> {
    source: BufReader<R>,
    width: usize,
    line: String,
    /// Output lines ready to hand back before pulling another word from the
    /// source. A paragraph break can produce two lines (the paragraph just
    /// finished, then the blank separator) from a single word read, and the
    /// iterator can only return one per `next()` call.
    queue: VecDeque<String>,
    /// Consecutive newlines seen in the whitespace since the last word
    /// started, carried across `next_word` calls since that whitespace can
    /// span more than one of them.
    newline_run: usize,
    /// Whether any word has been read yet. Used to suppress a paragraph
    /// break at the very start of the input.
    started: bool,
    finished: bool,
}

impl<R: Read> LineWrapper<R> {
    pub fn new(source: R, width: usize) -> Self {
        assert!(width > 0, "wrap width must be greater than zero");
        LineWrapper {
            source: BufReader::new(source),
            width,
            line: String::new(),
            queue: VecDeque::new(),
            newline_run: 0,
            started: false,
            finished: false,
        }
    }

    /// Reads the next whitespace-delimited word. `Ok(None)` means the
    /// source is exhausted. The returned `bool` is true when the whitespace
    /// immediately before the word contained a blank line.
    fn next_word(&mut self) -> io::Result<Option<(String, bool)>> {
        let mut word = Vec::new();
        let mut break_before = false;
        let mut byte = [0u8; 1];
        loop {
            let n = self.source.read(&mut byte)?;
            if n == 0 {
                return if word.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some((bytes_to_word(word)?, break_before)))
                };
            }
            let b = byte[0];
            if is_ascii_whitespace(b) {
                if b == b'\n' {
                    self.newline_run += 1;
                }
                if word.is_empty() {
                    continue;
                }
                return Ok(Some((bytes_to_word(word)?, break_before)));
            }
            if word.is_empty() {
                break_before = self.newline_run >= 2;
                self.newline_run = 0;
            }
            word.push(b);
        }
    }
}

fn bytes_to_word(bytes: Vec<u8>) -> io::Result<String> {
    String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn is_ascii_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

impl<R: Read> Iterator for LineWrapper<R> {
    type Item = io::Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(line) = self.queue.pop_front() {
            return Some(Ok(line));
        }
        if self.finished {
            return None;
        }
        loop {
            match self.next_word() {
                Ok(Some((word, break_before))) => {
                    if break_before && self.started {
                        if !self.line.is_empty() {
                            self.queue.push_back(std::mem::take(&mut self.line));
                        }
                        self.queue.push_back(String::new());
                    }
                    self.started = true;

                    let joiner = if self.line.is_empty() { 0 } else { 1 };
                    let fits = self.line.chars().count() + joiner + word.chars().count()
                        <= self.width;
                    if !self.line.is_empty() && !fits {
                        let finished_line = std::mem::replace(&mut self.line, word);
                        self.queue.push_back(finished_line);
                    } else {
                        if !self.line.is_empty() {
                            self.line.push(' ');
                        }
                        self.line.push_str(&word);
                    }

                    if let Some(line) = self.queue.pop_front() {
                        return Some(Ok(line));
                    }
                }
                Ok(None) => {
                    self.finished = true;
                    if self.line.is_empty() {
                        return None;
                    }
                    return Some(Ok(std::mem::take(&mut self.line)));
                }
                Err(e) => {
                    self.finished = true;
                    return Some(Err(e));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn wrap_all(text: &str, width: usize) -> Vec<String> {
        LineWrapper::new(Cursor::new(text.as_bytes()), width)
            .collect::<io::Result<Vec<_>>>()
            .unwrap()
    }

    #[test]
    fn wraps_on_word_boundaries() {
        let lines = wrap_all("the quick brown fox jumps", 10);
        assert_eq!(lines, vec!["the quick", "brown fox", "jumps"]);
    }

    #[test]
    fn collapses_whitespace_within_a_paragraph() {
        let lines = wrap_all("a   b\t\tc", 10);
        assert_eq!(lines, vec!["a b c"]);
    }

    #[test]
    fn single_newline_is_treated_as_a_space() {
        let lines = wrap_all("a\nb", 10);
        assert_eq!(lines, vec!["a b"]);
    }

    #[test]
    fn blank_line_starts_a_new_paragraph() {
        let lines = wrap_all("hello world\n\nfoo bar", 20);
        assert_eq!(lines, vec!["hello world", "", "foo bar"]);
    }

    #[test]
    fn a_run_of_blank_lines_collapses_to_one_break() {
        let lines = wrap_all("a\n\n\n\nb", 10);
        assert_eq!(lines, vec!["a", "", "b"]);
    }

    #[test]
    fn whitespace_only_blank_line_still_breaks() {
        let lines = wrap_all("a\n   \nb", 10);
        assert_eq!(lines, vec!["a", "", "b"]);
    }

    #[test]
    fn leading_blank_lines_produce_no_leading_break() {
        let lines = wrap_all("\n\nfoo", 10);
        assert_eq!(lines, vec!["foo"]);
    }

    #[test]
    fn trailing_blank_lines_produce_no_trailing_break() {
        let lines = wrap_all("foo\n\n", 10);
        assert_eq!(lines, vec!["foo"]);
    }

    #[test]
    fn empty_input_yields_no_lines() {
        let lines = wrap_all("   \n\t ", 10);
        assert!(lines.is_empty());
    }

    #[test]
    fn word_longer_than_width_gets_its_own_line() {
        let lines = wrap_all("short superlongwordthatdoesnotfit ok", 8);
        assert_eq!(lines, vec!["short", "superlongwordthatdoesnotfit", "ok"]);
    }

    #[test]
    fn memory_use_does_not_grow_with_input_size() {
        // Not a real memory probe, but pulling lines one at a time from a
        // huge source and only ever holding the current line proves the
        // iterator isn't buffering the whole thing up front.
        let huge = "word ".repeat(1_000_000);
        let mut wrapper = LineWrapper::new(Cursor::new(huge.as_bytes()), 20);
        let first = wrapper.next().unwrap().unwrap();
        assert_eq!(first, "word word word word");
    }
}
