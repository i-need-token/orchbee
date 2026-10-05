//! Bounded complete-row collection shared by CLI and native copy-motion adapters.

use std::ops::Range;

use orchbee_terminal_protocol::TerminalTextWindowRow;

use super::{CopyMotion, TerminalTextPoint, motion};

/// Collect only the complete physical-row context required to settle one motion.
///
/// The injected reader owns transport and route validation. This planner preserves the existing
/// one-row line motions, bounded paragraph context, directional expansion, join validation, and
/// 65,536-row assembled limit without owning viewer or process state.
pub(crate) fn read_motion_with<E>(
    columns: u16,
    total: u64,
    cursor: TerminalTextPoint,
    kind: CopyMotion,
    mut read_rows: impl FnMut(Range<u64>) -> Result<Vec<TerminalTextWindowRow>, E>,
    mut limit: impl FnMut() -> E,
    mut invalid_join: impl FnMut() -> E,
) -> Result<TerminalTextPoint, E> {
    let row = u64::from(cursor.row);
    if matches!(kind, CopyMotion::LineEnd | CopyMotion::FirstNonBlank) {
        let rows = read_rows(row..row + 1)?;
        return Ok(motion(&rows, columns, cursor, kind));
    }
    let paragraph = matches!(
        kind,
        CopyMotion::PreviousParagraph | CopyMotion::NextParagraph
    );
    let (minimum, maximum) = if paragraph {
        (row.saturating_sub(999), row.saturating_add(1000).min(total))
    } else {
        (0, total)
    };
    let backward = kind.backward();
    let mut start = if backward {
        row.saturating_sub(63).max(minimum)
    } else {
        row
    };
    let mut end = if backward {
        row + 1
    } else {
        row.saturating_add(64).min(maximum)
    };
    let mut rows = read_rows(start..end)?;
    loop {
        let target = motion(&rows, columns, cursor, kind);
        let edge = if backward {
            start == minimum
        } else {
            end == maximum
        };
        let cut_start = !paragraph
            && backward
            && rows
                .first()
                .is_some_and(|first| first.wrap_continuation && target.row as u64 == first.index);
        let cut_end = matches!(kind, CopyMotion::NextWordEnd | CopyMotion::NextBigWordEnd)
            && rows
                .last()
                .is_some_and(|last| last.wrap && target.row as u64 == last.index);
        if edge || (target != cursor && !cut_start && !cut_end) {
            return Ok(target);
        }
        let remaining = 65_536_usize.saturating_sub(rows.len()) as u64;
        let extension = (rows.len() as u64).min(remaining);
        if extension == 0 {
            return Err(limit());
        }
        if backward {
            let next = start.saturating_sub(extension).max(minimum);
            let mut incoming = read_rows(next..start)?;
            if let (Some(prior), Some(after)) = (incoming.last(), rows.first())
                && prior.wrap != after.wrap_continuation
            {
                return Err(invalid_join());
            }
            incoming.extend(rows);
            rows = incoming;
            start = next;
        } else {
            let next = end.saturating_add(extension).min(maximum);
            let incoming = read_rows(end..next)?;
            if let (Some(prior), Some(after)) = (rows.last(), incoming.first())
                && prior.wrap != after.wrap_continuation
            {
                return Err(invalid_join());
            }
            rows.extend(incoming);
            end = next;
        }
    }
}
