use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};

use crate::{
    AttachmentId, MessageRole, ProtocolError, ProtocolErrorCode, TerminalId,
    control::{decode_wire, preflight_json, serialize_wire},
    terminal_fact::canonical_u64,
};

const ATTACH_TERMINAL_KIND: u16 = 20;
const DETACH_TERMINAL_KIND: u16 = 21;
const ATTACHMENT_CAPABILITIES_KIND: u16 = 22;
const RESIZE_LEASE_CHANGED_KIND: u16 = 23;
const UPDATE_ATTACHMENT_INPUT_CAPABILITY_KIND: u16 = 24;
const CLAIM_RESIZE_LEASE_KIND: u16 = 25;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachmentCapabilityState {
    pub input: bool,
    pub read: bool,
    pub resize: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachTerminal {
    pub attachment_id: AttachmentId,
    pub capabilities: AttachmentCapabilityState,
    pub request_resize_lease: bool,
    pub terminal_id: TerminalId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachmentTarget {
    pub attachment_id: AttachmentId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachmentCapabilities {
    pub attachment_id: AttachmentId,
    pub capability_revision: NonZeroU64,
    pub capabilities: AttachmentCapabilityState,
    pub resize_lease: bool,
    pub terminal_id: TerminalId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResizeLeaseChanged {
    pub attachment_id: AttachmentId,
    pub resize_lease: bool,
    pub terminal_id: TerminalId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpdateAttachmentInputCapability {
    pub attachment_id: AttachmentId,
    pub capability_revision: NonZeroU64,
    pub input: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentControl {
    AttachTerminal(AttachTerminal),
    DetachTerminal(AttachmentTarget),
    DetachTerminalAccepted(AttachmentTarget),
    AttachmentCapabilities(AttachmentCapabilities),
    ResizeLeaseChanged(ResizeLeaseChanged),
    UpdateAttachmentInputCapability(UpdateAttachmentInputCapability),
    ClaimResizeLease(AttachmentTarget),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedAttachmentControl {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

pub fn decode_attachment_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<AttachmentControl, ProtocolError> {
    preflight_json(payload)?;
    match (kind, role) {
        (ATTACH_TERMINAL_KIND, MessageRole::Request) => decode_wire::<AttachTerminalWire>(payload)
            .and_then(attach_terminal_from_wire)
            .map(AttachmentControl::AttachTerminal),
        (DETACH_TERMINAL_KIND, MessageRole::Request) => {
            decode_wire::<AttachmentTargetWire>(payload)
                .and_then(attachment_target_from_wire)
                .map(AttachmentControl::DetachTerminal)
        }
        (DETACH_TERMINAL_KIND, MessageRole::Response) => {
            decode_wire::<AttachmentTargetWire>(payload)
                .and_then(attachment_target_from_wire)
                .map(AttachmentControl::DetachTerminalAccepted)
        }
        (ATTACHMENT_CAPABILITIES_KIND, MessageRole::Response) => {
            decode_wire::<AttachmentCapabilitiesWire>(payload)
                .and_then(attachment_capabilities_from_wire)
                .map(AttachmentControl::AttachmentCapabilities)
        }
        (RESIZE_LEASE_CHANGED_KIND, MessageRole::Event) => {
            decode_wire::<ResizeLeaseChangedWire>(payload)
                .and_then(resize_lease_changed_from_wire)
                .map(AttachmentControl::ResizeLeaseChanged)
        }
        (UPDATE_ATTACHMENT_INPUT_CAPABILITY_KIND, MessageRole::Request) => {
            decode_wire::<UpdateAttachmentInputCapabilityWire>(payload)
                .and_then(update_attachment_input_capability_from_wire)
                .map(AttachmentControl::UpdateAttachmentInputCapability)
        }
        (CLAIM_RESIZE_LEASE_KIND, MessageRole::Request) => {
            decode_wire::<AttachmentTargetWire>(payload)
                .and_then(attachment_target_from_wire)
                .map(AttachmentControl::ClaimResizeLease)
        }
        _ => Err(invalid_field(
            "message kind and role do not select an attachment-control payload",
        )),
    }
}

pub fn encode_attachment_control(
    value: &AttachmentControl,
) -> Result<EncodedAttachmentControl, ProtocolError> {
    let (kind, role, payload) = match value {
        AttachmentControl::AttachTerminal(attach) => (
            ATTACH_TERMINAL_KIND,
            MessageRole::Request,
            serialize_wire(&attach_terminal_to_wire(*attach)?)?,
        ),
        AttachmentControl::DetachTerminal(target) => (
            DETACH_TERMINAL_KIND,
            MessageRole::Request,
            serialize_wire(&attachment_target_to_wire(*target))?,
        ),
        AttachmentControl::DetachTerminalAccepted(target) => (
            DETACH_TERMINAL_KIND,
            MessageRole::Response,
            serialize_wire(&attachment_target_to_wire(*target))?,
        ),
        AttachmentControl::AttachmentCapabilities(capabilities) => (
            ATTACHMENT_CAPABILITIES_KIND,
            MessageRole::Response,
            serialize_wire(&attachment_capabilities_to_wire(*capabilities)?)?,
        ),
        AttachmentControl::ResizeLeaseChanged(changed) => (
            RESIZE_LEASE_CHANGED_KIND,
            MessageRole::Event,
            serialize_wire(&resize_lease_changed_to_wire(*changed))?,
        ),
        AttachmentControl::UpdateAttachmentInputCapability(update) => (
            UPDATE_ATTACHMENT_INPUT_CAPABILITY_KIND,
            MessageRole::Request,
            serialize_wire(&update_attachment_input_capability_to_wire(*update))?,
        ),
        AttachmentControl::ClaimResizeLease(target) => (
            CLAIM_RESIZE_LEASE_KIND,
            MessageRole::Request,
            serialize_wire(&attachment_target_to_wire(*target))?,
        ),
    };
    Ok(EncodedAttachmentControl {
        kind,
        payload,
        role,
    })
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AttachmentCapabilityStateWire {
    input: bool,
    read: bool,
    resize: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AttachTerminalWire {
    attachment_id: String,
    capabilities: AttachmentCapabilityStateWire,
    request_resize_lease: bool,
    terminal_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AttachmentTargetWire {
    attachment_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AttachmentCapabilitiesWire {
    attachment_id: String,
    capability_revision: String,
    capabilities: AttachmentCapabilityStateWire,
    resize_lease: bool,
    terminal_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResizeLeaseChangedWire {
    attachment_id: String,
    resize_lease: bool,
    terminal_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateAttachmentInputCapabilityWire {
    attachment_id: String,
    capability_revision: String,
    input: bool,
}

fn attach_terminal_from_wire(wire: AttachTerminalWire) -> Result<AttachTerminal, ProtocolError> {
    let value = AttachTerminal {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        capabilities: capability_state_from_wire(wire.capabilities),
        request_resize_lease: wire.request_resize_lease,
        terminal_id: parse_terminal_id(&wire.terminal_id)?,
    };
    validate_attach_terminal(value)?;
    Ok(value)
}

fn attach_terminal_to_wire(value: AttachTerminal) -> Result<AttachTerminalWire, ProtocolError> {
    validate_attach_terminal(value)?;
    Ok(AttachTerminalWire {
        attachment_id: value.attachment_id.to_text(),
        capabilities: capability_state_to_wire(value.capabilities),
        request_resize_lease: value.request_resize_lease,
        terminal_id: value.terminal_id.to_text(),
    })
}

fn attachment_target_from_wire(
    wire: AttachmentTargetWire,
) -> Result<AttachmentTarget, ProtocolError> {
    Ok(AttachmentTarget {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
    })
}

fn attachment_target_to_wire(value: AttachmentTarget) -> AttachmentTargetWire {
    AttachmentTargetWire {
        attachment_id: value.attachment_id.to_text(),
    }
}

fn attachment_capabilities_from_wire(
    wire: AttachmentCapabilitiesWire,
) -> Result<AttachmentCapabilities, ProtocolError> {
    let value = AttachmentCapabilities {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        capability_revision: positive_canonical_u64(&wire.capability_revision)?,
        capabilities: capability_state_from_wire(wire.capabilities),
        resize_lease: wire.resize_lease,
        terminal_id: parse_terminal_id(&wire.terminal_id)?,
    };
    validate_attachment_capabilities(value)?;
    Ok(value)
}

fn attachment_capabilities_to_wire(
    value: AttachmentCapabilities,
) -> Result<AttachmentCapabilitiesWire, ProtocolError> {
    validate_attachment_capabilities(value)?;
    Ok(AttachmentCapabilitiesWire {
        attachment_id: value.attachment_id.to_text(),
        capability_revision: value.capability_revision.get().to_string(),
        capabilities: capability_state_to_wire(value.capabilities),
        resize_lease: value.resize_lease,
        terminal_id: value.terminal_id.to_text(),
    })
}

fn resize_lease_changed_from_wire(
    wire: ResizeLeaseChangedWire,
) -> Result<ResizeLeaseChanged, ProtocolError> {
    Ok(ResizeLeaseChanged {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        resize_lease: wire.resize_lease,
        terminal_id: parse_terminal_id(&wire.terminal_id)?,
    })
}

fn resize_lease_changed_to_wire(value: ResizeLeaseChanged) -> ResizeLeaseChangedWire {
    ResizeLeaseChangedWire {
        attachment_id: value.attachment_id.to_text(),
        resize_lease: value.resize_lease,
        terminal_id: value.terminal_id.to_text(),
    }
}

fn update_attachment_input_capability_from_wire(
    wire: UpdateAttachmentInputCapabilityWire,
) -> Result<UpdateAttachmentInputCapability, ProtocolError> {
    Ok(UpdateAttachmentInputCapability {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        capability_revision: positive_canonical_u64(&wire.capability_revision)?,
        input: wire.input,
    })
}

fn update_attachment_input_capability_to_wire(
    value: UpdateAttachmentInputCapability,
) -> UpdateAttachmentInputCapabilityWire {
    UpdateAttachmentInputCapabilityWire {
        attachment_id: value.attachment_id.to_text(),
        capability_revision: value.capability_revision.get().to_string(),
        input: value.input,
    }
}

const fn capability_state_from_wire(
    wire: AttachmentCapabilityStateWire,
) -> AttachmentCapabilityState {
    AttachmentCapabilityState {
        input: wire.input,
        read: wire.read,
        resize: wire.resize,
    }
}

const fn capability_state_to_wire(
    value: AttachmentCapabilityState,
) -> AttachmentCapabilityStateWire {
    AttachmentCapabilityStateWire {
        input: value.input,
        read: value.read,
        resize: value.resize,
    }
}

fn validate_attach_terminal(value: AttachTerminal) -> Result<(), ProtocolError> {
    if !value.capabilities.input && !value.capabilities.read && !value.capabilities.resize {
        return Err(invalid_field(
            "at least one attachment capability is required",
        ));
    }
    if value.request_resize_lease && !value.capabilities.resize {
        return Err(invalid_field(
            "a requested resize lease requires resize capability",
        ));
    }
    Ok(())
}

fn validate_attachment_capabilities(value: AttachmentCapabilities) -> Result<(), ProtocolError> {
    if value.resize_lease && !value.capabilities.resize {
        return Err(invalid_field(
            "a granted resize lease requires resize capability",
        ));
    }
    Ok(())
}

fn positive_canonical_u64(value: &str) -> Result<NonZeroU64, ProtocolError> {
    NonZeroU64::new(canonical_u64(value)?)
        .ok_or_else(|| invalid_field("capability revision must be greater than zero"))
}

fn parse_attachment_id(value: &str) -> Result<AttachmentId, ProtocolError> {
    AttachmentId::parse(value).map_err(|_error| invalid_identifier())
}

fn parse_terminal_id(value: &str) -> Result<TerminalId, ProtocolError> {
    TerminalId::parse(value).map_err(|_error| invalid_identifier())
}

const fn invalid_identifier() -> ProtocolError {
    ProtocolError::new(
        ProtocolErrorCode::InvalidIdentifier,
        "attachment payload identity is invalid",
    )
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
