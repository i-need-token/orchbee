//! One attached connection: its reader, which hands each frame to the connection's state machine
//! under the connection's own lock and answers a request under the hub state's lock and then the
//! connection's, and its writer, which writes whole frames, up to a batch at a time, until `bye` or
//! the connection's end. A frame wakes only its own connection's threads. A `connect` that has to
//! wait is answered later by its Machine's transition, and a `stop-session` by its run; `claim` and
//! `dismiss` go to the Machine whose login episode they name.

use super::super::shared_carrier::frames;
use super::{
    catalog::Machines,
    draft_ledger::DraftLedger,
    machine::{Answers, Kind, Machine, Route, Waiter},
    state::{Hub, Line, Shared},
    stop,
};
use orchbee_machine_protocol::{
    Delivery, FrameDecoder, Id, Message,
    hub::{
        AttachOutcome, CapacityOutcome, ClaimOutcome, Connect, ConnectFailure, ConnectOutcome,
        ConnectResult, DisconnectOutcome, DismissOutcome, Draft, MachineState, Origin,
        ReloadOutcome, Reply, Request, StopSessionOutcome,
    },
};
use orchbee_workbench_client::application::machine_endpoint::LocalStream;
use orchbee_workbench_client::application::machines::valid_target;
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
            request: Request::Connect(connect),
            ..
        }) => {
            connect_request(shared, line, id, request, connect);
            // The lifetime may have changed, or a carrier may wait to start.
            shared.wake();
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
            request: Request::Capacity { machine },
            ..
        }) => {
            let answered = shared.lock().capacity_request(id, request, machine);
            if let Some(outcome) = answered {
                let _replied = line
                    .lock()
                    .connection
                    .reply(request, Reply::Capacity(outcome));
            }
            // A wait began, which the supervisor bounds.
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

/// Answers `connect`, flushing a newly recorded draft ledger off lock before starting a carrier.
fn connect_request(shared: &Arc<Shared>, line: &Line, id: u64, request: Id, connect: Connect) {
    let mut hub = shared.lock();
    if let Some(reply) = connect_reply(&hub.machines, &connect, id, hub.lifetime.is_closing()) {
        drop(hub);
        let _replied = line.lock().connection.reply(request, reply);
        return;
    }
    let answers = match connect.origin {
        Origin::User => Answers::User,
        Origin::Command { answers: true } => Answers::Command,
        Origin::Command { answers: false } => Answers::Never,
    };
    if let Some(route) = connect.draft.as_ref().and_then(route)
        && hub.machines.state(&connect.machine).is_none()
        && hub.machines.draft(&connect.machine).is_none()
    {
        if let Some(older) = hub.machines.owned_draft(id).cloned() {
            hub.drop_draft(&older, |machine, _| machine.remove());
        }
        hub.add_draft(Machine::draft(connect.machine.clone(), route, id));
        let persister = hub.drafts.persister();
        drop(hub);
        persister.flush();
        hub = shared.lock();
        if let Some(reply) = connect_reply(&hub.machines, &connect, id, hub.lifetime.is_closing()) {
            drop(hub);
            let _replied = line.lock().connection.reply(request, reply);
            return;
        }
    }
    let waiter = Waiter::new(id, request, answers);
    hub.machine(&connect.machine, |machine, cx| machine.connect(waiter, cx));
}

/// Answers every request other than `reload`, `stop-session`, `connect` and `capacity`, under the
/// hub lock.
fn answer(hub: &mut Hub, line: &Line, id: u64, request: Id, body: Request) {
    let closing = hub.lifetime.is_closing();
    let reply = match body {
        Request::Attach(_) if closing => Reply::Attach(AttachOutcome::Stopping),
        Request::Attach(attach) => {
            let Some(peer) = hub.peers.get_mut(&id) else {
                return;
            };
            // A client whose version does not show offered logins never asks, whatever it states.
            let asks = attach.asks && peer.shows_offers;
            let gained = attach.presence && !peer.presence;
            let dropped = !attach.presence && peer.presence;
            let watching = attach.watch && !peer.watch;
            let asking = asks && !peer.asks;
            let unasked = !asks && peer.asks;
            peer.presence = attach.presence;
            peer.asks = asks;
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
            // A connection that starts asking is offered the running `command` episodes; one
            // that stops no longer is, and an episode it claimed is offered to the others again.
            if asking {
                hub.machines(|machine, _| machine.asking(id));
            }
            if unasked {
                hub.machines(|machine, cx| machine.stopped_asking(id, cx));
            }
            if watching {
                let events = hub.machines.events(id);
                let catalog_parts = hub.peers.get(&id).is_some_and(|peer| peer.catalog_parts);
                let mut wire = line.lock();
                for event in events {
                    wire.queue(catalog_parts, event);
                }
            }
            return;
        }
        Request::Disconnect { .. } if closing => Reply::Disconnect(DisconnectOutcome::Stopping),
        // Disconnecting a draft drops it; another connection's draft is not there for this one.
        Request::Disconnect { machine } if hub.machines.state(&machine).is_none() => {
            if hub
                .machines
                .draft(&machine)
                .is_some_and(|draft| draft.owner() == Some(id))
            {
                hub.drop_draft(&machine, |machine, _| machine.disconnect());
                Reply::Disconnect(DisconnectOutcome::Disconnected)
            } else {
                Reply::Disconnect(DisconnectOutcome::NotFound)
            }
        }
        Request::Disconnect { machine } => {
            hub.machine(&machine, |machine, _| machine.disconnect());
            Reply::Disconnect(DisconnectOutcome::Disconnected)
        }
        Request::Claim { .. } if closing => Reply::Claim(ClaimOutcome::Stopping),
        Request::Claim { login } => {
            let mut outcome = ClaimOutcome::Ended;
            if let Some(machine) = hub
                .machines
                .login_owner(login, id)
                .map(|machine| machine.id().clone())
            {
                hub.machine(&machine, |machine, cx| {
                    let (claimed, effects) = machine.claim(id, login, cx);
                    outcome = claimed;
                    effects
                });
            }
            Reply::Claim(outcome)
        }
        Request::Dismiss { .. } if closing => Reply::Dismiss(DismissOutcome::Stopping),
        Request::Dismiss { login } => {
            let mut outcome = DismissOutcome::Ended;
            if let Some(machine) = hub
                .machines
                .login_owner(login, id)
                .map(|machine| machine.id().clone())
            {
                hub.machine(&machine, |machine, cx| {
                    let (dismissed, effects) = machine.dismiss(id, login, cx);
                    outcome = dismissed;
                    effects
                });
            }
            Reply::Dismiss(outcome)
        }
        // `deliver` hands `reload`, `stop-session`, `connect` and `capacity` to their own paths.
        Request::Connect(_) => Reply::Connect(ConnectResult {
            outcome: ConnectOutcome::NotOnline(ConnectFailure::Stopping),
            state: None,
        }),
        Request::Reload => Reply::Reload(ReloadOutcome::Stopping),
        Request::StopSession { .. } => Reply::StopSession(StopSessionOutcome::Stopping),
        Request::Capacity { .. } => Reply::Capacity(CapacityOutcome::Stopping),
    };
    let _answered = line.lock().connection.reply(request, reply);
}

/// Where a draft goes, when its target passes the catalog record's own rules.
fn route(draft: &Draft) -> Option<Route> {
    valid_target(draft.target.as_str()).then(|| Route {
        target: draft.target.as_str().to_owned(),
        session: draft.session.clone(),
    })
}

/// The `connect-result` the hub gives without asking the Machine, or none when the Machine itself
/// answers. A draft is `invalid-draft` when its target breaks the record's rules or its id names
/// a Machine it does not describe, or another connection's draft; a draft the catalog already
/// holds as described connects as that Machine.
fn connect_reply(
    machines: &Machines,
    connect: &Connect,
    peer: u64,
    closing: bool,
) -> Option<Reply> {
    let result = |failure, state: Option<&MachineState>| {
        Some(Reply::Connect(ConnectResult {
            outcome: ConnectOutcome::NotOnline(failure),
            state: state.map(MachineState::name),
        }))
    };
    if closing {
        return result(ConnectFailure::Stopping, None);
    }
    if let Some(draft) = &connect.draft {
        let Some(route) = route(draft) else {
            return result(ConnectFailure::InvalidDraft, None);
        };
        let kind = Kind::Ssh(route);
        let described = match (
            machines.state(&connect.machine),
            machines.draft(&connect.machine),
        ) {
            (Some(_), _) => machines.saved_as(&connect.machine, &kind),
            (None, Some(other)) => other.owner() == Some(peer) && other.drafts(&kind),
            (None, None) => true,
        };
        if !described {
            return result(ConnectFailure::InvalidDraft, None);
        }
        // A draft not yet saved is connected as itself; a saved one goes on as its record.
        machines.state(&connect.machine)?;
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
fn reload(shared: &Arc<Shared>, line: &Line, request: Id) {
    let mut hub = shared.lock();
    let mut sweep = Vec::new();
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
                sweep = hub.finish_reload(order, read);
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
    DraftLedger::spawn_sweep(Arc::clone(shared), sweep);
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
