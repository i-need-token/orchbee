//! The Windows holder thread and its request threads. A request thread holds the owner only for
//! its request, and never the shared lock while it waits.
use super::{Ask, Hub, Reach, Shared};
use crate::{
    background_thread::BackgroundThread,
    machine_discovery::overview::Blocked,
    machine_hub::{
        AttachFailure, Attachment, HubClient, HubWatch, Incompatible, Notice, Opening, data_root,
        present,
    },
    machine_retry::Backoff,
};
use orchbee_machine_protocol::{
    MachineId,
    hub::{Connect, ConnectOutcome, ConnectResult, Event, HubState, Origin, Reply, Request},
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
/// How long one attachment, which may start a hub, may take; the stop request ends it at once.
const ATTACH: Duration = Duration::from_secs(10);
/// How long joining a hub somebody else started may take.
const JOIN: Duration = Duration::from_secs(3);
/// How long the `query` that names a hub the holder waits out may take.
const QUERY: Duration = Duration::from_secs(2);
/// How often the holder looks for its stop request and for a selection while it waits.
const POLL: Duration = Duration::from_millis(100);
/// How often a stopped holder looks for a hub somebody else started.
const LOOK: Duration = Duration::from_secs(1);
/// How long a `reload` may take.
const RELOAD: Duration = Duration::from_secs(10);
/// How long a selection's `connect` may take, its wait for a hub included; the selection itself
/// waits at most as long.
const CONNECT: Duration = Duration::from_secs(60);

/// The holder's attachments, as selections waiting for a hub read them.
#[derive(Default)]
pub(super) struct Attempts {
    /// A selection waits for a hub the holder does not reach, so the holder's next attachment may
    /// start one. The holder clears it as that attachment begins, or once it attaches.
    asked: bool,
    /// The attachments begun, and those that ended in an owner or a failure.
    begun: u64,
    settled: u64,
}

/// What the holder does next.
enum Next {
    /// Attach on a person's act, the TUI's start or a selection: start a hub when none runs, once
    /// a closing one is gone.
    Start,
    /// Attach after a hub crashed: start one when none runs, but none in place of a hub that
    /// answers it is stopping.
    Restart,
    /// Attach to the hub somebody else started.
    Join,
    /// Restart after the retry policy's pause.
    Retry,
    /// Start none: wait for a selection, or for a hub somebody else starts; `going` when the hub
    /// last reached may still be closing.
    Stopped { going: bool },
    /// Wait for the cause to go, or for a selection.
    Blocked(Blocked),
}

/// Why following an owner ended.
enum Left {
    Stop,
    /// The hub is closing and a selection waits for a hub: the next one may start now.
    Asked,
    /// The owner ended; whether its hub said it was closing.
    Ended(AttachFailure, bool),
}

pub(super) fn spawn(shared: Arc<Shared>) -> io::Result<BackgroundThread> {
    BackgroundThread::spawn("orchbee-machine-hub", move || hold(&shared))
}

/// Attaches and follows the hub's events, until the stop request. A hub that crashed is started
/// again under the machine retry policy; a hub that was asked to stop is not: only a selection
/// starts the next, and a hub somebody else starts is joined. A failure that trying again cannot
/// cure waits for its cause to go.
fn hold(shared: &Arc<Shared>) {
    let mut backoff = Backoff::new();
    // Whether a hub may start without a selection: not once the hub was asked to stop, until a
    // person acts again.
    let mut may_start = true;
    let mut next = Next::Start;
    while !shared.stopping() {
        next = match next {
            Next::Start => {
                may_start = true;
                attach(shared, Opening::Start, &mut backoff, &mut may_start)
            }
            Next::Restart => attach(shared, Opening::Restart, &mut backoff, &mut may_start),
            Next::Join => attach(shared, Opening::Join, &mut backoff, &mut may_start),
            Next::Retry => {
                shared.reach(Reach::Unavailable);
                let resume = Instant::now() + backoff.fail(Instant::now());
                if !wait(shared, |until| {
                    sleep(until);
                    Instant::now() >= resume
                }) {
                    return;
                }
                if asked(shared) {
                    Next::Start
                } else {
                    Next::Restart
                }
            }
            Next::Stopped { going } => {
                shared.reach(Reach::Stopped);
                match stopped(shared, going) {
                    Some(next) => next,
                    None => return,
                }
            }
            Next::Blocked(blocked) => {
                // A missing data root names no hub to wait for: only a selection tries again.
                let watch = (blocked != Blocked::DataRoot)
                    .then(data_root)
                    .and_then(Result::ok)
                    .map(|root| HubWatch::new(&root, Instant::now() + QUERY));
                shared.reach(Reach::Blocked(blocked));
                if !wait(shared, |until| match &watch {
                    Some(watch) => watch.gone(until),
                    None => sleep(until),
                }) {
                    return;
                }
                if asked(shared) {
                    Next::Start
                } else if may_start {
                    Next::Restart
                } else {
                    Next::Stopped { going: false }
                }
            }
        };
    }
}

/// Sleeps until `until`; never done.
fn sleep(until: Instant) -> bool {
    thread::sleep(until.saturating_duration_since(Instant::now()));
    false
}

/// Calls `done` with a deadline one poll away until it says done or a selection asks for a hub;
/// false at the stop request.
fn wait(shared: &Shared, mut done: impl FnMut(Instant) -> bool) -> bool {
    loop {
        if shared.stopping() {
            return false;
        }
        if asked(shared) || done(Instant::now() + POLL) {
            return true;
        }
    }
}

fn asked(shared: &Shared) -> bool {
    shared.lock().attempts.asked
}

/// After the hub was asked to stop: waits for a hub still `going` to go, then for a selection,
/// which starts a hub, or for a hub somebody else started, which the holder joins; `None` at the
/// stop request.
fn stopped(shared: &Shared, going: bool) -> Option<Next> {
    if let Ok(root) = data_root() {
        if going && present(&root) {
            let watch = HubWatch::new(&root, Instant::now() + QUERY);
            if !wait(shared, |until| watch.gone(until)) {
                return None;
            }
        }
        let mut look = Instant::now();
        if !wait(shared, |until| {
            if Instant::now() >= look {
                look = Instant::now() + LOOK;
                if present(&root) {
                    return true;
                }
            }
            sleep(until)
        }) {
            return None;
        }
    } else if !wait(shared, sleep) {
        return None;
    }
    Some(if asked(shared) {
        Next::Start
    } else {
        Next::Join
    })
}

/// One attachment and, once attached, the hub's events until the owner ends; what follows.
fn attach(
    shared: &Arc<Shared>,
    opening: Opening,
    backoff: &mut Backoff,
    may_start: &mut bool,
) -> Next {
    shared.update(|state| {
        state.attempts.begun += 1;
        if opening == Opening::Start {
            state.attempts.asked = false;
        }
    });
    if opening == Opening::Start {
        shared.reach(Reach::Attaching);
    }
    let bound = match opening {
        Opening::Start | Opening::Restart => ATTACH,
        Opening::Join => JOIN,
    };
    let client =
        match HubClient::open_with(ATTACHMENT, Instant::now() + bound, opening, &shared.cancel) {
            Ok((client, _attached)) => Arc::new(client),
            Err(failure) => {
                let next = failed(failure, opening);
                if matches!(next, Next::Stopped { .. }) {
                    *may_start = false;
                }
                // Published with the count, so a selection waiting on this attachment reads both.
                let reach = match &next {
                    Next::Blocked(blocked) => Some(Reach::Blocked(blocked.clone())),
                    Next::Retry => Some(Reach::Unavailable),
                    Next::Stopped { .. } => Some(Reach::Stopped),
                    Next::Start | Next::Restart | Next::Join => None,
                };
                shared.update(|state| {
                    state.attempts.settled = state.attempts.begun;
                    if let Some(reach) = reach {
                        state.view.reach = reach;
                    }
                });
                return next;
            }
        };
    backoff.online(Instant::now());
    shared.update(|state| {
        state.link = Some(client.clone());
        state.view.reach = Reach::Attached;
        state.view.epoch = state.view.epoch.wrapping_add(1);
        state.view.states.clear();
        state.view.catalog = None;
        state.reloading = None;
        state.attempts.asked = false;
        state.attempts.settled = state.attempts.begun;
    });
    let left = follow(shared, &client);
    shared.update(|state| {
        state.link = None;
        state.view.reach = Reach::Attaching;
        state.view.states.clear();
        state.view.catalog = None;
    });
    // A carrier or request thread may still hold the owner; the last of them closes it.
    drop(client);
    match left {
        Left::Stop | Left::Asked => Next::Start,
        Left::Ended(failure, closing) => {
            // The hub said it was closing, or the owner found it so: it was asked to stop, or
            // expired with nothing holding it. A hub of another version that replaced it is waited
            // out, not replaced.
            if closing || failure == AttachFailure::Closing {
                *may_start = false;
            }
            match failed(failure, Opening::Restart) {
                Next::Retry if !*may_start => Next::Stopped { going: true },
                next => next,
            }
        }
    }
}

/// What follows a failed attachment, or an owner's end, as `opening` attached.
fn failed(failure: AttachFailure, opening: Opening) -> Next {
    let blocked = match failure {
        AttachFailure::Incompatible(Incompatible::Held { version }) => Blocked::Held { version },
        AttachFailure::Incompatible(Incompatible::Environment { environment }) => {
            Blocked::Environment { environment }
        }
        AttachFailure::Incompatible(Incompatible::Protocol) | AttachFailure::Lost => {
            Blocked::Protocol
        }
        AttachFailure::Denied => Blocked::Denied,
        AttachFailure::DataRoot => Blocked::DataRoot,
        // A hub that is closing was asked to stop: start none in its place.
        AttachFailure::Closing => return Next::Stopped { going: true },
        // Joining found no hub: stay stopped.
        AttachFailure::Unavailable | AttachFailure::Timeout if opening == Opening::Join => {
            return Next::Stopped { going: true };
        }
        // No hub could start for now, or the bound passed otherwise.
        AttachFailure::Unavailable | AttachFailure::Timeout => return Next::Retry,
    };
    Next::Blocked(blocked)
}

/// Publishes the hub's Machine states until the owner ends or the holder is asked to stop. From a
/// lost connection until the re-attachment, and once the hub says it is closing, no state holds.
fn follow(shared: &Arc<Shared>, client: &HubClient) -> Left {
    let mut live = true;
    let mut closing = false;
    loop {
        if shared.stopping() {
            return Left::Stop;
        }
        if closing && asked(shared) {
            return Left::Asked;
        }
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
            Some(Notice::Event(Event::Hub(HubState::Closing))) => {
                closing = true;
                shared.reach(Reach::Closing);
            }
            // A draft Machine is another client's; a catalog Machine's state replaces the last.
            Some(Notice::Event(Event::Machine(event))) if !event.draft && live && !closing => {
                shared.update(|state| {
                    state
                        .view
                        .states
                        .insert(event.machine.to_string(), event.state.name());
                });
            }
            Some(Notice::Event(_)) => {}
            // A new hub reports every Machine again.
            Some(Notice::Reattached(_)) => {
                live = true;
                closing = false;
                shared.update(|state| {
                    state.view.reach = Reach::Attached;
                    state.view.states.clear();
                    state.view.catalog = None;
                    state.reloading = None;
                    state.attempts.asked = false;
                });
            }
            Some(Notice::Ended(failure)) => return Left::Ended(failure, closing),
        }
        if live && !closing && !client.connected() {
            live = false;
            shared.reach(Reach::Attaching);
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
            let online = request_connect(&shared, &machine, number);
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

/// Waits for a live hub, asking the holder for one, and sends `connect` on it. An attachment that
/// begins after the ask and fails answers the selection; so does the deadline.
fn request_connect(shared: &Shared, machine: &str, number: u64) -> bool {
    let Ok(id) = MachineId::new(machine) else {
        return false;
    };
    let deadline = Instant::now() + CONNECT;
    // The attachments begun when the selection asked for a hub.
    let mut asked_at: Option<u64> = None;
    loop {
        if shared.stopping() || Instant::now() >= deadline {
            return false;
        }
        let (client, failed) = {
            let mut state = shared.lock();
            if state.view.asks.get(machine) != Some(&Ask::Pending(number)) {
                // A later ask replaced this one.
                return false;
            }
            let client = state
                .link
                .clone()
                .filter(|_| state.view.reach == Reach::Attached);
            let answered = asked_at.is_some_and(|at| state.attempts.settled > at);
            // A closing hub that outlasted the attachment goes by itself: ask for the next.
            if client.is_none()
                && (asked_at.is_none()
                    || (answered && matches!(state.view.reach, Reach::Closing | Reach::Stopped)))
            {
                state.attempts.asked = true;
                asked_at = Some(state.attempts.begun);
            }
            let failed =
                answered && matches!(state.view.reach, Reach::Unavailable | Reach::Blocked(_));
            (client, failed)
        };
        if failed {
            return false;
        }
        let Some(client) = client else {
            thread::sleep(POLL);
            continue;
        };
        // A Machine saved after the hub read its catalog is unknown to it until it rereads.
        if let Some(stale) = shared.reload_due() {
            let _reply = stale.request(Request::Reload, Instant::now() + RELOAD);
        }
        let request = Request::Connect(Connect {
            machine: id.clone(),
            origin: Origin::Command { answers: false },
            draft: None,
        });
        let online = matches!(
            client.request(request, deadline),
            Ok(Reply::Connect(ConnectResult {
                outcome: ConnectOutcome::Online { .. },
                ..
            }))
        );
        if online {
            return true;
        }
        // The hub went away or began closing meanwhile: wait for the next one.
        if shared.lock().view.reach == Reach::Attached {
            return false;
        }
        asked_at = None;
    }
}
