//! Codex renders a timed usage error as a red history entry and puts the cursor
//! back at the empty composer. Only the latest visible entry can arm a resume.
use orchbee_local_runtime::{LocalClock, local_clock, unix_milliseconds_at_local};
use orchbee_terminal_protocol::{
    TerminalTextCell, TerminalTextFrame, TerminalTextRow, terminal_palette_index,
};

const FAINT: u16 = 1 << 2;
const EMPTY_COMPOSER: &str = "Ask Codex to do anything";
const MAX_NOTICE_ROWS: usize = 12;
const MAX_FOOTER_ROWS: usize = 6;
const MINUTE_MS: u64 = 60_000;
const MAX_WAIT_MS: u64 = 8 * 24 * 60 * MINUTE_MS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RetryTime {
    date: Option<(u16, u16, u16)>,
    hour: u16,
    minute: u16,
}

pub(super) enum Composer {
    Clear,
    Quota(RetryTime),
}

impl RetryTime {
    /// Codex prints the host-local time to the minute. Wait through that minute so
    /// the time it rounded down has passed. A time without a date means today.
    pub(super) fn deadline(self, now: u64, observed_before: u64) -> Option<u64> {
        let (year, month, day) = match self.date {
            Some(date) => date,
            None => {
                let today = local_clock(now)?;
                let since = local_clock(observed_before)?;
                if (since.year, since.month, since.day) != (today.year, today.month, today.day) {
                    return None;
                }
                (today.year, today.month, today.day)
            }
        };
        let minute = unix_milliseconds_at_local(LocalClock {
            year,
            month,
            day,
            hour: self.hour,
            minute: self.minute,
        })?;
        let deadline = minute.checked_add(MINUTE_MS)?;
        (deadline > now && deadline - now <= MAX_WAIT_MS).then_some(deadline)
    }
}

pub(super) fn composer(frame: &TerminalTextFrame) -> Option<Composer> {
    // Codex renders its conversation in either terminal buffer.
    if frame.route.base_revision.is_some()
        || !frame.cursor.visible
        || !frame.interaction.bracketed_paste()
    {
        return None;
    }
    let composer = usize::from(frame.cursor.row);
    let (above, rest) = frame.rows.split_at_checked(composer)?;
    let (input, below) = rest.split_first()?;
    composer_rows(above, input, below, usize::from(frame.cursor.column))
}

/// The current turn's visible interrupt hint. A working title can lag behind the
/// transcript, so it does not by itself prove that a new turn replaced a notice.
pub(super) fn active_turn(frame: &TerminalTextFrame) -> bool {
    if frame.route.base_revision.is_some()
        || !frame.cursor.visible
        || !frame.interaction.bracketed_paste()
    {
        return false;
    }
    let Some(above) = frame.rows.get(..usize::from(frame.cursor.row)) else {
        return false;
    };
    latest_entry(above).is_some_and(|entry| {
        let Some(first) = entry.first() else {
            return false;
        };
        let line = row_text(first);
        let Some((label, duration)) = line
            .trim_end()
            .split_once("s • esc to interrupt)")
            .and_then(|(prefix, _)| prefix.rsplit_once(" ("))
        else {
            return false;
        };
        !label.trim().is_empty()
            && duration
                .split_ascii_whitespace()
                .last()
                .is_some_and(|seconds| digits(seconds).is_some())
    })
}

fn composer_rows(
    above: &[TerminalTextRow],
    input: &TerminalTextRow,
    below: &[TerminalTextRow],
    cursor: usize,
) -> Option<Composer> {
    if !empty_composer(input, cursor) || !normal_footer(below) {
        return None;
    }
    let entry = latest_entry(above)?;
    let [first, ..] = entry else {
        return None;
    };
    let lead = first_text_cell(first)?;
    if matches!(terminal_palette_index(lead.style.foreground), Some(1 | 9)) {
        if lead.text.as_ref() != "■" || entry.len() > MAX_NOTICE_ROWS {
            return None;
        }
        let message = entry
            .iter()
            .map(row_text)
            .map(|row| row.trim().to_owned())
            .collect::<Vec<_>>()
            .join(" ");
        return retry_time(&message).map(Composer::Quota);
    }
    // An empty spacer or the tail of a clipped/error entry cannot establish a
    // new baseline. Require a visible Codex message marker or welcome card.
    (matches!(lead.text.as_ref(), "›" | "»" | "•" | "◦")
        || entry
            .iter()
            .take(MAX_NOTICE_ROWS)
            .any(|row| row_text(row).contains(">_ OpenAI Codex (v")))
    .then_some(Composer::Clear)
}

fn latest_entry(above: &[TerminalTextRow]) -> Option<&[TerminalTextRow]> {
    let last = above.iter().rposition(|row| !blank_row(row))?;
    let first = above[..last]
        .iter()
        .rposition(blank_row)
        .map_or(0, |blank| blank + 1);
    Some(&above[first..=last])
}

fn blank_cell(cell: &TerminalTextCell) -> bool {
    cell.text.trim().is_empty()
}

fn blank_row(row: &TerminalTextRow) -> bool {
    row.cells.iter().all(blank_cell)
}

fn first_text_cell(row: &TerminalTextRow) -> Option<&TerminalTextCell> {
    row.cells.iter().find(|cell| !blank_cell(cell))
}

fn row_text(row: &TerminalTextRow) -> String {
    cells_text(&row.cells)
}

fn cells_text(cells: &[TerminalTextCell]) -> String {
    let mut text = String::new();
    for cell in cells.iter().filter(|cell| cell.width != 2) {
        text.push_str(if cell.text.is_empty() {
            " "
        } else {
            &cell.text
        });
    }
    text
}

fn empty_composer(row: &TerminalTextRow, cursor: usize) -> bool {
    let Some(marker) = cursor.checked_sub(2).filter(|marker| *marker <= 1) else {
        return false;
    };
    let Some((gutter, cells)) = row.cells.split_at_checked(marker) else {
        return false;
    };
    let [prompt, space, body @ ..] = cells else {
        return false;
    };
    gutter.iter().all(blank_cell)
        && matches!(prompt.text.as_ref(), "›" | "»")
        && blank_cell(space)
        && cells_text(body).trim_end() == EMPTY_COMPOSER
        && body
            .iter()
            .all(|cell| blank_cell(cell) || cell.style.flags & FAINT != 0)
}

fn normal_footer(rows: &[TerminalTextRow]) -> bool {
    let mut rows = rows.iter().enumerate().filter(|(_, row)| !blank_row(row));
    let Some((mut previous_index, mut previous)) = rows.next() else {
        return true;
    };
    let status = row_text(previous);
    if shortcut_row(&status) {
        return rows.next().is_none();
    }
    if !model_status(&status) {
        return false;
    }
    for (offset, (index, row)) in (&mut rows).enumerate() {
        let line = row_text(row);
        if shortcut_row(&line) {
            return rows.next().is_none();
        }
        if offset >= MAX_FOOTER_ROWS - 1
            || index != previous_index + 1
            || !line.starts_with("  ")
            || !continues_wrapped_row(previous, row)
        {
            return false;
        }
        (previous_index, previous) = (index, row);
    }
    true
}

fn shortcut_row(line: &str) -> bool {
    let Some(line) = line.strip_prefix("  ") else {
        return false;
    };
    let line = line.trim_end();
    let (hints, warnings) = match line.split_once("  ") {
        Some((hints, rest)) => (hints, Some(rest.trim_start())),
        None => (line, None),
    };
    matches!(
        hints,
        "? for shortcuts" | "← for agents · ? for shortcuts" | "← for agents"
    ) && warnings.is_none_or(warnings_hint)
}

/// Codex right-aligns its retained warnings on the shortcut row, for example
/// `⚠ 7 warnings · f2 to view`.
fn warnings_hint(text: &str) -> bool {
    let Some(text) = text.strip_prefix('⚠') else {
        return false;
    };
    let text = text.strip_prefix('\u{fe0f}').unwrap_or(text);
    let fields: Vec<_> = text.split_ascii_whitespace().collect();
    matches!(
        fields.as_slice(),
        [count, "warning" | "warnings", "·", _key, "to", "view"] if digits(count).is_some()
    )
}

fn model_status(line: &str) -> bool {
    let Some(line) = line.strip_prefix("  ") else {
        return false;
    };
    let model = line.split_once(" · ").map_or(line, |(model, _)| model);
    let mut fields = model.split_ascii_whitespace();
    let Some(name) = fields.next() else {
        return false;
    };
    let Some(version) = name
        .strip_prefix("GPT-")
        .or_else(|| name.strip_prefix("gpt-"))
    else {
        return false;
    };
    version.starts_with(|ch: char| ch.is_ascii_digit())
        && version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        && fields.all(|field| {
            matches!(
                field,
                "default"
                    | "none"
                    | "minimal"
                    | "low"
                    | "medium"
                    | "high"
                    | "xhigh"
                    | "max"
                    | "ultra"
                    | "fast"
            )
        })
}

/// A wrapped status uses the continuation only if its first word could not fit
/// in the space that remained on the preceding row.
fn continues_wrapped_row(previous: &TerminalTextRow, next: &TerminalTextRow) -> bool {
    let Some(last) = previous.cells.iter().rposition(|cell| !blank_cell(cell)) else {
        return false;
    };
    let word_width = next
        .cells
        .iter()
        .skip_while(|cell| blank_cell(cell))
        .take_while(|cell| !blank_cell(cell) || cell.width == 2)
        .count();
    word_width != 0 && previous.cells.len() - last - 1 <= word_width
}

fn retry_time(message: &str) -> Option<RetryTime> {
    let body = message
        .strip_prefix("■ You’ve hit your usage limit")
        .or_else(|| message.strip_prefix("■ You've hit your usage limit"))?;
    if !body.starts_with('.') && !body.starts_with(" for ") {
        return None;
    }
    let (_, retry) = body
        .rsplit_once(" try again at ")
        .or_else(|| body.rsplit_once(" Try again at "))?;
    let fields: Vec<_> = retry.strip_suffix('.')?.split_ascii_whitespace().collect();
    let (date, clock, period) = match fields.as_slice() {
        [clock, period] => (None, *clock, *period),
        [month, day, year, clock, period] => {
            let month = [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ]
            .iter()
            .position(|name| name == month)?;
            let day = day.strip_suffix(',')?;
            let (day, suffix) = day.split_at_checked(day.len().checked_sub(2)?)?;
            let day = digits(day)?;
            let ordinal = match (day % 100, day % 10) {
                (11..=13, _) => "th",
                (_, 1) => "st",
                (_, 2) => "nd",
                (_, 3) => "rd",
                _ => "th",
            };
            if !(1..=31).contains(&day) || suffix != ordinal || year.len() != 4 {
                return None;
            }
            let date = (digits(year)?, u16::try_from(month + 1).ok()?, day);
            (Some(date), *clock, *period)
        }
        _ => return None,
    };
    let (hour, minute) = clock.split_once(':')?;
    if hour.len() > 2 || minute.len() != 2 {
        return None;
    }
    let (hour, minute) = (digits(hour)?, digits(minute)?);
    if !(1..=12).contains(&hour) || minute >= 60 {
        return None;
    }
    let half_day = match period {
        "AM" => 0,
        "PM" => 12,
        _ => return None,
    };
    Some(RetryTime {
        date,
        hour: hour % 12 + half_day,
        minute,
    })
}

fn digits(value: &str) -> Option<u16> {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| value.parse().ok())
        .flatten()
}

#[cfg(test)]
mod tests;
