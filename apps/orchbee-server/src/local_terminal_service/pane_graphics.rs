//! Serial graphics preparation; canonical lock is held only for membership and atomic commit.
use super::{LocalTerminalService, viewer_session::process_sampling::screen};
use orchbee_server::{DecodedGraphicsImage, PaneGraphicsAuthority, PaneGraphicsFrameCommit};
use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_SET_BYTES, MAXIMUM_GRAPHICS_STREAM_BYTES, PaneGraphicsAction as Action,
    PaneGraphicsFailure as Failure, PaneGraphicsOutcome as Outcome, PaneGraphicsRequest,
    PaneGraphicsResult, PaneGraphicsStreamImage, SemanticEventSource, SemanticEventsFailure,
    SemanticPane, SiteId, Snapshot,
};
use std::{num::NonZeroU64, sync::atomic::AtomicBool};

impl LocalTerminalService {
    fn graphics_snapshot<T>(
        &self,
        site: &SiteId,
        use_snapshot: impl FnOnce(&Snapshot, &mut PaneGraphicsAuthority) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let coordinator = self.coordinator.lock().map_err(|_| Failure::Unavailable)?;
        let snapshot = coordinator.graphics_snapshot(site)?;
        let owner = coordinator.graphics_owner().ok_or(Failure::Unavailable)?;
        let mut layers = owner.lock().map_err(|_| Failure::Unavailable)?;
        use_snapshot(&snapshot, &mut layers)
    }
    pub(crate) fn execute_graphics(
        &self,
        connection: NonZeroU64,
        request: PaneGraphicsRequest,
    ) -> PaneGraphicsResult {
        let outcome = self
            .apply_graphics(connection, &request)
            .unwrap_or_else(|reason| Outcome::Failed { reason });
        PaneGraphicsResult {
            protocol_version: request.protocol_version,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome,
        }
    }
    fn apply_graphics(
        &self,
        connection: NonZeroU64,
        request: &PaneGraphicsRequest,
    ) -> Result<Outcome, Failure> {
        request.validate().map_err(|_| Failure::InvalidIntent)?;
        if self.workbench_site_matches(&request.site_id) != Some(true) {
            return Err(Failure::SiteMismatch);
        }
        let _preparation = if request.action.writes() {
            Some(
                self.graphics_preparation
                    .try_lock()
                    .map_err(|_| Failure::Busy)?,
            )
        } else {
            None
        };
        let site = &request.site_id;
        match &request.action {
            Action::Info { target, .. } => {
                let Outcome::Catalog {
                    target,
                    revision,
                    layers: catalog,
                } = self
                    .graphics_snapshot(site, |snapshot, layers| layers.catalog(snapshot, target))?
                else {
                    return Err(Failure::Unavailable);
                };
                // Renderer facts are supplied only by an actual Client publication.
                Ok(Outcome::Info {
                    target,
                    revision,
                    layer_count: catalog.len() as u16,
                    client_id: None,
                    capabilities: None,
                    pane_visible: None,
                    drawn_revision: None,
                    file_frame_directory: None,
                })
            }
            Action::Catalog { target } => {
                self.graphics_snapshot(site, |snapshot, layers| layers.catalog(snapshot, target))
            }
            Action::Pixels {
                target,
                layer_id,
                resource_revision,
                offset,
                maximum_bytes,
            } => self.graphics_snapshot(site, |snapshot, layers| {
                layers.pixels(
                    snapshot,
                    target,
                    layer_id,
                    *resource_revision,
                    *offset,
                    *maximum_bytes,
                )
            }),
            Action::Set {
                target,
                layer_id,
                z_index,
                image,
            } => {
                let source = self.graphics_snapshot(site, |snapshot, _layers| {
                    PaneGraphicsAuthority::resolve(snapshot, target)
                })?;
                let decoded = DecodedGraphicsImage::decode(
                    image,
                    MAXIMUM_GRAPHICS_SET_BYTES,
                    |png, width, height| {
                        self.decode_graphics_png(site, &source, png, width, height)
                    },
                )?;
                self.graphics_snapshot(site, |snapshot, layers| {
                    let current = PaneGraphicsAuthority::resolve(snapshot, target)?;
                    if current.presentation_id != source.presentation_id
                        || current.binding != source.binding
                    {
                        return Err(Failure::SourceChanged);
                    }
                    layers.set(snapshot, &source, layer_id, *z_index, decoded)
                })
            }
            Action::Clear { target, layer_id } => self
                .graphics_snapshot(site, |snapshot, layers| {
                    layers.clear(snapshot, target, layer_id)
                }),
            Action::StreamOpen {
                stream_id,
                target,
                layer_id,
                z_index,
            } => self.graphics_snapshot(site, |snapshot, layers| {
                layers.open(snapshot, connection, stream_id, target, layer_id, *z_index)
            }),
            Action::StreamFrame {
                stream_id,
                sequence,
                revision,
                image,
                ..
            } => {
                let source = self.graphics_snapshot(site, |snapshot, layers| {
                    layers.producer_source(snapshot, connection, stream_id, *sequence)
                })?;
                let PaneGraphicsStreamImage::Inline { image } = image else {
                    return Err(Failure::InvalidFile);
                };
                let decoded = DecodedGraphicsImage::decode(
                    image,
                    MAXIMUM_GRAPHICS_STREAM_BYTES,
                    |png, width, height| {
                        self.decode_graphics_png(site, &source, png, width, height)
                    },
                )?;
                self.graphics_snapshot(site, |snapshot, layers| {
                    layers.frame(
                        snapshot,
                        PaneGraphicsFrameCommit {
                            connection,
                            stream_id,
                            source: &source,
                            sequence: *sequence,
                            frame_revision: *revision,
                            image: decoded,
                        },
                    )
                })
            }
            Action::StreamClose { stream_id } => self
                .graphics_snapshot(site, |_snapshot, layers| {
                    layers.close(connection, stream_id)
                }),
        }
    }
    pub(crate) fn disconnect_graphics(&self, connection: NonZeroU64) {
        let coordinator = self
            .coordinator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(owner) = coordinator.graphics_owner() {
            owner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .disconnect(connection);
        }
    }
    fn decode_graphics_png(
        &self,
        site: &SiteId,
        source: &SemanticPane,
        png: &[u8],
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, Failure> {
        let selector = SemanticEventSource {
            panel_id: source.panel_id.clone(),
            presentation_id: Some(source.presentation_id.clone()),
        };
        let before = self.event_source(site, &selector).map_err(source_failure)?;
        if before.0.binding != source.binding {
            return Err(Failure::SourceChanged);
        }
        let mut pending = self.observer.claim(self).map_err(|reason| match reason {
            super::observation::Failure::Busy => Failure::Busy,
            super::observation::Failure::PhysicalUnknown => Failure::PhysicalUnknown,
        })?;
        let stop = AtomicBool::new(false);
        let mut result = Err(Failure::SourceUnavailable);
        *pending = self.with_observation_viewer(&before.2, &stop, |viewer, template| {
            result = screen::graphics_png(viewer, template, png, width, height);
        });
        if pending.is_some() {
            return Err(Failure::PhysicalUnknown);
        }
        let after = self.event_source(site, &selector).map_err(source_failure)?;
        if before.2 != after.2 || after.0.binding != source.binding {
            return Err(Failure::SourceChanged);
        }
        result
    }
}
fn source_failure(reason: SemanticEventsFailure) -> Failure {
    match reason {
        SemanticEventsFailure::Busy => Failure::Busy,
        SemanticEventsFailure::PhysicalUnknown => Failure::PhysicalUnknown,
        SemanticEventsFailure::SourceChanged => Failure::SourceChanged,
        _ => Failure::SourceUnavailable,
    }
}
