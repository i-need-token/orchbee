//! The askpass helper on Windows: it reaches the hub, reads the question, binds its own one-shot
//! answer endpoint, hands the question to the hub, and waits for the answer. With an answer it
//! prints it for `ssh` and exits 0. Without one it prints nothing and does not exit before its own
//! bound, since `ssh` would send the empty answer of a helper that exits: a helper that cannot ask
//! tells the hub, which ends the carrier, and ending the carrier ends the helper with it. It never
//! reads its standard input, which `ssh` shares with the link, and never writes its standard error.

use super::{Asked, Pairing, Raise, TAKEN, hex, question};
use orchbee_local_ipc::{LocalListener, LocalStream, Readiness};
use orchbee_machine_protocol::{
    ANSWER_HEADER_BYTES, ANSWER_TAKEN, AnswerToken, MAXIMUM_ANSWER_BYTES, admissible_answer,
    decode_answer_header, hub::QuestionKind, wipe,
};
use std::{
    ffi::OsStr,
    io::{self, Read, Write},
    path::Path,
    process::ExitCode,
    thread,
    time::{Duration, Instant},
};

/// The bound on reaching the hub and handing it the question.
const HUB: Duration = Duration::from_secs(5);
/// The helper's own bound, past the hub's 120-second question bound and 60-second attempt bound,
/// should the hub stop working.
const WAIT: Duration = Duration::from_secs(130);
/// The bound on reading one delivery record.
const RECORD: Duration = Duration::from_secs(5);
/// The pause between looks at the endpoint and the hub.
const PAUSE: Duration = Duration::from_millis(10);
/// Connections with a malformed record or another token before the helper gives up.
const STRANGERS: usize = 8;

/// The answer and the line feed `ssh` reads, wiped when dropped. It has no debug form.
struct Answer([u8; MAXIMUM_ANSWER_BYTES + 1]);

impl Drop for Answer {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// A question ready to hand the hub: its record, the endpoint its answer comes to, and the token
/// that answer must carry.
struct Ready {
    record: Vec<u8>,
    listener: LocalListener,
    token: AnswerToken,
}

pub(super) fn run(prompt: &OsStr, pairing: &OsStr) -> ExitCode {
    let deadline = Instant::now() + WAIT;
    let Some(pairing) = pairing.to_str().and_then(Pairing::parse) else {
        return linger(deadline);
    };
    let Ok(hub) = LocalStream::connect_before(Path::new(&pairing.endpoint), Instant::now() + HUB)
    else {
        return linger(deadline);
    };
    if hub.set_write_timeout(Some(HUB)).is_err() {
        return linger(deadline);
    }
    let ticket = hex(&pairing.ticket);
    let Some(ready) = prepare(prompt, ticket.clone()) else {
        // The hub ends the carrier of a helper that cannot ask; its connection stays open meanwhile.
        if let Some(record) = Raise::unable(ticket).encode() {
            let _told = (&hub).write_all(&record);
        }
        return linger(deadline);
    };
    if wait(&hub, ready, deadline).is_some() {
        return ExitCode::SUCCESS;
    }
    // Leaving with the question open ends the attempt, unless the hub closed it first.
    drop(hub);
    linger(deadline)
}

/// Waits for the hub to end the carrier, which ends this helper, or for the helper's own bound.
fn linger(deadline: Instant) -> ExitCode {
    if let Some(rest) = deadline.checked_duration_since(Instant::now()) {
        thread::sleep(rest);
    }
    ExitCode::from(1)
}

/// Reads the question, binds its endpoint and builds its record; `None` when it cannot ask.
fn prepare(prompt: &OsStr, ticket: String) -> Option<Ready> {
    let prompt = question::prompt_text(&prompt.to_string_lossy())?;
    let kind = question::classify(prompt.as_str());
    let mut token = [0; 16];
    let mut suffix = [0; 8];
    getrandom::fill(&mut token).ok()?;
    getrandom::fill(&mut suffix).ok()?;
    let endpoint = format!(
        r"\\.\pipe\orchbee-answer-{}-{}",
        std::process::id(),
        hex(&suffix)
    );
    let listener = LocalListener::bind(&endpoint).ok()?;
    listener.set_nonblocking(true).ok()?;
    let (name, host_key) = match kind {
        QuestionKind::Password => ("password", None),
        QuestionKind::KeyPassphrase => ("key-passphrase", None),
        QuestionKind::HostKey { host_key } => ("host-key", Some(host_key)),
        QuestionKind::Other => ("other", None),
    };
    let record = Raise {
        ticket,
        question: Some(Asked {
            kind: name.to_owned(),
            prompt: prompt.as_str().to_owned(),
            host_key,
            endpoint,
            token: hex(&token),
        }),
    }
    .encode()?;
    Some(Ready {
        record,
        listener,
        token: AnswerToken::from_bytes(token),
    })
}

/// Hands the question to the hub and waits for its answer, the hub's close, or `deadline`; with
/// the answer it prints it for `ssh`.
fn wait(hub: &LocalStream, ready: Ready, deadline: Instant) -> Option<()> {
    let Ready {
        record,
        listener,
        token,
    } = ready;
    (&*hub).write_all(&record).ok()?;
    let mut answer = Answer([0; MAXIMUM_ANSWER_BYTES + 1]);
    let mut strangers = 0;
    while Instant::now() < deadline {
        // The hub closes the connection to close the question, after it ended the carrier.
        match hub.readiness() {
            Ok(Readiness::Pending) => {}
            Ok(Readiness::Readable) => {
                let mut ignored = [0; 1];
                let _read = (&*hub).read(&mut ignored);
            }
            Ok(Readiness::Closed) | Err(_) => return None,
        }
        let Ok((stream, ())) = listener.accept() else {
            thread::sleep(PAUSE);
            continue;
        };
        let Some(length) = take(&stream, &token, &mut answer.0) else {
            strangers += 1;
            if strangers >= STRANGERS {
                return None;
            }
            continue;
        };
        // The hub learns the question was answered before `ssh` reads the answer.
        let _told = (&*hub).write_all(&[TAKEN]);
        let _receipt = (&stream).write_all(&[ANSWER_TAKEN]);
        drop(listener);
        let line = answer.0.get_mut(..=length)?;
        *line.last_mut()? = b'\n';
        // One write of a whole line goes straight to the pipe, past the standard output's buffer.
        let mut output = io::stdout().lock();
        return output.write_all(line).and_then(|()| output.flush()).ok();
    }
    None
}

/// Reads one delivery record for `token` from `stream` into `answer` within its bound; the length
/// of an admissible answer, or `None`, after which the connection closes without a byte.
fn take(stream: &LocalStream, token: &AnswerToken, answer: &mut [u8]) -> Option<usize> {
    let deadline = Instant::now() + RECORD;
    let mut header = [0; ANSWER_HEADER_BYTES];
    read_before(stream, &mut header, deadline)?;
    let length = decode_answer_header(&header, token).ok()?;
    let bytes = answer.get_mut(..length)?;
    let read = read_before(stream, bytes, deadline);
    if read.is_none() || !admissible_answer(bytes) {
        wipe(answer);
        return None;
    }
    Some(length)
}

/// Fills `buffer` from `stream` before `deadline`.
fn read_before(stream: &LocalStream, buffer: &mut [u8], deadline: Instant) -> Option<()> {
    let mut filled = 0;
    while filled < buffer.len() {
        let remaining = deadline.checked_duration_since(Instant::now())?;
        stream
            .set_read_timeout(Some(remaining.max(Duration::from_millis(1))))
            .ok()?;
        match (&*stream).read(buffer.get_mut(filled..)?) {
            Ok(0) | Err(_) => return None,
            Ok(read) => filled += read,
        }
    }
    Some(())
}
