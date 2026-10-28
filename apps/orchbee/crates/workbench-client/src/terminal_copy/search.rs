//! Literal smart-case search keeps complete glyph boundaries, exact totals and a bounded window.
use super::{
    CopyError, CopySearchDirection, CopySearchResult, MAXIMUM_LOGICAL_BYTES, Segment,
    TerminalTextPoint, TerminalTextRange, TerminalTextWindowRow, segments,
};
use regex::RegexBuilder;
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
fn logical_lines(rows: &[TerminalTextWindowRow], columns: u16) -> Result<Vec<Line>, CopyError> {
    let mut lines = Vec::new();
    let mut line = Line::default();
    let mut bytes = 0;
    for (i, row) in rows.iter().enumerate() {
        let mut failed = false;
        segments(row, rows.get(i + 1), columns, |segment| {
            let (count, body) = match segment {
                Segment::Text {
                    text,
                    col,
                    width,
                    ascii,
                } => (text.len(), Some((text, col, width, ascii))),
                Segment::Blank { col, width } if row.wrap || col + width < columns => {
                    let count = usize::from(width);
                    if count > MAXIMUM_LOGICAL_BYTES - bytes {
                        failed = true;
                        return;
                    }
                    let text = " ".repeat(count);
                    line.append(&text, row.index as u32, col, width, true);
                    bytes += count;
                    return;
                }
                Segment::Blank { .. } | Segment::Spacer { .. } => return,
            };
            if count > MAXIMUM_LOGICAL_BYTES - bytes {
                failed = true;
                return;
            }
            if let Some((text, col, width, ascii)) = body {
                line.append(text, row.index as u32, col, width, ascii);
            }
            bytes += count;
        });
        if failed {
            return Err(CopyError::LimitExceeded);
        }
        if !row.wrap {
            line.trim();
            lines.push(std::mem::take(&mut line));
        }
    }
    if !line.spans.is_empty() {
        lines.push(line);
    }
    Ok(lines)
}

pub fn search(
    rows: &[TerminalTextWindowRow],
    columns: u16,
    query: &str,
    direction: CopySearchDirection,
    cursor: TerminalTextPoint,
    previous: Option<TerminalTextRange>,
) -> Result<CopySearchResult, CopyError> {
    if query.is_empty() {
        return Ok(CopySearchResult {
            matches: Vec::new(),
            total: 0,
            current: None,
            current_global: None,
        });
    }
    let regex = RegexBuilder::new(&regex::escape(query))
        .case_insensitive(!query.chars().any(char::is_uppercase))
        .size_limit(4 * 1024 * 1024)
        .build()
        .map_err(|_| CopyError::PatternTooComplex)?;
    let lines = logical_lines(rows, columns)?;
    let origin = previous.map_or(cursor, |previous| match direction {
        CopySearchDirection::Forward => previous.end,
        CopySearchDirection::Backward => previous.start,
    });
    let matches = || {
        lines.iter().flat_map(|line| {
            regex
                .find_iter(&line.text)
                .filter_map(|found| line.text_match(found.start(), found.end()))
        })
    };
    let mut total = 0_usize;
    let mut target = None;
    for found in matches() {
        match direction {
            CopySearchDirection::Forward if target.is_none() && found.start > origin => {
                target = Some(total)
            }
            CopySearchDirection::Backward if found.end < origin => target = Some(total),
            _ => {}
        }
        total += 1;
    }
    if total == 0 {
        return Ok(CopySearchResult {
            matches: Vec::new(),
            total: 0,
            current: None,
            current_global: None,
        });
    }
    let target = target.unwrap_or(match direction {
        CopySearchDirection::Forward => 0,
        CopySearchDirection::Backward => total - 1,
    });
    let retained = total.min(1024);
    let start = target.saturating_sub(retained / 2).min(total - retained);
    let result = matches().skip(start).take(retained).collect();
    Ok(CopySearchResult {
        matches: result,
        total: total as u64,
        current: Some(target - start),
        current_global: Some(target as u64),
    })
}
