//! Where a path an Agent shows would land, judged by its spelling alone: no file system is read,
//! so a link that already exists is outside what this module can see.
use super::Wait;

/// The directories a confirmation is judged against, both absolute and in the Server's native
/// form. Without either one nothing can be placed and every path waits.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Scope<'a> {
    /// The Workspace's directory.
    pub(crate) root: Option<&'a str>,
    /// The Agent process's working directory, from the same process sample as the screen.
    pub(crate) directory: Option<&'a str>,
}

const PATH_BYTES: usize = 1024;

/// Names that wait wherever they are: repository internals, secrets, the Agents' own
/// configuration and instructions, and shell start files. Compared without case.
const PROTECTED: &[&str] = &[
    ".git",
    ".env",
    ".claude",
    ".codex",
    ".gemini",
    ".opencode",
    ".hermes",
    ".mcp.json",
    "opencode.json",
    "claude.md",
    "agents.md",
    "gemini.md",
    ".bashrc",
    ".bash_profile",
    ".bash_login",
    ".profile",
    ".zshrc",
    ".zprofile",
    ".zshenv",
    ".zlogin",
    "config.fish",
    "profile.ps1",
    "microsoft.powershell_profile.ps1",
    "microsoft.vscode_profile.ps1",
];

#[derive(Debug, Eq, PartialEq)]
enum Anchor {
    Relative,
    Posix,
    Drive(char),
}

struct Parsed {
    anchor: Anchor,
    steps: Vec<String>,
}

/// Windows opens `.git.`, `.git ` and `.git::$DATA` as `.git`, so a step is compared without its
/// stream and without trailing dots and spaces.
fn protected(step: &str) -> bool {
    let step = step.to_lowercase();
    let step = step.split(':').next().unwrap_or(&step);
    let step = step.trim_end_matches(['.', ' ']);
    PROTECTED.contains(&step) || step.starts_with(".env.")
}

/// Splits a spelling into its anchor and steps. `None` means the spelling is one this module
/// does not read: a network or device path, a drive-relative path, a stream, a short name.
fn parse(text: &str) -> Option<Parsed> {
    let bytes = text.as_bytes();
    let (anchor, rest) = if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        if !matches!(bytes.get(2), Some(b'/' | b'\\')) {
            return None;
        }
        (
            Anchor::Drive(char::from(bytes[0]).to_ascii_lowercase()),
            &text[2..],
        )
    } else if text.starts_with("//") || text.starts_with('\\') {
        return None;
    } else if text.starts_with('/') {
        (Anchor::Posix, text)
    } else {
        (Anchor::Relative, text)
    };
    let mut steps = Vec::new();
    for step in rest.split(['/', '\\']) {
        if step.is_empty() || step == "." {
            continue;
        }
        if step.contains([':', '~', '*', '?', '"', '<', '>', '|'])
            || (step != ".." && (step.ends_with('.') || step.ends_with(' ')))
            || step.starts_with(' ')
        {
            return None;
        }
        steps.push(step.to_owned());
    }
    Some(Parsed { anchor, steps })
}

/// A POSIX spelling of a Windows drive path, as Git Bash writes it: `/c/work` for `C:\work`.
fn drive_form(parsed: Parsed) -> Parsed {
    if parsed.anchor != Anchor::Posix {
        return parsed;
    }
    let Some(first) = parsed.steps.first() else {
        return parsed;
    };
    let mut letters = first.chars();
    match (letters.next(), letters.next()) {
        (Some(letter), None) if letter.is_ascii_alphabetic() => Parsed {
            anchor: Anchor::Drive(letter.to_ascii_lowercase()),
            steps: parsed.steps[1..].to_vec(),
        },
        _ => parsed,
    }
}

fn same_step(left: &str, right: &str, fold: bool) -> bool {
    if fold {
        left.to_lowercase() == right.to_lowercase()
    } else {
        left == right
    }
}

fn inside(path: &Parsed, root: &Parsed) -> bool {
    // A drive's file system does not tell case apart; a POSIX one may, so its steps must match
    // exactly.
    let fold = matches!(root.anchor, Anchor::Drive(_));
    path.anchor == root.anchor
        && path.steps.len() >= root.steps.len()
        && path
            .steps
            .iter()
            .zip(&root.steps)
            .all(|(step, expected)| same_step(step, expected, fold))
}

fn plain(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= PATH_BYTES
        && text
            .chars()
            .all(|c| matches!(c, ' '..='~') || c.is_alphanumeric())
}

/// Whether the Agent works inside the Workspace, the condition for a relative spelling to mean a
/// place inside it.
pub(super) fn settled(scope: Scope<'_>) -> bool {
    let (Some(root), Some(directory)) = (scope.root, scope.directory) else {
        return false;
    };
    match (parse(root), parse(directory)) {
        (Some(root), Some(directory)) => {
            root.anchor != Anchor::Relative
                && !root.steps.is_empty()
                && !root.steps.iter().any(|step| step == "..")
                && !directory.steps.iter().any(|step| step == "..")
                && inside(&directory, &root)
        }
        _ => false,
    }
}

/// Judges one path spelling: inside the Workspace and not protected, or the reason it waits.
pub(crate) fn check(text: &str, scope: Scope<'_>) -> Result<(), Wait> {
    if !plain(text) {
        return Err(Wait::NotUnderstood);
    }
    if text.split(['/', '\\']).any(protected) {
        return Err(Wait::ProtectedFile);
    }
    if text.starts_with('~') || text.contains(['$', '%', '`']) {
        return Err(Wait::LeavesWorkspace);
    }
    let Some(parsed) = parse(text) else {
        return Err(Wait::LeavesWorkspace);
    };
    if parsed.steps.iter().any(|step| step == "..") || !settled(scope) {
        return Err(Wait::LeavesWorkspace);
    }
    if parsed.anchor == Anchor::Relative {
        return Ok(());
    }
    let Some(root) = scope.root.and_then(parse) else {
        return Err(Wait::LeavesWorkspace);
    };
    let direct = inside(&parsed, &root);
    if direct || inside(&drive_form(parsed), &root) {
        Ok(())
    } else {
        Err(Wait::LeavesWorkspace)
    }
}

fn absolute(text: &str) -> bool {
    let bytes = text.as_bytes();
    text.starts_with(['/', '\\'])
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
}

/// Judges one command argument, which may or may not be a path. A relative word without a
/// parent step stays where the Agent works; a home form, a parent step, a short name, a
/// protected name and an absolute place outside the Workspace wait.
pub(super) fn argument(text: &str, scope: Scope<'_>) -> Result<(), Wait> {
    let mut steps = text.split(['/', '\\']);
    if steps.clone().any(protected) {
        return Err(Wait::ProtectedFile);
    }
    if text.starts_with('~') || steps.any(|step| step == ".." || step.contains('~')) {
        return Err(Wait::LeavesWorkspace);
    }
    if absolute(text) {
        return check(text, scope);
    }
    Ok(())
}
