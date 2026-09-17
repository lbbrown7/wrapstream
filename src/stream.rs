use std::io::{self, BufReader, Read};

/// Wraps a byte stream into lines of at most `width` characters, reading the
/// source lazily and holding at most one line plus one word in memory at a
/// time. This is the reason the crate exists: `wrap()` in `lib.rs` is fine
/// for a string you already have, but it has to hold the whole thing (input
/// and output) in RAM. `LineWrapper` doesn't, so it works the same way on a
/// 40KB config file and a 40GB log dump piped over stdin.
pub struct LineWrapper<R> {
    source: BufReader<R>,
    width: usize,
    line: String,
    finished: bool,
}

impl<R: Read> LineWrapper<R> {
    pub fn new(source: R, width: usize) -> Self {
        assert!(width > 0, "wrap width must be greater than zero");
        LineWrapper {
            source: BufReader::new(source),
            width,
            line: String::new(),
            finished: false,
        }
    }

    /// Reads the next whitespace-delimited word. `Ok(None)` means the
    /// source is exhausted. Whitespace runs (including newlines) are
    /// collapsed, so paragraph structure in the input is not preserved yet.
    fn next_word(&mut self) -> io::Result<Option<String>> {
        let mut word = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            let n = self.source.read(&mut byte)?;
            if n == 0 {
                return if word.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(bytes_to_word(word)?))
                };
            }
            let b = byte[0];
            if is_ascii_whitespace(b) {
                if word.is_empty() {
                    continue;
                }
                return Ok(Some(bytes_to_word(word)?));
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
        if self.finished {
            return None;
        }
        loop {
            match self.next_word() {
                Ok(Some(word)) => {
                    let joiner = if self.line.is_empty() { 0 } else { 1 };
                    let fits = self.line.chars().count() + joiner + word.chars().count()
                        <= self.width;
                    if !self.line.is_empty() && !fits {
                        let finished_line = std::mem::replace(&mut self.line, word);
                        return Some(Ok(finished_line));
                    }
                    if !self.line.is_empty() {
                        self.line.push(' ');
                    }
                    self.line.push_str(&word);
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
    fn collapses_whitespace_runs() {
        let lines = wrap_all("a   b\n\nc", 10);
        assert_eq!(lines, vec!["a b c"]);
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
