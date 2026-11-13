//! Destructive admission for `server stop`, `session stop`, `session delete` (ORC-382),
//! `--machine <selector> server stop` and `uninstall`.
//!
//! A release Windows build runs them only when standard input and standard output are both
//! interactive consoles and the person types the exact Session name, or `orchbee` for `uninstall`,
//! at the prompt; it refuses
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

/// How one invocation names itself in the request log, its prompt and its refusals.
enum Wording<'a> {
    /// A local Session command: the prompt and the command to run by hand name the Session.
    Session(DestructiveCommand),
    /// `uninstall`: `listing`, what goes and what stays, precedes the prompt.
    Uninstall { listing: &'a str },
    /// A remote stop: the prompt names the Machine `label`, and `hand` is the command to run by
    /// hand.
    Remote { label: &'a str, hand: &'a str },
}

impl Wording<'_> {
    const fn words(&self) -> &'static str {
        match self {
            Self::Session(command) => command.words(),
            Self::Uninstall { .. } => "uninstall",
            Self::Remote { .. } => "machine server stop",
        }
    }

    fn prompt(&self, text: CliText, target: &str) -> String {
        match self {
            Self::Session(command) => {
                text.cli_destructive_prompt(target, *command == DestructiveCommand::SessionDelete)
            }
            Self::Uninstall { listing } => format!("{listing}{}", text.cli_uninstall_prompt()),
            Self::Remote { label, .. } => text.cli_machine_stop_prompt(label, target),
        }
    }

    fn refusal(&self, text: CliText, refusal: Refusal, target: Option<&str>) -> String {
        let name = target.unwrap_or_default();
        match self {
            Self::Session(command) => {
                text.cli_destructive_refusal(refusal.reason(), &command.by_hand(target), name)
            }
            Self::Uninstall { .. } => text.cli_uninstall_refusal(refusal.reason()),
            Self::Remote { hand, .. } => text.cli_destructive_refusal(refusal.reason(), hand, name),
        }
    }
}

/// What admission reads from its environment and does to it: the platform's facts and effects in
/// a build, and a script in tests, which judge the release rule a development build cannot run.
trait Host {
    fn release(&self) -> bool;
    fn console(&self) -> Console;
    fn prompt(&mut self, text: &str) -> Typed;
    fn record(&mut self, command: &str, target: Option<&str>, console: Console, outcome: &str);
}

/// The platform this build runs on.
struct Platform;

impl Host for Platform {
    fn release(&self) -> bool {
        platform::release()
    }
    fn console(&self) -> Console {
        platform::console()
    }
    fn prompt(&mut self, text: &str) -> Typed {
        platform::prompt(text)
    }
    fn record(&mut self, command: &str, target: Option<&str>, console: Console, outcome: &str) {
        platform::record(command, target, console, outcome);
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
type Typed = Result<String, Box<super::SourceFinalization>>;

/// The settled admission of one invocation.
enum Verdict {
    Run,
    Refuse(Refusal),
    Cancelled(Box<super::SourceFinalization>),
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
) -> Result<(), Box<Output>> {
    let resolved = match target {
        Some(name) => Some(name.to_owned()),
        None => platform::target_session_name(),
    };
    admit_with(
        &mut Platform,
        &Wording::Session(command),
        confirm,
        resolved.as_deref(),
        format,
        locale,
    )
}

/// The name a person types to admit `uninstall`.
pub(super) const UNINSTALL_NAME: &str = "orchbee";

/// Admits `uninstall`; a release prompt shows `listing`, what goes and what stays, before it asks
/// for [`UNINSTALL_NAME`].
pub(super) fn admit_uninstall(
    confirm: bool,
    listing: &str,
    format: OutputFormat,
    locale: Locale,
) -> Result<(), Box<Output>> {
    admit_with(
        &mut Platform,
        &Wording::Uninstall { listing },
        confirm,
        Some(UNINSTALL_NAME),
        format,
        locale,
    )
}

/// The saved Machine a remote stop names: `selector` as the person typed it, and the catalog's
/// `id`, `label` and remote `session`.
pub(super) struct RemoteTarget<'a> {
    pub(super) selector: &'a str,
    pub(super) id: &'a str,
    pub(super) label: &'a str,
    pub(super) session: &'a str,
}

/// Admits `--machine <selector> server stop` under the rule a local `server stop` follows in this
/// build. The person types the remote Session's name; the prompt names the Machine and says that
/// the Session and every terminal in it stop.
pub(super) fn admit_remote_stop(
    confirm: bool,
    target: &RemoteTarget<'_>,
    format: OutputFormat,
    locale: Locale,
) -> Result<(), Box<Output>> {
    remote_stop_with(&mut Platform, confirm, target, format, locale)
}

fn remote_stop_with(
    host: &mut impl Host,
    confirm: bool,
    target: &RemoteTarget<'_>,
    format: OutputFormat,
    locale: Locale,
) -> Result<(), Box<Output>> {
    // A selector a shell would split or expand is replaced by the Machine's ID.
    let selector = if target
        .selector
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        target.selector
    } else {
        target.id
    };
    let hand = format!("orchbee --machine {selector} server stop");
    admit_with(
        host,
        &Wording::Remote {
            label: target.label,
            hand: &hand,
        },
        confirm,
        Some(target.session),
        format,
        locale,
    )
}

fn admit_with(
    host: &mut impl Host,
    wording: &Wording<'_>,
    confirm: bool,
    target: Option<&str>,
    format: OutputFormat,
    locale: Locale,
) -> Result<(), Box<Output>> {
    let release = host.release();
    let console = host.console();
    let verdict = match decide(release, console, confirm, target, None) {
        Decision::Run => Verdict::Run,
        Decision::Refuse(refusal) => Verdict::Refuse(refusal),
        Decision::Prompt => {
            let prompt = wording.prompt(CliText::new(locale), target.unwrap_or_default());
            match host.prompt(&prompt) {
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
    host.record(wording.words(), target, console, verdict.outcome());
    match verdict {
        Verdict::Run => Ok(()),
        Verdict::Cancelled(finalization) => {
            let mut output =
                failure_output(Failure::new(FailureKind::Cancelled), Some(format), locale);
            output.finalization = Some(*finalization);
            Err(Box::new(output))
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
            output.stderr = wording.refusal(CliText::new(locale), refusal, target);
            Err(Box::new(output))
        }
    }
}

#[cfg(test)]
mod tests;
