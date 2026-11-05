//! Defines attachment-scoped controller actions, bounded input values, and their admissible
//! result classes, including the exact resize echo. These values carry requested authority;
//! they neither grant a controller nor validate live attachment ownership.

use crate::{ControllerId, Geometry, ProtocolError, RequestId, ViewerId};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::num::NonZeroU64;

pub const MAXIMUM_CONTROL_INPUT_BYTES: usize = 4096;
pub const MAXIMUM_PASTE_CONTENT_BYTES: usize = 256 * 1024;

/// Bounded raw content for one semantic `Paste` action, carried only by the
/// dedicated `OBCQ` record and never by the `OBCS` JSON control envelope.
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

/// Encoded byte count the host admitted into its bounded input queue for one
/// paste operation; always positive and capped by `Self::MAXIMUM`.
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
impl Serialize for PasteAcceptedBytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(u64::from(self.0))
    }
}
impl<'de> Deserialize<'de> for PasteAcceptedBytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = u64::deserialize(deserializer)?;
        Self::new(u32::try_from(value).map_err(de::Error::custom)?).map_err(de::Error::custom)
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
fn serialize_hex<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    use std::fmt::Write;
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(hex, "{byte:02x}").map_err(serde::ser::Error::custom)?;
    }
    serializer.serialize_str(&hex)
}
fn deserialize_hex<'de, D: Deserializer<'de>>(
    deserializer: D,
    maximum_bytes: usize,
) -> Result<Vec<u8>, D::Error> {
    let hex = String::deserialize(deserializer)?;
    if hex.is_empty()
        || hex.len() > maximum_bytes * 2
        || !hex.len().is_multiple_of(2)
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(de::Error::custom("expected bounded lowercase hex input"));
    }
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(de::Error::custom)
}
impl Serialize for ControlInput {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_hex(&self.0, serializer)
    }
}
impl<'de> Deserialize<'de> for ControlInput {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(deserialize_hex(deserializer, MAXIMUM_CONTROL_INPUT_BYTES)?)
            .map_err(de::Error::custom)
    }
}

/// One semantic input record's largest payload; its hexadecimal text fits one OBCS frame.
pub const MAXIMUM_SEMANTIC_INPUT_BYTES: usize = 16 * 1024;

/// The Host semantic input record a `semantic-input` action carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticInputKind {
    Focus,
    Key,
    Mouse,
    Text,
}

/// One encoded Host semantic input payload. Server bounds and relays it unread; the Worker
/// validates it against the Host codec and rebinds it to the controller grant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticInputPayload(Vec<u8>);
impl SemanticInputPayload {
    pub fn new(bytes: Vec<u8>) -> Result<Self, ProtocolError> {
        if bytes.is_empty() || bytes.len() > MAXIMUM_SEMANTIC_INPUT_BYTES {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self(bytes))
    }
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
impl Serialize for SemanticInputPayload {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_hex(&self.0, serializer)
    }
}
impl<'de> Deserialize<'de> for SemanticInputPayload {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(deserialize_hex(deserializer, MAXIMUM_SEMANTIC_INPUT_BYTES)?)
            .map_err(de::Error::custom)
    }
}
/// One 24-bit RGB default color, encoded as a JSON integer from 0 through 16,777,215.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalRgb(u32);
impl TerminalRgb {
    pub const fn new(value: u32) -> Result<Self, ProtocolError> {
        if value > 0x00ff_ffff {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}
impl Serialize for TerminalRgb {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(u64::from(self.0))
    }
}
impl<'de> Deserialize<'de> for TerminalRgb {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = crate::number::Binary64SafeInteger::deserialize(deserializer)?.value();
        Self::new(u32::try_from(value).map_err(de::Error::custom)?).map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalControlAction {
    /// Read the Server's current ownership and recent focus order without acquiring control.
    Observe {
        attachment_id: ViewerId,
    },
    /// Record one foreground activation and the geometry to restore if this viewer is promoted.
    Activate {
        attachment_id: ViewerId,
        geometry: Geometry,
    },
    Acquire {
        attachment_id: ViewerId,
    },
    Takeover {
        attachment_id: ViewerId,
    },
    Clear {
        attachment_id: ViewerId,
    },
    /// A full terminal reset, admitted like `Clear` and independent of the controller.
    Reset {
        attachment_id: ViewerId,
    },
    Input {
        attachment_id: ViewerId,
        controller_id: ControllerId,
        sequence: NonZeroU64,
        bytes: ControlInput,
    },
    /// One Host semantic input record, encoded by the Client under this controller grant.
    SemanticInput {
        attachment_id: ViewerId,
        controller_id: ControllerId,
        sequence: NonZeroU64,
        input_kind: SemanticInputKind,
        payload: SemanticInputPayload,
    },
    Resize {
        attachment_id: ViewerId,
        controller_id: ControllerId,
        sequence: NonZeroU64,
        geometry: Geometry,
    },
    /// Semantic auto-mode paste; only decodable from the `OBCQ` record
    /// encoding, never from `OBCS` JSON.
    Paste {
        attachment_id: ViewerId,
        controller_id: ControllerId,
        sequence: NonZeroU64,
        content: PasteContent,
    },
    /// The controlling Client's effective defaults and known cursor/ANSI colors.
    HostColors {
        attachment_id: ViewerId,
        controller_id: ControllerId,
        sequence: NonZeroU64,
        foreground: TerminalRgb,
        background: TerminalRgb,
        cursor: Option<TerminalRgb>,
        palette: [Option<TerminalRgb>; 16],
    },
}

#[derive(Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum WireControlAction {
    Observe {
        attachment_id: ViewerId,
    },
    Activate {
        attachment_id: ViewerId,
        geometry: Geometry,
    },
    Acquire {
        attachment_id: ViewerId,
    },
    Takeover {
        attachment_id: ViewerId,
    },
    Clear {
        attachment_id: ViewerId,
    },
    Reset {
        attachment_id: ViewerId,
    },
    Input {
        attachment_id: ViewerId,
        controller_id: ControllerId,
        #[serde(with = "crate::number::positive_decimal")]
        sequence: NonZeroU64,
        bytes: ControlInput,
    },
    SemanticInput {
        attachment_id: ViewerId,
        controller_id: ControllerId,
        #[serde(with = "crate::number::positive_decimal")]
        sequence: NonZeroU64,
        input_kind: SemanticInputKind,
        payload: SemanticInputPayload,
    },
    Resize {
        attachment_id: ViewerId,
        controller_id: ControllerId,
        #[serde(with = "crate::number::positive_decimal")]
        sequence: NonZeroU64,
        geometry: Geometry,
    },
    HostColors {
        attachment_id: ViewerId,
        controller_id: ControllerId,
        #[serde(with = "crate::number::positive_decimal")]
        sequence: NonZeroU64,
        foreground: TerminalRgb,
        background: TerminalRgb,
        #[serde(deserialize_with = "Option::deserialize")]
        cursor: Option<TerminalRgb>,
        palette: [Option<TerminalRgb>; 16],
    },
}

impl Serialize for TerminalControlAction {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Observe { attachment_id } => WireControlAction::Observe {
                attachment_id: attachment_id.clone(),
            },
            Self::Activate {
                attachment_id,
                geometry,
            } => WireControlAction::Activate {
                attachment_id: attachment_id.clone(),
                geometry: *geometry,
            },
            Self::Takeover { attachment_id } => WireControlAction::Takeover {
                attachment_id: attachment_id.clone(),
            },
            Self::Clear { attachment_id } => WireControlAction::Clear {
                attachment_id: attachment_id.clone(),
            },
            Self::Reset { attachment_id } => WireControlAction::Reset {
                attachment_id: attachment_id.clone(),
            },
            Self::Acquire { attachment_id } => WireControlAction::Acquire {
                attachment_id: attachment_id.clone(),
            },
            Self::Input {
                attachment_id,
                controller_id,
                sequence,
                bytes,
            } => WireControlAction::Input {
                attachment_id: attachment_id.clone(),
                controller_id: controller_id.clone(),
                sequence: *sequence,
                bytes: bytes.clone(),
            },
            Self::SemanticInput {
                attachment_id,
                controller_id,
                sequence,
                input_kind,
                payload,
            } => WireControlAction::SemanticInput {
                attachment_id: attachment_id.clone(),
                controller_id: controller_id.clone(),
                sequence: *sequence,
                input_kind: *input_kind,
                payload: payload.clone(),
            },
            Self::Resize {
                attachment_id,
                controller_id,
                sequence,
                geometry,
            } => WireControlAction::Resize {
                attachment_id: attachment_id.clone(),
                controller_id: controller_id.clone(),
                sequence: *sequence,
                geometry: *geometry,
            },
            Self::HostColors {
                attachment_id,
                controller_id,
                sequence,
                foreground,
                background,
                cursor,
                palette,
            } => WireControlAction::HostColors {
                attachment_id: attachment_id.clone(),
                controller_id: controller_id.clone(),
                sequence: *sequence,
                foreground: *foreground,
                background: *background,
                cursor: *cursor,
                palette: *palette,
            },
            Self::Paste { .. } => {
                return Err(serde::ser::Error::custom(
                    "paste actions require the OBCQ record encoding",
                ));
            }
        }
        .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for TerminalControlAction {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match WireControlAction::deserialize(deserializer)? {
            WireControlAction::Observe { attachment_id } => Self::Observe { attachment_id },
            WireControlAction::Activate {
                attachment_id,
                geometry,
            } => Self::Activate {
                attachment_id,
                geometry,
            },
            WireControlAction::Clear { attachment_id } => Self::Clear { attachment_id },
            WireControlAction::Reset { attachment_id } => Self::Reset { attachment_id },
            WireControlAction::Acquire { attachment_id } => Self::Acquire { attachment_id },
            WireControlAction::Takeover { attachment_id } => Self::Takeover { attachment_id },
            WireControlAction::Input {
                attachment_id,
                controller_id,
                sequence,
                bytes,
            } => Self::Input {
                attachment_id,
                controller_id,
                sequence,
                bytes,
            },
            WireControlAction::SemanticInput {
                attachment_id,
                controller_id,
                sequence,
                input_kind,
                payload,
            } => Self::SemanticInput {
                attachment_id,
                controller_id,
                sequence,
                input_kind,
                payload,
            },
            WireControlAction::Resize {
                attachment_id,
                controller_id,
                sequence,
                geometry,
            } => Self::Resize {
                attachment_id,
                controller_id,
                sequence,
                geometry,
            },
            WireControlAction::HostColors {
                attachment_id,
                controller_id,
                sequence,
                foreground,
                background,
                cursor,
                palette,
            } => Self::HostColors {
                attachment_id,
                controller_id,
                sequence,
                foreground,
                background,
                cursor,
                palette,
            },
        })
    }
}

impl TerminalControlAction {
    /// Admits only this action's result, including an exact resize geometry echo.
    /// Rejection, unavailability, and an unknown outcome apply to every action.
    #[must_use]
    pub fn admits(&self, outcome: &TerminalControlOutcome) -> bool {
        let failed = matches!(
            outcome,
            TerminalControlOutcome::Rejected { .. }
                | TerminalControlOutcome::Unavailable { .. }
                | TerminalControlOutcome::OutcomeUnknown {}
        );
        match self {
            Self::Observe { attachment_id } | Self::Activate { attachment_id, .. } => {
                matches!(outcome, TerminalControlOutcome::Observed { state }
                    if state.recent_focus.len() <= 8
                    && state.recent_focus.iter().collect::<std::collections::BTreeSet<_>>().len() == state.recent_focus.len()
                    && (state.controller_id.is_none() || state.controller.as_ref() == Some(attachment_id)))
                    || failed
            }
            Self::Acquire { .. } | Self::Takeover { .. } => {
                matches!(outcome, TerminalControlOutcome::Acquired { .. }) || failed
            }
            Self::Clear { .. } => {
                matches!(outcome, TerminalControlOutcome::Cleared { .. }) || failed
            }
            Self::Reset { .. } => {
                matches!(outcome, TerminalControlOutcome::ResetAccepted { .. }) || failed
            }
            Self::Input { .. } | Self::SemanticInput { .. } => {
                matches!(outcome, TerminalControlOutcome::InputAccepted {}) || failed
            }
            Self::Resize { geometry, .. } => {
                matches!(outcome, TerminalControlOutcome::Resized { geometry: actual } if geometry == actual)
                    || failed
            }
            Self::Paste { .. } => {
                matches!(outcome, TerminalControlOutcome::Pasted { .. }) || failed
            }
            Self::HostColors { .. } => {
                matches!(outcome, TerminalControlOutcome::HostColorsAccepted {}) || failed
            }
        }
    }

    #[must_use]
    pub const fn attachment_id(&self) -> &ViewerId {
        match self {
            Self::Observe { attachment_id }
            | Self::Activate { attachment_id, .. }
            | Self::Acquire { attachment_id }
            | Self::Clear { attachment_id }
            | Self::Reset { attachment_id }
            | Self::Takeover { attachment_id }
            | Self::Input { attachment_id, .. }
            | Self::SemanticInput { attachment_id, .. }
            | Self::Resize { attachment_id, .. }
            | Self::Paste { attachment_id, .. }
            | Self::HostColors { attachment_id, .. } => attachment_id,
        }
    }
}

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControlRejectedReason {
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
    Observed {
        state: Box<TerminalControlState>,
    },
    Acquired {
        controller_id: ControllerId,
    },
    InputAccepted {},
    Resized {
        geometry: Geometry,
    },
    Pasted {
        accepted_bytes: PasteAcceptedBytes,
    },
    HostColorsAccepted {},
    Cleared {
        #[serde(with = "crate::number::positive_decimal")]
        generation: NonZeroU64,
        #[serde(with = "crate::number::positive_decimal")]
        revision: NonZeroU64,
        cleanup_confirmed: bool,
    },
    /// The Host applied the reset; the fields mean what they mean for `Cleared`.
    ResetAccepted {
        #[serde(with = "crate::number::positive_decimal")]
        generation: NonZeroU64,
        #[serde(with = "crate::number::positive_decimal")]
        revision: NonZeroU64,
        cleanup_confirmed: bool,
    },
    Rejected {
        reason: ControlRejectedReason,
    },
    Unavailable {
        reason: ControlUnavailableReason,
    },
    OutcomeUnknown {},
}

/// Published attachment identities are readable by every attached Client. A controller grant is
/// included only for the requesting attachment that already owns it; observation grants nothing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    deny_unknown_fields,
    try_from = "WireTerminalControlState"
)]
pub struct TerminalControlState {
    pub controller: Option<ViewerId>,
    pub controller_id: Option<ControllerId>,
    #[serde(with = "crate::number::decimal")]
    pub sequence: u64,
    pub geometry: Option<Geometry>,
    pub recent_focus: Vec<ViewerId>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireTerminalControlState {
    #[serde(deserialize_with = "Option::deserialize")]
    controller: Option<ViewerId>,
    #[serde(deserialize_with = "Option::deserialize")]
    controller_id: Option<ControllerId>,
    #[serde(with = "crate::number::decimal")]
    sequence: u64,
    #[serde(deserialize_with = "Option::deserialize")]
    geometry: Option<Geometry>,
    recent_focus: Vec<ViewerId>,
}
impl TryFrom<WireTerminalControlState> for TerminalControlState {
    type Error = ProtocolError;
    fn try_from(wire: WireTerminalControlState) -> Result<Self, Self::Error> {
        if wire.recent_focus.len() > 8
            || wire
                .recent_focus
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != wire.recent_focus.len()
            || (wire.controller.is_none() && wire.controller_id.is_some())
            || (wire.controller_id.is_none() && (wire.sequence != 0 || wire.geometry.is_some()))
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self {
            controller: wire.controller,
            controller_id: wire.controller_id,
            sequence: wire.sequence,
            geometry: wire.geometry,
            recent_focus: wire.recent_focus,
        })
    }
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
