//! Resolves one frame's decoded Kitty update against the attachment's retained resources. The
//! retained set is always the Host's complete current set: a snapshot replaces it atomically,
//! so an image that left the Host's visible set is released at once.

use orchbee_client_server_protocol::ViewerSnapshotReason as Reason;
use orchbee_terminal_protocol::{TerminalGraphics, TerminalGraphicsUpdateKind};
use std::sync::Arc;

pub(super) enum Resolved {
    Unchanged,
    Replaced(Option<Arc<TerminalGraphics>>),
}

/// A delta without graphics keeps the retained set; a snapshot without graphics has none.
pub(super) fn resolve(
    retained: Option<&TerminalGraphics>,
    update: Option<TerminalGraphics>,
    delta: bool,
) -> Result<Resolved, Reason> {
    let Some(mut update) = update else {
        return Ok(if delta {
            Resolved::Unchanged
        } else {
            Resolved::Replaced(None)
        });
    };
    match (delta, update.kind) {
        (false, TerminalGraphicsUpdateKind::Snapshot) => {
            // An unchanged Host identity keeps its shared pixels, so renderers see one resource.
            for image in &mut update.images {
                if let Some(old) = retained
                    .and_then(|retained| retained.image(image.id))
                    .filter(|old| old.same_resource(image))
                {
                    image.pixels = Arc::clone(&old.pixels);
                }
            }
            Ok(Resolved::Replaced(Some(Arc::new(update))))
        }
        (true, TerminalGraphicsUpdateKind::Placements) => {
            // No retained set is the empty set at revision zero.
            let (revision, tick, images) = retained.map_or((0, 0, &[][..]), |retained| {
                (
                    retained.resource_revision,
                    retained.animation_tick,
                    retained.images.as_slice(),
                )
            });
            if update.resource_revision != revision || update.animation_tick < tick {
                return Err(Reason::BaseMismatch);
            }
            for placement in &update.placements {
                let image = images
                    .iter()
                    .find(|image| image.id == placement.image_id)
                    .ok_or(Reason::BaseMismatch)?;
                if u64::from(placement.source_x) + u64::from(placement.source_width)
                    > u64::from(image.width)
                    || u64::from(placement.source_y) + u64::from(placement.source_height)
                        > u64::from(image.height)
                {
                    return Err(Reason::DecoderRejected);
                }
            }
            update.kind = TerminalGraphicsUpdateKind::Snapshot;
            update.images = images.to_vec();
            Ok(Resolved::Replaced(Some(Arc::new(update))))
        }
        // The Host promotes any resource change to a terminal snapshot; the decoder already
        // refused a placement-only update inside one.
        (true, TerminalGraphicsUpdateKind::Snapshot)
        | (false, TerminalGraphicsUpdateKind::Placements) => Err(Reason::DecoderRejected),
    }
}
