//! The one rule that turns an untrusted raw terminal title into the text Tab names use: strip an
//! elevation prefix and an activity mark, then, for publication, drop control characters and
//! bound the length. It neither names console programs nor localizes; Clients do that.

use super::MAXIMUM_PRESENTATION_TITLE_BYTES;

/// The prefix Windows programs such as cmd and Windows PowerShell put on an elevated console's
/// title, in the display language's own wording. Only the `en-US` and `zh-CN` forms are
/// recognized; `zh-CN` is accepted with either an ASCII or a full-width colon.
const ELEVATION_PREFIXES: [&str; 3] = ["Administrator: ", "管理员: ", "管理员："];

/// Marks a truncated published title.
const ELLIPSIS: char = '…';

/// The title without one elevation prefix and one activity mark, trimmed. An activity mark is one
/// braille cell or one of `·✢✳✶✻✽◐◓◑◒`, alone or followed by whitespace, which programs animate in
/// front of a steady title. `None` when nothing is left.
#[must_use]
pub fn strip_terminal_title(title: &str) -> Option<&str> {
    let title = ELEVATION_PREFIXES
        .iter()
        .find_map(|prefix| title.strip_prefix(prefix))
        .unwrap_or(title)
        .trim();
    let first = title.chars().next()?;
    let rest = &title[first.len_utf8()..];
    let activity = matches!(first, '\u{2800}'..='\u{28ff}') || "·✢✳✶✻✽◐◓◑◒".contains(first);
    let title = if activity && rest.chars().next().is_none_or(char::is_whitespace) {
        rest.trim()
    } else {
        title
    };
    (!title.is_empty()).then_some(title)
}

/// The title the Server publishes for a raw terminal title: [`strip_terminal_title`], without
/// control characters and trimmed again, then cut at a character boundary and ended with `…` when
/// it exceeds [`MAXIMUM_PRESENTATION_TITLE_BYTES`]. `None` when nothing is left.
#[must_use]
pub fn project_terminal_title(title: &str) -> Option<String> {
    let visible: String = strip_terminal_title(title)?
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    let visible = visible.trim();
    if visible.is_empty() {
        return None;
    }
    if visible.len() <= MAXIMUM_PRESENTATION_TITLE_BYTES {
        return Some(visible.to_owned());
    }
    let mut end = MAXIMUM_PRESENTATION_TITLE_BYTES - ELLIPSIS.len_utf8();
    while !visible.is_char_boundary(end) {
        end -= 1;
    }
    let mut projected = visible[..end].to_owned();
    projected.push(ELLIPSIS);
    Some(projected)
}

/// Whether a published title keeps the guarantees [`project_terminal_title`] gives every result.
pub(super) fn publishable(title: &str) -> bool {
    !title.is_empty()
        && title.len() <= MAXIMUM_PRESENTATION_TITLE_BYTES
        && !title.chars().any(char::is_control)
}

#[cfg(test)]
mod tests;
