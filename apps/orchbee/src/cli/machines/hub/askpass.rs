//! The hub's askpass endpoint: each askpass helper hands it one question, which the hub matches to
//! a running carrier by its ticket and gives that Machine; the helper's connection then says only
//! whether the answer was taken. No answer ever reaches this endpoint.
//! `docs/machines.md#askpass-helper` owns the pairing.

use super::{machine::Raised, state::Shared};
use crate::cli::machines::askpass::{
    Asked, RAISE_HEADER_BYTES, Raise, TAKEN, TICKET_BYTES, hex, question, unhex,
};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_machine_protocol::{AnswerToken, PathText, hub::AnswerEndpoint, hub::QuestionKind};
use std::{
    io::Read,
    net::Shutdown,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

/// Helper connections served at once.
pub(super) const HELPERS: usize = 16;
/// The bound on reading a helper's record.
const RECORD: Duration = Duration::from_secs(5);
/// The listener's pause between accept attempts.
const ACCEPT_PAUSE: Duration = Duration::from_millis(10);
/// The prefix every answer endpoint a helper binds carries.
const ANSWER_PREFIX: &str = r"\\.\pipe\orchbee-answer-";

/// Binds the askpass endpoint under a fresh name; its name and listener.
pub(super) fn bind() -> Option<(String, LocalListener)> {
    let mut suffix = [0; 8];
    getrandom::fill(&mut suffix).ok()?;
    let name = format!(
        r"\\.\pipe\orchbee-askpass-{}-{}",
        std::process::id(),
        hex(&suffix)
    );
    let listener = LocalListener::bind(&name).ok()?;
    listener.set_nonblocking(true).ok()?;
    Some((name, listener))
}

/// Accepts helpers until `stop`; each proves its user before a byte is read and is read on its own
/// thread, at most 16 at once.
pub(super) fn listen(shared: &Arc<Shared>, listener: &LocalListener, stop: &AtomicBool) {
    while !stop.load(Ordering::Acquire) {
        let Ok((stream, ())) = listener.accept() else {
            thread::sleep(ACCEPT_PAUSE);
            continue;
        };
        {
            let mut hub = shared.lock();
            if hub.helpers >= HELPERS {
                continue;
            }
            hub.helpers += 1;
        }
        let started = {
            let shared = shared.clone();
            thread::Builder::new()
                .name("orchbee-hub-askpass".into())
                .spawn(move || serve(&shared, &stream))
        };
        if started.is_err() {
            shared.lock().helpers -= 1;
        }
    }
}

fn serve(shared: &Shared, stream: &LocalStream) {
    if let Some((carrier, serial)) = admit(shared, stream) {
        follow(shared, stream, carrier, serial);
    }
    let _closed = stream.shutdown(Shutdown::Both);
    shared.lock().helpers -= 1;
    shared.wake();
}

/// Reads the helper's record and hands its question to the Machine whose carrier its ticket names;
/// that carrier and the helper's registration, or `None` to close it. A helper the hub cannot ask
/// for a carrier it knows ends that carrier first, so `ssh` never reads the empty answer a refused
/// helper would leave; a record without a known ticket names no carrier to end.
fn admit(shared: &Shared, stream: &LocalStream) -> Option<(u64, u64)> {
    let raise = read(stream)?;
    let ticket = unhex(&raise.ticket).filter(|ticket| *ticket != [0; TICKET_BYTES])?;
    let raised = raise.question.and_then(raised);
    let mut hub = shared.lock();
    let carrier = hub
        .carriers
        .iter()
        .find(|(_, handle)| handle.ticket == ticket)
        .map(|(carrier, _)| *carrier)?;
    // `ssh` asks one question at a time; a second helper of one carrier is not asked.
    let admitted =
        raised.and_then(|raised| Some((raised, hub.admit_asker(carrier, stream.clone())?)));
    let serial = match admitted {
        Some((raised, serial)) => {
            hub.machine_of(carrier, |machine, cx| machine.question(carrier, raised, cx));
            Some(serial)
        }
        None => {
            hub.machine_of(carrier, |machine, cx| machine.refuse(carrier, cx));
            None
        }
    };
    drop(hub);
    shared.wake();
    serial.map(|serial| (carrier, serial))
}

/// Waits for the helper's one byte, or its leaving, and tells the Machine, unless the hub already
/// closed this helper.
fn follow(shared: &Shared, stream: &LocalStream, carrier: u64, serial: u64) {
    let mut byte = [0; 1];
    let taken = stream.set_read_timeout(None).is_ok()
        && matches!((&*stream).read(&mut byte), Ok(1))
        && byte[0] == TAKEN;
    let mut hub = shared.lock();
    if hub.release_asker(carrier, serial) {
        if taken {
            hub.machine_of(carrier, |machine, cx| machine.taken(carrier, cx));
        } else {
            hub.machine_of(carrier, |machine, cx| machine.unanswered(carrier, cx));
        }
    }
    drop(hub);
    shared.wake();
}

/// The helper's record within its bound.
fn read(stream: &LocalStream) -> Option<Raise> {
    let deadline = Instant::now() + RECORD;
    let mut header = [0; RAISE_HEADER_BYTES];
    read_before(stream, &mut header, deadline)?;
    let mut json = vec![0; Raise::length(&header)?];
    read_before(stream, &mut json, deadline)?;
    serde_json::from_slice(&json).ok()
}

/// The question a record asks, as the hub admits it.
fn raised(raise: Asked) -> Option<Raised> {
    let kind = match (raise.kind.as_str(), raise.host_key) {
        ("password", None) => QuestionKind::Password,
        ("key-passphrase", None) => QuestionKind::KeyPassphrase,
        ("host-key", Some(host_key)) => QuestionKind::HostKey { host_key },
        ("other", None) => QuestionKind::Other,
        _ => return None,
    };
    let endpoint = PathText::new(raise.endpoint)
        .ok()
        .filter(|endpoint| endpoint.as_str().starts_with(ANSWER_PREFIX))?;
    Some(Raised {
        kind,
        prompt: question::prompt_text(&raise.prompt)?,
        answer: AnswerEndpoint {
            endpoint,
            token: AnswerToken::from_bytes(unhex(&raise.token)?),
        },
    })
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
