//! Settings › Machines in the TUI: carries out the request the person made on the page and gathers
//! the facts the page shows, once per shell tick. A record changes only through `Store::commit`;
//! the catalog a commit leaves goes back to the shell, which refreshes from it at once.
//! `apps/orchbee/docs/machines.md#machines-in-settings` owns the rules.

// module-layer: workflow

use crate::machine_credentials::Remembering;
use crate::machine_discovery::{Discovery, Forgotten, Observation, SessionStop};
use crate::machine_settings::{Action, Facts, Machine, Outcome, Request, State, natural_order};
use crate::machine_status::Explanation;
use crate::machines::{
    Catalog, Edit, Error, Profile, ProfileId, Store, names_this_computer, valid_label,
};
use orchbee_machine_protocol::hub::MachineStateName;
use std::time::{Duration, Instant};

/// How long a login asked for on the page waits for its question; the person's answer stops the
/// clock.
const LOGIN: Duration = Duration::from_secs(60);

/// What the page asked that is still under way, and how the last request ended.
#[derive(Debug, Default)]
pub struct Exchange {
    /// How the last request ended, until the next facts carry it.
    outcome: Option<Outcome>,
    /// The machines whose terminals are being stopped.
    stopping: Vec<ProfileId>,
    /// A forget of a saved password the page asked for runs.
    forgetting: bool,
    /// The machine whose login the page asked for, and when the wait for its question ends.
    login: Option<(ProfileId, Instant)>,
    /// The machine Add Machine saved for the page, until the facts list it.
    added: Option<ProfileId>,
}

/// What one request changed in the machine records.
#[derive(Debug, Default)]
pub struct Carried {
    /// The catalog a commit left, for the shell's catalog refresh.
    pub catalog: Option<Catalog>,
    /// The machines the view leaves for this computer when it shows one of them: one removed or
    /// disconnected, and one whose terminals are being stopped.
    pub left: Vec<ProfileId>,
}

impl Exchange {
    /// Carries out `request` on the saved machines in `profiles`; the shell opens Add Machine
    /// itself.
    pub fn carry(
        &mut self,
        request: Request,
        discovery: &mut Discovery,
        store: Option<&Store>,
        profiles: &[Profile],
        now: Instant,
    ) -> Carried {
        let mut carried = Carried::default();
        let machine = match &request {
            Request::Add => return carried,
            Request::Rename { machine, .. }
            | Request::Act { machine, .. }
            | Request::ForgetPassword(machine)
            | Request::Stop(machine)
            | Request::Remove(machine) => machine,
        };
        // A machine removed meanwhile takes no request.
        let Some(profile) = profiles
            .iter()
            .find(|profile| profile.id.as_str() == machine)
        else {
            return carried;
        };
        let id = profile.id.clone();
        let label = profile.draft.label.clone();
        match request {
            Request::Add => {}
            Request::Rename { label, .. } => {
                let label = label.trim().to_owned();
                if !valid_label(&label) || names_this_computer(&label) {
                    self.outcome = Some(Outcome::NameRefused);
                } else {
                    carried.catalog = self.commit(store, &Edit::Rename { id, label });
                }
            }
            Request::Act {
                action: Action::Disconnect,
                ..
            } => {
                let edit = Edit::Enable {
                    id: id.clone(),
                    enabled: false,
                };
                // A saved disconnect also ends the link, so nothing connects the machine again;
                // one that was not saved leaves it as it was.
                if let Some(catalog) = self.commit(store, &edit) {
                    carried.catalog = Some(catalog);
                    discovery.disconnect(&id);
                    if self.login.as_ref().is_some_and(|(login, _)| *login == id) {
                        self.login = None;
                    }
                    carried.left.push(id);
                }
            }
            // The hub connects a machine saved enabled once it reloads the catalog, as it keeps
            // every enabled machine while this TUI holds it; that login asks this TUI.
            Request::Act {
                action: Action::Connect,
                ..
            } => {
                let edit = Edit::Enable {
                    id: id.clone(),
                    enabled: true,
                };
                carried.catalog = self.commit(store, &edit);
                if carried.catalog.is_some() {
                    self.login = Some((id, now + LOGIN));
                }
            }
            // A connection the person asks for: its login asks this TUI.
            Request::Act {
                action: Action::Retry | Action::RetryNow | Action::LogIn,
                ..
            } => {
                discovery.retry(&id, now);
                self.login = Some((id, now + LOGIN));
            }
            Request::ForgetPassword(_) => {
                if discovery.forget_password(&id) {
                    self.forgetting = true;
                } else {
                    self.outcome = Some(Outcome::ForgetFailed);
                }
            }
            // A stop already running is answered once.
            Request::Stop(_) if self.stopping.contains(&id) => {}
            Request::Stop(_) => {
                if discovery.online(&id) {
                    discovery.stop_session(&id);
                    self.stopping.push(id.clone());
                    // Its terminals end now: the view leaves it instead of showing them end.
                    carried.left.push(id);
                } else {
                    self.outcome = Some(Outcome::NotConnected(label));
                }
            }
            Request::Remove(_) => {
                if let Some(catalog) = self.commit(store, &Edit::Remove(id.clone())) {
                    discovery.forget_removed(&id);
                    carried.catalog = Some(catalog);
                    carried.left.push(id);
                    self.outcome = Some(Outcome::Removed(label));
                }
            }
        }
        carried
    }

    /// Add Machine, opened from the page, saved `machine`; the page focuses it once listed.
    pub fn added(&mut self, machine: ProfileId) {
        self.added = Some(machine);
    }

    /// Settles what earlier requests started: each stop that ended, the forget in flight, and the
    /// wait for a login. Runs on every tick, before anything else reads those outcomes.
    pub fn settle(&mut self, discovery: &mut Discovery, profiles: &[Profile], now: Instant) {
        let label = |id: &ProfileId| {
            profiles
                .iter()
                .find(|profile| &profile.id == id)
                .map_or_else(
                    || id.as_str().to_owned(),
                    |profile| profile.draft.label.clone(),
                )
        };
        let outcome = &mut self.outcome;
        self.stopping.retain(|id| {
            let Some(stop) = discovery.stopped(id) else {
                return true;
            };
            *outcome = Some(match stop {
                SessionStop::Stopped => Outcome::Stopped(label(id)),
                SessionStop::NotOnline => Outcome::NotConnected(label(id)),
                SessionStop::Failed => Outcome::StopFailed(label(id)),
            });
            false
        });
        if self.forgetting
            && let Some(forgotten) = discovery.forgotten()
        {
            self.forgetting = false;
            self.outcome = Some(match forgotten {
                Forgotten::Removed => Outcome::PasswordForgotten,
                Forgotten::NothingSaved => Outcome::NoPasswordSaved,
                Forgotten::Failed => Outcome::ForgetFailed,
            });
        }
        if let Some((machine, until)) = &mut self.login {
            if discovery.login_active(machine) {
                *until = now + LOGIN;
            } else if now >= *until || discovery.online(machine) {
                self.login = None;
            }
        }
    }

    /// The machine whose login the page asked for, while the wait for it lasts.
    pub fn login(&self) -> Option<&ProfileId> {
        self.login.as_ref().map(|(machine, _)| machine)
    }

    /// What the page shows now: this computer, every saved machine by name, and the outcome and
    /// the added machine not yet shown. `agents` counts a machine's working Agents.
    pub fn facts(
        &mut self,
        discovery: &mut Discovery,
        profiles: &[Profile],
        agents: impl Fn(&ProfileId) -> Option<usize>,
        now: Instant,
    ) -> Facts {
        let observations = discovery.observations(now);
        let explanations = discovery.explanations(now);
        let mut machines: Vec<Machine> = profiles
            .iter()
            .map(|profile| {
                let id = &profile.id;
                let observation = observations
                    .iter()
                    .find(|(observed, _)| observed == id)
                    .map(|(_, observation)| observation);
                // A connection kind this build does not know is never observed.
                let state = if profile.enabled {
                    observation.map_or(State::Other("unsupported"), state)
                } else {
                    State::Disabled
                };
                let overview = match observation {
                    Some(Observation::Online(overview)) => Some(overview),
                    _ => None,
                };
                let online = state == State::Online;
                let explanation = if profile.enabled {
                    explanations
                        .iter()
                        .find(|(explained, _)| explained == id)
                        .map(|(_, explanation)| explanation.clone())
                        .unwrap_or_default()
                } else {
                    Explanation::disabled()
                };
                Machine {
                    id: id.as_str().to_owned(),
                    label: profile.draft.label.clone(),
                    address: profile.draft.ssh_target().map(str::to_owned),
                    state,
                    capacity: online.then(|| discovery.capacity(Some(id))).flatten(),
                    workspaces: overview.map(|overview| overview.workspaces),
                    terminals: overview.map(|overview| overview.terminals),
                    agents: online.then(|| agents(id)).flatten(),
                    last_attempt: discovery.last_attempt(id),
                    explanation,
                    stopping: self.stopping.contains(id),
                }
            })
            .collect();
        machines.sort_by(|left, right| {
            natural_order(&left.label, &right.label).then_with(|| left.id.cmp(&right.id))
        });
        let added = self
            .added
            .take_if(|added| machines.iter().any(|machine| machine.id == added.as_str()))
            .map(|added| added.as_str().to_owned());
        Facts {
            local: discovery.capacity(None),
            machines,
            passwords: discovery
                .remembering()
                .is_some_and(|remembering| remembering != Remembering::Unavailable),
            outcome: self.outcome.take(),
            added,
        }
    }

    fn commit(&mut self, store: Option<&Store>, edit: &Edit) -> Option<Catalog> {
        match store.map(|store| store.commit(edit)) {
            Some(Ok((catalog, _))) => Some(catalog),
            Some(Err(Error::InvalidCatalog | Error::Reserved))
                if matches!(edit, Edit::Rename { .. }) =>
            {
                self.outcome = Some(Outcome::NameRefused);
                None
            }
            _ => {
                self.outcome = Some(Outcome::SaveFailed);
                None
            }
        }
    }
}

/// A machine's state as the page words it; states the page has no word of its own for keep the
/// machine list's.
fn state(observation: &Observation) -> State {
    let name = match observation {
        Observation::Online(_) => return State::Online,
        Observation::Hub(_) => return State::Other(observation.key()),
        Observation::State(name) => *name,
        Observation::Login(login) => login.name(),
    };
    match name {
        MachineStateName::Online => State::Online,
        MachineStateName::Connecting => State::Connecting,
        MachineStateName::Reconnecting | MachineStateName::Unresponsive => State::Reconnecting,
        MachineStateName::Offline => State::Offline,
        MachineStateName::Disabled => State::Disabled,
        MachineStateName::NeedsLogin | MachineStateName::LoginFailed => State::NeedsLogin,
        MachineStateName::NeedsUpdate => State::NeedsUpdate,
        MachineStateName::LocalNeedsUpdate => State::LocalNeedsUpdate,
        _ => State::Other(observation.key()),
    }
}
