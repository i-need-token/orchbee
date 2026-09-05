use crate::{ControllerId, Geometry, ProtocolError, RequestId};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::num::NonZeroU64;

pub const MAXIMUM_CONTROL_INPUT_BYTES: usize = 4096;

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
impl Serialize for ControlInput {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use std::fmt::Write;
        let mut hex = String::with_capacity(self.0.len() * 2);
        for byte in &self.0 {
            write!(hex, "{byte:02x}").map_err(serde::ser::Error::custom)?;
        }
        serializer.serialize_str(&hex)
    }
}
impl<'de> Deserialize<'de> for ControlInput {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let hex = String::deserialize(deserializer)?;
        if hex.is_empty()
            || hex.len() > MAXIMUM_CONTROL_INPUT_BYTES * 2
            || !hex.len().is_multiple_of(2)
            || !hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(de::Error::custom("expected bounded lowercase hex input"));
        }
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(de::Error::custom)?;
        Self::new(bytes).map_err(de::Error::custom)
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TerminalControlAction {
    Acquire {},
    Input {
        controller_id: ControllerId,
        #[serde(with = "crate::number::positive_decimal")]
        sequence: NonZeroU64,
        bytes: ControlInput,
    },
    Resize {
        controller_id: ControllerId,
        #[serde(with = "crate::number::positive_decimal")]
        sequence: NonZeroU64,
        geometry: Geometry,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControlRejectedReason {
    ViewerUnavailable,
    ControllerBusy,
    NotController,
    SequenceMismatch,
    HostRejected,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControlUnavailableReason {
    ServerBusy,
    TerminalRuntimeUnavailable,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TerminalControlOutcome {
    Acquired { controller_id: ControllerId },
    InputAccepted {},
    Resized { geometry: Geometry },
    Rejected { reason: ControlRejectedReason },
    Unavailable { reason: ControlUnavailableReason },
    OutcomeUnknown {},
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalControlRequest {
    pub(crate) request_id: RequestId,
    pub(crate) action: TerminalControlAction,
}
impl TerminalControlRequest {
    #[must_use]
    pub const fn new(request_id: RequestId, action: TerminalControlAction) -> Self {
        Self { request_id, action }
    }
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }
    #[must_use]
    pub const fn action(&self) -> &TerminalControlAction {
        &self.action
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalControlResult {
    pub(crate) request_id: RequestId,
    pub(crate) outcome: TerminalControlOutcome,
}
impl TerminalControlResult {
    #[must_use]
    pub const fn new(request_id: RequestId, outcome: TerminalControlOutcome) -> Self {
        Self {
            request_id,
            outcome,
        }
    }
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }
    #[must_use]
    pub const fn outcome(&self) -> &TerminalControlOutcome {
        &self.outcome
    }
    #[must_use]
    pub fn into_outcome(self) -> TerminalControlOutcome {
        self.outcome
    }
}
