//! The TUI's hold on the machine hub, the hub's state of each saved machine, and one read-only
//! observer per Machine the hub reads `online`. The UI alone owns selection.
mod credentials;
mod hub;
pub(crate) mod overview;
mod policy;
pub use overview::{Blocked, Context, HubStatus, LoginState, Observation, Overview, display_label};
#[cfg(test)]
mod tests;
mod worker;
use crate::local_client::cancellation::Cancellation;
use crate::local_client::{LocalClient, LocalClientOrigin, LocalOperationClock};
use crate::machine_login::Notice as LoginNotice;
use crate::machine_login::{Act, Effect, Ending, Logins, Presented, Token};
use crate::machine_refusal::Refusal;
use crate::machines::{Connection as MachineConnection, Profile, ProfileId};
use crate::ui_wakeup::UiWakeup;
use credentials::Credentials;
pub(crate) use credentials::Forgotten;
use hub::{Ask, Holder, Reach, View};
use orchbee_machine_protocol::hub::{ConnectFailure, MachineStateName};
#[cfg(any(test, feature = "test-support"))]
use orchbee_workbench_protocol::{SessionId, WorkbenchRevision};
use policy::{Phase, Policy};
use std::{io, time::Instant};
use worker::{Event, Update, Worker};

#[derive(Clone)]
pub struct Connection {
    origin: LocalClientOrigin,
    clock: LocalOperationClock,
}
impl Connection {
    /// Uses the carrier's shared application-progress deadline, including pending probe expiry.
    pub(crate) fn is_current(&self, now: Instant) -> bool {
        self.origin.is_current(now)
    }
    /// Distinguishes replacement carriers even when a saved profile's logical choice is unchanged.
    pub(crate) fn same_owner(&self, other: &Self) -> bool {
        self.origin.shares_connection(&other.origin)
    }
    pub(crate) fn watch_changes(
        &self,
    ) -> io::Result<crate::local_client::subscription::ChangeSubscription> {
        self.origin.watch_changes()
    }
    pub fn from_origin(origin: LocalClientOrigin, clock: LocalOperationClock) -> Self {
        Self { origin, clock }
    }
    pub fn reconnect(
        &self,
        deadline: Instant,
        cancel: Cancellation<'_>,
    ) -> io::Result<LocalClient> {
        self.origin
            .lane_before_cancellable(deadline, self.clock.clone(), cancel.wake())
            .map_err(crate::machine_carrier::admission_error)
    }
}
struct Ready {
    overview: Overview,
    connection: Connection,
}
struct Entry {
    profile: Profile,
    generation: u64,
    policy: Policy,
    worker: Option<Worker>,
    ready: Option<Ready>,
    /// The refusal of the latest failed attempt; a new attempt or a healthy carrier clears it.
    refusal: Option<Refusal>,
    retiring: bool,
}
impl Entry {
    fn accept(&mut self, update: Update) {
        if update.generation != self.generation {
            return;
        }
        match update.event {
            Event::Healthy {
                ready,
                online_since,
            } => {
                self.policy.healthy(online_since);
                self.ready = Some(ready);
                self.refusal = None;
            }
            Event::Failed {
                permanent,
                refusal,
                at,
            } => {
                self.ready = None;
                self.refusal = refusal;
                self.policy.fail(permanent, at);
                self.retiring = true;
            }
            Event::Reset => {
                self.ready = None;
                self.refusal = None;
                self.policy.recovering();
            }
        }
    }
    /// Ends the observer and starts the policy over, as the hub leaving `online` does.
    fn withdraw(&mut self, retired: &mut Vec<Worker>, now: Instant) {
        self.ready = None;
        self.retiring = false;
        self.policy = Policy::new(now);
        if let Some(worker) = self.worker.take() {
            worker.cancel();
            retired.push(worker);
        }
    }
}
/// The state word a refusal shows: the side that must change.
const fn refused(refusal: Refusal) -> MachineStateName {
    match refusal {
        Refusal::OtherEnvironment => MachineStateName::Incompatible,
        Refusal::ClientTooOld => MachineStateName::LocalNeedsUpdate,
        Refusal::SessionTooOld | Refusal::DevelopmentBuildMismatch => MachineStateName::NeedsUpdate,
    }
}
pub fn same_endpoint(a: &Profile, b: &Profile) -> bool {
    a.id == b.id && a.draft.connection == b.draft.connection && a.draft.session == b.draft.session
}
/// A discovery made with `Default` never holds a hub: tests drive its view directly.
#[derive(Default)]
pub struct Discovery {
    entries: Vec<Entry>,
    retired: Vec<Worker>,
    generation: u64,
    /// Present when this discovery may hold the hub.
    wakeup: Option<UiWakeup>,
    holds: bool,
    holder: Option<Holder>,
    /// The holder's view as the last tick copied it.
    hub: View,
    seen: u64,
    /// The login episode this TUI claimed for the person's selection.
    logins: Logins,
    /// The credential store remembered answers are written to and forgotten from.
    credentials: Credentials,
}
impl Discovery {
    /// A discovery that holds the hub once the catalog holds a Machine it can observe, and wakes
    /// `wakeup` when the hub reports a change.
    pub fn new(wakeup: UiWakeup) -> Self {
        Self {
            entries: Vec::new(),
            retired: Vec::new(),
            generation: 0,
            wakeup: Some(wakeup),
            holds: true,
            holder: None,
            hub: View::default(),
            seen: 0,
            logins: Logins::default(),
            credentials: Credentials::new(),
        }
    }
    /// Reconciles the enabled profiles of the catalog of `revision`; `None` for an unreadable
    /// catalog, which suspends every observer.
    pub fn reconcile(&mut self, profiles: &[Profile], revision: Option<u64>, now: Instant) {
        let mut old = std::mem::take(&mut self.entries);
        // A connection kind this build does not know is never contacted.
        for profile in profiles
            .iter()
            .filter(|p| p.enabled && p.draft.connection.supported())
        {
            if let Some(index) = old.iter().position(|e| same_endpoint(&e.profile, profile)) {
                let mut entry = old.remove(index);
                entry.profile = profile.clone();
                self.entries.push(entry);
            } else if let Some(generation) = self.generation.checked_add(1) {
                self.generation = generation;
                self.entries.push(Entry {
                    profile: profile.clone(),
                    generation,
                    policy: Policy::new(now),
                    worker: None,
                    ready: None,
                    refusal: None,
                    retiring: false,
                });
            }
        }
        for mut entry in old {
            entry.ready = None;
            if let Some(worker) = entry.worker.take() {
                worker.cancel();
                self.retired.push(worker);
            }
        }
        // The first observable Machine starts the one holder, kept until the TUI exits.
        if self.holds && self.holder.is_none() && !self.entries.is_empty() {
            match Holder::start(self.wakeup.clone()) {
                Ok(holder) => self.holder = Some(holder),
                Err(_) => self.hub.reach = Reach::Unavailable,
            }
        }
        if let Some(holder) = &self.holder {
            holder.want(revision);
        }
    }
    pub fn tick(&mut self, now: Instant) {
        if let Some((seen, view)) = self.holder.as_ref().and_then(|h| h.read(self.seen)) {
            self.seen = seen;
            self.adopt(view, now);
        }
        let notices = self.holder.as_ref().map(Holder::take_logins);
        for notice in notices.into_iter().flatten() {
            // The store is asked once, when the first question that may be remembered arrives.
            if let LoginNotice::Question(question) = &notice
                && crate::machine_credentials::SecretKind::of(&question.kind).is_some()
            {
                self.remembering();
            }
            let entries = &self.entries;
            let effects = self.logins.notice(notice, |machine| {
                entries
                    .iter()
                    .find(|e| e.profile.id.as_str() == machine)
                    .and_then(|e| match &e.profile.draft.connection {
                        MachineConnection::Ssh { target } => Some(target.clone()),
                        MachineConnection::Unsupported(_) => None,
                    })
            });
            self.carry(effects);
        }
        self.retired.retain(|w| !w.finished());
        for entry in &mut self.entries {
            if let Some(update) = entry.worker.as_ref().and_then(Worker::take) {
                entry.accept(update);
            }
            if entry.worker.as_ref().is_some_and(Worker::finished) {
                // A worker may publish between the first drain and the completion check.
                let worker = entry.worker.take().expect("finished observer");
                if let Some(update) = worker.take() {
                    entry.accept(update);
                }
                if matches!(
                    entry.policy.phase,
                    Phase::Connecting | Phase::Online | Phase::Reconnecting
                ) && !worker.cancelled()
                {
                    entry.ready = None;
                    entry.policy.fail(false, now);
                }
                entry.retiring = false;
            }
        }
        let link = self.holder.as_ref().map(|h| h.link(self.hub.epoch));
        self.start_due(now, |profile, generation| match &link {
            Some(link) => Worker::start(link.clone(), profile, generation),
            None => Err(io::ErrorKind::NotConnected.into()),
        });
    }
    /// Takes the holder's latest view: an observer ends when the hub stops reading its Machine
    /// `online`, and a Machine the hub reads `online` anew starts without its last refusal.
    fn adopt(&mut self, view: View, now: Instant) {
        let renewed = view.epoch != self.hub.epoch;
        for entry in &mut self.entries {
            let was = self.hub.state(&entry.profile.id) == Some(MachineStateName::Online);
            let is = view.state(&entry.profile.id) == Some(MachineStateName::Online);
            if was && (!is || renewed) {
                entry.withdraw(&mut self.retired, now);
            }
            if is && (!was || renewed) {
                entry.refusal = None;
                entry.policy = Policy::new(now);
            }
        }
        self.hub = view;
    }
    /// The manager acknowledges after both collectors have retired cached source/watch owners.
    /// Until then a failed worker keeps its last carrier guard on the background thread.
    pub fn finish_retirement(&mut self) {
        for entry in &self.entries {
            if entry.retiring
                && let Some(worker) = &entry.worker
            {
                worker.cancel();
            }
        }
    }
    /// Starts an observer for each Machine the hub reads `online` whose policy is due.
    fn start_due(
        &mut self,
        now: Instant,
        mut start: impl FnMut(Profile, u64) -> io::Result<Worker>,
    ) {
        for entry in &mut self.entries {
            if entry.worker.is_none()
                && self.hub.state(&entry.profile.id) == Some(MachineStateName::Online)
                && entry.policy.due(now)
            {
                entry.policy.begin();
                entry.refusal = None;
                match start(entry.profile.clone(), entry.generation) {
                    Ok(worker) => entry.worker = Some(worker),
                    Err(_) => entry.policy.fail(false, now),
                }
            }
        }
    }
    /// Why no state of the hub's holds now, while the holder has no live attachment.
    fn hub_status(&self) -> Option<HubStatus> {
        match &self.hub.reach {
            Reach::Attached => None,
            Reach::Attaching => Some(HubStatus::Unknown),
            // A closing hub was asked to stop, or expired: it reads stopped while it goes.
            Reach::Closing | Reach::Stopped => Some(HubStatus::Stopped),
            Reach::Unavailable => Some(HubStatus::Unavailable),
            Reach::Blocked(blocked) => Some(HubStatus::Blocked(blocked.clone())),
        }
    }
    fn observation(&self, entry: &Entry, now: Instant) -> Observation {
        if let Some(status) = self.hub_status() {
            return Observation::Hub(status);
        }
        let Some(state) = self.hub.state(&entry.profile.id) else {
            return Observation::Hub(HubStatus::Unknown);
        };
        if state != MachineStateName::Online {
            let host = match &entry.profile.draft.connection {
                MachineConnection::Ssh { target } => crate::machines::known_hosts_name(target),
                MachineConnection::Unsupported(_) => String::new(),
            };
            return self
                .hub
                .facts(&entry.profile.id)
                .and_then(|facts| LoginState::of(facts, &host))
                .map_or(Observation::State(state), Observation::Login);
        }
        if let Some(refusal) = entry.refusal {
            return Observation::State(refused(refusal));
        }
        entry
            .ready
            .as_ref()
            .filter(|r| r.connection.is_current(now))
            .map_or(Observation::State(state), |r| {
                Observation::Online(r.overview.clone())
            })
    }
    /// One observation per Machine: the hub's current state, or why none holds now.
    pub fn observations(&self, now: Instant) -> Vec<(ProfileId, Observation)> {
        self.entries
            .iter()
            .map(|e| (e.profile.id.clone(), self.observation(e, now)))
            .collect()
    }
    pub fn connection(&self, profile: &Profile, now: Instant) -> Option<Connection> {
        self.entries
            .iter()
            .find(|e| {
                same_endpoint(&e.profile, profile)
                    && matches!(self.observation(e, now), Observation::Online(_))
            })
            .and_then(|e| e.ready.as_ref().map(|r| r.connection.clone()))
    }
    /// Whether a selection of `profile` fails now: no holder can reach a hub, the selection's ask
    /// found no hub or ended without `online`, or the observer of an `online` Machine failed.
    pub fn failed(&self, profile: &Profile) -> bool {
        (self.holder.is_none() && self.hub.reach != Reach::Attached)
            || matches!(self.hub.asks.get(profile.id.as_str()), Some(Ask::Failed(_)))
            || self.entries.iter().any(|e| {
                same_endpoint(&e.profile, profile)
                    && self.hub.state(&e.profile.id) == Some(MachineStateName::Online)
                    && matches!(e.policy.phase, Phase::Attention | Phase::Reconnecting)
            })
    }
    /// Why the remote refused this Client in the endpoint's latest failed attempt, when it said
    /// so; selection names this instead of a generic failure.
    pub fn refusal(&self, profile: &Profile) -> Option<Refusal> {
        self.entries
            .iter()
            .find(|e| same_endpoint(&e.profile, profile))
            .and_then(|entry| entry.refusal)
    }
    /// The picker's message while machine connections are out of reach for longer than an
    /// attachment: stopped, unavailable, or blocked.
    pub fn hub_message(&self) -> Option<&'static str> {
        if self.entries.is_empty() {
            return None;
        }
        match self.hub_status()? {
            HubStatus::Unknown => None,
            HubStatus::Stopped => Some("hub-message-stopped"),
            HubStatus::Unavailable => Some("hub"),
            HubStatus::Blocked(_) => Some("hub-message-blocked"),
        }
    }
    /// Carries out what the login episode asks: deliveries and dismissals go to the holder's
    /// threads; without a holder an answer is wiped as it drops.
    fn carry(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match (effect, &self.holder) {
                (
                    Effect::Deliver {
                        token,
                        endpoint,
                        answer,
                    },
                    Some(holder),
                ) => holder.deliver(token, endpoint, answer),
                (Effect::Dismiss(login), Some(holder)) => holder.dismiss(login),
                (Effect::Deliver { .. } | Effect::Dismiss(_), None) => {}
                (
                    Effect::Remember {
                        machine,
                        kind,
                        answer,
                    },
                    _,
                ) => {
                    let label = self
                        .entries
                        .iter()
                        .find(|entry| entry.profile.id.as_str() == machine)
                        .map_or(machine.as_str(), |entry| entry.profile.draft.label.as_str());
                    self.credentials.remember(&machine, kind, answer, label);
                }
            }
        }
    }
    /// The person ticked or cleared the remember box of the question `token` names.
    pub(crate) fn login_remember(&mut self, token: Token, ticked: bool) {
        self.logins.remember(token, ticked);
    }
    /// Starts forgetting both remembered answers of `machine`; false when it cannot start now.
    pub(crate) fn forget_password(&mut self, machine: &ProfileId) -> bool {
        self.credentials
            .forget(machine.as_str(), self.wakeup.clone())
    }
    /// How long this platform's store keeps an answer in this login; the store is asked once.
    pub(crate) fn remembering(&mut self) -> crate::machine_credentials::Remembering {
        if let Some(remembering) = self.logins.remembering() {
            return remembering;
        }
        let remembering = self.credentials.remembering();
        self.logins.set_remembering(remembering);
        remembering
    }
    /// How the forget in flight ended, once it has.
    pub(crate) fn forgotten(&mut self) -> Option<Forgotten> {
        self.credentials.forgotten()
    }
    pub(crate) fn login_act(&mut self, token: Token, act: Act) {
        let effects = self.logins.act(token, act);
        self.carry(effects);
    }
    pub(crate) fn login_presented(&self) -> Option<Presented> {
        self.logins.presented()
    }
    /// The episodes, for a test that plays the hub's login notices without a hub.
    #[cfg(test)]
    pub(crate) const fn logins_mut(&mut self) -> &mut Logins {
        &mut self.logins
    }
    /// Whether a login of `machine` waits for the person: its selection's clock stops meanwhile.
    pub fn login_active(&self, machine: &ProfileId) -> bool {
        self.logins.active(machine.as_str())
    }
    /// A selection of `machine` ended before its login did: the login is dismissed.
    pub fn abandon_login(&mut self, machine: &ProfileId) {
        let effects = self.logins.abandon(machine.as_str());
        self.carry(effects);
    }
    /// The Machine whose selection the person waits on now, if any. Only its login may ask; a
    /// Machine no longer waited on has its login abandoned.
    pub fn wait_for(&mut self, machine: Option<&ProfileId>) {
        let effects = self.logins.wait_for(machine.map(ProfileId::as_str));
        self.carry(effects);
    }
    /// Whether the hub says `profile` waits for a login the person can give: a remote pane of it
    /// has no live link, and choosing it logs in.
    pub fn login_needed(&self, profile: &Profile) -> bool {
        matches!(
            self.hub.state(&profile.id),
            Some(MachineStateName::NeedsLogin | MachineStateName::LoginFailed)
        )
    }
    /// The picker message of a selection of `profile` that ended without `online` because of its
    /// login; `None` leaves the generic one.
    pub fn login_failure(&mut self, profile: &Profile) -> Option<&'static str> {
        if self.logins.take_ending(profile.id.as_str()) == Some(Ending::Undelivered) {
            return Some("login-undelivered");
        }
        match self.hub.asks.get(profile.id.as_str()) {
            Some(Ask::Failed(Some(ConnectFailure::AuthFailed))) => Some("login-failed-message"),
            Some(Ask::Failed(Some(ConnectFailure::HostKeyChanged))) => {
                Some("login-key-changed-message")
            }
            Some(Ask::Failed(Some(
                ConnectFailure::AuthRequired | ConnectFailure::HostUntrusted,
            ))) => Some("login-unanswered-message"),
            _ => None,
        }
    }
    /// A selection asks again: the observer's hold and refusal clear, and a Machine the hub does
    /// not read `online` is asked to `connect`, which starts a hub when none is reached.
    pub fn retry(&mut self, id: &ProfileId, now: Instant) {
        // A new selection reads only how its own login ends.
        let _ = self.logins.take_ending(id.as_str());
        if let Some(entry) = self.entries.iter_mut().find(|e| &e.profile.id == id) {
            entry.refusal = None;
            entry.policy.retry(now);
        }
        let Some(holder) = &self.holder else {
            return;
        };
        if self.hub.state(id) == Some(MachineStateName::Online) {
            holder.forget(id);
            self.hub.asks.remove(id.as_str());
        } else {
            let ask = holder.connect(id);
            self.hub.asks.insert(id.as_str().to_owned(), ask);
        }
    }
}
impl Discovery {
    /// Process exit: observes every observer thread against the shared deadline after all of them
    /// and the holder were cancelled. The holder is not waited for: after the stop request it
    /// starts nothing and writes nothing to the terminal, and exit leaves it detached.
    ///
    /// # Errors
    /// TimedOut when an observer is still running at the deadline; it is detached on drop.
    pub fn stop_before(&mut self, deadline: Instant) -> io::Result<()> {
        self.request_stop();
        self.entries
            .iter_mut()
            .filter_map(|entry| entry.worker.as_mut())
            .chain(&mut self.retired)
            .map(|worker| worker.stop_before(deadline))
            .fold(Ok(()), io::Result::and)
    }

    /// Wakes all I/O owners and cancels every observer and the holder without waiting.
    pub fn request_stop(&mut self) {
        for entry in &mut self.entries {
            entry.ready = None;
        }
        if let Some(holder) = &self.holder {
            holder.request_stop();
        }
        for worker in self
            .entries
            .iter()
            .filter_map(|e| e.worker.as_ref())
            .chain(&self.retired)
        {
            worker.cancel();
        }
    }
}
impl Drop for Discovery {
    fn drop(&mut self) {
        // Only stop_before waits; unfinished observers are detached here.
        self.request_stop();
    }
}

/// Field-for-field `Overview` fixture the application's tests build values from.
#[cfg(any(test, feature = "test-support"))]
pub struct OverviewFixture {
    pub session: SessionId,
    pub revision: WorkbenchRevision,
    pub projects: usize,
    pub workspaces: usize,
    pub tabs: usize,
    pub labels: Vec<String>,
    pub contexts: std::sync::Arc<[Context]>,
}

#[cfg(any(test, feature = "test-support"))]
impl From<OverviewFixture> for Overview {
    fn from(fixture: OverviewFixture) -> Self {
        let OverviewFixture {
            session,
            revision,
            projects,
            workspaces,
            tabs,
            labels,
            contexts,
        } = fixture;
        Self {
            session,
            revision,
            projects,
            workspaces,
            tabs,
            labels,
            contexts,
        }
    }
}
