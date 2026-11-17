//! Carries a replacement Worker's adoption of the Session's running Terminal Host: the recorded
//! Host and, in pages, the Server's terminal bindings, then the Worker's one answer. Construction
//! checks each page and answer alone; whether the pages agree, the Host verification and the
//! import belong to the Worker, and the rebinding to the Server.

use std::{
    collections::BTreeSet,
    num::{NonZeroU32, NonZeroU64},
};

use crate::{
    HostEpoch, HostTerminalIdentity, LifecycleStepOrdinal, OperationId, ProtocolError,
    TerminalFence, TerminalId, WorkerGeneration, WorkerId,
};

/// The most bindings one page carries.
pub const MAXIMUM_ADOPT_HOST_PAGE_BINDINGS: usize = 256;

/// The most bindings a whole adoption carries, and so the most it reports missing: the
/// generation-local binding bound of terminal-launch capability version 1.
pub const MAXIMUM_ADOPT_HOST_BINDINGS: usize = 1_024;

/// The Host process the Session record registered: its process ID and creation time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdoptHostProcess {
    pub process_id: NonZeroU32,
    /// The operating system's creation time for that process, compared exactly.
    pub creation_time: NonZeroU64,
}

/// The Host to adopt, or none.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdoptHostTarget {
    /// No Host was started: the replacement starts one on the next Launch.
    NoHost,
    Host {
        /// The registered process, where the Session record registers one (Windows).
        process: Option<AdoptHostProcess>,
        /// The Host epoch, when the Server knows it from a binding's Host identity.
        epoch: Option<HostEpoch>,
    },
}

/// Whether the Host terminal behind a binding has started its program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdoptBindingState {
    AwaitingStart,
    Started,
}

/// A Close the lost Worker was carrying for a binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdoptCloseInFlight {
    pub close_operation_id: OperationId,
    /// The highest attempt generation the Server issued.
    pub attempt_generation: NonZeroU64,
    /// The highest step ordinal the Server issued in that attempt.
    pub step_ordinal: LifecycleStepOrdinal,
    /// Whether that step may have reached the Host without a known result.
    pub step_effect_unknown: bool,
}

/// One Server binding: a product terminal, its Host terminal and the authority the Server issued.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdoptBinding {
    pub terminal_id: TerminalId,
    pub identity: HostTerminalIdentity,
    pub state: AdoptBindingState,
    /// The highest fence the Server issued for the terminal.
    pub fence: TerminalFence,
    pub close: Option<AdoptCloseInFlight>,
}

/// A page's place in its adoption: its index from zero, and whether it is the last.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdoptHostPage {
    pub index: u16,
    pub last: bool,
}

/// What an answer echoes of the page it answers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdoptHostPageKey {
    pub adoption_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub page: AdoptHostPage,
}

/// One page of an adoption. Every page names the adoption, the Worker and the Host target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdoptHostRequest {
    key: AdoptHostPageKey,
    target: AdoptHostTarget,
    bindings: Vec<AdoptBinding>,
}

impl AdoptHostRequest {
    /// # Errors
    ///
    /// Returns [`ProtocolError::InvalidPayload`] for more than
    /// [`MAXIMUM_ADOPT_HOST_PAGE_BINDINGS`] bindings, a binding with no Host target, a binding whose
    /// Host epoch differs from the target's, or two bindings naming one product terminal, Host
    /// terminal or creation.
    pub fn new(
        key: AdoptHostPageKey,
        target: AdoptHostTarget,
        bindings: Vec<AdoptBinding>,
    ) -> Result<Self, ProtocolError> {
        let epoch = match target {
            AdoptHostTarget::NoHost if !bindings.is_empty() => {
                return Err(ProtocolError::InvalidPayload);
            }
            AdoptHostTarget::NoHost => None,
            AdoptHostTarget::Host { epoch, .. } => epoch,
        };
        let mut terminals = BTreeSet::new();
        let mut host_terminals = BTreeSet::new();
        let mut creations = BTreeSet::new();
        let valid = bindings.len() <= MAXIMUM_ADOPT_HOST_PAGE_BINDINGS
            && bindings.iter().all(|binding| {
                epoch.is_none_or(|epoch| binding.identity.host_epoch() == epoch)
                    && terminals.insert(binding.terminal_id)
                    && host_terminals.insert(binding.identity.terminal_id())
                    && creations.insert(binding.identity.creation_id())
            });
        if !valid {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self {
            key,
            target,
            bindings,
        })
    }

    #[must_use]
    pub const fn key(&self) -> AdoptHostPageKey {
        self.key
    }

    #[must_use]
    pub const fn target(&self) -> AdoptHostTarget {
        self.target
    }

    #[must_use]
    pub fn bindings(&self) -> &[AdoptBinding] {
        &self.bindings
    }
}

/// Why an adoption failed, with nothing imported.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdoptHostFailure {
    /// No process has the recorded ID, or no Host holds the Host lock.
    HostGone,
    /// The Host endpoint could not be dialed.
    HostUnreachable,
    /// The creation time or the endpoint's server process differs from the record.
    HostUnverified,
    /// The Host epoch differs from the recorded one.
    HostMismatch,
    /// The Host refused the `worker-host` handshake other than for capacity.
    HostRefused,
    /// The Host inventory could not be read, or never stayed the same while it was read.
    InventoryFailed,
}

/// A page's answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdoptHostOutcome {
    /// A page before the last was kept for the import.
    PageAccepted,
    /// The import took effect: the Host terminals imported and quarantined, and the bindings the
    /// complete inventory lacks.
    Adopted {
        imported: u32,
        quarantined: u32,
        missing: Vec<TerminalId>,
    },
    Failed(AdoptHostFailure),
}

/// The answer to one page: a page before the last is accepted, and the last is adopted or failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdoptHostResult {
    key: AdoptHostPageKey,
    outcome: AdoptHostOutcome,
}

impl AdoptHostResult {
    /// # Errors
    ///
    /// Returns [`ProtocolError::InvalidPayload`] when a page before the last is not answered
    /// `PageAccepted`, the last page is, or the missing list exceeds
    /// [`MAXIMUM_ADOPT_HOST_BINDINGS`] or names one terminal twice.
    pub fn new(key: AdoptHostPageKey, outcome: AdoptHostOutcome) -> Result<Self, ProtocolError> {
        let valid = match &outcome {
            AdoptHostOutcome::PageAccepted => !key.page.last,
            AdoptHostOutcome::Failed(_) => key.page.last,
            AdoptHostOutcome::Adopted { missing, .. } => {
                let mut seen = BTreeSet::new();
                key.page.last
                    && missing.len() <= MAXIMUM_ADOPT_HOST_BINDINGS
                    && missing.iter().all(|terminal| seen.insert(*terminal))
            }
        };
        if !valid {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self { key, outcome })
    }

    #[must_use]
    pub const fn key(&self) -> AdoptHostPageKey {
        self.key
    }

    #[must_use]
    pub const fn outcome(&self) -> &AdoptHostOutcome {
        &self.outcome
    }

    #[must_use]
    pub fn into_outcome(self) -> AdoptHostOutcome {
        self.outcome
    }
}
