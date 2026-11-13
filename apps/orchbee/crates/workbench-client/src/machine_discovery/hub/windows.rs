//! The Windows holder thread and its request threads. A request thread holds the owner only for
//! its request, and never the shared lock while it waits.
use super::{Ask, Hub, Reach, Shared};
use crate::{
    background_thread::BackgroundThread,
    machine_hub::{Attachment, HubClient, Notice},
    machine_retry::Backoff,
};
use orchbee_machine_protocol::{
    MachineId,
    hub::{Connect, ConnectOutcome, ConnectResult, Event, Origin, Reply, Request},
};
use std::{
    io,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

/// What the TUI is to the hub. It answers no login question until it shows login notices.
const ATTACHMENT: Attachment = Attachment {
    presence: true,
    asks: false,
    watch: true,
};
/// How long one attachment, which may start a hub, may take; within the Client's exit budget.
const ATTACH: Duration = Duration::from_secs(10);
/// How often the holder looks for its stop request while it waits.
const POLL: Duration = Duration::from_millis(100);
/// How long a `reload` may take.
const RELOAD: Duration = Duration::from_secs(10);
/// How long a selection's `connect` may take; the selection itself waits at most as long.
const CONNECT: Duration = Duration::from_secs(60);

pub(super) fn spawn(shared: Arc<Shared>) -> io::Result<BackgroundThread> {
    BackgroundThread::spawn("orchbee-machine-hub", move || hold(&shared))
}

/// Attaches, follows the hub's events until the owner ends, and attaches again under the machine
/// retry policy, until the stop request.
fn hold(shared: &Arc<Shared>) {
    let mut backoff = Backoff::new();
    while !shared.stopping() {
        if let Ok((client, _attached)) = HubClient::open(ATTACHMENT, Instant::now() + ATTACH) {
            let client = Arc::new(client);
            backoff.online(Instant::now());
            shared.update(|state| {
                state.link = Some(client.clone());
                state.view.reach = Reach::Attached;
                state.view.epoch = state.view.epoch.wrapping_add(1);
                state.view.states.clear();
                state.view.catalog = None;
                state.reloading = None;
            });
            follow(shared, &client);
            shared.update(|state| {
                state.link = None;
                state.view.reach = Reach::Unavailable;
                state.view.states.clear();
                state.view.catalog = None;
                state.view.asks.clear();
            });
            // A carrier or request thread may still hold the owner; the last of them closes it.
            drop(client);
            if shared.stopping() {
                return;
            }
        } else {
            shared.update(|state| state.view.reach = Reach::Unavailable);
        }
        let resume = Instant::now() + backoff.fail(Instant::now());
        while !shared.stopping() {
            let remaining = resume.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            thread::sleep(remaining.min(POLL));
        }
    }
}

/// Publishes the hub's Machine states until the owner ends or the holder is asked to stop.
fn follow(shared: &Arc<Shared>, client: &HubClient) {
    while !shared.stopping() {
        match client.next(Instant::now() + POLL) {
            None => {}
            Some(Notice::Event(Event::Catalog(catalog))) => {
                shared.update(|state| {
                    state.view.catalog = Some(catalog.revision().get());
                    let listed: Vec<String> =
                        catalog.machines().iter().map(ToString::to_string).collect();
                    state.view.states.retain(|id, _| listed.contains(id));
                });
                if let Some(client) = shared.reload_due() {
                    reload(client);
                }
            }
            // A draft Machine is another client's; a catalog Machine's state replaces the last.
            Some(Notice::Event(Event::Machine(event))) if !event.draft => {
                shared.update(|state| {
                    state
                        .view
                        .states
                        .insert(event.machine.to_string(), event.state.name());
                });
            }
            Some(Notice::Event(_)) => {}
            // A new hub reports every Machine again.
            Some(Notice::Reattached(_)) => shared.update(|state| {
                state.view.states.clear();
                state.view.catalog = None;
                state.reloading = None;
            }),
            Some(Notice::Ended(_)) => return,
        }
    }
}

/// Asks the hub to reread the catalog, which it reads at another revision than the TUI.
pub(super) fn reload(client: Hub) {
    // A thread that cannot start leaves the catalog to the next revision.
    let _started = thread::Builder::new()
        .name("orchbee-machine-reload".into())
        .spawn(move || {
            // The hub's `catalog` event reports the outcome.
            let _reply = client.request(Request::Reload, Instant::now() + RELOAD);
        });
}

/// Asks the hub to bring `machine` online, joining its attempt or starting one, and settles ask
/// `number` unless a later ask replaced it.
pub(super) fn connect(shared: &Arc<Shared>, machine: String, number: u64) {
    let key = machine.clone();
    let shared_for_thread = shared.clone();
    let started = thread::Builder::new()
        .name("orchbee-machine-connect".into())
        .spawn(move || {
            let shared = shared_for_thread;
            let online = request_connect(&shared, &machine);
            shared.update(|state| {
                if state.view.asks.get(&machine) == Some(&Ask::Pending(number)) {
                    if online {
                        state.view.asks.remove(&machine);
                    } else {
                        state.view.asks.insert(machine, Ask::Failed);
                    }
                }
            });
        });
    if started.is_err() {
        shared.update(|state| {
            state.view.asks.insert(key, Ask::Failed);
        });
    }
}

fn request_connect(shared: &Shared, machine: &str) -> bool {
    let Some(client) = shared.lock().link.clone() else {
        return false;
    };
    let Ok(machine) = MachineId::new(machine) else {
        return false;
    };
    // A Machine saved after the hub read its catalog is unknown to it until it rereads.
    if let Some(stale) = shared.reload_due() {
        let _reply = stale.request(Request::Reload, Instant::now() + RELOAD);
    }
    let request = Request::Connect(Connect {
        machine,
        origin: Origin::Command { answers: false },
        draft: None,
    });
    matches!(
        client.request(request, Instant::now() + CONNECT),
        Ok(Reply::Connect(ConnectResult {
            outcome: ConnectOutcome::Online { .. },
            ..
        }))
    )
}
