//! `machine connect` asking on the console. When standard input and standard error are both
//! consoles the command's `connect` carries `answers`, the hub makes it the claimant of the login
//! episode, and each question is `ssh`'s own prompt, escaped, and one line of Orchbee's wording on
//! standard error, answered on standard input: without echo for a secret, `yes` for a new host. An
//! answer goes straight to the question's endpoint; one it does not take is reported at once. An
//! empty line or anything but `yes` for a host dismisses the episode, and an interrupt dismisses
//! it and ends the command. Every `connect` of a command waits here, with a console or without:
//! one the hub offers to an interface that asks says so on standard error and waits for the
//! person there. `docs/machines.md#connect-and-disconnect` owns the rule and
//! `docs/machines.md#answer-path` the buffers that hold an answer. With `--remember` a password or
//! passphrase answer is kept until the login ends; see [`super::remember`].

use super::super::{Context, Error, Remote};
use super::remember::{Kept, Remember};
use crate::{
    cli::machines::askpass::question::attributed,
    cli_text::{CliText, MachineQuestion, RememberedLine},
    localization::Locale,
};
use orchbee_machine_protocol::{
    Id, MachineId,
    hub::{
        Claimant, ConnectOutcome, ConnectResult, Event, LoginEnd, LoginPhase, Question,
        QuestionKind, Reply, Request,
    },
    wipe,
};
use orchbee_workbench_client::application::machine_hub::{
    DeliveryFailure, HubClient, Notice, RequestFailure, deliver,
};
use std::{
    io::{self, Write},
    sync::{
        Arc,
        mpsc::{self, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(unix)]
#[path = "asking/unix.rs"]
mod platform;
#[cfg(windows)]
#[path = "asking/windows.rs"]
mod platform;

/// How often the wait looks for the reply, a typed line, an event and an interrupt.
const POLL: Duration = Duration::from_millis(50);
/// The bound on a `dismiss` request.
const DISMISS: Duration = Duration::from_secs(3);
/// Where the `connect` request's own wait ends. The command's deadline, which time at a question
/// and time waiting for a person elsewhere extend, ends the wait sooner, and the hub bounds every
/// question and the attempt.
const REQUEST: Duration = Duration::from_secs(60 * 60);

/// One command's wait for a person who answers elsewhere, across its requests: the line it writes
/// once, and whether it was written.
pub(super) struct Waited<'a> {
    line: &'a str,
    said: bool,
}

impl<'a> Waited<'a> {
    pub(super) const fn new(line: &'a str) -> Self {
        Self { line, said: false }
    }
}

/// The console the command asks on, and the machine it names in its questions.
pub(in super::super) struct Console {
    /// The input mode before any question, restored after each.
    mode: platform::Earlier,
    target: String,
    text: CliText,
    /// `--remember`: where answers typed here go once the login succeeds.
    remember: Option<Remember>,
}

impl Console {
    /// The console when standard input and standard error are both consoles; `target` is the
    /// Machine's `ssh` target.
    pub(in super::super) fn detect(
        target: String,
        locale: Locale,
        remember: Option<Remember>,
    ) -> Option<Self> {
        let mode = platform::detect()?;
        Some(Self {
            mode,
            target,
            text: CliText::new(locale),
            remember,
        })
    }
}

/// Sends the `connect` request and waits for its reply under the command's deadline, asking each
/// question the hub puts to this command on `console` meanwhile. A request the hub sends
/// `connect-waiting` without making this command the claimant waits for a person who answers
/// elsewhere: `waiting`'s line goes on standard error once per command, and the deadline stands
/// still for as long as that wait lasts, which the hub bounds. `Err` is an interrupt, which
/// dismisses the episode, or a failure to wait at all. An interrupt at an open question, or while
/// waiting for a person elsewhere, leaves `remote` unchanged, since `ssh` asks before the far side
/// runs anything.
pub(super) fn converse(
    client: &Arc<HubClient>,
    request: Request,
    machine: &MachineId,
    console: Option<&Console>,
    waiting: &mut Waited<'_>,
    context: &mut Context,
    remote: &mut Remote,
) -> Result<Result<Reply, RequestFailure>, Error> {
    let (sender, replies) = mpsc::channel();
    let worker = Arc::clone(client);
    let bound = Instant::now() + REQUEST;
    thread::Builder::new()
        .name("machine-command".to_owned())
        .spawn(move || {
            let _ = sender.send(worker.request(request, bound));
        })
        .map_err(|_| Error::Internal)?;
    let mut login: Option<Id> = None;
    let mut asking: Option<Asking> = None;
    let mut kept = Kept::new(console.and_then(|console| console.remember.as_ref()));
    // `connect-waiting` came and no claim followed it yet; and since when the deadline last moved
    // for the wait that followed.
    let mut pending = false;
    let mut elsewhere: Option<Instant> = None;
    loop {
        match replies.try_recv() {
            Ok(reply) => {
                if let Some(open) = asking.take() {
                    open.abandon();
                }
                // `online` follows a login that succeeded, whether or not its end was seen.
                if let Ok(Reply::Connect(ConnectResult {
                    outcome: ConnectOutcome::Online { .. },
                    ..
                })) = &reply
                {
                    kept.ended(LoginEnd::Succeeded);
                }
                return Ok(reply);
            }
            Err(TryRecvError::Disconnected) => return Err(Error::Internal),
            Err(TryRecvError::Empty) => {}
        }
        if context.interrupted().map_err(|_| Error::Internal)? {
            if let Some(open) = asking.take() {
                open.abandon();
                *remote = Remote::Unchanged;
            } else if elsewhere.is_some() {
                *remote = Remote::Unchanged;
            }
            dismiss(client, login);
            return Err(Error::Cancelled);
        }
        if let Some(open) = &asking {
            let line = match open.lines.try_recv() {
                Ok(line) => Some(line),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(Line::Failed),
            };
            if let Some(line) = line
                && let Some(open) = asking.take()
            {
                // Time at a question does not count against the command's deadline.
                context.deadline += open.since.elapsed();
                let text = open.text;
                let question = open.finish();
                match reply(&line, &question.kind) {
                    Act::Deliver(answer) => match deliver(&question.answer, answer) {
                        Ok(()) => kept.delivered(&question.kind, answer),
                        // Nothing reached the endpoint, which still waits: the same question
                        // again.
                        Err(DeliveryFailure::Inadmissible) => {
                            say(&text.cli_machine_answer_undelivered(true));
                            match console.and_then(|console| Asking::start(console, question)) {
                                Some(open) => asking = Some(open),
                                None => dismiss(client, login),
                            }
                        }
                        // The endpoint is gone or refused it: the login stops, and the reply
                        // says how the attempt ended.
                        Err(DeliveryFailure::Unreachable | DeliveryFailure::Refused) => {
                            say(&text.cli_machine_answer_undelivered(false));
                            dismiss(client, login);
                        }
                    },
                    Act::Dismiss => dismiss(client, login),
                    Act::Cancel => {
                        *remote = Remote::Unchanged;
                        dismiss(client, login);
                        return Err(Error::Cancelled);
                    }
                }
            }
        } else if let Some(since) = elsewhere.as_mut() {
            // Time waiting for a person elsewhere does not count against the command's deadline.
            let now = Instant::now();
            context.deadline += now.saturating_duration_since(*since);
            *since = now;
        } else if Instant::now() >= context.deadline {
            return Err(Error::Timeout);
        }
        match client.next(Instant::now() + POLL) {
            // The claimant hears of its claim with the same write; once nothing more is queued,
            // a request that was not claimed waits for a person elsewhere. A request sent again
            // through the hub that replaced a closing one says nothing more.
            None if pending => {
                pending = false;
                if !waiting.said {
                    waiting.said = true;
                    say(waiting.line);
                }
                elsewhere = Some(Instant::now());
            }
            Some(Notice::Event(Event::ConnectWaiting(_)))
                if elsewhere.is_none() && login.is_none() =>
            {
                pending = true;
            }
            Some(Notice::Event(Event::Login(event))) if event.machine == *machine => {
                match event.phase {
                    LoginPhase::Claimed {
                        claim: Claimant::You,
                    } => {
                        login = Some(event.login);
                        pending = false;
                        if let Some(since) = elsewhere.take() {
                            context.deadline += since.elapsed();
                        }
                    }
                    LoginPhase::Ended { end } if login == Some(event.login) => {
                        login = None;
                        kept.ended(end);
                    }
                    _ => {}
                }
            }
            Some(Notice::Event(Event::Question(question)))
                if login == Some(question.login) && asking.is_none() =>
            {
                // A command without a console never claims, so it is never put a question.
                let Some(console) = console else {
                    dismiss(client, login);
                    continue;
                };
                if question.remembered_rejected {
                    say(console
                        .text
                        .cli_machine_remembered(RememberedLine::Rejected));
                }
                if let Some(line) = kept.asked(&question.kind) {
                    say(console.text.cli_machine_remembered(line));
                }
                match Asking::start(console, question) {
                    Some(open) => asking = Some(open),
                    // A secret is never read with echo: a console that keeps echoing is not asked.
                    None => dismiss(client, login),
                }
            }
            _ => {}
        }
    }
}

/// Dismisses the episode this command claimed, if it still runs; the hub's reply to `connect`
/// then says how the attempt ended.
fn dismiss(client: &HubClient, login: Option<Id>) {
    if let Some(login) = login {
        let _ = client.request(Request::Dismiss { login }, Instant::now() + DISMISS);
    }
}

/// What the command does with a typed line.
enum Act<'a> {
    Deliver(&'a [u8]),
    Dismiss,
    Cancel,
}

/// A host is trusted only by `yes`, which the answer then is exactly; anything else, and an empty
/// line for any question, dismisses.
fn reply<'a>(line: &'a Line, kind: &QuestionKind) -> Act<'a> {
    match (line, kind) {
        (Line::Answer(typed), QuestionKind::HostKey { .. }) => {
            if typed.0.trim_ascii().eq_ignore_ascii_case(b"yes") {
                Act::Deliver(b"yes")
            } else {
                Act::Dismiss
            }
        }
        (Line::Answer(typed), _) => Act::Deliver(&typed.0),
        (Line::Aborted, _) => Act::Cancel,
        (Line::Empty | Line::TooLong | Line::Failed, _) => Act::Dismiss,
    }
}

/// One question on the console: its line written, the input mode set for it, and a thread
/// reading the answer.
struct Asking {
    question: Question,
    since: Instant,
    mode: platform::Mode,
    lines: mpsc::Receiver<Line>,
    text: CliText,
}

impl Asking {
    /// Writes the question, sets the console for it and starts the read; `None` when the console
    /// cannot be set, so a secret would echo. Only a new host, which the hub reads from `ssh`'s
    /// own sentence naming this Machine's host, is read with echo.
    fn start(console: &Console, mut question: Question) -> Option<Self> {
        // The client reads the prompt as the hub does, so nothing else turns echo on or takes `yes`.
        question.kind = attributed(question.kind, question.prompt.as_str(), &console.target);
        let echo = matches!(question.kind, QuestionKind::HostKey { .. });
        let mode = platform::Mode::set(&console.mode, echo)?;
        let shown = MachineQuestion::of(&question.kind);
        let line = console.text.cli_machine_question(
            &console.target,
            &shown,
            question.prompt.as_str(),
            question.repeated,
        );
        let mut stderr = io::stderr().lock();
        stderr
            .write_all(line.as_bytes())
            .and_then(|()| stderr.flush())
            .ok()?;
        drop(stderr);
        let (sender, lines) = mpsc::channel();
        thread::Builder::new()
            .name("machine-question".to_owned())
            .spawn(move || {
                // The line is wiped where it is dropped, here when nobody waits for it any more.
                let _ = sender.send(platform::read_line());
            })
            .ok()?;
        Some(Self {
            question,
            since: Instant::now(),
            mode,
            lines,
            text: console.text,
        })
    }

    /// The answer was read: the console's mode returns, and a line the console did not echo is
    /// ended on standard error.
    fn finish(self) -> Question {
        let echo = self.mode.echo;
        drop(self.mode);
        if !echo {
            end_line();
        }
        self.question
    }

    /// The wait ended with the question open: the console's mode returns and the line is ended.
    /// The read stays pending until the process ends.
    fn abandon(self) {
        drop(self.mode);
        end_line();
    }
}

fn end_line() {
    say("\n");
}

/// Writes `text` on standard error.
fn say(text: &str) {
    let mut stderr = io::stderr().lock();
    let _ = stderr
        .write_all(text.as_bytes())
        .and_then(|()| stderr.flush());
}

/// What one read of the console gave.
enum Line {
    /// A non-empty answer, its line ending removed.
    Answer(Typed),
    Empty,
    /// More than the longest admissible answer; the whole line was read and discarded.
    TooLong,
    /// An interrupt ended the read.
    Aborted,
    Failed,
}

/// The UTF-8 answer typed at a question, in a buffer sized up front so it never moves; wiped when
/// dropped. It has no debug form.
struct Typed(Vec<u8>);

impl Drop for Typed {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}
