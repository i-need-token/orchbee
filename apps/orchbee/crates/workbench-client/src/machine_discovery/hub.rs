//! The TUI's hold on the machine hub: one background attachment with presence and watch, the
//! Machine states the hub reports, published into one shared view, and the `reload` and `connect`
//! requests discovery asks for. A frame copies the view and never waits for the hub. Windows only,
//! as the hub is; `apps/orchbee/docs/machines.md#the-tui-as-a-hub-client` owns the rules.
#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
use unix as platform;
#[cfg(windows)]
use windows as platform;

use crate::{
    background_thread::BackgroundThread, machine_carrier::Gate, machine_carrier::Hub,
    machines::ProfileId, ui_wakeup::UiWakeup,
};
use orchbee_machine_protocol::hub::MachineStateName;
use std::{
    collections::HashMap,
    io,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

/// Whether the holder reaches a hub.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum Reach {
    /// The first attachment is still being made.
    #[default]
    Starting,
    Attached,
    /// No hub could be started or reached, or the owner ended; the holder tries again.
    Unavailable,
}

/// A `connect` discovery asked for on a selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Ask {
    /// Waiting for the reply of the request with this number.
    Pending(u64),
    /// The reply said the Machine is not online, or no reply came.
    Failed,
}

/// What the holder last learned, as discovery copies it.
#[derive(Clone, Debug, Default)]
pub(super) struct View {
    pub(super) reach: Reach,
    /// Counts the owners the holder opened, so a carrier of an ended owner never reads online.
    pub(super) epoch: u64,
    /// The hub's state of each catalog Machine it reported, by id.
    pub(super) states: HashMap<String, MachineStateName>,
    /// The revision of the hub's last `catalog` event.
    pub(super) catalog: Option<u64>,
    pub(super) asks: HashMap<String, Ask>,
}

impl View {
    pub(super) fn state(&self, machine: &ProfileId) -> Option<MachineStateName> {
        self.states.get(machine.as_str()).copied()
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
}

/// The view and the owner, shared by the holder thread, discovery, observers, carriers, and
/// request threads. Only the holder thread ever holds the owner while it is the last one.
pub(super) struct Shared {
    state: Mutex<State>,
    stop: AtomicBool,
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
            }),
            stop: AtomicBool::new(false),
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
            && state.view.states.get(machine) == Some(&MachineStateName::Online)
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
    /// The owner, while the epoch is current.
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
    task: BackgroundThread,
}

impl Holder {
    pub(super) fn start(wakeup: Option<UiWakeup>) -> io::Result<Self> {
        let shared = Arc::new(Shared::new(wakeup));
        let task = platform::spawn(shared.clone())?;
        Ok(Self { shared, task })
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

    /// Asks the hub to bring `machine` online and returns the ask discovery waits on.
    pub(super) fn connect(&self, machine: &ProfileId) -> Ask {
        let mut ask = Ask::Failed;
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

    /// Forgets the last ask for `machine`, as a selection of an online Machine does.
    pub(super) fn forget(&self, machine: &ProfileId) {
        self.shared.lock().view.asks.remove(machine.as_str());
    }

    /// Asks the holder to leave without waiting; requests in flight end on their own threads.
    pub(super) fn request_stop(&self) {
        self.shared.stop.store(true, Ordering::Release);
    }

    pub(super) fn stop_before(&mut self, deadline: Instant) -> io::Result<()> {
        self.request_stop();
        self.task.join_before(deadline)
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        // Only stop_before waits; an unfinished holder is detached here.
        self.request_stop();
    }
}
