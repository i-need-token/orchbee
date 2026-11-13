//! `machine connect` asking on the console. When standard input and standard error are both
//! consoles the command's `connect` carries `answers`, the hub makes it the claimant of the login
//! episode, and each question is `ssh`'s own prompt, escaped, and one line of Orchbee's wording on
//! standard error, answered on standard input: without echo for a secret, `yes` for a new host. An
//! answer goes straight to the question's endpoint; one it does not take is reported at once. An
//! empty line or anything but `yes` for a host dismisses the episode, and an interrupt dismisses
//! it and ends the command. `docs/machines.md#connect-and-disconnect` owns the rule and
//! `docs/machines.md#answer-path` the buffers that hold an answer.

use super::super::{Context, Error, Remote};
use crate::{
    cli::machines::askpass::question::attributed,
    cli_text::{CliText, MachineQuestion},
    localization::Locale,
};
use orchbee_machine_protocol::{
    Id, MAXIMUM_ANSWER_BYTES, MachineId,
    hub::{Claimant, Event, LoginPhase, Question, QuestionKind, Reply, Request},
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
use winsafe::{self as w, co};

/// How often the wait looks for the reply, a typed line, an event and an interrupt.
const POLL: Duration = Duration::from_millis(50);
/// The bound on a `dismiss` request.
const DISMISS: Duration = Duration::from_secs(3);
/// Where the `connect` request's own wait ends. The command's deadline, which time at a question
/// extends, ends the wait sooner, and the hub bounds every question and the attempt.
const REQUEST: Duration = Duration::from_secs(60 * 60);
/// UTF-16 units one read takes: the longest admissible answer, one unit for each byte, and the
/// line's CR LF.
const UNITS: usize = MAXIMUM_ANSWER_BYTES + 2;
const LINE_FEED: u16 = 0x0A;
const CARRIAGE_RETURN: u16 = 0x0D;

/// The console the command asks on, and the machine it names in its questions.
pub(in super::super) struct Console {
    /// The input mode before any question, restored after each.
    mode: co::CONSOLE,
    target: String,
    text: CliText,
}

impl Console {
    /// The console when standard input and standard error are both consoles; `target` is the
    /// Machine's `ssh` target.
    pub(in super::super) fn detect(target: String, locale: Locale) -> Option<Self> {
        let mode = standard(co::STD_HANDLE::INPUT)?.GetConsoleMode().ok()?;
        standard(co::STD_HANDLE::ERROR)?.GetConsoleMode().ok()?;
        Some(Self {
            mode,
            target,
            text: CliText::new(locale),
        })
    }
}

/// Sends the `connect` request and waits for its reply under the command's deadline, asking each
/// question the hub puts to this command meanwhile. `Err` is an interrupt, which dismisses the
/// episode, or a failure to wait at all. An interrupt at an open question leaves `remote`
/// unchanged, since `ssh` asks before the far side runs anything.
pub(super) fn converse(
    client: &Arc<HubClient>,
    request: Request,
    machine: &MachineId,
    console: &Console,
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
    loop {
        match replies.try_recv() {
            Ok(reply) => {
                if let Some(open) = asking.take() {
                    open.abandon();
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
                let question = open.finish();
                match reply(&line, &question.kind) {
                    Act::Deliver(answer) => match deliver(&question.answer, answer) {
                        Ok(()) => {}
                        // Nothing reached the endpoint, which still waits: the same question
                        // again.
                        Err(DeliveryFailure::Inadmissible) => {
                            say(&console.text.cli_machine_answer_undelivered(true));
                            match Asking::start(console, question) {
                                Some(open) => asking = Some(open),
                                None => dismiss(client, login),
                            }
                        }
                        // The endpoint is gone or refused it: the login stops, and the reply
                        // says how the attempt ended.
                        Err(DeliveryFailure::Unreachable | DeliveryFailure::Refused) => {
                            say(&console.text.cli_machine_answer_undelivered(false));
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
        } else if Instant::now() >= context.deadline {
            return Err(Error::Timeout);
        }
        match client.next(Instant::now() + POLL) {
            Some(Notice::Event(Event::Login(event))) if event.machine == *machine => {
                match event.phase {
                    LoginPhase::Claimed {
                        claim: Claimant::You,
                    } => login = Some(event.login),
                    LoginPhase::Ended { .. } if login == Some(event.login) => login = None,
                    _ => {}
                }
            }
            Some(Notice::Event(Event::Question(question)))
                if login == Some(question.login) && asking.is_none() =>
            {
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
    mode: Mode,
    lines: mpsc::Receiver<Line>,
}

impl Asking {
    /// Writes the question, sets the console for it and starts the read; `None` when the console
    /// cannot be set, so a secret would echo. Only a new host, which the hub reads from `ssh`'s
    /// own sentence naming this Machine's host, is read with echo.
    fn start(console: &Console, mut question: Question) -> Option<Self> {
        // The client reads the prompt as the hub does, so nothing else turns echo on or takes `yes`.
        question.kind = attributed(question.kind, question.prompt.as_str(), &console.target);
        let echo = matches!(question.kind, QuestionKind::HostKey { .. });
        let mode = Mode::set(console.mode, echo)?;
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
                let _ = sender.send(read_line());
            })
            .ok()?;
        Some(Self {
            question,
            since: Instant::now(),
            mode,
            lines,
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

/// The console's input mode while a question reads; the earlier mode returns when it drops.
struct Mode {
    earlier: co::CONSOLE,
    echo: bool,
}

impl Mode {
    fn set(earlier: co::CONSOLE, echo: bool) -> Option<Self> {
        let line = earlier | co::CONSOLE::ENABLE_LINE_INPUT | co::CONSOLE::ENABLE_PROCESSED_INPUT;
        let reading = if echo {
            line | co::CONSOLE::ENABLE_ECHO_INPUT
        } else {
            line & !co::CONSOLE::ENABLE_ECHO_INPUT
        };
        let input = standard(co::STD_HANDLE::INPUT)?;
        // A failed set may still have changed the mode, so the earlier one returns either way.
        let mode = Self { earlier, echo };
        input.SetConsoleMode(reading).ok()?;
        Some(mode)
    }
}

impl Drop for Mode {
    fn drop(&mut self) {
        if let Some(input) = standard(co::STD_HANDLE::INPUT) {
            let _ = input.SetConsoleMode(self.earlier);
        }
    }
}

/// A standard handle, left open: it belongs to the process, not to this command.
fn standard(which: co::STD_HANDLE) -> Option<w::HSTD> {
    let mut guard = w::HSTD::GetStdHandle(which).ok()?;
    Some(guard.leak())
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

/// The console's UTF-16 read buffer, wiped when dropped. It has no debug form.
struct Units(w::WString);

impl Drop for Units {
    fn drop(&mut self) {
        wipe_units(self.0.as_mut_slice());
    }
}

fn wipe_units(units: &mut [u16]) {
    units.fill(0);
    std::hint::black_box(units);
}

/// Reads one line from the console, to its end even when it is too long, so nothing typed is left
/// for whatever reads the console next.
fn read_line() -> Line {
    let Some(input) = standard(co::STD_HANDLE::INPUT) else {
        return Line::Failed;
    };
    let mut units = Units(w::WString::new_alloc_buf(UNITS));
    // One character past the bound is refused before it is pushed, so the buffer never grows.
    let mut typed = Typed(Vec::with_capacity(MAXIMUM_ANSWER_BYTES));
    let mut fits = true;
    loop {
        let read = match input.ReadConsole(&mut units.0, None) {
            Ok(read) => usize::try_from(read).unwrap_or(0),
            Err(_) => return Line::Failed,
        };
        if read == 0 {
            return Line::Aborted;
        }
        let buffer = units.0.as_slice();
        let chunk = buffer.get(..read).unwrap_or(buffer);
        let ended = chunk.last() == Some(&LINE_FEED);
        let mut content = chunk;
        while let [rest @ .., LINE_FEED | CARRIAGE_RETURN] = content {
            content = rest;
        }
        fits = fits && append(&mut typed.0, content);
        wipe_units(units.0.as_mut_slice());
        if ended {
            break;
        }
    }
    match (fits, typed.0.is_empty()) {
        (false, _) => Line::TooLong,
        (true, true) => Line::Empty,
        (true, false) => Line::Answer(typed),
    }
}

/// Appends `units` as UTF-8 within the longest admissible answer; `false` once it would not fit.
fn append(typed: &mut Vec<u8>, units: &[u16]) -> bool {
    for decoded in char::decode_utf16(units.iter().copied()) {
        let mut bytes = [0; 4];
        let length = decoded
            .unwrap_or(char::REPLACEMENT_CHARACTER)
            .encode_utf8(&mut bytes)
            .len();
        let fits = typed.len() + length <= MAXIMUM_ANSWER_BYTES;
        if fits {
            typed.extend_from_slice(&bytes[..length]);
        }
        wipe(&mut bytes);
        if !fits {
            return false;
        }
    }
    true
}
