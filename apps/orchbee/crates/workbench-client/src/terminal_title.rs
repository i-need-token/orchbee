//! Terminal title handling shared by queries and the main TUI: activity prefixes, and the program
//! that a console default title or a launch path names for native chrome.

/// The prefix Windows programs such as cmd and Windows PowerShell put on an elevated console's
/// title, in the display language's own wording. Only the `en-US` and `zh-CN` forms are
/// recognized; `zh-CN` is accepted with either an ASCII or a full-width colon.
const ELEVATION_PREFIXES: [&str; 3] = ["Administrator: ", "管理员: ", "管理员："];

pub fn stripped(title: &str) -> Option<&str> {
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

/// The name a terminal title shows in human output, in the words native chrome uses. Pass the raw
/// Host frame title: a launched program is named by its program, and any other title stays as its
/// terminal set it.
pub fn display_name(title: &str, locale: crate::localization::Locale) -> Option<String> {
    ChromeName::of_title(title).map(|name| locale.chrome_name(name).into_owned())
}

/// What native chrome shows for an automatic name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChromeName<'a> {
    /// A program-set title or a person's Panel name, shown as written.
    Text(&'a str),
    /// A launched program, with the command that cmd appends to its console default title while
    /// it runs one.
    Program {
        program: Program<'a>,
        command: Option<&'a str>,
    },
}

impl<'a> ChromeName<'a> {
    /// Console hosts title a window with the absolute path of its launched program until that
    /// program sets a title, and cmd extends it to `<path> - <command>` while it runs a command.
    /// The path is launch identity rather than a task name, so chrome names the program instead.
    pub(crate) fn of_title(title: &'a str) -> Option<Self> {
        let title = stripped(title)?;
        let console = console_program(title)
            .map(|program| Self::Program {
                program,
                command: None,
            })
            .or_else(|| {
                title.match_indices(" - ").find_map(|(end, separator)| {
                    let program = console_program(&title[..end])?;
                    let command = title[end + separator.len()..].trim();
                    Some(Self::Program {
                        program,
                        command: (!command.is_empty()).then_some(command),
                    })
                })
            });
        Some(console.unwrap_or(Self::Text(title)))
    }
}

/// A launched program as native chrome names it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Program<'a> {
    /// A shell that chrome names by its product name.
    Shell(KnownShell),
    /// Any other program's file name without its directory or Windows executable extension.
    File(&'a str),
}

impl<'a> Program<'a> {
    /// Names the program at a path written with either separator. Matching ignores ASCII case, as
    /// Windows file names do, so every platform applies one rule.
    pub(crate) fn from_path(path: &'a str) -> Option<Self> {
        let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
        let file = executable_stem(name).unwrap_or(name);
        if file.trim().is_empty() {
            return None;
        }
        Some(
            SHELLS
                .iter()
                .find(|(stem, _)| file.eq_ignore_ascii_case(stem))
                .map_or(Self::File(file), |&(_, shell)| Self::Shell(shell)),
        )
    }
}

/// Shells whose product name differs from their executable's file name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KnownShell {
    PowerShell,
    WindowsPowerShell,
    CommandPrompt,
    Bash,
    Zsh,
}

const SHELLS: [(&str, KnownShell); 5] = [
    ("pwsh", KnownShell::PowerShell),
    ("powershell", KnownShell::WindowsPowerShell),
    ("cmd", KnownShell::CommandPrompt),
    ("bash", KnownShell::Bash),
    ("zsh", KnownShell::Zsh),
];

/// The program that a console default title names: an absolute drive or UNC path to a Windows
/// executable.
fn console_program(title: &str) -> Option<Program<'_>> {
    let mut prefix = title.chars();
    let absolute = matches!(
        (prefix.next(), prefix.next(), prefix.next()),
        (Some(drive), Some(':'), Some('\\' | '/')) if drive.is_ascii_alphabetic()
    ) || title.starts_with(r"\\");
    let name = title.rsplit(['\\', '/']).next().unwrap_or(title);
    if absolute && executable_stem(name).is_some() {
        Program::from_path(title)
    } else {
        None
    }
}

/// A file name's stem when the name ends in a Windows executable extension.
fn executable_stem(name: &str) -> Option<&str> {
    let (stem, extension) = name.rsplit_once('.')?;
    (!stem.is_empty()
        && ["exe", "com", "bat", "cmd"]
            .iter()
            .any(|known| extension.eq_ignore_ascii_case(known)))
    .then_some(stem)
}

#[cfg(test)]
mod tests;
