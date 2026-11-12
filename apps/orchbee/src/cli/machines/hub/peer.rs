//! One attached connection: its reader, which decodes frames into the state machine and answers
//! each request under the hub state's lock, and its writer, which writes one whole frame at a time
//! until `bye` or the connection's end.

use super::super::shared_carrier::frames;
use super::{
    catalog::Machines,
    state::{Hub, Shared},
};
use orchbee_local_ipc::LocalStream;
use orchbee_machine_protocol::{
    Delivery, FrameDecoder, Id, Message, ResetReason,
    hub::{
        AttachOutcome, ClaimOutcome, Connect, ConnectFailure, ConnectOutcome, ConnectResult,
        DisconnectOutcome, DismissOutcome, MachineState, ReloadOutcome, Reply, Request,
        StopSessionOutcome,
    },
};
use std::{
    io::{self, Read, Write},
    sync::{Arc, MutexGuard},
    thread,
};

/// One read of a connection.
const READ_BUFFER_BYTES: usize = 64 * 1024;
/// The reader stops reading while this many frames wait to be written.
const BACKLOG: usize = 256;

/// Starts the writer of the connection `id`; false when no thread could start.
pub(super) fn start_writer(shared: &Arc<Shared>, id: u64, stream: LocalStream) -> bool {
    let shared = shared.clone();
    thread::Builder::new()
        .name("orchbee-hub-writer".into())
        .spawn(move || write(&shared, id, &stream))
        .is_ok()
}

/// The reader of the connection `id`, on the thread that admitted it.
pub(super) fn read(shared: &Shared, id: u64, stream: &LocalStream) {
    let mut decoder = FrameDecoder::new();
    let mut buffer = vec![0; READ_BUFFER_BYTES];
    let mut input = stream;
    loop {
        {
            let mut hub = shared.lock();
            loop {
                let Some(peer) = hub.peers.get(&id) else {
                    return;
                };
                if !peer.open || peer.connection.violation().is_some() {
                    hub.release(id);
                    shared.wake();
                    return;
                }
                if peer.connection.queued_frames() < BACKLOG {
                    break;
                }
                hub = shared.wait(hub);
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
            frames(&mut decoder, bytes, |frame| receive(shared, id, frame))
        };
        let mut hub = shared.lock();
        match taken {
            Ok(true) => continue,
            // The connection ended, or committed a violation whose `bye` the writer still sends.
            Ok(false) => {
                if read == 0 {
                    hub.close(id);
                }
            }
            Err(_) => {
                if let Some(peer) = hub.peers.get_mut(&id) {
                    peer.connection.frame_failed();
                }
            }
        }
        hub.release(id);
        shared.wake();
        return;
    }
}

/// Takes one frame; false once the connection stops reading.
fn receive(shared: &Shared, id: u64, frame: &orchbee_machine_protocol::Frame) -> bool {
    let mut hub = shared.lock();
    let Some(peer) = hub.peers.get_mut(&id).filter(|peer| peer.open) else {
        return false;
    };
    let delivery = match peer.connection.receive(frame) {
        Ok(receipt) => receipt.delivery,
        Err(_) => {
            shared.wake();
            return false;
        }
    };
    let (mut hub, reading) = match delivery {
        Some(delivery) => deliver(shared, hub, id, delivery),
        None => (hub, true),
    };
    if !reading {
        hub.close(id);
    }
    shared.wake();
    reading
}

fn deliver<'a>(
    shared: &'a Shared,
    mut hub: MutexGuard<'a, Hub>,
    id: u64,
    delivery: Delivery<orchbee_machine_protocol::hub::Hub>,
) -> (MutexGuard<'a, Hub>, bool) {
    match delivery {
        Delivery::Message(Message::Request {
            id: request,
            request: Request::Reload,
            ..
        }) => (reload(shared, hub, id, request), true),
        Delivery::Message(Message::Request {
            id: request,
            request: body,
            ..
        }) => {
            answer(&mut hub, id, request, body);
            (hub, true)
        }
        Delivery::Message(Message::Bye { .. }) => (hub, false),
        Delivery::Open { channel, open, .. } => {
            let reason = if hub.lifetime.is_closing() {
                ResetReason::Stopping
            } else if hub.machines.state(open.machine()).is_some() {
                ResetReason::NotOnline
            } else {
                ResetReason::NotFound
            };
            if let Some(peer) = hub.peers.get_mut(&id) {
                let _reset = peer.connection.reset(channel, reason);
            }
            (hub, true)
        }
        // The state machine answered an unknown request itself; nothing else reaches a hub with
        // no channel open.
        _ => (hub, true),
    }
}

/// Answers every request but `reload` at once, under the lock.
fn answer(hub: &mut Hub, id: u64, request: Id, body: Request) {
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
            if let Some(peer) = hub.peers.get_mut(&id) {
                let _replied = peer.connection.reply(
                    request,
                    Reply::Attach(AttachOutcome::Attached { hub: state }),
                );
            }
            if changed {
                hub.announce();
            }
            if watching {
                let events = hub.machines.events();
                if let Some(peer) = hub.peers.get_mut(&id) {
                    for event in events {
                        let _queued = peer.connection.event(event);
                    }
                }
            }
            None
        }
        Request::Connect(connect) => connect_reply(&hub.machines, &connect, closing),
        Request::Disconnect { .. } if closing => {
            Some(Reply::Disconnect(DisconnectOutcome::Stopping))
        }
        Request::Disconnect { machine } => hub
            .machines
            .state(&machine)
            .is_none()
            .then_some(Reply::Disconnect(DisconnectOutcome::NotFound)),
        Request::Claim { .. } => Some(Reply::Claim(if closing {
            ClaimOutcome::Stopping
        } else {
            ClaimOutcome::Ended
        })),
        Request::Dismiss { .. } => Some(Reply::Dismiss(if closing {
            DismissOutcome::Stopping
        } else {
            DismissOutcome::Ended
        })),
        Request::StopSession { machine } => Some(Reply::StopSession(if closing {
            StopSessionOutcome::Stopping
        } else if hub.machines.state(&machine).is_some() {
            StopSessionOutcome::NotOnline
        } else {
            StopSessionOutcome::NotFound
        })),
        // `deliver` hands `reload` to its own path.
        Request::Reload => Some(Reply::Reload(ReloadOutcome::Stopping)),
    };
    let Some(peer) = hub.peers.get_mut(&id) else {
        return;
    };
    let _answered = match reply {
        Some(reply) => peer.connection.reply(request, reply),
        // A request the hub cannot serve until it can connect.
        None => peer.connection.unsupported(request),
    };
}

/// The `connect-result` the hub can give without connecting, or none for `unsupported`.
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

/// Rereads `machines.json` outside the lock and replies with the catalog then in use.
fn reload<'a>(
    shared: &'a Shared,
    mut hub: MutexGuard<'a, Hub>,
    id: u64,
    request: Id,
) -> MutexGuard<'a, Hub> {
    let outcome = if hub.lifetime.is_closing() {
        ReloadOutcome::Stopping
    } else {
        let order = hub.begin_reload();
        drop(hub);
        let read = Machines::read();
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
    if let Some(peer) = hub.peers.get_mut(&id) {
        let _replied = peer.connection.reply(request, Reply::Reload(outcome));
    }
    hub
}

/// The writer of the connection `id`: frames in the state machine's order until `bye` is written
/// or the connection is over.
fn write(shared: &Shared, id: u64, stream: &LocalStream) {
    let mut output = stream;
    loop {
        let frame = {
            let mut hub = shared.lock();
            loop {
                let Some(peer) = hub.peers.get_mut(&id) else {
                    break None;
                };
                if let Some(frame) = peer.connection.poll_transmit().filter(|_| peer.open) {
                    break Some(frame);
                }
                if !peer.open || peer.connection.is_finished() {
                    break None;
                }
                hub = shared.wait(hub);
            }
        };
        shared.wake();
        let Some(frame) = frame else {
            break;
        };
        if output.write_all(&frame.encode()).is_err() {
            break;
        }
    }
    let mut hub = shared.lock();
    if let Some(peer) = hub.peers.get_mut(&id) {
        peer.writer_done = true;
    }
    hub.close(id);
    hub.release(id);
    shared.wake();
}
