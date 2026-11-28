//! rg's `--json` output read as VS Code's `RipgrepParser` reads it at ef5ca45: lines split at line
//! feeds and trimmed, `match` and `context` messages kept and the others skipped, ranges counted
//! against the run's result limit, and columns in UTF-16 code units of each line's text. A line
//! longer than 16 MiB is skipped and counted instead.

use serde::Deserialize;

use super::{
    js,
    preview::{self, Located, PreviewOptions, Range},
};

/// The longest output line read.
pub(super) const MAXIMUM_LINE_BYTES: usize = 16 * 1024 * 1024;

/// One result rg reported, with its path as rg printed it.
pub(super) enum Event {
    Match { path: Text, found: Found },
    Context { path: Text, lines: Vec<ContextLine> },
}

/// A message's text: rg's `text`, or its `bytes` for what is not UTF-8.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Text {
    Utf8(String),
    /// Decoded with replacement characters, as VS Code shows it.
    Bytes(String),
}

impl Text {
    pub(super) fn as_str(&self) -> &str {
        match self {
            Self::Utf8(text) | Self::Bytes(text) => text,
        }
    }
}

/// One matched line's ranges and preview.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Found {
    pub(super) ranges: Vec<Located>,
    pub(super) preview: String,
}

/// One line of context, numbered from 0.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ContextLine {
    pub(super) line: u64,
    pub(super) text: String,
}

/// An output line that is not JSON, which ends the search as VS Code's parser throws.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct Malformed;

/// The parser of one rg run.
pub(super) struct Parser {
    preview: PreviewOptions,
    maximum: u64,
    reported: u64,
    hit_limit: bool,
    done: bool,
    pending: Vec<u8>,
    /// The rest of a line over the bound is being skipped.
    skipping: bool,
    omitted_long_lines: u64,
}

#[derive(Deserialize)]
struct Message {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    data: Option<Data>,
}

#[derive(Deserialize)]
struct Data {
    path: Option<WireText>,
    lines: Option<WireText>,
    line_number: Option<u64>,
    #[serde(default)]
    submatches: Vec<Submatch>,
}

#[derive(Deserialize)]
struct WireText {
    text: Option<String>,
    bytes: Option<String>,
}

#[derive(Deserialize)]
struct Submatch {
    #[serde(rename = "match")]
    matched: WireText,
    start: usize,
    end: usize,
}

impl Parser {
    /// A parser that reports at most `maximum` ranges.
    pub(super) fn new(preview: PreviewOptions, maximum: u64) -> Self {
        Self {
            preview,
            maximum,
            reported: 0,
            hit_limit: false,
            done: false,
            pending: Vec::new(),
            skipping: false,
            omitted_long_lines: 0,
        }
    }

    /// Whether the run reported as many ranges as it may; it reads nothing more.
    pub(super) const fn hit_limit(&self) -> bool {
        self.hit_limit
    }

    /// The lines over the bound so far.
    pub(super) const fn omitted_long_lines(&self) -> u64 {
        self.omitted_long_lines
    }

    /// Reads the next of rg's output and hands `sink` each result it completes. Output after the
    /// limit is ignored, and an unterminated last line is never read, as VS Code's parser does.
    ///
    /// # Errors
    ///
    /// [`Malformed`] for a line that is not JSON.
    pub(super) fn feed(
        &mut self,
        mut bytes: &[u8],
        sink: &mut impl FnMut(Event),
    ) -> Result<(), Malformed> {
        while !self.done && !bytes.is_empty() {
            let Some(end) = bytes.iter().position(|byte| *byte == b'\n') else {
                self.keep(bytes);
                return Ok(());
            };
            self.keep(&bytes[..end]);
            bytes = &bytes[end + 1..];
            if std::mem::take(&mut self.skipping) {
                continue;
            }
            let line = std::mem::take(&mut self.pending);
            self.line(&line, sink)?;
        }
        Ok(())
    }

    fn keep(&mut self, bytes: &[u8]) {
        if self.skipping {
            return;
        }
        if self.pending.len() + bytes.len() > MAXIMUM_LINE_BYTES {
            self.pending = Vec::new();
            self.skipping = true;
            self.omitted_long_lines += 1;
            return;
        }
        self.pending.extend_from_slice(bytes);
    }

    /// `handleLine`: a match or context message becomes an event, any other JSON is skipped, and
    /// what is not JSON, or is `null`, whose type VS Code fails to read, is [`Malformed`]. rg writes UTF-8, which is decoded as Node's decoder
    /// does, with replacement characters.
    fn line(&mut self, line: &[u8], sink: &mut impl FnMut(Event)) -> Result<(), Malformed> {
        let line = String::from_utf8_lossy(line);
        let line = js::trim(&line);
        if line.is_empty() {
            return Ok(());
        }
        let message = match serde_json::from_str::<Message>(line) {
            Ok(message) => message,
            // Valid JSON that is not a message of this shape is skipped unless it is `null` or
            // claims to be a match or a context, whose fields VS Code would fail to read.
            Err(_) => {
                let value: serde_json::Value = serde_json::from_str(line).map_err(|_| Malformed)?;
                let kind = value.get("type").and_then(serde_json::Value::as_str);
                return match kind {
                    Some("match" | "context") => Err(Malformed),
                    _ if value.is_null() => Err(Malformed),
                    _ => Ok(()),
                };
            }
        };
        match message.kind.as_str() {
            "match" => {
                let data = message.data.ok_or(Malformed)?;
                let path = data.path.as_ref().and_then(text).ok_or(Malformed)?;
                let found = self.found(&data).ok_or(Malformed)?;
                sink(Event::Match { path, found });
                if self.hit_limit {
                    self.done = true;
                }
            }
            "context" => {
                let data = message.data.ok_or(Malformed)?;
                let path = data.path.as_ref().and_then(text).ok_or(Malformed)?;
                let lines = context(&data).ok_or(Malformed)?;
                sink(Event::Context { path, lines });
            }
            _ => {}
        }
        Ok(())
    }

    /// `createTextSearchMatch`.
    fn found(&mut self, data: &Data) -> Option<Found> {
        let line_number = i64::try_from(data.line_number?).ok()? - 1;
        let lines = text(data.lines.as_ref()?)?;
        let full_text = lines.as_str();
        let bytes = full_text.as_bytes();
        let submatches: Vec<(usize, usize, String)> = if data.submatches.is_empty() {
            // A match rg reports without submatches covers the first character.
            let first = full_text.encode_utf16().next();
            vec![match first {
                Some(unit) => (0, 1, String::from_utf16_lossy(&[unit])),
                None => (0, 0, String::new()),
            }]
        } else {
            data.submatches
                .iter()
                .map(|submatch| {
                    let matched = text(&submatch.matched)?;
                    Some((submatch.start, submatch.end, matched.as_str().to_owned()))
                })
                .collect::<Option<_>>()?
        };
        let (mut previous_end, mut previous_column, mut previous_line) = (0, 0, line_number);
        let mut ranges = Vec::new();
        for (start, end, matched) in submatches {
            if self.hit_limit {
                break;
            }
            self.reported += 1;
            if self.reported >= self.maximum {
                self.hit_limit = true;
            }
            let between = slice(bytes, previous_end, start);
            let between = String::from_utf8_lossy(between);
            let (between_lines, between_last) = line_stats(&between);
            let start_column = if between_lines > 0 {
                between_last
            } else {
                between_last + previous_column
            };
            let (match_lines, match_last) = line_stats(&matched);
            let start_line = between_lines + previous_line;
            let end_line = match_lines + start_line;
            let end_column = if match_lines > 0 {
                match_last
            } else {
                match_last + start_column
            };
            previous_end = end;
            previous_column = end_column;
            previous_line = end_line;
            ranges.push(Range {
                start_line,
                start_column,
                end_line,
                end_column,
            });
        }
        let (ranges, preview) = preview::preview(full_text, &ranges, self.preview);
        Some(Found { ranges, preview })
    }
}

/// `createTextSearchContexts`: the text without one final line break, a line per line feed.
fn context(data: &Data) -> Option<Vec<ContextLine>> {
    let first = data.line_number?.checked_sub(1)?;
    let text = text(data.lines.as_ref()?)?;
    let text = text.as_str();
    let text = text
        .strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text);
    Some(
        text.split('\n')
            .zip(first..)
            .map(|(text, line)| ContextLine {
                line,
                text: text.to_owned(),
            })
            .collect(),
    )
}

/// `bytesOrTextToString`.
fn text(wire: &WireText) -> Option<Text> {
    match (&wire.bytes, &wire.text) {
        (Some(bytes), _) if !bytes.is_empty() => Some(Text::Bytes(
            String::from_utf8_lossy(&base64(bytes)).into_owned(),
        )),
        (_, Some(text)) => Some(Text::Utf8(text.clone())),
        _ => None,
    }
}

/// Node's lenient base64: characters outside the alphabets are skipped, and padding ends it.
fn base64(encoded: &str) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(encoded.len() / 4 * 3);
    let (mut buffer, mut bits) = (0_u32, 0);
    for byte in encoded.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            _ => continue,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            decoded.push(u8::try_from((buffer >> bits) & 0xFF).unwrap_or_default());
        }
    }
    decoded
}

/// `getNumLinesAndLastNewlineLength`: the line feeds in `text`, and the UTF-16 length after the
/// last.
fn line_stats(text: &str) -> (i64, i64) {
    let lines = text.bytes().filter(|byte| *byte == b'\n').count();
    let last = text.rfind('\n').map_or(text, |at| &text[at + 1..]);
    let length = last.encode_utf16().count();
    (
        i64::try_from(lines).unwrap_or(i64::MAX),
        i64::try_from(length).unwrap_or(i64::MAX),
    )
}

/// Node's `Buffer.slice`: bounds clamped to the buffer, empty when `start` is not before `end`.
fn slice(bytes: &[u8], start: usize, end: usize) -> &[u8] {
    let end = end.min(bytes.len());
    bytes.get(start.min(end)..end).unwrap_or_default()
}
