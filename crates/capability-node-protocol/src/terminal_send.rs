use crate::{HostTerminalIdentity, OperationId, ProtocolError, WorkerGeneration, WorkerId};

/// Authoritative byte bound for one send input record: the declared UTF-8 text
/// and the optional folded carriage return together must fit within it.
pub const MAXIMUM_SEND_TEXT_BYTES: usize = 4096;
/// Protocol bound for the Host-encoded input-queue byte count a send result
/// may echo; zero is legal and the bound only guards the envelope.
pub const MAXIMUM_SEND_ACCEPTED_BYTES: u32 = 1024 * 1024;

/// Validated non-empty UTF-8 text for one `text` send payload. The type alone
/// does not reserve room for the folded carriage return; the payload-level
/// bound is enforced by [`SendTextInput`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SendText(String);
impl SendText {
    pub fn new(text: String) -> Result<Self, ProtocolError> {
        if text.is_empty() || text.len() > MAXIMUM_SEND_TEXT_BYTES {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self(text))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One `text` payload: bounded UTF-8 text plus the folded carriage-return
/// flag. The combined byte bound is enforced by construction, so no value of
/// this type can exceed `MAXIMUM_SEND_TEXT_BYTES`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SendTextInput {
    text: SendText,
    enter: bool,
}
impl SendTextInput {
    pub fn new(text: SendText, enter: bool) -> Result<Self, ProtocolError> {
        if text.as_str().len() + usize::from(enter) > MAXIMUM_SEND_TEXT_BYTES {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self { text, enter })
    }
    #[must_use]
    pub const fn text(&self) -> &SendText {
        &self.text
    }
    #[must_use]
    pub const fn enter(&self) -> bool {
        self.enter
    }
    /// Encoded byte length of the single input record this payload produces.
    #[must_use]
    pub fn record_bytes(&self) -> usize {
        self.text.as_str().len() + usize::from(self.enter)
    }
}

/// The closed single-press semantic key set a `key` payload may name, encoded
/// as the documented numeric codes 1 through 15.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendKey {
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    CtrlC,
    CtrlD,
}
impl SendKey {
    pub(crate) fn from_code(code: u8) -> Result<Self, ProtocolError> {
        Ok(match code {
            1 => Self::Enter,
            2 => Self::Escape,
            3 => Self::Tab,
            4 => Self::Backspace,
            5 => Self::Delete,
            6 => Self::Up,
            7 => Self::Down,
            8 => Self::Left,
            9 => Self::Right,
            10 => Self::Home,
            11 => Self::End,
            12 => Self::PageUp,
            13 => Self::PageDown,
            14 => Self::CtrlC,
            15 => Self::CtrlD,
            _ => return Err(ProtocolError::InvalidPayload),
        })
    }
    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::Enter => 1,
            Self::Escape => 2,
            Self::Tab => 3,
            Self::Backspace => 4,
            Self::Delete => 5,
            Self::Up => 6,
            Self::Down => 7,
            Self::Left => 8,
            Self::Right => 9,
            Self::Home => 10,
            Self::End => 11,
            Self::PageUp => 12,
            Self::PageDown => 13,
            Self::CtrlC => 14,
            Self::CtrlD => 15,
        }
    }
}

/// Exactly one Host input record worth of content: bounded UTF-8 text with an
/// optional folded carriage return, or one semantic key press.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalSendPayload {
    Text(SendTextInput),
    Key { key: SendKey },
}
impl TerminalSendPayload {
    pub fn new_text(text: SendText, enter: bool) -> Result<Self, ProtocolError> {
        SendTextInput::new(text, enter).map(Self::Text)
    }
    #[must_use]
    pub const fn new_key(key: SendKey) -> Self {
        Self::Key { key }
    }
}

/// Encoded byte count the Host admitted into its bounded input queue for one
/// send; zero is legal because a terminal mode may ignore a semantic key.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SendAcceptedBytes(u32);
impl SendAcceptedBytes {
    pub const MAXIMUM: u32 = MAXIMUM_SEND_ACCEPTED_BYTES;

    pub fn new(value: u32) -> Result<Self, ProtocolError> {
        if value > Self::MAXIMUM {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Whether the temporary input-only Host attachment was provably released.
/// `Unconfirmed` means the attachment may persist until carrier teardown or
/// Host-epoch end reclaims it; it is never a claim of release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendCleanupStatus {
    NotNeeded,
    Confirmed,
    Unconfirmed,
}
impl SendCleanupStatus {
    pub(crate) fn from_code(code: u8) -> Result<Self, ProtocolError> {
        Ok(match code {
            1 => Self::NotNeeded,
            2 => Self::Confirmed,
            3 => Self::Unconfirmed,
            _ => return Err(ProtocolError::InvalidPayload),
        })
    }
    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::NotNeeded => 1,
            Self::Confirmed => 2,
            Self::Unconfirmed => 3,
        }
    }
}

/// Closed typed-refusal vocabulary a Worker may report for a send; every
/// variant means the input record provably did not reach the Host queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendRejection {
    WrongWorker,
    StaleWorkerGeneration,
    StaleHostEpoch,
    TerminalUnavailable,
    HostRejected,
    HostInputInvalid,
    HostInputLimitExceeded,
    HostInputBackpressure,
    HostInputUnavailable,
}
impl SendRejection {
    pub(crate) fn from_code(code: u8) -> Result<Self, ProtocolError> {
        Ok(match code {
            1 => Self::WrongWorker,
            2 => Self::StaleWorkerGeneration,
            3 => Self::StaleHostEpoch,
            4 => Self::TerminalUnavailable,
            5 => Self::HostRejected,
            6 => Self::HostInputInvalid,
            7 => Self::HostInputLimitExceeded,
            8 => Self::HostInputBackpressure,
            9 => Self::HostInputUnavailable,
            _ => return Err(ProtocolError::InvalidPayload),
        })
    }
    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::WrongWorker => 1,
            Self::StaleWorkerGeneration => 2,
            Self::StaleHostEpoch => 3,
            Self::TerminalUnavailable => 4,
            Self::HostRejected => 5,
            Self::HostInputInvalid => 6,
            Self::HostInputLimitExceeded => 7,
            Self::HostInputBackpressure => 8,
            Self::HostInputUnavailable => 9,
        }
    }
}

/// Closed not-submitted vocabulary a Worker may report: the send never reached
/// a Host write and no typed refusal applies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendUnavailable {
    HostUnavailable,
    HostProtocolFault,
}
impl SendUnavailable {
    pub(crate) fn from_code(code: u8) -> Result<Self, ProtocolError> {
        Ok(match code {
            1 => Self::HostUnavailable,
            2 => Self::HostProtocolFault,
            _ => return Err(ProtocolError::InvalidPayload),
        })
    }
    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::HostUnavailable => 1,
            Self::HostProtocolFault => 2,
        }
    }
}

/// Every outcome pairs the input status with the orthogonal cleanup axis;
/// `Accepted` says only that the Host queued the record, never that a child
/// process read, executed, or completed it. Combinations that cannot arise
/// from the Attach → input → Detach sequence are rejected at construction and
/// at both codec boundaries: an `Accepted` input implies a real attachment
/// (never `NotNeeded`), and a `Confirmed` cleanup implies a completed exchange
/// (never `OutcomeUnknown`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalSendOutcome {
    Unavailable {
        reason: SendUnavailable,
        cleanup: SendCleanupStatus,
    },
    Rejected {
        reason: SendRejection,
        cleanup: SendCleanupStatus,
    },
    Accepted {
        accepted_bytes: SendAcceptedBytes,
        cleanup: SendCleanupStatus,
    },
    OutcomeUnknown {
        cleanup: SendCleanupStatus,
    },
}
impl TerminalSendOutcome {
    pub(crate) fn validate(self) -> Result<(), ProtocolError> {
        match self {
            Self::Accepted {
                cleanup: SendCleanupStatus::NotNeeded,
                ..
            }
            | Self::OutcomeUnknown {
                cleanup: SendCleanupStatus::Confirmed,
            } => Err(ProtocolError::InvalidPayload),
            _ => Ok(()),
        }
    }
}

/// Server-authorized one-shot input delivery. `target` pins the exact
/// physical Host incarnation selected at admission; `attachment_id` is the
/// Server-assigned temporary input-only attachment identity the Worker must
/// use verbatim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSendRequest {
    pub request_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub target: HostTerminalIdentity,
    pub attachment_id: OperationId,
    pub payload: TerminalSendPayload,
}

/// Worker report for one send request. Construction validates the outcome
/// axes so impossible status/cleanup pairs cannot be encoded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSendResult {
    request_id: OperationId,
    outcome: TerminalSendOutcome,
}
impl TerminalSendResult {
    pub fn new(
        request_id: OperationId,
        outcome: TerminalSendOutcome,
    ) -> Result<Self, ProtocolError> {
        outcome.validate()?;
        Ok(Self {
            request_id,
            outcome,
        })
    }
    #[must_use]
    pub const fn request_id(&self) -> OperationId {
        self.request_id
    }
    #[must_use]
    pub const fn outcome(&self) -> TerminalSendOutcome {
        self.outcome
    }
}
