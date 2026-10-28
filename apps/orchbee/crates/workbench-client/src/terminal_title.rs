//! Terminal title activity-prefix handling shared by queries and the main TUI.
pub fn stripped(title: &str) -> Option<&str> {
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

/// Title shown by native chrome. Console hosts name a window after the absolute path of its
/// launched program until that program sets a title; the path is launch identity rather than a
/// task name, so chrome shows only the executable file name, matching the launch fallback.
pub(crate) fn display(title: &str) -> Option<&str> {
    let title = stripped(title)?;
    let mut prefix = title.chars();
    let absolute = matches!(
        (prefix.next(), prefix.next(), prefix.next()),
        (Some(drive), Some(':'), Some('\\' | '/')) if drive.is_ascii_alphabetic()
    ) || title.starts_with(r"\\");
    let name = title.rsplit(['\\', '/']).next().unwrap_or(title);
    let executable = name.rsplit_once('.').is_some_and(|(stem, extension)| {
        !stem.is_empty()
            && ["exe", "com", "bat", "cmd"]
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
    });
    Some(if absolute && executable { name } else { title })
}

#[cfg(test)]
mod tests;
