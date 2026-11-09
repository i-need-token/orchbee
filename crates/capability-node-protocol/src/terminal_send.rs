//! Defines physical-target-fenced send sequences and validates confirmed prefix progress
//! against cleanup and failure outcomes. It carries queue-admission facts without executing
//! input, guaranteeing atomicity, or authorizing replay after a partial or unknown result.

use crate::{HostTerminalIdentity, OperationId, ProtocolError, WorkerGeneration, WorkerId};

/// One admitted send request carries an ordered sequence of input records.
pub const MAXIMUM_SEND_RECORDS: usize = 64;
/// Authoritative byte bound for one `text` record: the declared UTF-8 text
/// and the optional folded carriage return together must fit within it.
pub const MAXIMUM_SEND_TEXT_BYTES: usize = 4096;
/// Authoritative byte bound for one `bytes` record's opaque payload.
pub const MAXIMUM_SEND_BYTES_BYTES: usize = 1024 * 1024;
/// Authoritative byte bound for one `paste` record's opaque content; the
/// Host adds its own bracket markers on top of this bound.
pub const MAXIMUM_SEND_PASTE_BYTES: usize = 256 * 1024;
/// Aggregate decoded payload bound for the whole sequence: the sum of every
/// record's decoded bytes (text plus folded carriage return, bytes payload,
/// paste content; semantic keys carry no decoded bytes) must fit within it.
/// This is a decoded-content bound, never a Host-encoded byte count.
pub const MAXIMUM_SEND_AGGREGATE_BYTES: usize = 1024 * 1024;
/// Protocol bound for the Host-encoded input-queue byte count a send result
/// may echo for one record; zero is legal and the bound only guards the
/// envelope.
pub const MAXIMUM_SEND_ACCEPTED_BYTES: u32 = 1024 * 1024;
/// Bound on the summed Host-encoded byte count a sequence outcome reports:
/// one per-record bound per admitted record.
pub const MAXIMUM_SEND_ACCEPTED_ENCODED_BYTES: u64 =
    (MAXIMUM_SEND_RECORDS as u64) * (MAXIMUM_SEND_ACCEPTED_BYTES as u64);

/// Validated non-empty UTF-8 text for one `text` record payload. The type
/// alone does not reserve room for the folded carriage return; the
/// record-level bound is enforced by [`SendTextInput`].
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

/// One `text` record payload: bounded UTF-8 text plus the folded
/// carriage-return flag. The combined byte bound is enforced by
/// construction, so no value of this type can exceed
/// `MAXIMUM_SEND_TEXT_BYTES`.
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
    /// Decoded byte length of the input record this payload produces: the
    /// literal text plus the folded carriage return.
    #[must_use]
    pub fn record_bytes(&self) -> usize {
        self.text.as_str().len() + usize::from(self.enter)
    }
}

/// Opaque non-empty bytes for one `bytes` record: literal input, never
/// bracketed or otherwise reframed by the Worker or Host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SendBytes(Vec<u8>);
impl SendBytes {
    pub fn new(bytes: Vec<u8>) -> Result<Self, ProtocolError> {
        if bytes.is_empty() || bytes.len() > MAXIMUM_SEND_BYTES_BYTES {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self(bytes))
    }
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Opaque non-empty content for one `paste` record. The Host encodes it
/// under the terminal's current paste mode — markers included — so the
/// encoded size is a Host fact this layer cannot predict.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SendPasteContent(Vec<u8>);
impl SendPasteContent {
    pub fn new(content: Vec<u8>) -> Result<Self, ProtocolError> {
        if content.is_empty() || content.len() > MAXIMUM_SEND_PASTE_BYTES {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self(content))
    }
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Named non-character identities carried by a semantic stroke (codes 1–13).
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
        }
    }
}

/// Exactly one Host input record worth of content: bounded UTF-8 text with
/// an optional folded carriage return, one semantic stroke,
/// one opaque byte payload, or one opaque paste whose
/// bracket encoding the Host owns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SendRecord {
    Stroke(crate::SendKeyStroke),
    Text(SendTextInput),
    Bytes { bytes: SendBytes },
    Paste { content: SendPasteContent },
}
impl SendRecord {
    /// The decoded payload bytes this record contributes to the aggregate
    /// bound: literal content plus any folded carriage return. Semantic key
    /// records carry no decoded bytes — their encoding is a Host fact.
    #[must_use]
    pub fn decoded_bytes(&self) -> usize {
        match self {
            Self::Text(input) => input.record_bytes(),
            Self::Stroke(_) => 0,
            Self::Bytes { bytes } => bytes.as_bytes().len(),
            Self::Paste { content } => content.as_bytes().len(),
        }
    }
}

/// The ordered record sequence one send request submits: 1 through 64
/// records whose aggregate decoded payload never exceeds
/// `MAXIMUM_SEND_AGGREGATE_BYTES`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SendRecords(Vec<SendRecord>);
impl SendRecords {
    pub fn new(records: Vec<SendRecord>) -> Result<Self, ProtocolError> {
        if records.is_empty() || records.len() > MAXIMUM_SEND_RECORDS {
            return Err(ProtocolError::InvalidPayload);
        }
        let aggregate = records.iter().try_fold(0usize, |total, record| {
            total.checked_add(record.decoded_bytes())
        });
        if aggregate.is_none_or(|total| total > MAXIMUM_SEND_AGGREGATE_BYTES) {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self(records))
    }
    #[must_use]
    pub fn as_slice(&self) -> &[SendRecord] {
        &self.0
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        false
    }
    pub fn iter(&self) -> impl Iterator<Item = &SendRecord> {
        self.0.iter()
    }
    /// The one prompt line these records form, when they are exactly a paste of one line of
    /// UTF-8 with no C0 control or DEL, an optional unmodified Right, and an unmodified Enter.
    #[must_use]
    pub fn prompt_line(&self) -> Option<SendPromptLine<'_>> {
        let (content, keys) = match self.0.as_slice() {
            [SendRecord::Paste { content }, keys @ ..] => (content, keys),
            _ => return None,
        };
        let right_arrow = match keys {
            [enter] => {
                plain_key(enter, SendKey::Enter)?;
                false
            }
            [right, enter] => {
                plain_key(right, SendKey::Right)?;
                plain_key(enter, SendKey::Enter)?;
                true
            }
            _ => return None,
        };
        let content = std::str::from_utf8(content.as_bytes()).ok()?;
        if content.bytes().any(|byte| byte < 0x20 || byte == 0x7f) {
            return None;
        }
        Some(SendPromptLine {
            content,
            right_arrow,
        })
    }
}

fn plain_key(record: &SendRecord, key: SendKey) -> Option<()> {
    match record {
        SendRecord::Stroke(stroke)
            if stroke.key() == crate::SendStrokeKey::Named(key) && stroke.modifiers() == 0 =>
        {
            Some(())
        }
        _ => None,
    }
}

/// The single shape an idle-prompt send admits: one line the Host types as a paste, then Right
/// when `right_arrow` is set, then Enter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SendPromptLine<'a> {
    pub content: &'a str,
    pub right_arrow: bool,
}

/// What must hold before any record reaches the Host.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SendPrecondition {
    /// None: every record is its own Host exchange, in order.
    #[default]
    None,
    /// The records are one prompt line ([`SendRecords::prompt_line`]) that the Host types in a
    /// single exchange, and only while the terminal's root shell waits at a confirmed fresh,
    /// empty prompt. A Host that does not offer the gate refuses it; it never falls back.
    IdlePrompt,
}

/// Encoded byte count the Host admitted into its bounded input queue for one
/// record; zero is legal because a terminal mode may ignore a semantic key.
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

/// The delivery cursor a sequence outcome reports: how many records were
/// confirmed accepted by a correlated Host acknowledgement, the zero-based
/// index of the first record that was not confirmed accepted, and the sum
/// of Host-encoded bytes those acknowledgements carried. The stopped record
/// is the frontier: an `outcome-unknown` stop may already have admitted that
/// one record, while a determinate refusal or unavailability means it
/// provably did not land. A fully accepted sequence reports
/// `first_stopped_record` equal to the record count — no record stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SendProgress {
    records_accepted: u32,
    first_stopped_record: u32,
    accepted_encoded_bytes: u64,
}
impl SendProgress {
    pub fn new(
        records_accepted: u32,
        first_stopped_record: u32,
        accepted_encoded_bytes: u64,
    ) -> Result<Self, ProtocolError> {
        let progress = Self {
            records_accepted,
            first_stopped_record,
            accepted_encoded_bytes,
        };
        progress.validate()?;
        Ok(progress)
    }
    fn validate(&self) -> Result<(), ProtocolError> {
        if self.records_accepted > MAXIMUM_SEND_RECORDS as u32
            || self.first_stopped_record > MAXIMUM_SEND_RECORDS as u32
            || self.records_accepted > self.first_stopped_record
            || self.accepted_encoded_bytes > MAXIMUM_SEND_ACCEPTED_ENCODED_BYTES
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(())
    }
    /// Records confirmed accepted by correlated Host acknowledgements.
    #[must_use]
    pub const fn records_accepted(self) -> u32 {
        self.records_accepted
    }
    /// Zero-based index of the first record not confirmed accepted; equal to
    /// the request's record count when every record was accepted.
    #[must_use]
    pub const fn first_stopped_record(self) -> u32 {
        self.first_stopped_record
    }
    /// Summed Host-encoded bytes the accepted records reported. This is the
    /// input-queue byte count the Host admitted, never the source payload
    /// size — paste markers and semantic-key encodings count toward it.
    #[must_use]
    pub const fn accepted_encoded_bytes(self) -> u64 {
        self.accepted_encoded_bytes
    }
    /// The all-records-accepted progress: count confirmed, none stopped.
    pub fn complete(records: usize, accepted_encoded_bytes: u64) -> Self {
        Self {
            records_accepted: records as u32,
            first_stopped_record: records as u32,
            accepted_encoded_bytes,
        }
    }
    /// A stop at `index`: every earlier record confirmed, `index` not
    /// confirmed, and nothing later attempted.
    pub fn stopped(index: usize, accepted_encoded_bytes: u64) -> Self {
        Self {
            records_accepted: index as u32,
            first_stopped_record: index as u32,
            accepted_encoded_bytes,
        }
    }
    /// Nothing reached the Host: zero accepted, stopped at record zero.
    pub fn none() -> Self {
        Self::stopped(0, 0)
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
/// variant means the stopped record provably did not reach the Host queue.
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
    /// The Host could not confirm an idle prompt and typed nothing.
    PromptUnconfirmed(SendPromptState),
    /// The terminal's Host did not negotiate `prompt-gated-input`, so an idle-prompt send was
    /// refused before any attachment.
    HostPromptGateUnsupported,
}
impl SendRejection {
    /// The wire code, followed by the prompt state's code for `PromptUnconfirmed`.
    pub(crate) fn decode(
        code: u8,
        state: impl FnOnce() -> Result<u8, ProtocolError>,
    ) -> Result<Self, ProtocolError> {
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
            10 => Self::PromptUnconfirmed(SendPromptState::from_code(state()?)?),
            11 => Self::HostPromptGateUnsupported,
            _ => return Err(ProtocolError::InvalidPayload),
        })
    }
    pub(crate) fn encode(self, bytes: &mut Vec<u8>) {
        bytes.push(match self {
            Self::WrongWorker => 1,
            Self::StaleWorkerGeneration => 2,
            Self::StaleHostEpoch => 3,
            Self::TerminalUnavailable => 4,
            Self::HostRejected => 5,
            Self::HostInputInvalid => 6,
            Self::HostInputLimitExceeded => 7,
            Self::HostInputBackpressure => 8,
            Self::HostInputUnavailable => 9,
            Self::PromptUnconfirmed(_) => 10,
            Self::HostPromptGateUnsupported => 11,
        });
        if let Self::PromptUnconfirmed(state) = self {
            bytes.push(state.code());
        }
    }
    /// Whether this refusal is decided before the Host types anything, so its progress is zero.
    const fn types_nothing(self) -> bool {
        matches!(
            self,
            Self::PromptUnconfirmed(_) | Self::HostPromptGateUnsupported
        )
    }
}

/// The first rule the Host found unmet when it refused an idle-prompt line, in its check
/// order. Only `Settling` is worth asking again, after the quiet period.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendPromptState {
    NotShell,
    Busy,
    FullScreen,
    Unmarked,
    NotAtPrompt,
    InputPending,
    Changed,
    Settling,
}
impl SendPromptState {
    fn from_code(code: u8) -> Result<Self, ProtocolError> {
        Ok(match code {
            1 => Self::NotShell,
            2 => Self::Busy,
            3 => Self::FullScreen,
            4 => Self::Unmarked,
            5 => Self::NotAtPrompt,
            6 => Self::InputPending,
            7 => Self::Changed,
            8 => Self::Settling,
            _ => return Err(ProtocolError::InvalidPayload),
        })
    }
    const fn code(self) -> u8 {
        match self {
            Self::NotShell => 1,
            Self::Busy => 2,
            Self::FullScreen => 3,
            Self::Unmarked => 4,
            Self::NotAtPrompt => 5,
            Self::InputPending => 6,
            Self::Changed => 7,
            Self::Settling => 8,
        }
    }
}

/// Closed not-submitted vocabulary a Worker may report: the stopped record
/// provably never reached a Host write and no typed refusal applies.
/// `InputBudgetExhausted` is the Worker's own execution deadline stopping
/// the sequence between records — the confirmed prefix stands and the
/// stopped record provably never left the Worker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendUnavailable {
    HostUnavailable,
    HostProtocolFault,
    InputBudgetExhausted,
}
impl SendUnavailable {
    pub(crate) fn from_code(code: u8) -> Result<Self, ProtocolError> {
        Ok(match code {
            1 => Self::HostUnavailable,
            2 => Self::HostProtocolFault,
            3 => Self::InputBudgetExhausted,
            _ => return Err(ProtocolError::InvalidPayload),
        })
    }
    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::HostUnavailable => 1,
            Self::HostProtocolFault => 2,
            Self::InputBudgetExhausted => 3,
        }
    }
}

/// Every outcome pairs the input status with the orthogonal cleanup axis and
/// the exact delivery frontier. `Accepted` says only that the Host queued
/// every record, never that a child process read, executed, or completed
/// one. Combinations that cannot arise from the Attach → records → Detach
/// sequence are rejected at construction and at both codec boundaries: an
/// `Accepted` input implies a real attachment (never `NotNeeded`), a
/// `Confirmed` cleanup implies a completed exchange (never `OutcomeUnknown`),
/// and any confirmed-accepted prefix implies a cleanup attempt ran.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalSendOutcome {
    Unavailable {
        reason: SendUnavailable,
        progress: SendProgress,
        cleanup: SendCleanupStatus,
    },
    Rejected {
        reason: SendRejection,
        progress: SendProgress,
        cleanup: SendCleanupStatus,
    },
    Accepted {
        progress: SendProgress,
        cleanup: SendCleanupStatus,
    },
    OutcomeUnknown {
        progress: SendProgress,
        cleanup: SendCleanupStatus,
    },
}
impl TerminalSendOutcome {
    pub(crate) fn validate(self) -> Result<(), ProtocolError> {
        let progress = self.progress();
        progress.validate()?;
        match self {
            Self::Accepted { cleanup, .. } => {
                if cleanup == SendCleanupStatus::NotNeeded
                    || progress.records_accepted() == 0
                    || progress.records_accepted() != progress.first_stopped_record()
                {
                    return Err(ProtocolError::InvalidPayload);
                }
            }
            Self::OutcomeUnknown {
                cleanup: SendCleanupStatus::Confirmed,
                ..
            } => {
                return Err(ProtocolError::InvalidPayload);
            }
            Self::Rejected { reason, .. } if reason.types_nothing() => {
                if progress != SendProgress::none() {
                    return Err(ProtocolError::InvalidPayload);
                }
            }
            Self::Rejected { .. } | Self::Unavailable { .. } | Self::OutcomeUnknown { .. } => {}
        }
        if progress.records_accepted() > 0 && self.cleanup() == SendCleanupStatus::NotNeeded {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(())
    }
    #[must_use]
    pub const fn progress(self) -> SendProgress {
        match self {
            Self::Unavailable { progress, .. }
            | Self::Rejected { progress, .. }
            | Self::Accepted { progress, .. }
            | Self::OutcomeUnknown { progress, .. } => progress,
        }
    }
    #[must_use]
    pub const fn cleanup(self) -> SendCleanupStatus {
        match self {
            Self::Unavailable { cleanup, .. }
            | Self::Rejected { cleanup, .. }
            | Self::Accepted { cleanup, .. }
            | Self::OutcomeUnknown { cleanup, .. } => cleanup,
        }
    }
}

/// Server-authorized one-shot input delivery. `target` pins the exact
/// physical Host incarnation selected at admission; `attachment_id` is the
/// Server-assigned temporary input-only attachment identity the Worker must
/// use verbatim. `records` is the ordered sequence — one temporary
/// attachment serves the whole ordered exchange, and the sequence is never
/// atomic across records or across other Clients' input, except that an
/// [`SendPrecondition::IdlePrompt`] line is one Host exchange.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSendRequest {
    pub request_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub target: HostTerminalIdentity,
    pub attachment_id: OperationId,
    pub precondition: SendPrecondition,
    pub records: SendRecords,
}
impl TerminalSendRequest {
    /// An idle-prompt precondition admits only one prompt line; both codec boundaries check it.
    pub(crate) fn validate(&self) -> Result<(), ProtocolError> {
        if self.precondition == SendPrecondition::IdlePrompt && self.records.prompt_line().is_none()
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(())
    }
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
