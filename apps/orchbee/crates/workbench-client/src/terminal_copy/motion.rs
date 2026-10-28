//! Copy cursor policy over complete validated buffer rows. No input or selection effects.
use super::{
    Class, CopyMotion, Segment, TerminalTextPoint, TerminalTextWindowRow, class, segments,
};
struct Run {
    first: Option<TerminalTextPoint>,
    first_end: u16,
    last: TerminalTextPoint,
    end_col: u16,
    class: Class,
}

fn glyph_head(row: &TerminalTextWindowRow, col: u16) -> u16 {
    row.mappings
        .iter()
        .find(|cell| {
            col > cell.column && u32::from(col) < u32::from(cell.column) + u32::from(cell.width)
        })
        .map_or(col, |cell| cell.column)
}

fn push_run(
    runs: &mut Vec<Run>,
    first: TerminalTextPoint,
    last: TerminalTextPoint,
    end_col: u16,
    class: Class,
) {
    if let Some(prior) = runs.last_mut()
        && prior.class == class
        && prior.first.is_some_and(|p| p.row == first.row)
        && u32::from(prior.end_col) + 1 == u32::from(first.col)
    {
        prior.last = last;
        prior.end_col = end_col;
    } else {
        let first_end = if first.col == last.col {
            end_col
        } else {
            first.col
        };
        runs.push(Run {
            first: Some(first),
            first_end,
            last,
            end_col,
            class,
        });
    }
}
fn word_runs(rows: &[TerminalTextWindowRow], columns: u16, big: bool) -> Vec<Run> {
    let mut runs = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let row_index = row.index as u32;
        segments(row, rows.get(i + 1), columns, |segment| {
            let mut append = |col, last_col, end_col, mut class| {
                if big && class != Class::Blank {
                    class = Class::Word;
                }
                push_run(
                    &mut runs,
                    TerminalTextPoint {
                        row: row_index,
                        col,
                    },
                    TerminalTextPoint {
                        row: row_index,
                        col: last_col,
                    },
                    end_col,
                    class,
                );
            };
            match segment {
                Segment::Text {
                    text,
                    col,
                    width,
                    ascii: true,
                } => {
                    for (offset, _) in text.bytes().enumerate() {
                        let at = col + offset as u16;
                        append(at, at, at, class(&text[offset..offset + 1]));
                    }
                    debug_assert_eq!(text.len(), usize::from(width));
                }
                Segment::Text {
                    text,
                    col,
                    width,
                    ascii: false,
                } => append(col, col, col + width - 1, class(text)),
                Segment::Blank { col, width } => {
                    append(col, col + width - 1, col + width - 1, Class::Blank)
                }
                Segment::Spacer { col } => {
                    let prior = runs.last().map_or(Class::Blank, |run| run.class);
                    push_run(
                        &mut runs,
                        TerminalTextPoint {
                            row: row_index,
                            col,
                        },
                        TerminalTextPoint {
                            row: row_index,
                            col,
                        },
                        col,
                        prior,
                    );
                }
            }
        });
        if !row.wrap {
            runs.push(Run {
                first: None,
                first_end: 0,
                last: TerminalTextPoint {
                    row: row_index,
                    col: 0,
                },
                end_col: 0,
                class: Class::Blank,
            });
        }
    }
    runs
}

pub(crate) fn motion(
    rows: &[TerminalTextWindowRow],
    columns: u16,
    cursor: TerminalTextPoint,
    motion: CopyMotion,
) -> TerminalTextPoint {
    if matches!(motion, CopyMotion::LineEnd | CopyMotion::FirstNonBlank) {
        let Some(row) = rows.iter().find(|row| row.index == u64::from(cursor.row)) else {
            return cursor;
        };
        let mut first = None;
        let mut last = None;
        segments(row, None, columns, |segment| {
            if let Segment::Text {
                text, col, ascii, ..
            } = segment
            {
                if ascii {
                    for (offset, byte) in text.bytes().enumerate() {
                        let at = col + offset as u16;
                        if !byte.is_ascii_whitespace() && first.is_none() {
                            first = Some(at);
                        }
                        if byte != b' ' {
                            last = Some(at);
                        }
                    }
                } else {
                    if text.chars().any(|ch| !ch.is_whitespace()) && first.is_none() {
                        first = Some(col);
                    }
                    if !text.chars().all(|ch| ch == ' ') {
                        last = Some(col);
                    }
                }
            }
        });
        return TerminalTextPoint {
            row: cursor.row,
            col: if motion == CopyMotion::LineEnd {
                last
            } else {
                first
            }
            .unwrap_or(0),
        };
    }
    if matches!(
        motion,
        CopyMotion::PreviousParagraph | CopyMotion::NextParagraph
    ) {
        let blank = |row: &&TerminalTextWindowRow| row.text.chars().all(char::is_whitespace);
        let row = if motion.backward() {
            rows.iter()
                .rev()
                .filter(|row| row.index < u64::from(cursor.row))
                .find(blank)
        } else {
            rows.iter()
                .filter(|row| row.index > u64::from(cursor.row))
                .find(blank)
        };
        return row.map_or(cursor, |row| TerminalTextPoint {
            row: row.index as u32,
            col: glyph_head(row, cursor.col),
        });
    }
    let big = matches!(
        motion,
        CopyMotion::NextBigWordStart
            | CopyMotion::PreviousBigWordStart
            | CopyMotion::NextBigWordEnd
    );
    let runs = word_runs(rows, columns, big);
    let Some(current) = runs.iter().position(|run| {
        run.first.is_some_and(|p| {
            p.row == cursor.row && cursor.col >= p.col && cursor.col <= run.end_col
        })
    }) else {
        return cursor;
    };
    let next_start = matches!(
        motion,
        CopyMotion::NextWordStart | CopyMotion::NextBigWordStart
    );
    let target = if next_start {
        let class = runs[current].class;
        let mut i = current + 1;
        if class != Class::Blank {
            while runs.get(i).is_some_and(|run| run.class == class) {
                i += 1;
            }
        }
        while runs.get(i).is_some_and(|run| run.class == Class::Blank) {
            i += 1;
        }
        runs.get(i).and_then(|run| run.first)
    } else if motion.backward() {
        let run = &runs[current];
        let mut i = if run.class != Class::Blank && cursor.col > run.first_end {
            Some(current)
        } else {
            current.checked_sub(1)
        };
        while i.is_some_and(|i| runs[i].class == Class::Blank) {
            i = i.and_then(|i| i.checked_sub(1));
        }
        i.and_then(|mut i| {
            let class = runs[i].class;
            while i > 0 && runs[i - 1].class == class {
                i -= 1;
            }
            runs[i].first
        })
    } else {
        let mut i = if cursor.col < runs[current].last.col && runs[current].class != Class::Blank {
            current
        } else {
            current + 1
        };
        while runs.get(i).is_some_and(|run| run.class == Class::Blank) {
            i += 1;
        }
        runs.get(i).map(|run| run.class).and_then(|class| {
            while runs.get(i + 1).is_some_and(|run| run.class == class) {
                i += 1;
            }
            runs[i].first.map(|_| runs[i].last)
        })
    };
    target.unwrap_or(cursor)
}
