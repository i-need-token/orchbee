//! Acquire one complete input before any Site discovery or graphics submission.
use super::{Failure, FailureKind, Signals};
use crate::{
    cli::source::{self, SourceFailure},
    invocation::pane_graphics::{Input, MAXIMUM_SET_DOCUMENT_BYTES, SetSpec},
};
use base64::Engine;
use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_SET_BYTES, PaneGraphicsFormat, PaneGraphicsFrame, PaneGraphicsPlacement,
};
use std::time::Instant;
pub(super) fn acquire(
    input: &Input,
    deadline: Instant,
    signals: &mut Signals,
) -> Result<SetSpec, Failure> {
    let path = match input {
        Input::Inline(spec) => return Ok((**spec).clone()),
        Input::File(path) => Some(path.as_str()),
        Input::Stdin => None,
    };
    let bytes = source::acquire_document(path, MAXIMUM_SET_DOCUMENT_BYTES, deadline, signals)
        .map_err(|error| match error {
            SourceFailure::Standard(mut failure) => {
                failure.usage = None;
                failure
            }
            SourceFailure::Timeout { .. } => Failure::new(FailureKind::Timeout),
            SourceFailure::Cancelled { .. } => Failure::new(FailureKind::Cancelled),
        })?;
    parse(&bytes)
}
pub(super) fn parse(bytes: &[u8]) -> Result<SetSpec, Failure> {
    let invalid = || Failure::new(FailureKind::InvalidUsage);
    if bytes.len() > MAXIMUM_SET_DOCUMENT_BYTES {
        return Err(invalid());
    }
    let spec = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        if bytes.len() < 33
            || bytes.len() > MAXIMUM_GRAPHICS_SET_BYTES
            || bytes[8..16] != [0, 0, 0, 13, b'I', b'H', b'D', b'R']
        {
            return Err(invalid());
        }
        let width = u32::from_be_bytes(bytes[16..20].try_into().map_err(|_| invalid())?);
        let height = u32::from_be_bytes(bytes[20..24].try_into().map_err(|_| invalid())?);
        SetSpec {
            layer_id: "primary".into(),
            z_index: 0,
            image: PaneGraphicsFrame {
                format: PaneGraphicsFormat::Png,
                width,
                height,
                data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                placement: PaneGraphicsPlacement::default(),
            },
        }
    } else {
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
        serde_json::from_slice(bytes).map_err(|_| invalid())?
    };
    if !spec.valid() {
        return Err(invalid());
    }
    Ok(spec)
}
