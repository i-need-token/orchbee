use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};

use crate::{
    AttachmentId, MessageRole, ProtocolError, ProtocolErrorCode,
    control::{decode_wire, preflight_json, serialize_wire},
    terminal_fact::canonical_u64,
};

const SET_HISTORY_DEMAND_KIND: u16 = 51;
const HISTORY_DEMAND_ACCEPTED_KIND: u16 = 52;
const HISTORY_CHANGED_KIND: u16 = 53;
const HISTORY_OBSERVED_KIND: u16 = 54;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryDemand {
    FollowingBottom,
    ViewingHistory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SetHistoryDemand {
    pub attachment_id: AttachmentId,
    pub demand: HistoryDemand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoryDemandAccepted {
    pub attachment_id: AttachmentId,
    pub demand: HistoryDemand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryAnchorStatus {
    Evicted,
    None,
    Valid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoryChanged {
    pub anchor_status: HistoryAnchorStatus,
    pub attachment_id: AttachmentId,
    pub generation: NonZeroU64,
    pub revision: u64,
    pub sequence: NonZeroU64,
    pub total_rows: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoryObserved {
    pub attachment_id: AttachmentId,
    pub generation: NonZeroU64,
    pub revision: u64,
    pub sequence: NonZeroU64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryControl {
    SetHistoryDemand(SetHistoryDemand),
    HistoryDemandAccepted(HistoryDemandAccepted),
    HistoryChanged(HistoryChanged),
    HistoryObservedRequest(HistoryObserved),
    HistoryObservedResponse(HistoryObserved),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedHistoryControl {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

pub fn decode_history_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<HistoryControl, ProtocolError> {
    preflight_json(payload)?;
    match (kind, role) {
        (SET_HISTORY_DEMAND_KIND, MessageRole::Request) => {
            decode_wire::<HistoryDemandWire>(payload)
                .and_then(set_history_demand_from_wire)
                .map(HistoryControl::SetHistoryDemand)
        }
        (HISTORY_DEMAND_ACCEPTED_KIND, MessageRole::Response) => {
            decode_wire::<HistoryDemandWire>(payload)
                .and_then(history_demand_accepted_from_wire)
                .map(HistoryControl::HistoryDemandAccepted)
        }
        (HISTORY_CHANGED_KIND, MessageRole::Event) => decode_wire::<HistoryChangedWire>(payload)
            .and_then(history_changed_from_wire)
            .map(HistoryControl::HistoryChanged),
        (HISTORY_OBSERVED_KIND, MessageRole::Request) => {
            decode_wire::<HistoryObservedWire>(payload)
                .and_then(history_observed_from_wire)
                .map(HistoryControl::HistoryObservedRequest)
        }
        (HISTORY_OBSERVED_KIND, MessageRole::Response) => {
            decode_wire::<HistoryObservedWire>(payload)
                .and_then(history_observed_from_wire)
                .map(HistoryControl::HistoryObservedResponse)
        }
        _ => Err(invalid_field(
            "message kind and role do not select a history-control payload",
        )),
    }
}

pub fn encode_history_control(
    value: &HistoryControl,
) -> Result<EncodedHistoryControl, ProtocolError> {
    let (kind, role, payload) = match value {
        HistoryControl::SetHistoryDemand(demand) => (
            SET_HISTORY_DEMAND_KIND,
            MessageRole::Request,
            serialize_wire(&history_demand_to_wire(demand.attachment_id, demand.demand))?,
        ),
        HistoryControl::HistoryDemandAccepted(demand) => (
            HISTORY_DEMAND_ACCEPTED_KIND,
            MessageRole::Response,
            serialize_wire(&history_demand_to_wire(demand.attachment_id, demand.demand))?,
        ),
        HistoryControl::HistoryChanged(changed) => (
            HISTORY_CHANGED_KIND,
            MessageRole::Event,
            serialize_wire(&history_changed_to_wire(*changed))?,
        ),
        HistoryControl::HistoryObservedRequest(observed) => (
            HISTORY_OBSERVED_KIND,
            MessageRole::Request,
            serialize_wire(&history_observed_to_wire(*observed))?,
        ),
        HistoryControl::HistoryObservedResponse(observed) => (
            HISTORY_OBSERVED_KIND,
            MessageRole::Response,
            serialize_wire(&history_observed_to_wire(*observed))?,
        ),
    };
    Ok(EncodedHistoryControl {
        kind,
        payload,
        role,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HistoryDemandWire {
    attachment_id: String,
    demand: HistoryDemandWireValue,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum HistoryDemandWireValue {
    FollowingBottom,
    ViewingHistory,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HistoryChangedWire {
    anchor_status: HistoryAnchorStatusWire,
    attachment_id: String,
    generation: String,
    revision: String,
    sequence: String,
    total_rows: String,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum HistoryAnchorStatusWire {
    Evicted,
    None,
    Valid,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HistoryObservedWire {
    attachment_id: String,
    generation: String,
    revision: String,
    sequence: String,
}

fn set_history_demand_from_wire(
    wire: HistoryDemandWire,
) -> Result<SetHistoryDemand, ProtocolError> {
    Ok(SetHistoryDemand {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        demand: history_demand_from_wire(wire.demand),
    })
}

fn history_demand_accepted_from_wire(
    wire: HistoryDemandWire,
) -> Result<HistoryDemandAccepted, ProtocolError> {
    Ok(HistoryDemandAccepted {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        demand: history_demand_from_wire(wire.demand),
    })
}

fn history_demand_to_wire(attachment_id: AttachmentId, demand: HistoryDemand) -> HistoryDemandWire {
    HistoryDemandWire {
        attachment_id: attachment_id.to_text(),
        demand: history_demand_to_wire_value(demand),
    }
}

fn history_changed_from_wire(wire: HistoryChangedWire) -> Result<HistoryChanged, ProtocolError> {
    Ok(HistoryChanged {
        anchor_status: history_anchor_status_from_wire(wire.anchor_status),
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        generation: positive_canonical_u64(&wire.generation)?,
        revision: canonical_u64(&wire.revision)?,
        sequence: positive_canonical_u64(&wire.sequence)?,
        total_rows: canonical_u64(&wire.total_rows)?,
    })
}

fn history_changed_to_wire(value: HistoryChanged) -> HistoryChangedWire {
    HistoryChangedWire {
        anchor_status: history_anchor_status_to_wire(value.anchor_status),
        attachment_id: value.attachment_id.to_text(),
        generation: value.generation.to_string(),
        revision: value.revision.to_string(),
        sequence: value.sequence.to_string(),
        total_rows: value.total_rows.to_string(),
    }
}

fn history_observed_from_wire(wire: HistoryObservedWire) -> Result<HistoryObserved, ProtocolError> {
    Ok(HistoryObserved {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        generation: positive_canonical_u64(&wire.generation)?,
        revision: canonical_u64(&wire.revision)?,
        sequence: positive_canonical_u64(&wire.sequence)?,
    })
}

fn history_observed_to_wire(value: HistoryObserved) -> HistoryObservedWire {
    HistoryObservedWire {
        attachment_id: value.attachment_id.to_text(),
        generation: value.generation.to_string(),
        revision: value.revision.to_string(),
        sequence: value.sequence.to_string(),
    }
}

const fn history_demand_from_wire(value: HistoryDemandWireValue) -> HistoryDemand {
    match value {
        HistoryDemandWireValue::FollowingBottom => HistoryDemand::FollowingBottom,
        HistoryDemandWireValue::ViewingHistory => HistoryDemand::ViewingHistory,
    }
}

const fn history_demand_to_wire_value(value: HistoryDemand) -> HistoryDemandWireValue {
    match value {
        HistoryDemand::FollowingBottom => HistoryDemandWireValue::FollowingBottom,
        HistoryDemand::ViewingHistory => HistoryDemandWireValue::ViewingHistory,
    }
}

const fn history_anchor_status_from_wire(value: HistoryAnchorStatusWire) -> HistoryAnchorStatus {
    match value {
        HistoryAnchorStatusWire::Evicted => HistoryAnchorStatus::Evicted,
        HistoryAnchorStatusWire::None => HistoryAnchorStatus::None,
        HistoryAnchorStatusWire::Valid => HistoryAnchorStatus::Valid,
    }
}

const fn history_anchor_status_to_wire(value: HistoryAnchorStatus) -> HistoryAnchorStatusWire {
    match value {
        HistoryAnchorStatus::Evicted => HistoryAnchorStatusWire::Evicted,
        HistoryAnchorStatus::None => HistoryAnchorStatusWire::None,
        HistoryAnchorStatus::Valid => HistoryAnchorStatusWire::Valid,
    }
}

fn parse_attachment_id(value: &str) -> Result<AttachmentId, ProtocolError> {
    AttachmentId::parse(value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidIdentifier,
            "history-control attachment identity is invalid",
        )
    })
}

fn positive_canonical_u64(value: &str) -> Result<NonZeroU64, ProtocolError> {
    NonZeroU64::new(canonical_u64(value)?).ok_or_else(|| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidField,
            "history-control counter must be greater than zero",
        )
    })
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
