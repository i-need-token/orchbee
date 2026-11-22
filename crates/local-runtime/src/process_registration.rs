//! Generation-local ownership facts. Operating-system observations confer no kill authority.
use serde::{Deserialize, Deserializer, Serialize, de};
use std::num::{NonZeroU32, NonZeroU64};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProcessIdentity {
    pid: NonZeroU32,
    creation_time: NonZeroU64,
}
impl ProcessIdentity {
    #[cfg(any(windows, test))]
    pub(crate) fn new(pid: u32, creation_time: u64) -> Option<Self> {
        Some(Self {
            pid: NonZeroU32::new(pid)?,
            creation_time: NonZeroU64::new(creation_time)?,
        })
    }
    pub(crate) fn pid(self) -> u32 {
        self.pid.get()
    }
    #[cfg(any(windows, test))]
    pub(crate) fn creation_time(self) -> u64 {
        self.creation_time.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProcessRole {
    Server,
    Worker,
    Host,
    /// The Terminal Host's Keeper, which the Worker starts once its first Host is ready and which
    /// keeps a Host's programs running when that Host ends.
    Keeper,
}
pub(crate) const ROLES: [ProcessRole; 4] = [
    ProcessRole::Server,
    ProcessRole::Worker,
    ProcessRole::Host,
    ProcessRole::Keeper,
];

#[cfg(any(windows, test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Creator {
    Activator,
    Process(ProcessIdentity),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum Registration {
    #[default]
    NotStarted,
    Spawning,
    Identified {
        identity: ProcessIdentity,
    },
}
impl Registration {
    fn is_not_started(&self) -> bool {
        *self == Self::NotStarted
    }
    pub(crate) fn identity(self) -> Option<ProcessIdentity> {
        match self {
            Self::Identified { identity } => Some(identity),
            Self::NotStarted | Self::Spawning => None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub(crate) struct SessionProcesses {
    server: Registration,
    worker: Registration,
    host: Registration,
    /// Written only once a Keeper is reserved, so a record without one reads as it did before.
    #[serde(skip_serializing_if = "Registration::is_not_started")]
    keeper: Registration,
}

impl<'de> Deserialize<'de> for SessionProcesses {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            server: Registration,
            worker: Registration,
            host: Registration,
            #[serde(default)]
            keeper: Registration,
        }
        let wire = Wire::deserialize(deserializer)?;
        let processes = Self {
            server: wire.server,
            worker: wire.worker,
            host: wire.host,
            keeper: wire.keeper,
        };
        // A Host or a Keeper is reserved only by an identified Worker. An identified Host or Keeper
        // needs no registered Worker: a replacement Worker returns its slot to spawning while the
        // Host and Keeper it adopts keep their identities, and a generation that adopts its
        // predecessor's running Host starts with that Host alone. An older reader refuses both
        // instead of misreading them, and a record with a Keeper.
        let reserved_by_worker = |slot: Registration| {
            slot == Registration::Spawning && processes.worker.identity().is_none()
        };
        if (processes.worker != Registration::NotStarted && processes.server.identity().is_none())
            || reserved_by_worker(processes.host)
            || reserved_by_worker(processes.keeper)
        {
            return Err(de::Error::custom(
                "child registration requires its identified parent",
            ));
        }
        let mut identities = Vec::new();
        for role in ROLES {
            if let Some(identity) = processes.registration(role).identity() {
                if identities.contains(&identity) {
                    return Err(de::Error::custom("process identity cannot own two roles"));
                }
                identities.push(identity);
            }
        }
        Ok(processes)
    }
}

#[cfg(any(windows, test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RegistrationError {
    WrongCreator,
    AlreadyStarted,
    NoReservation,
    DuplicateIdentity,
    /// A replacement needs a Worker to replace.
    NoWorker,
    /// A Host or a Keeper still spawning blocks a replacement Worker.
    HostSpawning,
    /// A recovery Host takes the place only of a registered Host.
    #[cfg(windows)]
    NoHost,
}

#[cfg_attr(
    not(any(windows, test)),
    allow(dead_code, reason = "POSIX observes no registered process")
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProcessObservation {
    Gone,
    Present(ProcessIdentity),
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecoveryVerdict {
    Gone,
    Live(ProcessRole),
    Uncertain(ProcessRole),
}

impl SessionProcesses {
    /// The registrations of a generation that adopts `host`, the Host its predecessor left
    /// running: its Server and Worker are not started yet.
    pub(crate) fn adopting(host: ProcessIdentity) -> Self {
        Self {
            host: Registration::Identified { identity: host },
            ..Self::default()
        }
    }

    /// Whether this generation created no process. Without a Server no Worker started and no Host
    /// spawned, so an identified Host is one an adopting generation kept from its predecessor.
    pub(crate) fn is_unstarted(&self) -> bool {
        self.server == Registration::NotStarted && self.worker == Registration::NotStarted
    }

    pub(crate) fn registration(&self, role: ProcessRole) -> Registration {
        match role {
            ProcessRole::Server => self.server,
            ProcessRole::Worker => self.worker,
            ProcessRole::Host => self.host,
            ProcessRole::Keeper => self.keeper,
        }
    }
    #[cfg(any(windows, test))]
    fn registration_mut(&mut self, role: ProcessRole) -> &mut Registration {
        match role {
            ProcessRole::Server => &mut self.server,
            ProcessRole::Worker => &mut self.worker,
            ProcessRole::Host => &mut self.host,
            ProcessRole::Keeper => &mut self.keeper,
        }
    }
    #[cfg(any(windows, test))]
    pub(crate) fn reserve(
        &mut self,
        role: ProcessRole,
        creator: Creator,
    ) -> Result<(), RegistrationError> {
        let required = match role {
            ProcessRole::Server => Some(Creator::Activator),
            ProcessRole::Worker => self.server.identity().map(Creator::Process),
            ProcessRole::Host | ProcessRole::Keeper => self.worker.identity().map(Creator::Process),
        };
        if required != Some(creator) {
            return Err(RegistrationError::WrongCreator);
        }
        if self.registration(role) != Registration::NotStarted {
            return Err(RegistrationError::AlreadyStarted);
        }
        *self.registration_mut(role) = Registration::Spawning;
        Ok(())
    }
    /// Returns the Worker slot to spawning for a replacement Worker. Only the recorded Server takes
    /// it, and only once the recorded Worker is proven gone through the Server's own handle, or a
    /// failed replacement is known to have left no process: the caller holds that proof. The Host
    /// and the Keeper keep their registrations, identified or not started; either still spawning
    /// blocks it.
    #[cfg(any(windows, test))]
    pub(crate) fn reserve_replacement_worker(
        &mut self,
        creator: Creator,
    ) -> Result<(), RegistrationError> {
        if self.server.identity().map(Creator::Process) != Some(creator) {
            return Err(RegistrationError::WrongCreator);
        }
        if self.worker == Registration::NotStarted {
            return Err(RegistrationError::NoWorker);
        }
        if self.host == Registration::Spawning || self.keeper == Registration::Spawning {
            return Err(RegistrationError::HostSpawning);
        }
        self.worker = Registration::Spawning;
        Ok(())
    }
    /// Returns the Host slot to spawning for a recovery Host, which takes the place of a Host that
    /// ended while the Keeper kept its programs. Only the recorded Worker takes it, and only once
    /// the recorded Host is proven gone: the caller observed its exact identity gone under the
    /// record's writer lock.
    #[cfg(windows)]
    pub(crate) fn reserve_recovery_host(
        &mut self,
        creator: Creator,
    ) -> Result<(), RegistrationError> {
        if self.worker.identity().map(Creator::Process) != Some(creator) {
            return Err(RegistrationError::WrongCreator);
        }
        if self.host.identity().is_none() {
            return Err(RegistrationError::NoHost);
        }
        self.host = Registration::Spawning;
        Ok(())
    }
    /// Reserves the Keeper slot for a Keeper the recorded Worker starts: the first one, or one that
    /// replaces a Keeper proven gone, which the caller observed under the record's writer lock.
    #[cfg(windows)]
    pub(crate) fn reserve_keeper(&mut self, creator: Creator) -> Result<(), RegistrationError> {
        if self.worker.identity().map(Creator::Process) != Some(creator) {
            return Err(RegistrationError::WrongCreator);
        }
        if self.keeper == Registration::Spawning {
            return Err(RegistrationError::AlreadyStarted);
        }
        self.keeper = Registration::Spawning;
        Ok(())
    }
    /// The registrations once a restart committed: `server` and `worker`, the successor pair, take
    /// the Server and Worker slots, and the Host and the Keeper keep their registrations. A Host or
    /// a Keeper still spawning, or an identity that would own two roles, refuses.
    #[cfg(windows)]
    pub(crate) fn hand_over(
        &mut self,
        server: ProcessIdentity,
        worker: ProcessIdentity,
    ) -> Result<(), RegistrationError> {
        if self.host == Registration::Spawning || self.keeper == Registration::Spawning {
            return Err(RegistrationError::HostSpawning);
        }
        let kept = [self.host.identity(), self.keeper.identity()];
        if server == worker || kept.contains(&Some(server)) || kept.contains(&Some(worker)) {
            return Err(RegistrationError::DuplicateIdentity);
        }
        self.server = Registration::Identified { identity: server };
        self.worker = Registration::Identified { identity: worker };
        Ok(())
    }
    #[cfg(any(windows, test))]
    pub(crate) fn identify(
        &mut self,
        role: ProcessRole,
        identity: ProcessIdentity,
    ) -> Result<(), RegistrationError> {
        if self.registration(role) != Registration::Spawning {
            return Err(RegistrationError::NoReservation);
        }
        if ROLES
            .into_iter()
            .any(|other| self.registration(other).identity() == Some(identity))
        {
            return Err(RegistrationError::DuplicateIdentity);
        }
        *self.registration_mut(role) = Registration::Identified { identity };
        Ok(())
    }
    /// Whether the registered processes are gone, judged from `observations` of their exact
    /// identities. A Spawning slot reads as never started: read under the record writer lock its
    /// creator held across the whole attempt, it names an attempt that ended before the identity
    /// write, whose child, if created, was never resumed and never runs. The identified roles,
    /// among them the slot's creator, decide alone.
    pub(crate) fn status(
        &self,
        observations: [ProcessObservation; ROLES.len()],
    ) -> RecoveryVerdict {
        let mut live = None;
        for (role, observation) in ROLES.into_iter().zip(observations) {
            match (self.registration(role), observation) {
                (Registration::Identified { .. }, ProcessObservation::Unknown) => {
                    return RecoveryVerdict::Uncertain(role);
                }
                (Registration::Identified { identity }, ProcessObservation::Present(observed))
                    if identity == observed =>
                {
                    live.get_or_insert(role);
                }
                _ => {}
            }
        }
        live.map_or(RecoveryVerdict::Gone, RecoveryVerdict::Live)
    }
}

#[cfg(test)]
mod tests;
