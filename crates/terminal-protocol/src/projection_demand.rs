use serde::{Deserialize, Serialize};

use crate::{
    AttachmentId, MessageRole, ProtocolError, ProtocolErrorCode,
    control::{decode_wire, preflight_json, serialize_wire},
    terminal_fact::canonical_u64,
};

const SET_PROJECTION_DEMAND_KIND: u16 = 47;
const PROJECTION_DEMAND_ACCEPTED_KIND: u16 = 48;
const PROJECTION_BLOCKED_KIND: u16 = 49;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionDemand {
    Active,
    Paused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SetProjectionDemand {
    pub attachment_id: AttachmentId,
    pub demand: ProjectionDemand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionDemandAccepted {
    pub attachment_id: AttachmentId,
    pub demand: ProjectionDemand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionBlockReason {
    CaptureAllocationFailed,
    ProjectionTooLarge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionBlocked {
    pub attachment_id: AttachmentId,
    pub reason: ProjectionBlockReason,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionDemandControl {
    SetProjectionDemand(SetProjectionDemand),
    ProjectionDemandAccepted(ProjectionDemandAccepted),
    ProjectionBlocked(ProjectionBlocked),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedProjectionDemandControl {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

pub fn decode_projection_demand_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<ProjectionDemandControl, ProtocolError> {
    preflight_json(payload)?;
    match (kind, role) {
        (SET_PROJECTION_DEMAND_KIND, MessageRole::Request) => {
            decode_wire::<ProjectionDemandWire>(payload)
                .and_then(projection_demand_from_wire)
                .map(ProjectionDemandControl::SetProjectionDemand)
        }
        (PROJECTION_DEMAND_ACCEPTED_KIND, MessageRole::Response) => {
            decode_wire::<ProjectionDemandWire>(payload)
                .and_then(projection_demand_accepted_from_wire)
                .map(ProjectionDemandControl::ProjectionDemandAccepted)
        }
        (PROJECTION_BLOCKED_KIND, MessageRole::Event) => {
            decode_wire::<ProjectionBlockedWire>(payload)
                .and_then(projection_blocked_from_wire)
                .map(ProjectionDemandControl::ProjectionBlocked)
        }
        _ => Err(invalid_field(
            "message kind and role do not select a projection-demand payload",
        )),
    }
}

pub fn encode_projection_demand_control(
    value: &ProjectionDemandControl,
) -> Result<EncodedProjectionDemandControl, ProtocolError> {
    let (kind, role, payload) = match value {
        ProjectionDemandControl::SetProjectionDemand(demand) => (
            SET_PROJECTION_DEMAND_KIND,
            MessageRole::Request,
            serialize_wire(&projection_demand_to_wire(
                demand.attachment_id,
                demand.demand,
            ))?,
        ),
        ProjectionDemandControl::ProjectionDemandAccepted(demand) => (
            PROJECTION_DEMAND_ACCEPTED_KIND,
            MessageRole::Response,
            serialize_wire(&projection_demand_to_wire(
                demand.attachment_id,
                demand.demand,
            ))?,
        ),
        ProjectionDemandControl::ProjectionBlocked(blocked) => (
            PROJECTION_BLOCKED_KIND,
            MessageRole::Event,
            serialize_wire(&projection_blocked_to_wire(*blocked))?,
        ),
    };
    Ok(EncodedProjectionDemandControl {
        kind,
        payload,
        role,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectionDemandWire {
    attachment_id: String,
    demand: ProjectionDemandWireValue,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ProjectionDemandWireValue {
    Active,
    Paused,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectionBlockedWire {
    attachment_id: String,
    reason: ProjectionBlockReasonWire,
    revision: String,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ProjectionBlockReasonWire {
    CaptureAllocationFailed,
    ProjectionTooLarge,
}

fn projection_demand_from_wire(
    wire: ProjectionDemandWire,
) -> Result<SetProjectionDemand, ProtocolError> {
    Ok(SetProjectionDemand {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        demand: projection_demand_value_from_wire(wire.demand),
    })
}

fn projection_demand_accepted_from_wire(
    wire: ProjectionDemandWire,
) -> Result<ProjectionDemandAccepted, ProtocolError> {
    Ok(ProjectionDemandAccepted {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        demand: projection_demand_value_from_wire(wire.demand),
    })
}

fn projection_demand_to_wire(
    attachment_id: AttachmentId,
    demand: ProjectionDemand,
) -> ProjectionDemandWire {
    ProjectionDemandWire {
        attachment_id: attachment_id.to_text(),
        demand: projection_demand_value_to_wire(demand),
    }
}

fn projection_blocked_from_wire(
    wire: ProjectionBlockedWire,
) -> Result<ProjectionBlocked, ProtocolError> {
    Ok(ProjectionBlocked {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        reason: projection_block_reason_from_wire(wire.reason),
        revision: canonical_u64(&wire.revision)?,
    })
}

fn projection_blocked_to_wire(value: ProjectionBlocked) -> ProjectionBlockedWire {
    ProjectionBlockedWire {
        attachment_id: value.attachment_id.to_text(),
        reason: projection_block_reason_to_wire(value.reason),
        revision: value.revision.to_string(),
    }
}

const fn projection_demand_value_from_wire(value: ProjectionDemandWireValue) -> ProjectionDemand {
    match value {
        ProjectionDemandWireValue::Active => ProjectionDemand::Active,
        ProjectionDemandWireValue::Paused => ProjectionDemand::Paused,
    }
}

const fn projection_demand_value_to_wire(value: ProjectionDemand) -> ProjectionDemandWireValue {
    match value {
        ProjectionDemand::Active => ProjectionDemandWireValue::Active,
        ProjectionDemand::Paused => ProjectionDemandWireValue::Paused,
    }
}

const fn projection_block_reason_from_wire(
    value: ProjectionBlockReasonWire,
) -> ProjectionBlockReason {
    match value {
        ProjectionBlockReasonWire::CaptureAllocationFailed => {
            ProjectionBlockReason::CaptureAllocationFailed
        }
        ProjectionBlockReasonWire::ProjectionTooLarge => ProjectionBlockReason::ProjectionTooLarge,
    }
}

const fn projection_block_reason_to_wire(
    value: ProjectionBlockReason,
) -> ProjectionBlockReasonWire {
    match value {
        ProjectionBlockReason::CaptureAllocationFailed => {
            ProjectionBlockReasonWire::CaptureAllocationFailed
        }
        ProjectionBlockReason::ProjectionTooLarge => ProjectionBlockReasonWire::ProjectionTooLarge,
    }
}

fn parse_attachment_id(value: &str) -> Result<AttachmentId, ProtocolError> {
    AttachmentId::parse(value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidIdentifier,
            "projection-demand attachment identity is invalid",
        )
    })
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
