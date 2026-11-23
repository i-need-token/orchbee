//! Carries a replacement Worker's adoption of the Session's running Terminal Hosts: the recorded
//! Host slots and, in pages, the Server's terminal bindings, then the Worker's one answer.
//! Construction checks each page and answer alone; whether the pages agree, the Host verification
//! and the import belong to the Worker, and the rebinding to the Server.

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

/// The most Host slots one adoption names: a Session's Host slots.
pub const MAXIMUM_ADOPT_HOST_SLOTS: usize = 4;

/// What runs in one Host slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdoptHostKind {
    /// A running Host.
    Host {
        /// The registered process, where the Session record registers one (Windows).
        process: Option<AdoptHostProcess>,
    },
    /// The Host ended while the Session's Keeper kept its programs: the replacement starts a
    /// recovery Host in the slot and recovers each terminal and program of the lost epoch into it.
    Recovery,
}

/// One Host slot to adopt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdoptHostSlot {
    /// The slot's number, 0 through 3.
    pub slot: u8,
    pub kind: AdoptHostKind,
    /// The Host's epoch, or the lost Host's for a recovery, when the Server knows it.
    pub epoch: Option<HostEpoch>,
}

/// The Hosts to adopt: none, when no Host was started and the replacement starts one on the next
/// Launch, or up to four slots, with the Keeper the Session record registered when any is a
/// recovery.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdoptHostTarget {
    slots: Vec<AdoptHostSlot>,
    keeper: Option<AdoptHostProcess>,
}

impl AdoptHostTarget {
    /// No Host to adopt.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            slots: Vec::new(),
            keeper: None,
        }
    }

    /// # Errors
    ///
    /// Returns [`ProtocolError::InvalidPayload`] for more than [`MAXIMUM_ADOPT_HOST_SLOTS`] slots,
    /// a slot number past 3 or named twice, two slots with one epoch, or a Keeper given exactly
    /// when no slot is a recovery.
    pub fn new(
        slots: Vec<AdoptHostSlot>,
        keeper: Option<AdoptHostProcess>,
    ) -> Result<Self, ProtocolError> {
        let mut numbers = BTreeSet::new();
        let mut epochs = BTreeSet::new();
        let recovering = slots
            .iter()
            .any(|slot| slot.kind == AdoptHostKind::Recovery);
        let valid = slots.len() <= MAXIMUM_ADOPT_HOST_SLOTS
            && slots.iter().all(|slot| {
                usize::from(slot.slot) < MAXIMUM_ADOPT_HOST_SLOTS
                    && numbers.insert(slot.slot)
                    && slot.epoch.is_none_or(|epoch| epochs.insert(epoch))
            })
            && recovering == keeper.is_some();
        if !valid {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self { slots, keeper })
    }

    /// The slots, in the order the Server named them.
    #[must_use]
    pub fn slots(&self) -> &[AdoptHostSlot] {
        &self.slots
    }

    /// The Keeper, present exactly when a slot is a recovery.
    #[must_use]
    pub const fn keeper(&self) -> Option<AdoptHostProcess> {
        self.keeper
    }

    /// Whether no Host is to be adopted.
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.slots.is_empty()
    }

    /// The epochs the slots name, when every slot names one.
    fn epochs(&self) -> Option<Vec<HostEpoch>> {
        self.slots.iter().map(|slot| slot.epoch).collect()
    }
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

/// One page of an adoption. Every page names the adoption, the Worker and the Host targets.
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
    /// Host epoch is none of the targets' when every target names one, or two bindings naming one
    /// product terminal, or one Host terminal or creation of one epoch.
    pub fn new(
        key: AdoptHostPageKey,
        target: AdoptHostTarget,
        bindings: Vec<AdoptBinding>,
    ) -> Result<Self, ProtocolError> {
        if target.is_none() && !bindings.is_empty() {
            return Err(ProtocolError::InvalidPayload);
        }
        let epochs = target.epochs();
        let mut terminals = BTreeSet::new();
        let mut host_terminals = BTreeSet::new();
        let mut creations = BTreeSet::new();
        let valid = bindings.len() <= MAXIMUM_ADOPT_HOST_PAGE_BINDINGS
            && bindings.iter().all(|binding| {
                let epoch = binding.identity.host_epoch();
                epochs.as_ref().is_none_or(|epochs| epochs.contains(&epoch))
                    && terminals.insert(binding.terminal_id)
                    && host_terminals.insert((epoch, binding.identity.terminal_id()))
                    && creations.insert((epoch, binding.identity.creation_id()))
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
    pub const fn target(&self) -> &AdoptHostTarget {
        &self.target
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
    /// A program the Keeper kept was not recovered into the recovery Host; it runs on in the
    /// Keeper, and a terminal already recovered stays recovered.
    RecoveryFailed,
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
