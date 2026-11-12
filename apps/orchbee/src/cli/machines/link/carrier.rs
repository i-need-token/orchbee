//! The link's carrier: the console refusal, the `OBHS` admission under the candidate deadline, the
//! one reader of standard input, the one writer of standard output, and the supervisor that ends
//! the link on idleness and settles what its channels started.

use super::super::shared_carrier::{acceptor_profile, answer, frames, query};
use super::state::{End, Shared, State};
use orchbee_connection_handshake::{
    AcceptorLoad, AcceptorMessage, CANDIDATE_DEADLINE, ConnectionKind, Decision, DrainResult,
    ExitResult, Extensions, ProcessRole, ProcessState, RejectionReason, RequestKind,
    read_initiator_message,
};
use orchbee_machine_protocol::{Connection, Frame, FrameDecoder, FrameKind, PathText, Token, link};
use std::{
    fs::File,
    io::{self, IsTerminal, Read, Write},
    os::windows::io::{AsHandle, BorrowedHandle},
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

/// The status of a link that ended without a violation.
const ENDED: u8 = 0;
/// The status of a refused start, a refused `OBHS` frame, or a violation.
const REFUSED: u8 = 5;
/// Silence after which the link counts as lost.
const IDLE: Duration = Duration::from_secs(90);
/// One read of standard input.
const READ_BUFFER_BYTES: usize = 64 * 1024;
/// The reader stops reading while this many frames other than `data` wait to be written.
const CONTROL_BACKLOG: usize = 256;
/// How long the supervisor lets the writer finish with `bye`.
const FAREWELL: Duration = Duration::from_secs(2);
/// How long the supervisor waits for cancelled routed commands to end.
const COMMANDS_END: Duration = Duration::from_secs(5);
/// How long the supervisor waits for Session starts, past their own 30-second deadline.
const STARTS_END: Duration = Duration::from_secs(35);

/// Runs the helper and returns its exit status.
pub(super) fn serve() -> u8 {
    let started = Instant::now();
    if io::stdin().is_terminal() || io::stdout().is_terminal() {
        return REFUSED;
    }
    // Owned copies of the two handles: the writer may block on a hub that stopped reading, and
    // must never hold the process's own stdout lock while it does.
    let (Ok(input), Ok(mut output)) = (
        duplicate(io::stdin().as_handle()),
        duplicate(io::stdout().as_handle()),
    ) else {
        return REFUSED;
    };
    let Some(facts) = facts() else {
        return REFUSED;
    };
    match admit(input, &mut output, started) {
        Admission::Link(input) => run(input, output, facts),
        Admission::Done(status) => status,
    }
}

fn duplicate(handle: BorrowedHandle<'_>) -> io::Result<File> {
    handle.try_clone_to_owned().map(File::from)
}

/// The `facts` this helper reports: its own build's platform and architecture, its path, and the
/// release store root when that resolves.
fn facts() -> Option<link::Facts> {
    let executable = std::env::current_exe().ok()?;
    let store_root = crate::cli::update::store_root()
        .ok()
        .and_then(|root| root.to_str().and_then(|text| PathText::new(text).ok()));
    Some(link::Facts {
        platform: Token::new("windows").ok()?,
        architecture: Token::new(std::env::consts::ARCH).ok()?,
        executable: PathText::new(executable.to_str()?).ok()?,
        store_root,
    })
}

enum Admission {
    /// The offer was answered; the link runs on this input.
    Link(File),
    /// The frame was answered and the helper exits with this status.
    Done(u8),
}

/// Reads one `OBHS` frame within the candidate deadline counted from `started` and answers it.
fn admit(mut input: File, output: &mut File, started: Instant) -> Admission {
    let profile = acceptor_profile(ConnectionKind::MachineLink);
    let (sender, received) = mpsc::sync_channel(1);
    // A pipe read cannot time out, so a thread reads and the deadline is kept here; on timeout the
    // process exits with that read still pending.
    let reader = thread::Builder::new()
        .name("orchbee-link-admission".into())
        .spawn(move || {
            let message = read_initiator_message(&mut input);
            let _receiver_left = sender.send((message, input));
        });
    if reader.is_err() {
        return Admission::Done(REFUSED);
    }
    let remaining = CANDIDATE_DEADLINE.saturating_sub(started.elapsed());
    let (message, input) = match received.recv_timeout(remaining) {
        Ok((Ok(message), input)) => (message, input),
        Ok((Err(_), _)) => {
            answer(
                output,
                &AcceptorMessage::Rejection(profile.reject(RejectionReason::Malformed)),
            );
            return Admission::Done(REFUSED);
        }
        Err(_) => {
            answer(
                output,
                &AcceptorMessage::Rejection(profile.reject(RejectionReason::Timeout)),
            );
            return Admission::Done(REFUSED);
        }
    };
    let load = AcceptorLoad {
        session_slot_free: true,
        control_slot_free: true,
        state: ProcessState::Ready,
    };
    match profile.decide(&message, load) {
        Decision::Accept(reply) => {
            if answer(output, &AcceptorMessage::Answer(reply)) {
                Admission::Link(input)
            } else {
                Admission::Done(ENDED)
            }
        }
        Decision::Reject(rejection) => {
            answer(output, &AcceptorMessage::Rejection(rejection));
            Admission::Done(REFUSED)
        }
        Decision::Unsupported(reply) => {
            answer(output, &AcceptorMessage::Unsupported(reply));
            Admission::Done(ENDED)
        }
        Decision::Serve(RequestKind::Query) => {
            if let Some(result) = query(&profile, ProcessRole::MachineLink, ProcessState::Ready) {
                answer(output, &AcceptorMessage::QueryResult(result));
            }
            Admission::Done(ENDED)
        }
        Decision::Serve(RequestKind::Drain) => {
            let result = DrainResult {
                state: ProcessState::Stopping,
                extensions: Extensions::new(),
            };
            answer(output, &AcceptorMessage::DrainResult(result));
            Admission::Done(ENDED)
        }
        Decision::Serve(RequestKind::Exit) => {
            answer(output, &AcceptorMessage::ExitResult(ExitResult::default()));
            Admission::Done(ENDED)
        }
    }
}

/// Runs the admitted link until it ends and its farewell and commands settle.
fn run(input: File, output: File, facts: link::Facts) -> u8 {
    let link = Arc::new(Shared::new(State::new(Connection::<link::Link>::acceptor(
        facts,
    ))));
    let writer = {
        let link = link.clone();
        thread::Builder::new()
            .name("orchbee-link-writer".into())
            .spawn(move || write(&link, output))
    };
    if writer.is_err() {
        return REFUSED;
    }
    let reader = {
        let link = link.clone();
        thread::Builder::new()
            .name("orchbee-link-reader".into())
            .spawn(move || read(&link, input))
    };
    if reader.is_err() {
        link.lock().end(End::InputClosed);
        link.wake();
    }
    supervise(&link)
}

/// Ends the link after 90 silent seconds, then lets the writer finish with `bye` and what the
/// channels started settle, each within its bound.
fn supervise(link: &Shared) -> u8 {
    let mut state = link.lock();
    while state.running() {
        let idle_at = state.last_received + IDLE;
        if Instant::now() >= idle_at {
            state.end(End::Idle);
            link.wake();
            break;
        }
        state = link.wait_until(state, idle_at);
    }
    let end = state.end.unwrap_or(End::InputClosed);
    let deadline = Instant::now() + FAREWELL;
    while !state.writer_done && Instant::now() < deadline {
        state = link.wait_until(state, deadline);
    }
    let deadline = Instant::now() + COMMANDS_END;
    while state.channels.commands() > 0 && Instant::now() < deadline {
        state = link.wait_until(state, deadline);
    }
    // A Session start is never abandoned halfway: its own deadline bounds it.
    let deadline = Instant::now() + STARTS_END;
    while state.channels.starts() > 0 && Instant::now() < deadline {
        state = link.wait_until(state, deadline);
    }
    if end == End::Violation {
        REFUSED
    } else {
        ENDED
    }
}

/// The one reader of standard input: decodes frames into the state machine and pauses while the
/// frames it provokes wait to be written.
fn read(link: &Arc<Shared>, mut input: File) {
    let mut decoder = FrameDecoder::new();
    let mut buffer = vec![0; READ_BUFFER_BYTES];
    loop {
        {
            let mut state = link.lock();
            while state.running() && state.connection.queued_frames() >= CONTROL_BACKLOG {
                state = link.wait(state);
            }
            if !state.running() {
                return;
            }
        }
        let read = match input.read(&mut buffer) {
            Ok(0) => 0,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => 0,
        };
        if read == 0 {
            link.lock().end(End::InputClosed);
            link.wake();
            return;
        }
        let bytes = buffer.get(..read).unwrap_or_default();
        match frames(&mut decoder, bytes, |frame| receive(link, frame)) {
            Ok(true) => {}
            Ok(false) => return,
            Err(_) => {
                let mut state = link.lock();
                state.connection.frame_failed();
                state.end(End::Violation);
                link.wake();
                return;
            }
        }
    }
}

/// Takes one frame; false once the link ended.
fn receive(link: &Arc<Shared>, frame: &Frame) -> bool {
    let mut state = link.lock();
    if !state.running() {
        return false;
    }
    state.last_received = Instant::now();
    match state.connection.receive(frame) {
        Ok(receipt) => {
            if let Some(delivery) = receipt.delivery {
                super::channels::deliver(link, &mut state, delivery);
            }
        }
        Err(_) => state.end(End::Violation),
    }
    link.wake();
    state.running()
}

/// The one writer of standard output: one whole frame at a time, in the state machine's order,
/// until `bye` or nothing is left after the link ended.
fn write(link: &Shared, mut output: File) {
    loop {
        let frame = {
            let mut state = link.lock();
            loop {
                if state.end == Some(End::OutputFailed) {
                    break None;
                }
                if let Some(frame) = state.connection.poll_transmit() {
                    if frame.kind() == FrameKind::Data {
                        state.channels.signal(frame.channel());
                    }
                    break Some(frame);
                }
                if !state.running() {
                    break None;
                }
                state = link.wait(state);
            }
        };
        link.wake();
        let Some(frame) = frame else {
            break;
        };
        if output
            .write_all(&frame.encode())
            .and_then(|()| output.flush())
            .is_err()
        {
            link.lock().end(End::OutputFailed);
            break;
        }
    }
    link.lock().writer_done = true;
    link.wake();
}
