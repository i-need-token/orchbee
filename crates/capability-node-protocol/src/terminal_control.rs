//! Defines generation-fenced controller requests, bounded input and color values, and
//! admissible action-result pairs. It carries an authorization claim without checking live
//! Worker, viewer, controller, or capability state.

use crate::{OperationId, ProtocolError, TerminalPixelGeometry, WorkerGeneration, WorkerId};
use std::num::NonZeroU64;
pub const MAXIMUM_CONTROL_INPUT_BYTES: usize = 4096;
pub const MAXIMUM_PASTE_CONTENT_BYTES: usize = 256 * 1024;

/// Bounded content byte limit for one `Paste` record carried by `OBNQ`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PasteContent(Vec<u8>);
impl PasteContent {
    pub fn new(bytes: Vec<u8>) -> Result<Self, ProtocolError> {
        if bytes.is_empty() || bytes.len() > MAXIMUM_PASTE_CONTENT_BYTES {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self(bytes))
    }
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Host-reported encoded byte count accepted into the bounded input queue
/// for one paste operation; always positive and capped by `Self::MAXIMUM`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PasteAcceptedBytes(u32);
impl PasteAcceptedBytes {
    pub const MAXIMUM: u32 = 1024 * 1024;

    pub fn new(value: u32) -> Result<Self, ProtocolError> {
        if value == 0 || value > Self::MAXIMUM {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlInput(Vec<u8>);
impl ControlInput {
    pub fn new(bytes: Vec<u8>) -> Result<Self, ProtocolError> {
        if bytes.is_empty() || bytes.len() > MAXIMUM_CONTROL_INPUT_BYTES {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self(bytes))
    }
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
/// One semantic input record's largest payload.
pub const MAXIMUM_SEMANTIC_INPUT_BYTES: usize = 16 * 1024;

/// The Host semantic input record a `SemanticInput` action carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticInputKind {
    Focus,
    Key,
    Mouse,
    Text,
}

/// One encoded Host semantic input payload. Its attachment and capability fields are claims the
/// Worker replaces with the live controller grant before submission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticInput {
    kind: SemanticInputKind,
    payload: Vec<u8>,
}
impl SemanticInput {
    pub fn new(kind: SemanticInputKind, payload: Vec<u8>) -> Result<Self, ProtocolError> {
        if payload.is_empty() || payload.len() > MAXIMUM_SEMANTIC_INPUT_BYTES {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self { kind, payload })
    }
    #[must_use]
    pub const fn kind(&self) -> SemanticInputKind {
        self.kind
    }
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

/// A Client's observed outer-terminal default colors, each a 24-bit RGB value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostDefaultColors {
    foreground: u32,
    background: u32,
}
impl HostDefaultColors {
    pub const fn new(foreground: u32, background: u32) -> Result<Self, ProtocolError> {
        if foreground > 0x00ff_ffff || background > 0x00ff_ffff {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self {
            foreground,
            background,
        })
    }
    #[must_use]
    pub const fn foreground(self) -> u32 {
        self.foreground
    }
    #[must_use]
    pub const fn background(self) -> u32 {
        self.background
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalControlAction {
    Acquire,
    /// Independent transient input attachment; never acquires a controller lease.
    Clear,
    /// A full terminal reset through its own transient input attachment, as Clear.
    Reset,
    Takeover {
        previous_controller_id: OperationId,
    },
    Release,
    Input {
        sequence: NonZeroU64,
        bytes: ControlInput,
    },
    SemanticInput {
        sequence: NonZeroU64,
        input: SemanticInput,
    },
    Resize {
        sequence: NonZeroU64,
        geometry: TerminalPixelGeometry,
    },
    Paste {
        sequence: NonZeroU64,
        content: PasteContent,
    },
    /// The controlling Client's outer-terminal colors for Host-default terminals.
    HostColors {
        sequence: NonZeroU64,
        colors: HostDefaultColors,
    },
}
impl TerminalControlAction {
    /// Admits only this action's result, including its geometry or encoded paste bound.
    /// Any action may be rejected; only Clear and Reset admit their own unknown outcome.
    #[must_use]
    pub fn admits(&self, outcome: &TerminalControlOutcome) -> bool {
        let rejected = matches!(outcome, TerminalControlOutcome::Rejected(_));
        match self {
            Self::Acquire | Self::Takeover { .. } => {
                matches!(outcome, TerminalControlOutcome::Acquired) || rejected
            }
            Self::Clear => {
                matches!(
                    outcome,
                    TerminalControlOutcome::Cleared { .. }
                        | TerminalControlOutcome::ClearOutcomeUnknown
                ) || rejected
            }
            Self::Reset => {
                matches!(
                    outcome,
                    TerminalControlOutcome::ResetAccepted { .. }
                        | TerminalControlOutcome::ResetOutcomeUnknown
                ) || rejected
            }
            Self::Release => matches!(outcome, TerminalControlOutcome::Released) || rejected,
            Self::Input { .. } | Self::SemanticInput { .. } => {
                matches!(outcome, TerminalControlOutcome::InputAccepted) || rejected
            }
            Self::Resize { geometry, .. } => {
                matches!(outcome, TerminalControlOutcome::Resized(actual) if geometry == actual)
                    || rejected
            }
            Self::Paste { content, .. } => {
                matches!(outcome, TerminalControlOutcome::Pasted { accepted_bytes }
                    if content.as_bytes().len().checked_mul(2)
                        .and_then(|bound| bound.checked_add(12))
                        .is_some_and(|bound| usize::try_from(accepted_bytes.get()).is_ok_and(|value| value <= bound)))
                    || rejected
            }
            Self::HostColors { .. } => {
                matches!(outcome, TerminalControlOutcome::HostColorsAccepted) || rejected
            }
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalControlRequest {
    pub request_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub viewer_id: OperationId,
    pub controller_id: OperationId,
    pub action: TerminalControlAction,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlRejection {
    WrongWorker,
    StaleWorkerGeneration,
    ViewerUnavailable,
    ControllerBusy,
    NotController,
    SequenceMismatch,
    HostRejected,
    HostClearAlternateScreen,
    HostInputInvalid,
    HostInputLimitExceeded,
    HostInputBackpressure,
    HostInputUnavailable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalControlOutcome {
    Acquired,
    Cleared {
        generation: NonZeroU64,
        revision: NonZeroU64,
        cleanup_confirmed: bool,
    },
    ClearOutcomeUnknown,
    /// The Host applied the reset; the fields match `Cleared`.
    ResetAccepted {
        generation: NonZeroU64,
        revision: NonZeroU64,
        cleanup_confirmed: bool,
    },
    ResetOutcomeUnknown,
    Released,
    InputAccepted,
    Resized(TerminalPixelGeometry),
    Pasted {
        accepted_bytes: PasteAcceptedBytes,
    },
    HostColorsAccepted,
    Rejected(ControlRejection),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalControlResult {
    pub request_id: OperationId,
    pub viewer_id: OperationId,
    pub controller_id: OperationId,
    pub outcome: TerminalControlOutcome,
}
