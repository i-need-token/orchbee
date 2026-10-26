//! Strict image shapes, resource correlations and full-width producer sequences.
use super::{
    ContractError, MAXIMUM_GRAPHICS_CHUNK_BYTES, MAXIMUM_GRAPHICS_LAYERS_PER_PANE,
    MAXIMUM_GRAPHICS_RESIDENT_BYTES, MAXIMUM_GRAPHICS_SET_BYTES, MAXIMUM_GRAPHICS_STREAM_BYTES,
    PaneGraphicsAction, PaneGraphicsFormat, PaneGraphicsFrame, PaneGraphicsOutcome,
    PaneGraphicsRequest, PaneGraphicsResult, PaneGraphicsStreamImage, PaneGraphicsTarget,
    PaneGraphicsTransport, graphics_layer_id, graphics_rgba_bytes,
};
fn require(valid: bool) -> Result<(), ContractError> {
    valid.then_some(()).ok_or(ContractError::InvalidDocument)
}
fn resolved(target: &PaneGraphicsTarget) -> Result<(), ContractError> {
    require(target.presentation_id.is_some())
}
fn file_path(path: &str) -> Result<(), ContractError> {
    crate::WorkbenchDirectoryPath::parse(path)
        .map(|_| ())
        .map_err(|_| ContractError::InvalidDocument)
}
fn value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}
pub(super) fn base64_bytes(encoded: &str) -> Result<usize, ContractError> {
    let bytes = encoded.as_bytes();
    require(!bytes.is_empty() && bytes.len().is_multiple_of(4))?;
    let padding = if bytes.ends_with(b"==") {
        2
    } else {
        usize::from(bytes.ends_with(b"="))
    };
    require(
        bytes[..bytes.len() - padding]
            .iter()
            .all(|byte| value(*byte).is_some()),
    )?;
    if padding == 2 {
        require(value(bytes[bytes.len() - 3]).is_some_and(|v| v & 15 == 0))?;
    }
    if padding == 1 {
        require(value(bytes[bytes.len() - 2]).is_some_and(|v| v & 3 == 0))?;
    }
    bytes
        .len()
        .checked_div(4)
        .and_then(|length| length.checked_mul(3))
        .and_then(|length| length.checked_sub(padding))
        .ok_or(ContractError::InvalidDocument)
}
pub(super) fn frame(image: &PaneGraphicsFrame, maximum: usize) -> Result<(), ContractError> {
    let rgba = graphics_rgba_bytes(image.width, image.height)?;
    require(image.data_base64.len() <= maximum.div_ceil(3).saturating_mul(4))?;
    let bytes = base64_bytes(&image.data_base64)?;
    require(bytes <= maximum)?;
    match image.format {
        PaneGraphicsFormat::Png => require(bytes >= 33),
        PaneGraphicsFormat::Rgb => require(bytes == rgba / 4 * 3),
        PaneGraphicsFormat::Rgba | PaneGraphicsFormat::Bgra => require(bytes == rgba),
    }
}
pub(super) fn request(request: &PaneGraphicsRequest) -> Result<(), ContractError> {
    use PaneGraphicsAction as A;
    match &request.action {
        A::Observe { observation, .. } => {
            capabilities(&observation.capabilities)?;
            require(
                observation.panes.len() <= 64
                    && observation
                        .panes
                        .iter()
                        .map(|pane| &pane.target.panel_id)
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        == observation.panes.len(),
            )?;
            for pane in &observation.panes {
                resolved(&pane.target)?;
            }
            Ok(())
        }
        A::Info { .. } | A::Catalog { .. } | A::StreamClose { .. } => Ok(()),
        A::Set {
            layer_id, image, ..
        } => {
            require(graphics_layer_id(layer_id))?;
            frame(image, MAXIMUM_GRAPHICS_SET_BYTES)
        }
        A::Clear { layer_id, .. } | A::StreamOpen { layer_id, .. } => {
            require(graphics_layer_id(layer_id))
        }
        A::Pixels {
            layer_id,
            resource_revision,
            offset,
            maximum_bytes,
            ..
        } => require(
            graphics_layer_id(layer_id)
                && resource_revision.value() > 0
                && offset.is_multiple_of(4)
                && *maximum_bytes > 0
                && *maximum_bytes as usize <= MAXIMUM_GRAPHICS_CHUNK_BYTES
                && maximum_bytes.is_multiple_of(4),
        ),
        A::StreamFrame {
            sequence,
            image,
            damage,
            ..
        } => {
            require(sequence.value() > 0 && damage.len() <= 64)?;
            let (width, height) = match image {
                PaneGraphicsStreamImage::Inline { image } => {
                    frame(image, MAXIMUM_GRAPHICS_STREAM_BYTES)?;
                    (image.width, image.height)
                }
                PaneGraphicsStreamImage::File {
                    format,
                    width,
                    height,
                    path,
                    ..
                } => {
                    require(matches!(
                        format,
                        PaneGraphicsFormat::Rgba | PaneGraphicsFormat::Bgra
                    ))?;
                    require(
                        graphics_rgba_bytes(*width, *height)? <= MAXIMUM_GRAPHICS_STREAM_BYTES,
                    )?;
                    file_path(path)?;
                    (*width, *height)
                }
            };
            require(damage.iter().all(|d| {
                d.width > 0
                    && d.height > 0
                    && u64::from(d.x) + u64::from(d.width) <= u64::from(width)
                    && u64::from(d.y) + u64::from(d.height) <= u64::from(height)
            }))
        }
    }
}
pub(super) fn result(reply: &PaneGraphicsResult) -> Result<(), ContractError> {
    use PaneGraphicsOutcome as O;
    match &reply.outcome {
        O::Failed { .. } | O::Closed { .. } | O::Observed { .. } => Ok(()),
        O::Info {
            target,
            revision,
            layer_count,
            client_id,
            capabilities,
            pane_visible,
            drawn_revision,
            file_frame_directory,
        } => {
            resolved(target)?;
            require(
                *layer_count as usize <= MAXIMUM_GRAPHICS_LAYERS_PER_PANE
                    && drawn_revision.is_none_or(|drawn| drawn <= *revision),
            )?;
            require(
                client_id.is_some()
                    || capabilities.is_none() && pane_visible.is_none() && drawn_revision.is_none(),
            )?;
            if let Some(caps) = capabilities {
                self::capabilities(caps)?;
            }
            if let Some(path) = file_frame_directory {
                file_path(path)?;
            }
            Ok(())
        }
        O::Catalog {
            target,
            revision,
            layers,
        } => {
            resolved(target)?;
            require(
                layers.len() <= MAXIMUM_GRAPHICS_LAYERS_PER_PANE
                    && layers
                        .iter()
                        .map(|layer| &layer.layer_id)
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        == layers.len(),
            )?;
            let mut bytes = 0usize;
            for layer in layers {
                require(
                    graphics_layer_id(&layer.layer_id)
                        && layer.resource_revision.value() > 0
                        && layer.resource_revision <= *revision,
                )?;
                bytes = bytes
                    .checked_add(graphics_rgba_bytes(layer.width, layer.height)?)
                    .ok_or(ContractError::InvalidDocument)?;
            }
            require(bytes <= MAXIMUM_GRAPHICS_RESIDENT_BYTES)
        }
        O::Pixels {
            target,
            layer_id,
            resource_revision,
            width,
            height,
            offset,
            total_bytes,
            data_base64,
            complete,
        } => {
            resolved(target)?;
            let total = graphics_rgba_bytes(*width, *height)?;
            require(
                total == *total_bytes as usize
                    && graphics_layer_id(layer_id)
                    && resource_revision.value() > 0
                    && offset.is_multiple_of(4)
                    && data_base64.len() <= MAXIMUM_GRAPHICS_CHUNK_BYTES.div_ceil(3) * 4,
            )?;
            let bytes = base64_bytes(data_base64)?;
            let end = (*offset as usize)
                .checked_add(bytes)
                .ok_or(ContractError::InvalidDocument)?;
            require(
                bytes <= MAXIMUM_GRAPHICS_CHUNK_BYTES
                    && bytes.is_multiple_of(4)
                    && end <= total
                    && *complete == (end == total),
            )
        }
        O::Applied {
            target,
            layer_id,
            revision,
            changed,
        } => {
            resolved(target)?;
            require(graphics_layer_id(layer_id) && (!changed || revision.value() > 0))
        }
        O::StreamOpened {
            target,
            layer_id,
            revision,
            file_frame_directory,
            ..
        } => {
            resolved(target)?;
            require(graphics_layer_id(layer_id) && revision.value() > 0)?;
            if let Some(path) = file_frame_directory {
                file_path(path)?;
            }
            Ok(())
        }
        O::FrameApplied {
            target,
            layer_id,
            sequence,
            revision,
            ..
        } => {
            resolved(target)?;
            require(graphics_layer_id(layer_id) && sequence.value() > 0 && revision.value() > 0)
        }
    }
}
fn capabilities(caps: &super::PaneGraphicsCapabilities) -> Result<(), ContractError> {
    require(
        caps.cell_pixels
            .is_none_or(|cell| cell.width > 0 && cell.height > 0),
    )?;
    require(caps.transport != PaneGraphicsTransport::CellRaster || !caps.pixel_mouse)?;
    require(!caps.pixel_mouse || caps.cell_pixels.is_some())
}
fn target_matches(actual: &PaneGraphicsTarget, requested: &PaneGraphicsTarget) -> bool {
    actual.panel_id == requested.panel_id
        && requested
            .presentation_id
            .as_ref()
            .is_none_or(|id| Some(id) == actual.presentation_id.as_ref())
}
pub(super) fn answers(reply: &PaneGraphicsResult, request: &PaneGraphicsRequest) -> bool {
    if reply.session_id != request.session_id
        || reply.request_id != request.request_id
        || reply.validate().is_err()
        || request.validate().is_err()
    {
        return false;
    }
    use PaneGraphicsAction as A;
    use PaneGraphicsOutcome as O;
    {
        let shared_failure = matches!(&reply.outcome, O::Failed { .. });
        match &request.action {
            A::Observe {
                client_id,
                heartbeat,
                ..
            } => {
                matches!(&reply.outcome, O::Observed {
                client_id: actual,
                heartbeat: seen,
            } if actual == client_id && seen == heartbeat)
                    || shared_failure
            }
            A::Info { target, client_id } => {
                matches!(&reply.outcome, O::Info {
                        target: actual,
                        client_id: actual_client,
                        ..
                    } if target_matches(actual, target)
                        && client_id
                            .as_ref()
                            .is_none_or(|id| Some(id) == actual_client.as_ref()))
                    || shared_failure
            }
            A::Catalog { target } => {
                matches!(&reply.outcome, O::Catalog { target: actual, .. } if target_matches(actual, target))
                    || shared_failure
            }
            A::Pixels {
                target,
                layer_id,
                resource_revision,
                offset,
                maximum_bytes,
            } => {
                matches!(&reply.outcome, O::Pixels {
                        target: actual,
                        layer_id: layer,
                        resource_revision: revision,
                        offset: start,
                        data_base64,
                        ..
                    } if target_matches(actual, target)
                        && layer == layer_id
                        && revision == resource_revision
                        && start == offset
                        && base64_bytes(data_base64).is_ok_and(|bytes| bytes <= *maximum_bytes as usize))
                    || shared_failure
            }
            A::Set {
                target, layer_id, ..
            } => {
                matches!(&reply.outcome, O::Applied {
                target: actual,
                layer_id: layer,
                ..
            } if target_matches(actual, target) && layer == layer_id)
                    || shared_failure
            }
            A::Clear { target, layer_id } => {
                matches!(&reply.outcome, O::Applied {
                target: actual,
                layer_id: layer,
                ..
            } if target_matches(actual, target) && layer == layer_id)
                    || shared_failure
            }
            A::StreamOpen {
                stream_id,
                target,
                layer_id,
                z_index,
            } => {
                matches!(&reply.outcome, O::StreamOpened {
                        stream_id: actual_stream,
                        target: actual,
                        layer_id: layer,
                        z_index: z,
                        ..
                    } if actual_stream == stream_id
                        && target_matches(actual, target)
                        && layer == layer_id
                        && z == z_index)
                    || shared_failure
            }
            A::StreamFrame {
                stream_id,
                sequence,
                revision,
                ..
            } => {
                matches!(&reply.outcome, O::FrameApplied {
                        stream_id: actual_stream,
                        sequence: actual_sequence,
                        frame_revision,
                        ..
                    } if actual_stream == stream_id && sequence == actual_sequence && revision == frame_revision)
                    || shared_failure
            }
            A::StreamClose { stream_id } => {
                matches!(&reply.outcome, O::Closed {
                stream_id: actual, ..
            } if actual == stream_id)
                    || shared_failure
            }
        }
    }
}
