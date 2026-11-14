//! Literal search keeps complete glyph boundaries, exact totals and a bounded window; the caller
//! chooses smart, insensitive or sensitive case. Rows arrive in pages that are searched and dropped
//! in order, so a scan holds one page, the logical line still open and the retained matches.
use super::{
    CopyError, CopySearchCase, CopySearchDirection, CopySearchResult, MAXIMUM_LOGICAL_BYTES,
    Segment, TerminalTextPoint, TerminalTextRange, TerminalTextWindowRow, segments,
};
use regex::{Regex, RegexBuilder};
use std::collections::VecDeque;

/// Matches kept around the active one; the total stays exact.
const RETAINED: usize = 1024;

struct Span {
    from: usize,
    to: usize,
    start: TerminalTextPoint,
    end: TerminalTextPoint,
    ascii: bool,
}
#[derive(Default)]
struct Line {
    text: String,
    spans: Vec<Span>,
}
impl Line {
    fn append(&mut self, text: &str, row: u32, col: u16, width: u16, ascii: bool) {
        let from = self.text.len();
        self.text.push_str(text);
        self.spans.push(Span {
            from,
            to: self.text.len(),
            start: TerminalTextPoint { row, col },
            end: TerminalTextPoint {
                row,
                col: col + width - 1,
            },
            ascii,
        });
    }
    fn trim(&mut self) {
        let length = self.text.trim_end().len();
        while self.spans.last().is_some_and(|span| span.from >= length) {
            self.spans.pop();
        }
        if let Some(span) = self.spans.last_mut()
            && span.to > length
            && span.ascii
        {
            span.to = length;
            span.end.col = span.start.col + (length - span.from - 1) as u16;
        }
        self.text.truncate(length);
    }
    fn clear(&mut self) {
        self.text.clear();
        self.spans.clear();
    }
    fn text_match(&self, from: usize, to: usize) -> Option<TerminalTextRange> {
        let first = self
            .spans
            .get(self.spans.partition_point(|span| span.to <= from))?;
        let last = self
            .spans
            .get(self.spans.partition_point(|span| span.to < to))?;
        if from < first.from
            || to <= last.from
            || to > last.to
            || (!first.ascii && from != first.from)
            || (!last.ascii && to != last.to)
        {
            return None;
        }
        Some(TerminalTextRange {
            start: if first.ascii {
                TerminalTextPoint {
                    row: first.start.row,
                    col: first.start.col + (from - first.from) as u16,
                }
            } else {
                first.start
            },
            end: if last.ascii {
                TerminalTextPoint {
                    row: last.start.row,
                    col: last.start.col + (to - last.from - 1) as u16,
                }
            } else {
                last.end
            },
        })
    }
}

/// Joins physical rows into logical lines across page edges. A row's last cell depends on the row
/// after it (a wide-glyph spacer), so the last row of each page waits for the next page.
struct Lines {
    columns: u16,
    line: Line,
    bytes: usize,
    held: Option<TerminalTextWindowRow>,
}

impl Lines {
    fn new(columns: u16) -> Self {
        Self {
            columns,
            line: Line::default(),
            bytes: 0,
            held: None,
        }
    }

    fn page(
        &mut self,
        rows: &[TerminalTextWindowRow],
        emit: &mut impl FnMut(&Line),
    ) -> Result<(), CopyError> {
        let Some(last) = rows.last() else {
            return Ok(());
        };
        if let Some(held) = self.held.take() {
            self.row(&held, rows.first(), emit)?;
        }
        for pair in rows.windows(2) {
            if let [row, next] = pair {
                self.row(row, Some(next), emit)?;
            }
        }
        self.held = Some(last.clone());
        Ok(())
    }

    fn finish(mut self, emit: &mut impl FnMut(&Line)) -> Result<(), CopyError> {
        if let Some(held) = self.held.take() {
            self.row(&held, None, emit)?;
        }
        if !self.line.spans.is_empty() {
            emit(&self.line);
        }
        Ok(())
    }

    /// Appends one row; a row without a soft wrap closes its logical line. One logical line holds
    /// at most `MAXIMUM_LOGICAL_BYTES` of text.
    fn row(
        &mut self,
        row: &TerminalTextWindowRow,
        next: Option<&TerminalTextWindowRow>,
        emit: &mut impl FnMut(&Line),
    ) -> Result<(), CopyError> {
        let columns = self.columns;
        let line = &mut self.line;
        let bytes = &mut self.bytes;
        let mut failed = false;
        segments(row, next, columns, |segment| {
            let (count, body) = match segment {
                Segment::Text {
                    text,
                    col,
                    width,
                    ascii,
                } => (text.len(), Some((text, col, width, ascii))),
                Segment::Blank { col, width } if row.wrap || col + width < columns => {
                    let count = usize::from(width);
                    if count > MAXIMUM_LOGICAL_BYTES - *bytes {
                        failed = true;
                        return;
                    }
                    let text = " ".repeat(count);
                    line.append(&text, row.index as u32, col, width, true);
                    *bytes += count;
                    return;
                }
                Segment::Blank { .. } | Segment::Spacer { .. } => return,
            };
            if count > MAXIMUM_LOGICAL_BYTES - *bytes {
                failed = true;
                return;
            }
            if let Some((text, col, width, ascii)) = body {
                line.append(text, row.index as u32, col, width, ascii);
            }
            *bytes += count;
        });
        if failed {
            return Err(CopyError::LimitExceeded);
        }
        if !row.wrap {
            self.line.trim();
            emit(&self.line);
            self.line.clear();
            self.bytes = 0;
        }
        Ok(())
    }
}

/// Counts matches in order and keeps only what the result can need: the first and the latest
/// `RETAINED` matches, and the window around the active match once that window is complete.
struct Tally {
    direction: CopySearchDirection,
    origin: TerminalTextPoint,
    total: usize,
    target: Option<usize>,
    head: Vec<TerminalTextRange>,
    recent: VecDeque<TerminalTextRange>,
    around: Option<(usize, Vec<TerminalTextRange>)>,
}

impl Tally {
    fn new(direction: CopySearchDirection, origin: TerminalTextPoint) -> Self {
        Self {
            direction,
            origin,
            total: 0,
            target: None,
            head: Vec::new(),
            recent: VecDeque::new(),
            around: None,
        }
    }

    fn record(&mut self, found: TerminalTextRange) {
        let index = self.total;
        let target = match self.direction {
            CopySearchDirection::Forward => self.target.is_none() && found.start > self.origin,
            CopySearchDirection::Backward => found.end < self.origin,
        };
        if target {
            self.target = Some(index);
            self.around = None;
        }
        if self.head.len() < RETAINED {
            self.head.push(found);
        }
        if self.recent.len() == RETAINED {
            self.recent.pop_front();
        }
        self.recent.push_back(found);
        self.total += 1;
        // Here the latest RETAINED matches hold half a window on each side of the target.
        if self
            .target
            .is_some_and(|target| target >= RETAINED / 2 && index == target + RETAINED / 2 - 1)
        {
            self.around = Some((
                index + 1 - self.recent.len(),
                self.recent.iter().copied().collect(),
            ));
        }
    }

    fn finish(self) -> Result<CopySearchResult, CopyError> {
        let total = self.total;
        if total == 0 {
            return Ok(CopySearchResult {
                matches: Vec::new(),
                total: 0,
                current: None,
                current_global: None,
            });
        }
        let target = self.target.unwrap_or(match self.direction {
            CopySearchDirection::Forward => 0,
            CopySearchDirection::Backward => total - 1,
        });
        let retained = total.min(RETAINED);
        let start = target.saturating_sub(retained / 2).min(total - retained);
        // The window starts at the first match (head), ends at the last one (recent), or is
        // centred on a target followed by a full half window, which `around` captured.
        let matches = if let Some(window) = self.head.get(start..start + retained) {
            window.to_vec()
        } else if let Some(skip) = start.checked_sub(total - self.recent.len()) {
            self.recent.into_iter().skip(skip).take(retained).collect()
        } else if let Some((_, window)) = self.around.filter(|(first, _)| *first == start) {
            window
        } else {
            return Err(CopyError::LimitExceeded);
        };
        Ok(CopySearchResult {
            matches,
            total: total as u64,
            current: Some(target - start),
            current_global: Some(target as u64),
        })
    }
}

/// One search over consecutive pages of rows, from the first retained row to the last. Its result
/// equals [`search`] over the same rows in one page.
pub struct SearchScan {
    regex: Option<Regex>,
    lines: Lines,
    tally: Tally,
}

impl SearchScan {
    /// # Errors
    ///
    /// `PatternTooComplex` when the literal pattern cannot be compiled within its size bound.
    pub fn new(
        columns: u16,
        query: &str,
        case: CopySearchCase,
        direction: CopySearchDirection,
        cursor: TerminalTextPoint,
        previous: Option<TerminalTextRange>,
    ) -> Result<Self, CopyError> {
        let regex = if query.is_empty() {
            None
        } else {
            Some(
                RegexBuilder::new(&regex::escape(query))
                    .case_insensitive(case.insensitive_for(query))
                    .size_limit(4 * 1024 * 1024)
                    .build()
                    .map_err(|_| CopyError::PatternTooComplex)?,
            )
        };
        let origin = previous.map_or(cursor, |previous| match direction {
            CopySearchDirection::Forward => previous.end,
            CopySearchDirection::Backward => previous.start,
        });
        Ok(Self {
            regex,
            lines: Lines::new(columns),
            tally: Tally::new(direction, origin),
        })
    }

    /// Whether the scan needs rows; an empty query matches nothing without a read.
    pub const fn reads_rows(&self) -> bool {
        self.regex.is_some()
    }

    /// Searches the next page, which continues the rows of the page before it.
    ///
    /// # Errors
    ///
    /// `LimitExceeded` when one logical line outgrows its text bound.
    pub fn page(&mut self, rows: &[TerminalTextWindowRow]) -> Result<(), CopyError> {
        let Some(regex) = &self.regex else {
            return Ok(());
        };
        let tally = &mut self.tally;
        self.lines
            .page(rows, &mut |line: &Line| record_matches(regex, line, tally))
    }

    /// Closes the last logical line and settles the total, the active match and its window.
    ///
    /// # Errors
    ///
    /// `LimitExceeded` when the last logical line outgrows its text bound.
    pub fn finish(self) -> Result<CopySearchResult, CopyError> {
        let Self {
            regex,
            lines,
            mut tally,
        } = self;
        if let Some(regex) = &regex {
            lines.finish(&mut |line: &Line| record_matches(regex, line, &mut tally))?;
        }
        tally.finish()
    }
}

fn record_matches(regex: &Regex, line: &Line, tally: &mut Tally) {
    for found in regex
        .find_iter(&line.text)
        .filter_map(|found| line.text_match(found.start(), found.end()))
    {
        tally.record(found);
    }
}

/// Searches rows that are all in hand, as one page.
///
/// # Errors
///
/// `PatternTooComplex` for an uncompilable pattern, `LimitExceeded` for an oversized logical line.
pub fn search(
    rows: &[TerminalTextWindowRow],
    columns: u16,
    query: &str,
    case: CopySearchCase,
    direction: CopySearchDirection,
    cursor: TerminalTextPoint,
    previous: Option<TerminalTextRange>,
) -> Result<CopySearchResult, CopyError> {
    let mut scan = SearchScan::new(columns, query, case, direction, cursor, previous)?;
    scan.page(rows)?;
    scan.finish()
}
