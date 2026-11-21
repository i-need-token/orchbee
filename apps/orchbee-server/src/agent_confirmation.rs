//! Automatic confirmation: reads an Agent's confirmation dialog from a captured screen, judges
//! what it asks for, and decides for one Agent run whether its one-time accept may be pressed.
//! Everything here is pure and bounded. A screen, a rule or a command that cannot be read means
//! the confirmation waits for a person; nothing in this module can fail a process.
//! `../docs/agent-confirmation.md` owns the rules.

mod command;
mod layout;
mod place;
mod run;
#[cfg(test)]
mod tests;

pub(crate) use layout::{Dialog, Layouts, StyledRow};
pub(crate) use place::Scope;
pub(crate) use run::{Choice, Facts, Frame, Outcome, Refusal, Run, Seen, Settings, Step};

/// What a confirmation asks to do.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Command,
    FileCreate,
    FileEdit,
}

/// Why a confirmation waits for a person. The list is closed: each reason has one sentence in
/// each Client.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Wait {
    /// The command deletes, overwrites, elevates, stops or sends something out.
    Destructive,
    /// The command or file reaches outside the Workspace.
    LeavesWorkspace,
    /// The file is one Orchbee never changes unattended.
    ProtectedFile,
    /// The command has a shape the check does not read.
    NotUnderstood,
    /// The command or path is not completely on screen.
    NotVisible,
    /// The Agent itself marks the request as dangerous.
    MarkedDangerous,
    /// The Agent waits on something that is not a confirmation Orchbee answers.
    NotAnswered,
    /// A person typed or clicked in the terminal since the confirmation appeared.
    Person,
    /// The screen changed every time the key was about to be written.
    Changing,
    /// The same confirmation came back after every press.
    Repeating,
    /// The terminal's Host cannot check the screen before it types.
    HostTooOld,
}

/// Judges what a dialog asks for. `Ok` means its one-time accept may be pressed once every
/// other rule holds.
pub(crate) fn judge(dialog: &Dialog, scope: Scope<'_>) -> Result<(), Wait> {
    if dialog.marked {
        return Err(Wait::MarkedDangerous);
    }
    match dialog.kind {
        Kind::Command => command::check(&dialog.text, scope),
        Kind::FileCreate | Kind::FileEdit => match dialog.text.as_slice() {
            // An Agent shortens a long path with an ellipsis or with three dots.
            [path] if path.contains('…') || path.contains("...") => Err(Wait::NotVisible),
            // A run of spaces is a row's padding, not a name: the layout took more than the path.
            [path] if path.contains("  ") => Err(Wait::NotUnderstood),
            [path] => place::check(path, scope),
            _ => Err(Wait::NotVisible),
        },
    }
}
