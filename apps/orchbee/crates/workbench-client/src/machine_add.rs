//! Adding a machine: the one staged check pipeline the TUI form and `machine add` share. The new
//! Machine is connected through the hub as a draft under the id it will be saved with, so every
//! check after the first runs on one connection with one login; the record is saved only once the
//! hub reads the draft `online`, and the hub keeps that link when it rereads the catalog. A draft
//! that never comes `online` is dropped with its connection, and nothing was saved.
//!
//! Drawing-free: an interface shows [`Stage`] and the hub's state in its own words.
use crate::machines::{Catalog, Draft, Edit, Error, ProfileId, Store};
use orchbee_machine_protocol::{
    MachineId, SessionName, Target,
    hub::{
        Connect, ConnectFailure, MachineState, MachineStateName, Origin, Request, Stage as Step,
    },
};

/// The checks of an add, in the order the design lists them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    /// This computer has OpenSSH.
    Ssh,
    /// The host answers.
    Reach,
    /// The host key is trusted.
    Trust,
    /// The login succeeds.
    Login,
    /// The remote system and processor are supported.
    System,
    /// The remote has Orchbee.
    Orchbee,
    /// The two Orchbee builds can work together.
    Compatibility,
    /// The remote Session is ready.
    Session,
}

impl Stage {
    /// The check a draft that reads `state` is running.
    pub fn running(state: &MachineState) -> Self {
        match state {
            MachineState::Connecting { stage, .. } | MachineState::Reconnecting { stage, .. } => {
                match stage {
                    Some(Step::Authenticating) => Self::Login,
                    Some(Step::Starting) => Self::Session,
                    Some(Step::Copying) => Self::Orchbee,
                    Some(Step::Probing) | None => Self::Reach,
                }
            }
            MachineState::Untrusted { .. } | MachineState::KeyChanged { .. } => Self::Trust,
            MachineState::NeedsLogin { .. } | MachineState::LoginFailed { .. } => Self::Login,
            _ => Self::Reach,
        }
    }

    /// The check an add ended at, from the hub's `connect` failure and the state the draft read
    /// then; `None` when the add did not fail at a check: it was cancelled, the hub went away, or
    /// the request itself was refused.
    pub fn failed(failure: ConnectFailure, state: Option<MachineStateName>) -> Option<Self> {
        Some(match failure {
            ConnectFailure::SshUnavailable => Self::Ssh,
            ConnectFailure::HostUnreachable | ConnectFailure::SshFailed => Self::Reach,
            ConnectFailure::HostUntrusted | ConnectFailure::HostKeyChanged => Self::Trust,
            ConnectFailure::AuthRequired | ConnectFailure::AuthFailed => Self::Login,
            ConnectFailure::RemoteUnsupported => Self::System,
            ConnectFailure::RemoteNotInstalled | ConnectFailure::RemoteInstallFailed => {
                Self::Orchbee
            }
            ConnectFailure::VersionIncompatible | ConnectFailure::RemoteOtherEnvironment => {
                Self::Compatibility
            }
            ConnectFailure::RemoteNotReady | ConnectFailure::RemoteProtocol => Self::Session,
            // The attempt's bound passed: the state says which check was running.
            ConnectFailure::Timeout => match state? {
                MachineStateName::NeedsLogin | MachineStateName::LoginFailed => Self::Login,
                MachineStateName::Untrusted | MachineStateName::KeyChanged => Self::Trust,
                _ => Self::Reach,
            },
            ConnectFailure::NotFound
            | ConnectFailure::DisabledMachine
            | ConnectFailure::UnsupportedMachine
            | ConnectFailure::InvalidDraft
            | ConnectFailure::Cancelled
            | ConnectFailure::Stopping
            | ConnectFailure::Internal
            | ConnectFailure::Unknown => return None,
        })
    }
}

/// A machine being added: the record it will be, under the id it is connected and saved with.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: ProfileId,
    pub draft: Draft,
}

impl Proposal {
    /// A new machine `draft` under a fresh id; `catalog` must be able to take it.
    pub fn new(draft: Draft, catalog: &Catalog) -> Result<Self, Error> {
        let id = ProfileId::generate()?;
        let proposal = Self { id, draft };
        catalog.edit(&proposal.edit())?;
        Ok(proposal)
    }

    fn edit(&self) -> Edit {
        Edit::Add {
            id: self.id.clone(),
            draft: self.draft.clone(),
        }
    }

    /// The hub's id of the Machine.
    pub fn machine(&self) -> Option<MachineId> {
        MachineId::new(self.id.as_str()).ok()
    }

    /// The draft as the hub carries it; `None` for a draft the hub cannot carry, which
    /// [`Draft::parse`] never builds.
    pub fn hub_draft(&self) -> Option<orchbee_machine_protocol::hub::Draft> {
        let target = Target::new(self.draft.ssh_target()?).ok()?;
        let session = SessionName::new(self.draft.session.as_str()).ok()?;
        Some(orchbee_machine_protocol::hub::Draft { target, session })
    }

    /// The `connect` that brings the draft `online` for `origin`.
    pub fn request(&self, origin: Origin) -> Option<Request> {
        Some(Request::Connect(Connect {
            machine: self.machine()?,
            origin,
            draft: Some(self.hub_draft()?),
        }))
    }

    /// Saves the record once the draft is `online`; the hub that rereads the catalog keeps the
    /// draft's link.
    pub fn save(&self, store: &Store) -> Result<(Catalog, bool), Error> {
        store.commit(&self.edit())
    }
}
