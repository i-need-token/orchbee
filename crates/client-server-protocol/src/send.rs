use crate::{HostEpoch, HostTerminalId, ProtocolError, RequestId, SiteId, TerminalId, WorkerId};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::num::NonZeroU64;

/// Authoritative UTF-8 byte bound for one send payload: the declared text and
/// the optional folded carriage return together must fit within it.
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
impl Serialize for SendText {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for SendText {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// One `text` payload: bounded UTF-8 text plus the folded carriage-return flag.
/// The combined byte bound is enforced by construction, so no value of this
/// type can exceed `MAXIMUM_SEND_TEXT_BYTES`.
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

/// The closed single-press semantic key set a `key` payload may name.
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
#[derive(Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum WireSendPayload {
    Text { text: SendText, enter: bool },
    Key { key: SendKey },
}
impl Serialize for TerminalSendPayload {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Text(input) => WireSendPayload::Text {
                text: input.text().clone(),
                enter: input.enter(),
            },
            Self::Key { key } => WireSendPayload::Key { key: *key },
        }
        .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for TerminalSendPayload {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match WireSendPayload::deserialize(deserializer)? {
            WireSendPayload::Text { text, enter } => {
                Self::new_text(text, enter).map_err(de::Error::custom)?
            }
            WireSendPayload::Key { key } => Self::Key { key },
        })
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
impl Serialize for SendAcceptedBytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(u64::from(self.0))
    }
}
impl<'de> Deserialize<'de> for SendAcceptedBytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = u64::deserialize(deserializer)?;
        Self::new(u32::try_from(value).map_err(de::Error::custom)?).map_err(de::Error::custom)
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

/// Closed typed-refusal vocabulary for a send; every variant means the input
/// record provably did not reach the Host queue.
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

/// Closed not-submitted vocabulary: the send never reached a Host write and no
/// typed refusal applies. `HostUnavailable` and `HostProtocolFault` are
/// transport-level facts distinct from service-side unavailability.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SendUnavailableReason {
    ServerBusy,
    WorkerUnavailable,
    TerminalRuntimeUnavailable,
    HostUnavailable,
    HostProtocolFault,
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

/// Every outcome pairs the input status with the orthogonal cleanup axis;
/// `Accepted` says only that the Host queued the record, never that a child
/// process read, executed, or completed it. Combinations that cannot arise
/// from the Attach → input → Detach sequence are rejected at construction and
/// at both codec boundaries: an `Accepted` input implies a real attachment
/// (never `NotNeeded`), and a `Confirmed` cleanup implies a completed exchange
/// (never `OutcomeUnknown`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TerminalSendOutcome {
    Accepted {
        accepted_bytes: SendAcceptedBytes,
        cleanup: SendCleanupStatus,
    },
    Rejected {
        reason: SendRejectedReason,
        cleanup: SendCleanupStatus,
    },
    Unavailable {
        reason: SendUnavailableReason,
        cleanup: SendCleanupStatus,
    },
    OutcomeUnknown {
        cleanup: SendCleanupStatus,
    },
}
impl TerminalSendOutcome {
    pub(crate) fn validate(&self) -> Result<(), ProtocolError> {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSendRequest {
    pub(crate) request_id: RequestId,
    pub(crate) site_id: SiteId,
    pub(crate) worker_id: WorkerId,
    pub(crate) terminal_id: TerminalId,
    pub(crate) payload: TerminalSendPayload,
}
impl TerminalSendRequest {
    #[must_use]
    pub const fn new(
        request_id: RequestId,
        site_id: SiteId,
        worker_id: WorkerId,
        terminal_id: TerminalId,
        payload: TerminalSendPayload,
    ) -> Self {
        Self {
            request_id,
            site_id,
            worker_id,
            terminal_id,
            payload,
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
    pub const fn payload(&self) -> &TerminalSendPayload {
        &self.payload
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSendResult {
    pub(crate) request_id: RequestId,
    pub(crate) target: Option<SendTarget>,
    pub(crate) outcome: TerminalSendOutcome,
}
impl TerminalSendResult {
    /// `target` is the Server-resolved physical incarnation; pass `None` only
    /// for a rejection decided before any binding was admitted.
    pub fn new(
        request_id: RequestId,
        target: Option<SendTarget>,
        outcome: TerminalSendOutcome,
    ) -> Result<Self, ProtocolError> {
        outcome.validate()?;
        Ok(Self {
            request_id,
            target,
            outcome,
        })
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
