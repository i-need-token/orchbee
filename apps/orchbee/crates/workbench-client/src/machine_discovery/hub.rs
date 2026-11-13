//! The TUI's hold on the machine hub: one background attachment with presence, asks and watch,
//! the Machine states the hub reports, published into one shared view, the login notices of the
//! episodes this TUI claimed, queued in order, and the `reload`, `connect` and `dismiss` requests
//! and answer deliveries discovery asks for. A frame copies the view and never waits for the hub.
//! Windows only, as the hub is; `apps/orchbee/docs/machines.md#the-tui-as-a-hub-client` owns the
//! rules.
#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
use unix as platform;
#[cfg(windows)]
use windows as platform;

use super::overview::Blocked;
use crate::{
    background_thread::BackgroundThread,
    machine_carrier::Gate,
    machine_carrier::Hub,
    machine_login::{Answer, Notice, Token},
    machines::ProfileId,
    ui_wakeup::UiWakeup,
};
use orchbee_machine_protocol::{
    Id,
    hub::{AnswerEndpoint, ConnectFailure, MachineState, MachineStateName},
};
use std::{
    collections::HashMap,
    io,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

/// Whether the holder reaches a hub, and when it does not, why.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(
    not(windows),
    expect(dead_code, reason = "only the Windows holder reaches a hub")
)]
pub(super) enum Reach {
    /// An attachment is being made, or the owner is re-attaching after a loss: no Machine state is
    /// known now.
    #[default]
    Attaching,
    Attached,
    /// The attached hub said it is closing: its Machine states no longer hold.
    Closing,
    /// The hub was asked to stop and no hub took its place. The holder starts none; a selection
    /// starts one, and a hub somebody else starts is joined.
    Stopped,
    /// No hub could be started or reached; the holder tries again under the machine retry policy.
    Unavailable,
    /// A failure that trying again cannot cure; the holder waits for its cause to go, or for a
    /// selection, which tries once more.
    Blocked(Blocked),
}

/// A `connect` discovery asked for on a selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Ask {
    /// Waiting for a hub and then for the reply of the request with this number.
    Pending(u64),
    /// The reply said the Machine is not online, with the failure it named, or no hub could be
    /// reached for it, or no reply came.
    Failed(Option<ConnectFailure>),
}

/// What the holder last learned, as discovery copies it.
#[derive(Clone, Debug, Default)]
pub(super) struct View {
    pub(super) reach: Reach,
    /// Counts the owners the holder opened, so a carrier of an ended owner never reads online.
    pub(super) epoch: u64,
    /// The hub's state of each catalog Machine it reported, by id; empty unless attached.
    pub(super) states: HashMap<String, MachineState>,
    /// The revision of the hub's last `catalog` event.
    pub(super) catalog: Option<u64>,
    pub(super) asks: HashMap<String, Ask>,
}

impl View {
    pub(super) fn state(&self, machine: &ProfileId) -> Option<MachineStateName> {
        self.states.get(machine.as_str()).map(MachineState::name)
    }

    /// The hub's state of `machine` with the facts it carries.
    pub(super) fn facts(&self, machine: &ProfileId) -> Option<&MachineState> {
        self.states.get(machine.as_str())
    }
}

struct State {
    version: u64,
    view: View,
    /// The current owner; only the holder thread installs and removes it.
    link: Option<Hub>,
    /// The catalog revision the TUI last read.
    wanted: Option<u64>,
    /// The revision a `reload` was asked for on the current attachment.
    reloading: Option<u64>,
    next_ask: u64,
    /// The login notices of the episodes this TUI claimed, in the order they came, until
    /// discovery takes them.
    logins: Vec<Notice>,
    /// The holder's attachments, as selections waiting for a hub read them.
    #[cfg(windows)]
    attempts: platform::Attempts,
}

/// The view and the owner, shared by the holder thread, discovery, observers, carriers, and
/// request threads. Only the holder thread ever holds the owner while it is the last one.
pub(super) struct Shared {
    state: Mutex<State>,
    stop: AtomicBool,
    /// Ends the holder's attachment in progress at the stop request.
    #[cfg(windows)]
    cancel: crate::machine_hub::OpenCancel,
    wakeup: Option<UiWakeup>,
}

impl Shared {
    fn new(wakeup: Option<UiWakeup>) -> Self {
        Self {
            state: Mutex::new(State {
                version: 0,
                view: View::default(),
                link: None,
                wanted: None,
                reloading: None,
                next_ask: 0,
                logins: Vec::new(),
                #[cfg(windows)]
                attempts: platform::Attempts::default(),
            }),
            stop: AtomicBool::new(false),
            #[cfg(windows)]
            cancel: crate::machine_hub::OpenCancel::default(),
            wakeup,
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Applies `change`, publishes it, and wakes the UI loop.
    fn update(&self, change: impl FnOnce(&mut State)) {
        {
            let mut state = self.lock();
            change(&mut state);
            state.version = state.version.wrapping_add(1);
        }
        if let Some(wakeup) = &self.wakeup {
            wakeup.signal();
        }
    }

    /// Publishes `reach` when it changed; leaving `Attached` forgets every Machine state, and
    /// every login episode ends with the link.
    #[cfg(windows)]
    fn reach(&self, reach: Reach) {
        if self.lock().view.reach == reach {
            return;
        }
        self.update(|state| {
            if reach != Reach::Attached {
                state.view.states.clear();
                state.view.catalog = None;
                if state.view.reach == Reach::Attached {
                    state.logins.push(Notice::Lost);
                }
            }
            state.view.reach = reach;
        });
    }

    /// Queues a login notice for discovery and wakes the UI loop.
    #[cfg(windows)]
    fn login(&self, notice: Notice) {
        self.update(|state| state.logins.push(notice));
    }

    #[cfg(windows)]
    fn stopping(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    /// The view, when it changed since `seen`.
    pub(super) fn read(&self, seen: u64) -> Option<(u64, View)> {
        let state = self.lock();
        (state.version != seen).then(|| (state.version, state.view.clone()))
    }

    fn link(&self, epoch: u64) -> Option<Hub> {
        let state = self.lock();
        state
            .link
            .clone()
            .filter(|_| state.view.epoch == epoch && state.view.reach == Reach::Attached)
    }

    fn online(&self, machine: &str, epoch: u64) -> bool {
        let state = self.lock();
        state.view.reach == Reach::Attached
            && state.view.epoch == epoch
            && state.view.states.get(machine).map(MachineState::name)
                == Some(MachineStateName::Online)
    }

    /// The `reload` the current attachment still owes the catalog the TUI read: once per
    /// revision, and only after the hub said which revision it reads.
    fn reload_due(&self) -> Option<Hub> {
        let mut state = self.lock();
        let due = state.wanted.is_some()
            && state.view.catalog.is_some()
            && state.view.catalog != state.wanted
            && state.reloading != state.wanted;
        if !due {
            return None;
        }
        state.reloading = state.wanted;
        state.link.clone()
    }
}

/// The owner of one attachment epoch, as an observer reaches it.
#[derive(Clone)]
pub(super) struct Link {
    shared: Arc<Shared>,
    epoch: u64,
}

impl Link {
    /// The owner, while the epoch is current and its hub live.
    pub(super) fn hub(&self) -> Option<Hub> {
        self.shared.link(self.epoch)
    }

    /// Whether the hub reads `machine` online in this epoch, as its carrier asks per connection.
    pub(super) fn gate(&self, machine: &ProfileId) -> Gate {
        let shared = self.shared.clone();
        let epoch = self.epoch;
        let machine = machine.as_str().to_owned();
        Arc::new(move || shared.online(&machine, epoch))
    }
}

/// The background holder discovery starts once the catalog holds a Machine it can observe.
pub(super) struct Holder {
    shared: Arc<Shared>,
    /// Never joined: the holder starts nothing after the stop request and writes nothing to the
    /// terminal, so process exit leaves it detached.
    _task: BackgroundThread,
}

impl Holder {
    pub(super) fn start(wakeup: Option<UiWakeup>) -> io::Result<Self> {
        let shared = Arc::new(Shared::new(wakeup));
        let task = platform::spawn(shared.clone())?;
        Ok(Self {
            shared,
            _task: task,
        })
    }

    pub(super) fn read(&self, seen: u64) -> Option<(u64, View)> {
        self.shared.read(seen)
    }

    pub(super) fn link(&self, epoch: u64) -> Link {
        Link {
            shared: self.shared.clone(),
            epoch,
        }
    }

    /// Records the catalog revision the TUI read; the hub is asked to `reload` when it differs.
    pub(super) fn want(&self, revision: Option<u64>) {
        let changed = {
            let mut state = self.shared.lock();
            let changed = state.wanted != revision;
            state.wanted = revision;
            changed
        };
        if changed && let Some(hub) = self.shared.reload_due() {
            platform::reload(hub);
        }
    }

    /// Asks the hub to bring `machine` online, waiting for a hub first, and returns the ask
    /// discovery waits on. A selection is a person's act: it may start a hub.
    pub(super) fn connect(&self, machine: &ProfileId) -> Ask {
        let mut ask = Ask::Failed(None);
        self.shared.update(|state| {
            let number = state.next_ask;
            state.next_ask = number.wrapping_add(1);
            ask = Ask::Pending(number);
            state.view.asks.insert(machine.as_str().to_owned(), ask);
        });
        if let Ask::Pending(number) = ask {
            platform::connect(&self.shared, machine.as_str().to_owned(), number);
        }
        ask
    }

    /// The login notices that came since the last call, in order.
    pub(super) fn take_logins(&self) -> Vec<Notice> {
        std::mem::take(&mut self.shared.lock().logins)
    }

    /// Asks the hub to dismiss episode `login` without waiting; its `ended` event follows.
    pub(super) fn dismiss(&self, login: Id) {
        platform::dismiss(&self.shared, login);
    }

    /// Delivers `answer` to `endpoint` off the interface thread and reports the outcome as a
    /// login notice; the answer is wiped once the endpoint took it or the bound passed.
    pub(super) fn deliver(&self, token: Token, endpoint: AnswerEndpoint, answer: Answer) {
        platform::deliver(&self.shared, token, endpoint, answer);
    }

    /// Forgets the last ask for `machine`, as a selection of an online Machine does.
    pub(super) fn forget(&self, machine: &ProfileId) {
        self.shared.lock().view.asks.remove(machine.as_str());
    }

    /// Asks the holder to leave without waiting: an attachment in progress ends, starting no hub,
    /// and requests in flight end on their own threads.
    pub(super) fn request_stop(&self) {
        self.shared.stop.store(true, Ordering::Release);
        #[cfg(windows)]
        self.shared.cancel.cancel();
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        self.request_stop();
    }
}
