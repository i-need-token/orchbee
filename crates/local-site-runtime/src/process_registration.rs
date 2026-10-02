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
    pub(crate) fn new(pid: u32, creation_time: u64) -> Option<Self> {
        Some(Self {
            pid: NonZeroU32::new(pid)?,
            creation_time: NonZeroU64::new(creation_time)?,
        })
    }
    pub(crate) fn pid(self) -> u32 {
        self.pid.get()
    }
    #[cfg(test)]
    pub(crate) fn creation_time(self) -> u64 {
        self.creation_time.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProcessRole {
    Server,
    Worker,
    Host,
}
const ROLES: [ProcessRole; 3] = [ProcessRole::Server, ProcessRole::Worker, ProcessRole::Host];

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
    pub(crate) fn identity(self) -> Option<ProcessIdentity> {
        match self {
            Self::Identified { identity } => Some(identity),
            Self::NotStarted | Self::Spawning => None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub(crate) struct SiteProcesses {
    server: Registration,
    worker: Registration,
    host: Registration,
}

impl<'de> Deserialize<'de> for SiteProcesses {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            server: Registration,
            worker: Registration,
            host: Registration,
        }
        let wire = Wire::deserialize(deserializer)?;
        let processes = Self {
            server: wire.server,
            worker: wire.worker,
            host: wire.host,
        };
        if (processes.worker != Registration::NotStarted && processes.server.identity().is_none())
            || (processes.host != Registration::NotStarted && processes.worker.identity().is_none())
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RegistrationError {
    WrongCreator,
    AlreadyStarted,
    NoReservation,
    DuplicateIdentity,
}

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

impl SiteProcesses {
    pub(crate) fn is_unstarted(&self) -> bool {
        ROLES
            .into_iter()
            .all(|role| self.registration(role) == Registration::NotStarted)
    }

    pub(crate) fn registration(&self, role: ProcessRole) -> Registration {
        match role {
            ProcessRole::Server => self.server,
            ProcessRole::Worker => self.worker,
            ProcessRole::Host => self.host,
        }
    }
    fn registration_mut(&mut self, role: ProcessRole) -> &mut Registration {
        match role {
            ProcessRole::Server => &mut self.server,
            ProcessRole::Worker => &mut self.worker,
            ProcessRole::Host => &mut self.host,
        }
    }
    pub(crate) fn reserve(
        &mut self,
        role: ProcessRole,
        creator: Creator,
    ) -> Result<(), RegistrationError> {
        let required = match role {
            ProcessRole::Server => Some(Creator::Activator),
            ProcessRole::Worker => self.server.identity().map(Creator::Process),
            ProcessRole::Host => self.worker.identity().map(Creator::Process),
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
    pub(crate) fn status(&self, observations: [ProcessObservation; 3]) -> RecoveryVerdict {
        let mut live = None;
        for (role, observation) in ROLES.into_iter().zip(observations) {
            match (self.registration(role), observation) {
                (Registration::Spawning, _)
                | (Registration::Identified { .. }, ProcessObservation::Unknown) => {
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
