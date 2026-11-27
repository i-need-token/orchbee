//! Git Output: the commands this process ran for one Workspace and what Git wrote to standard error,
//! in a ring of at most 1 MiB held in memory only and never written to disk. A reader pages through
//! it by byte cursor and learns when the ring dropped text before it was read.
use super::json::escaped_width;
use std::{collections::VecDeque, time::Duration};

const CAPACITY: usize = 1024 * 1024;
/// The most a page's text takes once escaped into JSON, well inside a 128 KiB result.
const PAGE_ESCAPED: usize = 96 * 1024;

pub(super) struct GitOutput {
    bytes: VecDeque<u8>,
    /// The cursor of the first byte held.
    start: u64,
}

/// One read of the ring.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct Page {
    pub(super) text: String,
    /// Where the next read starts.
    pub(super) cursor: u64,
    /// The bytes between the requested cursor and this page's start that the ring dropped before
    /// they were read, with what was left of a character it cut in half.
    pub(super) dropped: u64,
}

impl GitOutput {
    pub(super) fn new() -> Self {
        Self {
            bytes: VecDeque::new(),
            start: 0,
        }
    }

    /// Records one finished command as VS Code's Git log does: `> git <arguments> [<n>ms]`, and
    /// ` (cancelled)` for one this process stopped.
    pub(super) fn command<'a>(
        &mut self,
        arguments: impl IntoIterator<Item = &'a str>,
        took: Duration,
        cancelled: bool,
    ) {
        let mut line = String::from("> git");
        for argument in arguments {
            line.push(' ');
            line.push_str(argument);
        }
        line.push_str(&format!(" [{}ms]", took.as_millis()));
        if cancelled {
            line.push_str(" (cancelled)");
        }
        line.push('\n');
        self.write(line.as_bytes());
    }

    /// Appends what a command wrote, dropping the oldest text past the capacity.
    pub(super) fn write(&mut self, bytes: &[u8]) {
        let bytes = bytes
            .len()
            .checked_sub(CAPACITY)
            .and_then(|skip| bytes.get(skip..))
            .unwrap_or(bytes);
        let excess = (self.bytes.len() + bytes.len()).saturating_sub(CAPACITY);
        self.bytes.drain(..excess);
        self.start += excess as u64;
        self.bytes.extend(bytes);
    }

    /// The text from `cursor`, or from the oldest text held when the cursor is absent or the text
    /// there was dropped. A cursor past the end reads nothing and stays where the text ends. Text
    /// that is not UTF-8 reads with replacement characters, and a page never splits a character.
    pub(super) fn read(&self, cursor: Option<u64>) -> Page {
        let end = self.start + self.bytes.len() as u64;
        let requested = cursor.unwrap_or(self.start);
        let mut from = requested.clamp(self.start, end);
        // A character the ring cut in half, or a cursor inside one, starts at the next character.
        while from < end && self.byte(from).is_some_and(continuation) {
            from += 1;
        }
        let mut until = from;
        let mut escaped = 0;
        while until < end {
            let width = self.byte(until).map_or(1, utf8_width);
            let character = (until..until + width as u64)
                .map_while(|at| (at < end).then(|| self.byte(at)).flatten())
                .collect::<Vec<_>>();
            let cost = String::from_utf8_lossy(&character)
                .chars()
                .map(escaped_width)
                .sum::<usize>();
            if escaped + cost > PAGE_ESCAPED {
                break;
            }
            escaped += cost;
            until += character.len().max(1) as u64;
        }
        let bytes = (from..until)
            .filter_map(|at| self.byte(at))
            .collect::<Vec<_>>();
        Page {
            text: String::from_utf8_lossy(&bytes).into_owned(),
            cursor: until,
            dropped: if requested < self.start {
                from - requested
            } else {
                0
            },
        }
    }

    fn byte(&self, cursor: u64) -> Option<u8> {
        let index = usize::try_from(cursor.checked_sub(self.start)?).ok()?;
        self.bytes.get(index).copied()
    }
}

fn continuation(byte: u8) -> bool {
    byte & 0b1100_0000 == 0b1000_0000
}

/// The bytes the UTF-8 character starting with `byte` takes; one for a byte that starts none.
fn utf8_width(byte: u8) -> usize {
    match byte {
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => 1,
    }
}

#[cfg(test)]
mod tests;
