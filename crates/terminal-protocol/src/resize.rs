use serde::{Deserialize, Serialize};

use crate::{
    AttachmentId, MessageRole, ProtocolError, ProtocolErrorCode, TerminalPixelGeometry,
    control::{SafeInteger, decode_wire, preflight_json, serialize_wire},
    geometry::{
        TerminalPixelGeometryWire, geometry_dimensions_from_wire, geometry_from_wire,
        geometry_to_wire,
    },
    terminal_fact::canonical_u64,
};

const RESIZE_TERMINAL_KIND: u16 = 32;
const RESIZE_ACCEPTED_KIND: u16 = 33;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResizeTerminal {
    pub attachment_id: AttachmentId,
    pub geometry: TerminalPixelGeometry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResizeAccepted {
    pub attachment_id: AttachmentId,
    pub geometry: TerminalPixelGeometry,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResizeControl {
    ResizeTerminal(ResizeTerminal),
    ResizeAccepted(ResizeAccepted),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedResizeControl {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

pub fn decode_resize_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<ResizeControl, ProtocolError> {
    preflight_json(payload)?;
    match (kind, role) {
        (RESIZE_TERMINAL_KIND, MessageRole::Request) => decode_wire::<ResizeTerminalWire>(payload)
            .and_then(resize_terminal_from_wire)
            .map(ResizeControl::ResizeTerminal),
        (RESIZE_ACCEPTED_KIND, MessageRole::Response) => decode_wire::<ResizeAcceptedWire>(payload)
            .and_then(resize_accepted_from_wire)
            .map(ResizeControl::ResizeAccepted),
        _ => Err(invalid_field(
            "message kind and role do not select a resize-control payload",
        )),
    }
}

pub fn encode_resize_control(value: &ResizeControl) -> Result<EncodedResizeControl, ProtocolError> {
    let (kind, role, payload) = match value {
        ResizeControl::ResizeTerminal(resize) => (
            RESIZE_TERMINAL_KIND,
            MessageRole::Request,
            serialize_wire(&resize_terminal_to_wire(*resize)?)?,
        ),
        ResizeControl::ResizeAccepted(accepted) => (
            RESIZE_ACCEPTED_KIND,
            MessageRole::Response,
            serialize_wire(&resize_accepted_to_wire(*accepted)?)?,
        ),
    };
    Ok(EncodedResizeControl {
        kind,
        payload,
        role,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResizeTerminalWire {
    attachment_id: String,
    cell_pixel_height: SafeInteger,
    cell_pixel_width: SafeInteger,
    columns: SafeInteger,
    rows: SafeInteger,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResizeAcceptedWire {
    attachment_id: String,
    cell_pixel_height: SafeInteger,
    cell_pixel_width: SafeInteger,
    columns: SafeInteger,
    revision: String,
    rows: SafeInteger,
}

fn resize_terminal_from_wire(wire: ResizeTerminalWire) -> Result<ResizeTerminal, ProtocolError> {
    Ok(ResizeTerminal {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        geometry: geometry_from_wire(TerminalPixelGeometryWire {
            cell_pixel_height: wire.cell_pixel_height,
            cell_pixel_width: wire.cell_pixel_width,
            columns: wire.columns,
            rows: wire.rows,
        })?,
    })
}

fn resize_terminal_to_wire(value: ResizeTerminal) -> Result<ResizeTerminalWire, ProtocolError> {
    let geometry = geometry_to_wire(value.geometry)?;
    Ok(ResizeTerminalWire {
        attachment_id: value.attachment_id.to_text(),
        cell_pixel_height: geometry.cell_pixel_height,
        cell_pixel_width: geometry.cell_pixel_width,
        columns: geometry.columns,
        rows: geometry.rows,
    })
}

fn resize_accepted_from_wire(wire: ResizeAcceptedWire) -> Result<ResizeAccepted, ProtocolError> {
    Ok(ResizeAccepted {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        geometry: geometry_dimensions_from_wire(TerminalPixelGeometryWire {
            cell_pixel_height: wire.cell_pixel_height,
            cell_pixel_width: wire.cell_pixel_width,
            columns: wire.columns,
            rows: wire.rows,
        })?,
        revision: canonical_u64(&wire.revision)?,
    })
}

fn resize_accepted_to_wire(value: ResizeAccepted) -> Result<ResizeAcceptedWire, ProtocolError> {
    let geometry = geometry_to_wire(value.geometry)?;
    Ok(ResizeAcceptedWire {
        attachment_id: value.attachment_id.to_text(),
        cell_pixel_height: geometry.cell_pixel_height,
        cell_pixel_width: geometry.cell_pixel_width,
        columns: geometry.columns,
        revision: value.revision.to_string(),
        rows: geometry.rows,
    })
}

fn parse_attachment_id(value: &str) -> Result<AttachmentId, ProtocolError> {
    AttachmentId::parse(value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidIdentifier,
            "resize attachment identity is invalid",
        )
    })
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
