use crate::{HostEpoch, HostTerminalId, ProtocolError, RequestId, SiteId, TerminalId, WorkerId};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::num::NonZeroU64;

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
/// envelope. It mirrors the terminal-protocol Host input bound
/// `MAXIMUM_INPUT_BYTES` (1024 * 1024) and deliberately does not equal any
/// serialized record length.
pub const MAXIMUM_SEND_ACCEPTED_BYTES: u32 = 1024 * 1024;
/// Bound on the summed Host-encoded byte count a sequence outcome reports:
/// one per-record bound per admitted record.
pub const MAXIMUM_SEND_ACCEPTED_ENCODED_BYTES: u64 =
    (MAXIMUM_SEND_RECORDS as u64) * (MAXIMUM_SEND_ACCEPTED_BYTES as u64);

/// Validated non-empty UTF-8 text for one `text` record. The type alone does
/// not reserve room for the folded carriage return; the record-level bound
/// is enforced by [`SendTextInput`].
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

/// The closed single-press semantic key set a `key` record may name, encoded
/// as the documented numeric codes 1 through 15. `CtrlC`/`CtrlD` are the
/// legacy spellings for a Control-modified letter press.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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

/// The modifier subset a `combo` record presses beside its base key. At
/// least one modifier must stand; a modifier-only value is rejected.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SendModifiers {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
}
impl SendModifiers {
    pub fn new(control: bool, alt: bool, shift: bool) -> Result<Self, ProtocolError> {
        if !(control || alt || shift) {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self {
            control,
            alt,
            shift,
        })
    }
}

/// The closed base vocabulary a `combo` record accepts: every named semantic
/// key except the already-modified `CtrlC`/`CtrlD`, plus the physical codes
/// the terminal protocol actually owns — Insert, F1 through F12, Space, and
/// ASCII letters and digits under their exact `KeyX`/`DigitN` identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendComboBase {
    Named(SendKey),
    Insert,
    /// Function key number, 1 through 12 inclusive.
    Function(u8),
    Space,
    /// ASCII lowercase letter `a` through `z`.
    Letter(u8),
    /// ASCII digit `0` through `9`.
    Digit(u8),
}
impl SendComboBase {
    pub fn new_named(key: SendKey) -> Result<Self, ProtocolError> {
        match key {
            SendKey::CtrlC | SendKey::CtrlD => Err(ProtocolError::InvalidPayload),
            key => Ok(Self::Named(key)),
        }
    }
    pub fn new_function(number: u8) -> Result<Self, ProtocolError> {
        if !(1..=12).contains(&number) {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self::Function(number))
    }
    pub fn new_letter(letter: u8) -> Result<Self, ProtocolError> {
        if !letter.is_ascii_lowercase() {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self::Letter(letter))
    }
    pub fn new_digit(digit: u8) -> Result<Self, ProtocolError> {
        if !digit.is_ascii_digit() {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self::Digit(digit))
    }
}

/// Exactly one Host input record worth of content: bounded UTF-8 text with
/// an optional folded carriage return, one semantic key press, one explicit
/// modifier combination, one opaque byte payload, or one opaque paste whose
/// bracket encoding the Host owns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SendRecord {
    Text(SendTextInput),
    Key {
        key: SendKey,
    },
    Combo {
        modifiers: SendModifiers,
        base: SendComboBase,
    },
    Bytes {
        bytes: SendBytes,
    },
    Paste {
        content: SendPasteContent,
    },
}
impl SendRecord {
    /// The decoded payload bytes this record contributes to the aggregate
    /// bound: literal content plus any folded carriage return. Semantic key
    /// records carry no decoded bytes — their encoding is a Host fact.
    #[must_use]
    pub fn decoded_bytes(&self) -> usize {
        match self {
            Self::Text(input) => input.record_bytes(),
            Self::Key { .. } | Self::Combo { .. } => 0,
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
    pub(crate) fn validate(&self) -> Result<(), ProtocolError> {
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
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireSendProgress {
    records_accepted: u64,
    first_stopped_record: u64,
    accepted_encoded_bytes: u64,
}
impl Serialize for SendProgress {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        WireSendProgress {
            records_accepted: u64::from(self.records_accepted),
            first_stopped_record: u64::from(self.first_stopped_record),
            accepted_encoded_bytes: self.accepted_encoded_bytes,
        }
        .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for SendProgress {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = WireSendProgress::deserialize(deserializer)?;
        let field = |value: u64| u32::try_from(value).map_err(de::Error::custom);
        Self::new(
            field(wire.records_accepted)?,
            field(wire.first_stopped_record)?,
            wire.accepted_encoded_bytes,
        )
        .map_err(de::Error::custom)
    }
}

/// Whether the temporary input-only Host attachment was provably released.
/// `Unconfirmed` means the attachment may persist until carrier teardown or
/// Host-epoch end reclaims it; it is never a claim of release.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SendCleanupStatus {
    NotNeeded,
    Confirmed,
    Unconfirmed,
}

/// Closed typed-refusal vocabulary for a send; every variant means the
/// stopped record provably did not reach the Host queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SendRejectedReason {
    TerminalUnavailable,
    StaleWorkerGeneration,
    StaleHostEpoch,
    HostRejected,
    HostInputInvalid,
    HostInputLimitExceeded,
    HostInputBackpressure,
    HostInputUnavailable,
}

/// Closed not-submitted vocabulary: the stopped record provably never
/// reached a Host write and no typed refusal applies. `HostUnavailable` and
/// `HostProtocolFault` are transport-level facts distinct from service-side
/// unavailability; `InputBudgetExhausted` is the Worker's own execution
/// deadline stopping the sequence between records — the confirmed prefix
/// stands and the stopped record provably never left the Worker.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SendUnavailableReason {
    ServerBusy,
    WorkerUnavailable,
    TerminalRuntimeUnavailable,
    HostUnavailable,
    HostProtocolFault,
    InputBudgetExhausted,
}

/// The exact physical incarnation the Server admitted for this send. It is
/// present only when the request was resolved to a current authorized binding;
/// pre-binding rejections carry no target rather than an invented one.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendTarget {
    #[serde(with = "crate::number::positive_decimal")]
    worker_generation: NonZeroU64,
    host_epoch: HostEpoch,
    host_terminal_id: HostTerminalId,
}
impl SendTarget {
    #[must_use]
    pub const fn new(
        worker_generation: NonZeroU64,
        host_epoch: HostEpoch,
        host_terminal_id: HostTerminalId,
    ) -> Self {
        Self {
            worker_generation,
            host_epoch,
            host_terminal_id,
        }
    }
    #[must_use]
    pub const fn worker_generation(&self) -> NonZeroU64 {
        self.worker_generation
    }
    #[must_use]
    pub const fn host_epoch(&self) -> &HostEpoch {
        &self.host_epoch
    }
    #[must_use]
    pub const fn host_terminal_id(&self) -> &HostTerminalId {
        &self.host_terminal_id
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TerminalSendOutcome {
    Accepted {
        progress: SendProgress,
        cleanup: SendCleanupStatus,
    },
    Rejected {
        reason: SendRejectedReason,
        progress: SendProgress,
        cleanup: SendCleanupStatus,
    },
    Unavailable {
        reason: SendUnavailableReason,
        progress: SendProgress,
        cleanup: SendCleanupStatus,
    },
    OutcomeUnknown {
        progress: SendProgress,
        cleanup: SendCleanupStatus,
    },
}
impl TerminalSendOutcome {
    pub(crate) fn validate(&self) -> Result<(), ProtocolError> {
        let progress = self.progress();
        progress.validate()?;
        match self {
            Self::Accepted { cleanup, .. } => {
                if *cleanup == SendCleanupStatus::NotNeeded
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
            _ => {}
        }
        if progress.records_accepted() > 0 && *self.cleanup() == SendCleanupStatus::NotNeeded {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(())
    }
    #[must_use]
    pub const fn progress(&self) -> &SendProgress {
        match self {
            Self::Accepted { progress, .. }
            | Self::Rejected { progress, .. }
            | Self::Unavailable { progress, .. }
            | Self::OutcomeUnknown { progress, .. } => progress,
        }
    }
    #[must_use]
    pub const fn cleanup(&self) -> &SendCleanupStatus {
        match self {
            Self::Accepted { cleanup, .. }
            | Self::Rejected { cleanup, .. }
            | Self::Unavailable { cleanup, .. }
            | Self::OutcomeUnknown { cleanup, .. } => cleanup,
        }
    }
    /// Whether the outcome proves the send was bound to a physical target:
    /// every `Accepted` or `OutcomeUnknown`, and any cleanup state other than
    /// `NotNeeded`, requires the result to record the admitted incarnation.
    pub(crate) const fn requires_target(&self) -> bool {
        match self {
            Self::Accepted { .. } | Self::OutcomeUnknown { .. } => true,
            Self::Rejected { cleanup, .. } | Self::Unavailable { cleanup, .. } => {
                !matches!(cleanup, SendCleanupStatus::NotNeeded)
            }
        }
    }
}

/// One admitted send: the resolved binding addresses plus the ordered record
/// sequence. The sequence is never atomic across records or across other
/// Clients' input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSendRequest {
    pub(crate) request_id: RequestId,
    pub(crate) site_id: SiteId,
    pub(crate) worker_id: WorkerId,
    pub(crate) terminal_id: TerminalId,
    pub(crate) records: SendRecords,
}
impl TerminalSendRequest {
    #[must_use]
    pub const fn new(
        request_id: RequestId,
        site_id: SiteId,
        worker_id: WorkerId,
        terminal_id: TerminalId,
        records: SendRecords,
    ) -> Self {
        Self {
            request_id,
            site_id,
            worker_id,
            terminal_id,
            records,
        }
    }
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }
    #[must_use]
    pub const fn site_id(&self) -> &SiteId {
        &self.site_id
    }
    #[must_use]
    pub const fn worker_id(&self) -> &WorkerId {
        &self.worker_id
    }
    #[must_use]
    pub const fn terminal_id(&self) -> &TerminalId {
        &self.terminal_id
    }
    #[must_use]
    pub const fn records(&self) -> &SendRecords {
        &self.records
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSendResult {
    pub(crate) request_id: RequestId,
    pub(crate) target: Option<SendTarget>,
    pub(crate) outcome: TerminalSendOutcome,
}
impl TerminalSendResult {
    /// `target` is the Server-resolved physical incarnation. `None` is only
    /// legal for a `Rejected`/`Unavailable` outcome with `NotNeeded` cleanup —
    /// the pre-admission refusals decided before any binding was resolved.
    /// Every `Accepted`, `OutcomeUnknown`, or `Confirmed`/`Unconfirmed` result
    /// must carry the actual admitted target.
    pub fn new(
        request_id: RequestId,
        target: Option<SendTarget>,
        outcome: TerminalSendOutcome,
    ) -> Result<Self, ProtocolError> {
        let result = Self {
            request_id,
            target,
            outcome,
        };
        result.validate()?;
        Ok(result)
    }
    /// The full result invariant: outcome axes plus the target-presence rule
    /// every construction and wire boundary must enforce identically.
    pub(crate) fn validate(&self) -> Result<(), ProtocolError> {
        self.outcome.validate()?;
        if self.target.is_none() && self.outcome.requires_target() {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(())
    }
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }
    #[must_use]
    pub const fn target(&self) -> Option<&SendTarget> {
        self.target.as_ref()
    }
    #[must_use]
    pub const fn outcome(&self) -> &TerminalSendOutcome {
        &self.outcome
    }
    #[must_use]
    pub fn into_outcome(self) -> TerminalSendOutcome {
        self.outcome
    }
}

#[cfg(test)]
mod tests;
