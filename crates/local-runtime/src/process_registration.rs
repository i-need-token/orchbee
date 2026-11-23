//! Generation-local ownership facts. Operating-system observations confer no kill authority.
use crate::{HostSlot, KeeperSlot, MAXIMUM_HOSTS};
use serde::{Deserialize, Deserializer, Serialize, de};
use std::num::{NonZeroU32, NonZeroU64};

/// A registered process: its ID and when it started, as Windows reports its creation time (100 ns
/// units since 1601) or as macOS reports its start time (microseconds since 1970).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProcessIdentity {
    pid: NonZeroU32,
    creation_time: NonZeroU64,
}
impl ProcessIdentity {
    #[cfg(any(windows, target_os = "macos", test))]
    pub(crate) fn new(pid: u32, creation_time: u64) -> Option<Self> {
        Some(Self {
            pid: NonZeroU32::new(pid)?,
            creation_time: NonZeroU64::new(creation_time)?,
        })
    }
    pub(crate) fn pid(self) -> u32 {
        self.pid.get()
    }
    #[cfg(any(windows, target_os = "macos", test))]
    pub(crate) fn creation_time(self) -> u64 {
        self.creation_time.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProcessRole {
    Server,
    Worker,
    /// The Terminal Host in one of the Session's Host slots.
    Host(HostSlot),
    /// The Terminal Host's Keeper in one of its two slots, which the Worker starts once its first
    /// Host is ready and which keeps a Host's programs running when that Host ends.
    Keeper(KeeperSlot),
}
pub(crate) const ROLES: [ProcessRole; 4 + MAXIMUM_HOSTS] = [
    ProcessRole::Server,
    ProcessRole::Worker,
    ProcessRole::Host(HostSlot::ALL[0]),
    ProcessRole::Host(HostSlot::ALL[1]),
    ProcessRole::Host(HostSlot::ALL[2]),
    ProcessRole::Host(HostSlot::ALL[3]),
    ProcessRole::Keeper(KeeperSlot::First),
    ProcessRole::Keeper(KeeperSlot::Second),
];
/// The Keeper role of both slots, in order.
pub(crate) const KEEPER_ROLES: [ProcessRole; 2] = [
    ProcessRole::Keeper(KeeperSlot::First),
    ProcessRole::Keeper(KeeperSlot::Second),
];
/// The Host role of every slot, in order.
pub(crate) const HOST_ROLES: [ProcessRole; MAXIMUM_HOSTS] = [
    ProcessRole::Host(HostSlot::ALL[0]),
    ProcessRole::Host(HostSlot::ALL[1]),
    ProcessRole::Host(HostSlot::ALL[2]),
    ProcessRole::Host(HostSlot::ALL[3]),
];

#[cfg(any(windows, target_os = "macos", test))]
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
        /// A Host's epoch, which the Worker that started or adopted it writes once connected.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        epoch: Option<[u8; 16]>,
    },
}
impl Registration {
    fn is_not_started(&self) -> bool {
        *self == Self::NotStarted
    }
    #[cfg(any(windows, target_os = "macos", test))]
    pub(crate) const fn identified(identity: ProcessIdentity) -> Self {
        Self::Identified {
            identity,
            epoch: None,
        }
    }
    pub(crate) fn identity(self) -> Option<ProcessIdentity> {
        match self {
            Self::Identified { identity, .. } => Some(identity),
            Self::NotStarted | Self::Spawning => None,
        }
    }
    pub(crate) fn epoch(self) -> Option<[u8; 16]> {
        match self {
            Self::Identified { epoch, .. } => epoch,
            Self::NotStarted | Self::Spawning => None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub(crate) struct SessionProcesses {
    server: Registration,
    worker: Registration,
    /// The first Host slot, the one Host a record named before several could run.
    host: Registration,
    /// The other Host slots, each written only once it is reserved, so a record with one Host
    /// reads as it did before.
    #[serde(skip_serializing_if = "Registration::is_not_started")]
    host1: Registration,
    #[serde(skip_serializing_if = "Registration::is_not_started")]
    host2: Registration,
    #[serde(skip_serializing_if = "Registration::is_not_started")]
    host3: Registration,
    /// Written only once a Keeper is reserved, so a record without one reads as it did before.
    #[serde(skip_serializing_if = "Registration::is_not_started")]
    keeper: Registration,
    /// The second Keeper slot, written only once a Keeper update reserves it.
    #[serde(skip_serializing_if = "Registration::is_not_started")]
    keeper1: Registration,
    /// The slot of the Session's current Keeper, written only while it is the second. While both
    /// slots name a Keeper, the other names one an update started and has not committed, or one a
    /// committed update replaced.
    #[serde(skip_serializing_if = "KeeperSlot::is_first")]
    current_keeper: KeeperSlot,
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
            host1: Registration,
            #[serde(default)]
            host2: Registration,
            #[serde(default)]
            host3: Registration,
            #[serde(default)]
            keeper: Registration,
            #[serde(default)]
            keeper1: Registration,
            #[serde(default)]
            current_keeper: KeeperSlot,
        }
        let wire = Wire::deserialize(deserializer)?;
        let processes = Self {
            server: wire.server,
            worker: wire.worker,
            host: wire.host,
            host1: wire.host1,
            host2: wire.host2,
            host3: wire.host3,
            keeper: wire.keeper,
            keeper1: wire.keeper1,
            current_keeper: wire.current_keeper,
        };
        // A Host or a Keeper is reserved only by an identified Worker. An identified Host or Keeper
        // needs no registered Worker: a replacement Worker returns its slot to spawning while the
        // Host and Keeper it adopts keep their identities, and a generation that adopts its
        // predecessor's running Host starts with that Host alone. An older reader refuses both
        // instead of misreading them, a record with a Keeper, and one with a second Keeper.
        let reserved_by_worker = |slot: Registration| {
            slot == Registration::Spawning && processes.worker.identity().is_none()
        };
        if (processes.worker != Registration::NotStarted && processes.server.identity().is_none())
            || HOST_ROLES
                .into_iter()
                .chain(KEEPER_ROLES)
                .any(|role| reserved_by_worker(processes.registration(role)))
        {
            return Err(de::Error::custom(
                "child registration requires its identified parent",
            ));
        }
        let mut identities = Vec::new();
        for role in ROLES {
            let registration = processes.registration(role);
            if let Some(identity) = registration.identity() {
                if identities.contains(&identity) {
                    return Err(de::Error::custom("process identity cannot own two roles"));
                }
                identities.push(identity);
            }
            if registration.epoch().is_some() && !matches!(role, ProcessRole::Host(_)) {
                return Err(de::Error::custom(
                    "only a Host registration carries an epoch",
                ));
            }
        }
        Ok(processes)
    }
}

#[cfg(any(windows, target_os = "macos", test))]
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
    #[cfg(any(windows, target_os = "macos"))]
    NoHost,
    /// An epoch is written only for the identified Host it names.
    #[cfg(any(windows, target_os = "macos"))]
    NotThatHost,
}

#[cfg_attr(
    not(any(windows, target_os = "macos", test)),
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
    /// The registrations of a generation that adopts `hosts`, the Hosts its predecessor left
    /// running, each in its slot with the epoch it recorded, and `keepers`, the Keepers it left
    /// running beside them: its Server and Worker are not started yet.
    pub(crate) fn adopting(hosts: &[KeptHost], keepers: KeptKeepers) -> Self {
        let mut processes = Self::default();
        for kept in hosts {
            *processes.registration_mut(ProcessRole::Host(kept.slot)) = Registration::Identified {
                identity: kept.identity,
                epoch: kept.epoch,
            };
        }
        for slot in KeeperSlot::ALL {
            if let Some(identity) = keepers.slots[slot.index()] {
                *processes.registration_mut(ProcessRole::Keeper(slot)) = Registration::Identified {
                    identity,
                    epoch: None,
                };
            }
        }
        processes.current_keeper = keepers.current;
        processes
    }

    /// The slot of the Session's current Keeper.
    pub(crate) const fn current_keeper(&self) -> KeeperSlot {
        self.current_keeper
    }

    /// The Hosts these registrations name as identified, each with its slot and epoch.
    #[cfg(any(windows, target_os = "macos"))]
    pub(crate) fn identified_hosts(&self) -> Vec<KeptHost> {
        HostSlot::ALL
            .into_iter()
            .filter_map(|slot| {
                let registration = self.registration(ProcessRole::Host(slot));
                Some(KeptHost {
                    slot,
                    identity: registration.identity()?,
                    epoch: registration.epoch(),
                })
            })
            .collect()
    }

    /// Whether any Host slot is still spawning.
    #[cfg(any(windows, target_os = "macos", test))]
    fn host_spawning(&self) -> bool {
        HOST_ROLES
            .into_iter()
            .any(|role| self.registration(role) == Registration::Spawning)
    }

    /// Whether either Keeper slot is still spawning.
    #[cfg(any(windows, target_os = "macos", test))]
    fn keeper_spawning(&self) -> bool {
        KEEPER_ROLES
            .into_iter()
            .any(|role| self.registration(role) == Registration::Spawning)
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
            ProcessRole::Host(slot) => match slot.index() {
                0 => self.host,
                1 => self.host1,
                2 => self.host2,
                _ => self.host3,
            },
            ProcessRole::Keeper(KeeperSlot::First) => self.keeper,
            ProcessRole::Keeper(KeeperSlot::Second) => self.keeper1,
        }
    }
    fn registration_mut(&mut self, role: ProcessRole) -> &mut Registration {
        match role {
            ProcessRole::Server => &mut self.server,
            ProcessRole::Worker => &mut self.worker,
            ProcessRole::Host(slot) => match slot.index() {
                0 => &mut self.host,
                1 => &mut self.host1,
                2 => &mut self.host2,
                _ => &mut self.host3,
            },
            ProcessRole::Keeper(KeeperSlot::First) => &mut self.keeper,
            ProcessRole::Keeper(KeeperSlot::Second) => &mut self.keeper1,
        }
    }
    #[cfg(any(windows, target_os = "macos", test))]
    pub(crate) fn reserve(
        &mut self,
        role: ProcessRole,
        creator: Creator,
    ) -> Result<(), RegistrationError> {
        let required = match role {
            ProcessRole::Server => Some(Creator::Activator),
            ProcessRole::Worker => self.server.identity().map(Creator::Process),
            ProcessRole::Host(_) | ProcessRole::Keeper(_) => {
                self.worker.identity().map(Creator::Process)
            }
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
    #[cfg(any(windows, target_os = "macos", test))]
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
        if self.host_spawning() || self.keeper_spawning() {
            return Err(RegistrationError::HostSpawning);
        }
        self.worker = Registration::Spawning;
        Ok(())
    }
    /// Returns `slot` to spawning for a recovery Host, which takes the place of the Host there that
    /// ended while the Keeper kept its programs. Only the recorded Worker takes it, and only once
    /// the recorded Host is proven gone: the caller observed its exact identity gone under the
    /// record's writer lock.
    #[cfg(any(windows, target_os = "macos"))]
    pub(crate) fn reserve_recovery_host(
        &mut self,
        slot: HostSlot,
        creator: Creator,
    ) -> Result<(), RegistrationError> {
        if self.worker.identity().map(Creator::Process) != Some(creator) {
            return Err(RegistrationError::WrongCreator);
        }
        let registration = self.registration_mut(ProcessRole::Host(slot));
        if registration.identity().is_none() {
            return Err(RegistrationError::NoHost);
        }
        *registration = Registration::Spawning;
        Ok(())
    }
    /// Writes `epoch` for the Host in `slot`, which must still be `identity`: the Worker that
    /// started or adopted it writes the epoch the Host answered with, so the Server can pair the
    /// Host with the terminals that carry it.
    #[cfg(any(windows, target_os = "macos"))]
    pub(crate) fn record_host_epoch(
        &mut self,
        slot: HostSlot,
        identity: ProcessIdentity,
        epoch: [u8; 16],
    ) -> Result<(), RegistrationError> {
        let registration = self.registration_mut(ProcessRole::Host(slot));
        if registration.identity() != Some(identity) {
            return Err(RegistrationError::NotThatHost);
        }
        *registration = Registration::Identified {
            identity,
            epoch: Some(epoch),
        };
        Ok(())
    }
    /// Frees `slot`, which must still name `identity`: the Worker that drained that Host saw it end
    /// normally, so no Host is left there to adopt or recover, and a later Host may start in it.
    #[cfg(any(windows, target_os = "macos"))]
    pub(crate) fn release_host(
        &mut self,
        slot: HostSlot,
        identity: ProcessIdentity,
    ) -> Result<(), RegistrationError> {
        let registration = self.registration_mut(ProcessRole::Host(slot));
        if registration.identity() != Some(identity) {
            return Err(RegistrationError::NotThatHost);
        }
        *registration = Registration::NotStarted;
        Ok(())
    }
    /// Reserves the Keeper slot `slot` for a Keeper the recorded Worker starts: the first one, one
    /// that replaces a Keeper proven gone, or the one a Keeper update starts beside the current
    /// one; the caller observed any Keeper the slot named gone under the record's writer lock.
    #[cfg(any(windows, target_os = "macos"))]
    pub(crate) fn reserve_keeper(
        &mut self,
        slot: KeeperSlot,
        creator: Creator,
    ) -> Result<(), RegistrationError> {
        if self.worker.identity().map(Creator::Process) != Some(creator) {
            return Err(RegistrationError::WrongCreator);
        }
        let registration = self.registration_mut(ProcessRole::Keeper(slot));
        if *registration == Registration::Spawning {
            return Err(RegistrationError::AlreadyStarted);
        }
        *registration = Registration::Spawning;
        Ok(())
    }
    /// Makes `slot`, which must name `identity`, the slot of the current Keeper: the Worker commits
    /// a Keeper update once the Keeper there holds everything the current one holds, or adopts the
    /// one live Keeper left in the other slot.
    #[cfg(any(windows, target_os = "macos"))]
    pub(crate) fn make_keeper_current(
        &mut self,
        slot: KeeperSlot,
        identity: ProcessIdentity,
    ) -> Result<(), RegistrationError> {
        if self.registration(ProcessRole::Keeper(slot)).identity() != Some(identity) {
            return Err(RegistrationError::NotThatHost);
        }
        self.current_keeper = slot;
        Ok(())
    }
    /// Frees `slot`, which must still name `identity` and not be the current Keeper's: the Worker
    /// ended the Keeper an update replaced or rolled back, which held nothing then.
    #[cfg(any(windows, target_os = "macos"))]
    pub(crate) fn release_keeper(
        &mut self,
        slot: KeeperSlot,
        identity: ProcessIdentity,
    ) -> Result<(), RegistrationError> {
        if slot == self.current_keeper
            || self.registration(ProcessRole::Keeper(slot)).identity() != Some(identity)
        {
            return Err(RegistrationError::NotThatHost);
        }
        *self.registration_mut(ProcessRole::Keeper(slot)) = Registration::NotStarted;
        Ok(())
    }
    /// The registrations once a restart committed: `server` and `worker`, the successor pair, take
    /// the Server and Worker slots, and the Host and the Keeper keep their registrations. A Host or
    /// a Keeper still spawning, or an identity that would own two roles, refuses.
    #[cfg(any(windows, target_os = "macos"))]
    pub(crate) fn hand_over(
        &mut self,
        server: ProcessIdentity,
        worker: ProcessIdentity,
    ) -> Result<(), RegistrationError> {
        if self.host_spawning() || self.keeper_spawning() {
            return Err(RegistrationError::HostSpawning);
        }
        let kept: Vec<_> = HOST_ROLES
            .into_iter()
            .chain(KEEPER_ROLES)
            .filter_map(|role| self.registration(role).identity())
            .collect();
        if server == worker || kept.contains(&server) || kept.contains(&worker) {
            return Err(RegistrationError::DuplicateIdentity);
        }
        self.server = Registration::identified(server);
        self.worker = Registration::identified(worker);
        Ok(())
    }
    #[cfg(any(windows, target_os = "macos", test))]
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
        *self.registration_mut(role) = Registration::identified(identity);
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
                (
                    Registration::Identified { identity, .. },
                    ProcessObservation::Present(observed),
                ) if identity == observed => {
                    live.get_or_insert(role);
                }
                _ => {}
            }
        }
        live.map_or(RecoveryVerdict::Gone, RecoveryVerdict::Live)
    }
}

/// The Keepers a generation keeps from its predecessor: each live one in its slot, and the slot of
/// the current one.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct KeptKeepers {
    pub(crate) slots: [Option<ProcessIdentity>; 2],
    pub(crate) current: KeeperSlot,
}

/// A Host a generation keeps from its predecessor: its slot, process and recorded epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct KeptHost {
    pub(crate) slot: HostSlot,
    pub(crate) identity: ProcessIdentity,
    pub(crate) epoch: Option<[u8; 16]>,
}

#[cfg(test)]
mod tests;
