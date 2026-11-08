//! Destructive admission for `server stop`, `session stop` and `session delete` (ORC-382).
//!
//! A release Windows build runs them only when standard input and standard output are both
//! interactive consoles and the person types the exact Session name at the prompt; it refuses
//! `--confirm` and every non-interactive invocation, including Agents, scripts and pipes.
//! Development and test builds keep `--confirm`, which the grammar already requires there.
//! Every invocation on Windows, accepted or refused, appends one record to the environment's
//! destructive request log before the command runs; a failed append changes nothing. This is a
//! speed bump for unintended Agent actions, not a security boundary.
use super::{Failure, FailureKind, Output, failure_output};
use crate::{cli_text::CliText, invocation::OutputFormat, localization::Locale};
use serde_json::{Map, json};

#[cfg(windows)]
#[path = "destructive_request/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "destructive_request/unix.rs"]
mod platform;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DestructiveCommand {
    ServerStop,
    SessionStop,
    SessionDelete,
}

impl DestructiveCommand {
    const fn words(self) -> &'static str {
        match self {
            Self::ServerStop => "server stop",
            Self::SessionStop => "session stop",
            Self::SessionDelete => "session delete",
        }
    }

    /// The command a person runs by hand in a console, without `--confirm`.
    fn by_hand(self, target: Option<&str>) -> String {
        match (self, target) {
            (Self::ServerStop, Some(name)) => format!("orchbee --session {name} server stop"),
            (Self::ServerStop, None) => "orchbee --session <name> server stop".to_owned(),
            (_, name) => format!("orchbee {} {}", self.words(), name.unwrap_or("<name>")),
        }
    }
}

/// Whether standard input and standard output are interactive consoles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Console {
    stdin: bool,
    stdout: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Decision {
    Run,
    /// Ask the person to type the Session name, then decide again with the typed line.
    Prompt,
    Refuse(Refusal),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    ConfirmOption,
    NotInteractive,
    SessionUnknown,
    NameMismatch,
}

impl Refusal {
    const fn reason(self) -> &'static str {
        match self {
            Self::ConfirmOption => "confirm-option",
            Self::NotInteractive => "not-interactive",
            Self::SessionUnknown => "session-unknown",
            Self::NameMismatch => "name-mismatch",
        }
    }
}

/// The admission rule. Outside release the grammar has already required `--confirm`, so the
/// command runs. In release, `--confirm` is refused first, then anything but two interactive
/// consoles, then an unknown target; otherwise the person is prompted, and the typed line,
/// ignoring surrounding whitespace, must equal the Session name exactly.
fn decide(
    release: bool,
    console: Console,
    confirm: bool,
    target: Option<&str>,
    typed: Option<&str>,
) -> Decision {
    if !release {
        return Decision::Run;
    }
    if confirm {
        return Decision::Refuse(Refusal::ConfirmOption);
    }
    if !(console.stdin && console.stdout) {
        return Decision::Refuse(Refusal::NotInteractive);
    }
    let Some(target) = target else {
        return Decision::Refuse(Refusal::SessionUnknown);
    };
    match typed {
        None => Decision::Prompt,
        Some(typed) if typed.trim() == target => Decision::Run,
        Some(_) => Decision::Refuse(Refusal::NameMismatch),
    }
}

/// The line a person typed at the prompt, or the finalization of an interrupt that cancelled it.
type Typed = Result<String, super::SourceFinalization>;

/// The settled admission of one invocation.
enum Verdict {
    Run,
    Refuse(Refusal),
    Cancelled(super::SourceFinalization),
}

impl Verdict {
    const fn outcome(&self) -> &'static str {
        match self {
            Self::Run => "accepted",
            Self::Refuse(Refusal::ConfirmOption) => "refused-confirm-option",
            Self::Refuse(Refusal::NotInteractive) => "refused-not-interactive",
            Self::Refuse(Refusal::SessionUnknown) => "refused-session-unknown",
            Self::Refuse(Refusal::NameMismatch) => "refused-name-mismatch",
            Self::Cancelled(_) => "cancelled",
        }
    }
}

/// Admits one destructive invocation. `target` names the Session for `session stop` and
/// `session delete`; `server stop` passes `None` and resolves the Session it would stop. `Err`
/// carries the complete refusal output.
pub(super) fn admit(
    command: DestructiveCommand,
    confirm: bool,
    target: Option<&str>,
    format: OutputFormat,
    locale: Locale,
) -> Result<(), Output> {
    let release = platform::release();
    let console = platform::console();
    let resolved = match target {
        Some(name) => Some(name.to_owned()),
        None => platform::target_session_name(),
    };
    let target = resolved.as_deref();
    let verdict = match decide(release, console, confirm, target, None) {
        Decision::Run => Verdict::Run,
        Decision::Refuse(refusal) => Verdict::Refuse(refusal),
        Decision::Prompt => {
            let prompt = CliText::new(locale).cli_destructive_prompt(
                target.unwrap_or_default(),
                command == DestructiveCommand::SessionDelete,
            );
            match platform::prompt(&prompt) {
                Ok(line) => match decide(release, console, confirm, target, Some(&line)) {
                    Decision::Run => Verdict::Run,
                    Decision::Refuse(refusal) => Verdict::Refuse(refusal),
                    // A typed line always settles the rule; a second prompt is never offered.
                    Decision::Prompt => Verdict::Refuse(Refusal::NameMismatch),
                },
                Err(finalization) => Verdict::Cancelled(finalization),
            }
        }
    };
    platform::record(command.words(), target, console, verdict.outcome());
    match verdict {
        Verdict::Run => Ok(()),
        Verdict::Cancelled(finalization) => {
            let mut output =
                failure_output(Failure::new(FailureKind::Cancelled), Some(format), locale);
            output.finalization = Some(finalization);
            Err(output)
        }
        Verdict::Refuse(refusal) => {
            let details = Map::from_iter([
                ("stage".to_owned(), json!("confirmation")),
                ("reason".to_owned(), json!(refusal.reason())),
            ]);
            let mut output = failure_output(
                Failure::new(FailureKind::Denied).details(details),
                Some(format),
                locale,
            );
            output.stderr = CliText::new(locale).cli_destructive_refusal(
                refusal.reason(),
                &command.by_hand(target),
                target.unwrap_or_default(),
            );
            Err(output)
        }
    }
}

#[cfg(test)]
mod tests;
