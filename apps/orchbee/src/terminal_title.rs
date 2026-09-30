//! Terminal title activity-prefix handling shared by queries and the main TUI.
pub(crate) fn stripped(title: &str) -> Option<&str> {
    let title = title
        .strip_prefix("Administrator: ")
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_title_stripping_matches_one_elevation_and_one_bounded_activity_prefix() {
        for raw in [
            "⠋ task",
            "✳ task",
            "  ⠙   task  ",
            "✢ task",
            "✻ task",
            "◐ task",
            "◓ task",
            "◑ task",
            "◒ task",
            "Administrator:   ⠋ task",
        ] {
            assert_eq!(stripped(raw), Some("task"));
        }
        for (raw, expected) in [
            ("⠋ ⠙ task", "⠙ task"),
            ("Administrator: Administrator: task", "Administrator: task"),
            ("★task", "★task"),
            ("✨ task", "✨ task"),
            ("@ task", "@ task"),
            ("task ⠋ detail", "task ⠋ detail"),
            ("⠋task", "⠋task"),
            (" ⠋ 修复🙂标题 ", "修复🙂标题"),
        ] {
            assert_eq!(stripped(raw), Some(expected));
        }
        for raw in ["", "  ", "⠋   ", "Administrator: "] {
            assert_eq!(stripped(raw), None);
        }
    }
}
