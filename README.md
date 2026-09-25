# wrapstream

Greedy word wrapping for Rust, with a streaming path that doesn't require
loading the whole input into memory first.

Most text-wrapping code assumes you already have the text as a `String`.
That's fine until the input is a multi-gigabyte log file or an unbounded
pipe, at which point "just read it all in and wrap it" stops being an
option. `wrapstream` gives you an iterator over wrapped lines that reads
from any `std::io::Read` lazily, holding at most one output line and one
in-progress word in memory at a time.

No third-party dependencies. Standard library only.

## Usage

For a string you already have in memory:

```rust
let text = "the quick brown fox jumps over the lazy dog";
let lines = wrapstream::wrap(text, 15);
assert_eq!(lines, vec!["the quick brown", "fox jumps over", "the lazy dog"]);
```

For anything you'd rather not read into memory all at once:

```rust
use std::io::{self, BufReader};
use wrapstream::LineWrapper;

fn print_wrapped(path: &str, width: usize) -> io::Result<()> {
    let file = std::fs::File::open(path)?;
    for line in LineWrapper::new(BufReader::new(file), width) {
        println!("{}", line?);
    }
    Ok(())
}
```

`LineWrapper` works the same way over stdin, a `TcpStream`, or anything
else that implements `Read` - it never buffers more than the current line.

## Current behavior

- Wraps greedily on whitespace, breaking a line as soon as the next word
  would push it past `width` characters.
- Width is measured in `char` count, not display width or bytes.
- A single newline is treated as ordinary whitespace and collapsed like any
  other run of spaces or tabs. A blank line (two or more consecutive
  newlines) is preserved instead: it ends the current paragraph and shows
  up in the output as an empty line, so paragraphs never bleed into each
  other. A run of several blank lines collapses to one empty output line,
  and a blank run at the very start or end of the input produces no empty
  line at all.
- A word longer than `width` is emitted on its own line rather than split.

## License

MIT. See `LICENSE`.
