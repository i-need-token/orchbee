//! One attached connection: its reader, which hands each frame to the connection's state machine
//! under the connection's own lock and answers a request under the hub state's lock and then the
//! connection's, and its writer, which writes whole frames, up to a batch at a time, until `bye` or
//! the connection's end. A frame wakes only its own connection's threads. A `connect` that has to
//! wait is answered later by its Machine's transition, and a `stop-session` by its run; `claim` and
//! `dismiss` go to the Machine whose login episode they name.

use super::super::shared_carrier::frames;
use super::{
    catalog::Machines,
    machine::{Answers, Waiter},
    state::{Hub, Line, Shared},
    stop,
};
use orchbee_local_ipc::LocalStream;
use orchbee_machine_protocol::{
    Delivery, FrameDecoder, Id, Message,
    hub::{
        AttachOutcome, ClaimOutcome, Connect, ConnectFailure, ConnectOutcome, ConnectResult,
        DisconnectOutcome, DismissOutcome, MachineState, Origin, ReloadOutcome, Reply, Request,
        StopSessionOutcome,
    },
};
use std::{
    io::{self, Read, Write},
    sync::Arc,
    thread,
};

/// One read of a connection.
const READ_BUFFER_BYTES: usize = 64 * 1024;
/// The reader stops reading while this many frames wait to be written.
const BACKLOG: usize = 256;
/// The most bytes of whole frames one write carries.
const WRITE_BATCH_BYTES: usize = 64 * 1024;

/// Starts the writer of the connection `id`; false when no thread could start.
pub(super) fn start_writer(
    shared: &Arc<Shared>,
    line: &Arc<Line>,
    id: u64,
    stream: LocalStream,
) -> bool {
    let (shared, line) = (shared.clone(), line.clone());
    thread::Builder::new()
        .name("orchbee-hub-writer".into())
        .spawn(move || write(&shared, &line, id, &stream))
        .is_ok()
}

/// The reader of the connection `id`, on the thread that admitted it.
pub(super) fn read(shared: &Arc<Shared>, line: &Line, id: u64, stream: &LocalStream) {
    let mut decoder = FrameDecoder::new();
    let mut buffer = vec![0; READ_BUFFER_BYTES];
    let mut input = stream;
    loop {
        {
            let mut wire = line.lock();
            loop {
                if !wire.open || wire.connection.violation().is_some() {
                    drop(wire);
                    let mut hub = shared.lock();
                    hub.release(id);
                    drop(hub);
                    shared.wake();
                    return;
                }
                if wire.connection.queued_frames() < BACKLOG {
                    break;
                }
                wire = line.wait(wire);
            }
        }
        let read = match input.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Ok(read) => read,
            Err(_) => 0,
        };
        let bytes = buffer.get(..read).unwrap_or_default();
        let taken = if read == 0 {
            Ok(false)
        } else {
            frames(&mut decoder, bytes, |frame| {
                receive(shared, line, id, frame)
            })
        };
        match taken {
            Ok(true) => continue,
            // The connection ended, or committed a violation whose `bye` the writer still sends.
            Ok(false) => {
                if read == 0 {
                    shared.lock().close(id);
                }
            }
            Err(_) => {
                let _violation = line.lock().connection.frame_failed();
                line.wake();
            }
        }
        shared.lock().release(id);
        shared.wake();
        return;
    }
}

/// Takes one frame; false once the connection stops reading.
fn receive(
    shared: &Arc<Shared>,
    line: &Line,
    id: u64,
    frame: &orchbee_machine_protocol::Frame,
) -> bool {
    let delivery = {
        let mut wire = line.lock();
        if !wire.open {
            return false;
        }
        match wire.connection.receive(frame) {
            Ok(receipt) => receipt.delivery,
            Err(_) => {
                drop(wire);
                line.wake();
                return false;
            }
        }
    };
    let reading = match delivery {
        Some(delivery) => deliver(shared, line, id, delivery),
        None => true,
    };
    if !reading {
        shared.lock().close(id);
        shared.wake();
    }
    line.wake();
    reading
}

/// Takes a delivery that depends on the hub state, under its lock and then the connection's.
fn deliver(
    shared: &Arc<Shared>,
    line: &Line,
    id: u64,
    delivery: Delivery<orchbee_machine_protocol::hub::Hub>,
) -> bool {
    match delivery {
        Delivery::Message(Message::Request {
            id: request,
            request: Request::Reload,
            ..
        }) => {
            reload(shared, line, request);
            true
        }
        Delivery::Message(Message::Request {
            id: request,
            request: Request::StopSession { machine },
            ..
        }) => {
            if let Some(outcome) = stop::request(shared, id, request, machine) {
                let _replied = line
                    .lock()
                    .connection
                    .reply(request, Reply::StopSession(outcome));
            }
            // A run began, or a request joined one.
            shared.wake();
            true
        }
        Delivery::Message(Message::Request {
            id: request,
            request: body,
            ..
        }) => {
            let mut hub = shared.lock();
            answer(&mut hub, line, id, request, body);
            drop(hub);
            // The lifetime may have changed, or a carrier may wait to start.
            shared.wake();
            true
        }
        Delivery::Message(Message::Bye { .. }) => false,
        Delivery::Open { channel, open, .. } => {
            shared.lock().open_channel(id, channel, &open);
            true
        }
        Delivery::Data { channel, bytes } => {
            shared.lock().client_data(id, channel, &bytes);
            true
        }
        Delivery::Writable { channel } => {
            let finished = shared.lock().client_writable(id, channel);
            // The last route ending lets an expired hub close.
            if finished {
                shared.wake();
            }
            true
        }
        Delivery::End { channel } => {
            shared.lock().client_end(id, channel);
            true
        }
        // The state machine already reset a `too-large` request toward the client.
        Delivery::Reset { channel, .. } | Delivery::TooLarge { channel } => {
            let mut hub = shared.lock();
            hub.client_reset(id, channel);
            drop(hub);
            // A closing hub waits for its routes to end.
            shared.wake();
            true
        }
        // The state machine answered an unknown request itself; a client sends no other frame.
        _ => true,
    }
}

/// Answers every request but `reload` at once, under the hub state's lock.
fn answer(hub: &mut Hub, line: &Line, id: u64, request: Id, body: Request) {
    let closing = hub.lifetime.is_closing();
    let reply = match body {
        Request::Attach(_) if closing => Some(Reply::Attach(AttachOutcome::Stopping)),
        Request::Attach(attach) => {
            let Some(peer) = hub.peers.get_mut(&id) else {
                return;
            };
            let gained = attach.presence && !peer.presence;
            let dropped = !attach.presence && peer.presence;
            let watching = attach.watch && !peer.watch;
            peer.presence = attach.presence;
            peer.asks = attach.asks;
            peer.watch = attach.watch;
            let changed = if gained {
                hub.lifetime.attach()
            } else {
                if dropped {
                    hub.lifetime.detach();
                }
                false
            };
            let state = hub.lifetime.state();
            {
                let mut wire = line.lock();
                let _replied = wire.connection.reply(
                    request,
                    Reply::Attach(AttachOutcome::Attached { hub: state }),
                );
            }
            if changed {
                hub.announce();
            }
            if gained {
                hub.machines(|machine, cx| machine.attach(cx));
            }
            if watching {
                let events = hub.machines.events();
                let mut wire = line.lock();
                for event in events {
                    let _queued = wire.connection.event(event);
                }
            }
            return;
        }
        Request::Connect(connect) => match connect_reply(&hub.machines, &connect, closing) {
            Some(reply) => Some(reply),
            None if connect.draft.is_some() => None,
            None => {
                let answers = match connect.origin {
                    Origin::User => Answers::User,
                    Origin::Command { answers: true } => Answers::Command,
                    Origin::Command { answers: false } => Answers::Never,
                };
                let waiter = Waiter {
                    peer: id,
                    request,
                    answers,
                };
                hub.machine(&connect.machine, |machine, cx| machine.connect(waiter, cx));
                return;
            }
        },
        Request::Disconnect { .. } if closing => {
            Some(Reply::Disconnect(DisconnectOutcome::Stopping))
        }
        Request::Disconnect { machine } if hub.machines.state(&machine).is_none() => {
            Some(Reply::Disconnect(DisconnectOutcome::NotFound))
        }
        Request::Disconnect { machine } => {
            hub.machine(&machine, |machine, _| machine.disconnect());
            Some(Reply::Disconnect(DisconnectOutcome::Disconnected))
        }
        Request::Claim { .. } if closing => Some(Reply::Claim(ClaimOutcome::Stopping)),
        Request::Claim { login } => Some(Reply::Claim(
            hub.machines
                .login_owner(login)
                .map_or(ClaimOutcome::Ended, |machine| machine.claim(id, login)),
        )),
        Request::Dismiss { .. } if closing => Some(Reply::Dismiss(DismissOutcome::Stopping)),
        Request::Dismiss { login } => {
            let mut outcome = DismissOutcome::Ended;
            if let Some(machine) = hub
                .machines
                .login_owner(login)
                .map(|machine| machine.id().clone())
            {
                hub.machine(&machine, |machine, cx| {
                    let (dismissed, effects) = machine.dismiss(id, login, cx);
                    outcome = dismissed;
                    effects
                });
            }
            Some(Reply::Dismiss(outcome))
        }
        // `deliver` hands `reload` and `stop-session` to their own paths.
        Request::Reload => Some(Reply::Reload(ReloadOutcome::Stopping)),
        Request::StopSession { .. } => Some(Reply::StopSession(StopSessionOutcome::Stopping)),
    };
    let mut wire = line.lock();
    let _answered = match reply {
        Some(reply) => wire.connection.reply(request, reply),
        // A request this build does not serve: a draft.
        None => wire.connection.unsupported(request),
    };
}

/// The `connect-result` the hub gives without asking the Machine, or none when a draft reads
/// `unsupported` or the Machine itself answers.
fn connect_reply(machines: &Machines, connect: &Connect, closing: bool) -> Option<Reply> {
    let result = |failure, state: Option<&MachineState>| {
        Some(Reply::Connect(ConnectResult {
            outcome: ConnectOutcome::NotOnline(failure),
            state: state.map(MachineState::name),
        }))
    };
    if closing {
        return result(ConnectFailure::Stopping, None);
    }
    if connect.draft.is_some() {
        return None;
    }
    match machines.state(&connect.machine) {
        None => result(ConnectFailure::NotFound, None),
        Some(state @ MachineState::Disabled) => {
            result(ConnectFailure::DisabledMachine, Some(state))
        }
        Some(state @ MachineState::Unsupported { .. }) => {
            result(ConnectFailure::UnsupportedMachine, Some(state))
        }
        Some(_) => None,
    }
}

/// Rereads `machines.json` outside every lock and replies with the catalog then in use.
fn reload(shared: &Shared, line: &Line, request: Id) {
    let mut hub = shared.lock();
    let outcome = if hub.lifetime.is_closing() {
        ReloadOutcome::Stopping
    } else {
        let order = hub.begin_reload();
        let root = hub.root.clone();
        drop(hub);
        let read = Machines::read(&root);
        hub = shared.lock();
        match read {
            _ if hub.lifetime.is_closing() => {
                hub.abandon_reload();
                ReloadOutcome::Stopping
            }
            Ok(read) => {
                hub.finish_reload(order, read);
                ReloadOutcome::Reloaded {
                    revision: hub.machines.revision(),
                }
            }
            Err(failure) => {
                hub.abandon_reload();
                failure
            }
        }
    };
    let _replied = line
        .lock()
        .connection
        .reply(request, Reply::Reload(outcome));
    drop(hub);
    // Closing waits for the reads in flight.
    shared.wake();
}

/// The writer of the connection `id`: whole frames in the state machine's order until `bye` is
/// written or the connection is over.
fn write(shared: &Shared, line: &Line, id: u64, stream: &LocalStream) {
    let mut output = stream;
    loop {
        let bytes = {
            let mut wire = line.lock();
            loop {
                let mut bytes = Vec::new();
                while wire.open && bytes.len() < WRITE_BATCH_BYTES {
                    match wire.connection.poll_transmit() {
                        Some(frame) => bytes.extend_from_slice(&frame.encode()),
                        None => break,
                    }
                }
                if !bytes.is_empty() {
                    break Some(bytes);
                }
                if !wire.open || wire.connection.is_finished() {
                    break None;
                }
                wire = line.wait(wire);
            }
        };
        // A reader paused at the backlog may read again.
        line.wake();
        let Some(bytes) = bytes else {
            break;
        };
        if output.write_all(&bytes).is_err() {
            break;
        }
    }
    let mut hub = shared.lock();
    if let Some(peer) = hub.peers.get_mut(&id) {
        peer.writer_done = true;
    }
    hub.close(id);
    hub.release(id);
    drop(hub);
    shared.wake();
}
