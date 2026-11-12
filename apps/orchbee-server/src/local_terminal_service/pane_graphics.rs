//! Serial graphics preparation; canonical lock is held only for membership and atomic commit.
use super::{LocalTerminalService, viewer_connection::process_sampling::screen};
use orchbee_server::{
    DecodedGraphicsImage, PaneGraphicsAuthority, PaneGraphicsFrameCommit, PaneGraphicsOpen,
    PaneGraphicsOpenPreparation,
};
use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_SET_BYTES, MAXIMUM_GRAPHICS_STREAM_BYTES, PaneGraphicsAction as Action,
    PaneGraphicsFailure as Failure, PaneGraphicsOutcome as Outcome, PaneGraphicsRequest,
    PaneGraphicsResult, PaneGraphicsStreamImage, SemanticEventSource, SemanticEventsFailure,
    SemanticPane, SessionId, Snapshot,
};
use std::{
    collections::BTreeSet,
    io,
    num::NonZeroU64,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
mod files;
pub(super) use files::GraphicsFiles;

impl LocalTerminalService {
    fn graphics_snapshot<T>(
        &self,
        session: &SessionId,
        use_snapshot: impl FnOnce(&Snapshot, &mut PaneGraphicsAuthority) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let coordinator = self.coordinator.lock().map_err(|_| Failure::Unavailable)?;
        let snapshot = coordinator.graphics_snapshot(session)?;
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
        if request.action.writes() {
            self.close_maintenance.notify();
        }
        PaneGraphicsResult {
            protocol_version: request.protocol_version,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }
    }
    fn apply_graphics(
        &self,
        connection: NonZeroU64,
        request: &PaneGraphicsRequest,
    ) -> Result<Outcome, Failure> {
        request.validate().map_err(|_| Failure::InvalidIntent)?;
        if self.workbench_session_matches(&request.session_id) != Some(true) {
            return Err(Failure::SessionMismatch);
        }
        let _preparation = if request.action.writes() {
            if self.graphics_stopping.load(Ordering::Acquire) {
                return Err(Failure::Unavailable);
            }
            Some(
                self.graphics_preparation
                    .try_lock()
                    .map_err(|_| Failure::Busy)?,
            )
        } else {
            None
        };
        if request.action.writes() && self.graphics_stopping.load(Ordering::Acquire) {
            return Err(Failure::Unavailable);
        }
        let session = &request.session_id;
        match &request.action {
            Action::Observe { .. } => Err(Failure::InvalidIntent),
            Action::Info { target, .. } => {
                let (catalog, file_frame_directory) =
                    self.graphics_snapshot(session, |snapshot, layers| {
                        let catalog = layers.catalog(snapshot, target)?;
                        let directory =
                            layers.connection_file_directory(snapshot, target, connection)?;
                        Ok((catalog, directory))
                    })?;
                let Outcome::Catalog {
                    target,
                    revision,
                    layers: catalog,
                } = catalog
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
                    panel_visible: None,
                    drawn_revision: None,
                    file_frame_directory,
                })
            }
            Action::Catalog { target } => {
                self.graphics_snapshot(session, |snapshot, layers| layers.catalog(snapshot, target))
            }
            Action::Pixels {
                target,
                layer_id,
                resource_revision,
                offset,
                maximum_bytes,
            } => self.graphics_snapshot(session, |snapshot, layers| {
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
                let source = self.graphics_snapshot(session, |snapshot, _layers| {
                    PaneGraphicsAuthority::resolve(snapshot, target)
                })?;
                let decoded = DecodedGraphicsImage::decode(
                    image,
                    MAXIMUM_GRAPHICS_SET_BYTES,
                    |png, width, height| {
                        self.decode_graphics_png(session, &source, png, width, height)
                    },
                )?;
                self.graphics_snapshot(session, |snapshot, layers| {
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
                .graphics_snapshot(session, |snapshot, layers| {
                    layers.clear(snapshot, target, layer_id)
                }),
            Action::StreamOpen {
                stream_id,
                target,
                layer_id,
                z_index,
            } => {
                let intent = PaneGraphicsOpen {
                    connection,
                    stream_id,
                    target,
                    layer_id,
                    z_index: *z_index,
                };
                let source = match self.graphics_snapshot(session, |snapshot, layers| {
                    layers.prepare_open(snapshot, &intent)
                })? {
                    PaneGraphicsOpenPreparation::Existing(outcome) => return Ok(outcome),
                    PaneGraphicsOpenPreparation::Ready(source) => source,
                };
                let lease = self
                    .graphics_files
                    .as_ref()
                    .map(|files| files.create(session, &source, connection, stream_id))
                    .transpose()?;
                let directory = lease
                    .as_ref()
                    .map(|lease| {
                        lease
                            .directory()
                            .map(|path| path.to_string_lossy().into_owned())
                    })
                    .transpose()?;
                self.graphics_snapshot(session, |snapshot, layers| {
                    layers.commit_open(snapshot, &intent, &source, directory)
                })
            }
            Action::StreamFrame {
                stream_id,
                sequence,
                revision,
                image,
                ..
            } => {
                let (source, directory) = self.graphics_snapshot(session, |snapshot, layers| {
                    let source =
                        layers.producer_source(snapshot, connection, stream_id, *sequence)?;
                    Ok((
                        source,
                        layers.producer_file_directory(connection, stream_id)?,
                    ))
                })?;
                let decoded = match image {
                    PaneGraphicsStreamImage::Inline { image } => DecodedGraphicsImage::decode(
                        image,
                        MAXIMUM_GRAPHICS_STREAM_BYTES,
                        |png, width, height| {
                            self.decode_graphics_png(session, &source, png, width, height)
                        },
                    )?,
                    PaneGraphicsStreamImage::File {
                        format,
                        width,
                        height,
                        path,
                        placement,
                    } => {
                        let directory = directory.ok_or(Failure::InvalidFile)?;
                        let files = self.graphics_files.as_ref().ok_or(Failure::InvalidFile)?;
                        let lease = files.lease(&directory)?;
                        let bytes =
                            orchbee_workbench_protocol::graphics_rgba_bytes(*width, *height)
                                .map_err(|_| Failure::InvalidImage)?;
                        let pixels = lease.read(Path::new(path), bytes)?;
                        DecodedGraphicsImage::from_file_pixels(
                            *format, *width, *height, *placement, pixels,
                        )?
                    }
                };
                self.graphics_snapshot(session, |snapshot, layers| {
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
                .graphics_snapshot(session, |_snapshot, layers| {
                    layers.close(connection, stream_id)
                }),
        }
    }
    pub(crate) fn validate_graphics_observation(
        &self,
        session: &SessionId,
        observation: &orchbee_workbench_protocol::PaneGraphicsClientObservation,
    ) -> Result<(), Failure> {
        self.graphics_snapshot(session, |snapshot, layers| {
            if observation.workbench_revision > snapshot.revision {
                return Err(Failure::SourceChanged);
            }
            for pane in &observation.panes {
                let current = PaneGraphicsAuthority::resolve(snapshot, &pane.target)?;
                let revision = layers.revision()?;
                if current.binding != pane.binding
                    || pane.drawn_revision.is_some_and(|drawn| drawn > revision)
                {
                    return Err(Failure::SourceChanged);
                }
            }
            Ok(())
        })
    }
    pub(crate) fn graphics_source(
        &self,
        session: &SessionId,
        target: &orchbee_workbench_protocol::PaneGraphicsTarget,
    ) -> Result<SemanticPane, Failure> {
        self.graphics_snapshot(session, |snapshot, _| {
            PaneGraphicsAuthority::resolve(snapshot, target)
        })
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
        drop(coordinator);
        self.close_maintenance.notify();
    }
    /// Enables private file frames only for the managed, already admitted runtime; creation stays lazy.
    pub(crate) fn configure_graphics_files(&mut self, runtime: &Path) {
        self.graphics_files = Some(GraphicsFiles::new(
            runtime,
            self.close_maintenance.release_waker(),
        ));
    }
    fn graphics_directories(&self) -> io::Result<BTreeSet<String>> {
        let coordinator = self
            .coordinator
            .lock()
            .map_err(|_| io::Error::other("graphics canonical owner poisoned"))?;
        let owner = coordinator
            .graphics_owner()
            .ok_or_else(|| io::Error::other("graphics canonical owner unavailable"))?;
        let directories = owner
            .lock()
            .map_err(|_| io::Error::other("graphics layer owner poisoned"))?
            .retained_file_directories();
        Ok(directories.into_iter().collect())
    }
    /// Retires filesystem resources outside canonical locks; unavailable source facts preserve all roots.
    pub(super) fn graphics_maintenance_turn(&self) -> Option<Instant> {
        let files = self.graphics_files.as_ref()?;
        let pending = self
            .graphics_directories()
            .and_then(|active| files.reap(&active))
            .unwrap_or(true);
        pending.then(|| Instant::now() + Duration::from_millis(250))
    }
    /// Settles all preparing/read tokens and owned directories before managed clean-stop publication.
    #[cfg(windows)]
    pub(crate) fn stop_graphics_before(&self, deadline: Instant) -> io::Result<()> {
        self.graphics_stopping.store(true, Ordering::Release);
        loop {
            match self.graphics_preparation.try_lock() {
                Ok(preparation) => {
                    let coordinator = self
                        .coordinator
                        .lock()
                        .map_err(|_| io::Error::other("graphics canonical owner poisoned"))?;
                    if let Some(owner) = coordinator.graphics_owner() {
                        owner
                            .lock()
                            .map_err(|_| io::Error::other("graphics layer owner poisoned"))?
                            .retire_streams();
                    }
                    drop(coordinator);
                    drop(preparation);
                    break;
                }
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    return Err(io::Error::other("graphics preparation owner poisoned"));
                }
                Err(std::sync::TryLockError::WouldBlock) => {}
            }
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let Some(files) = &self.graphics_files else {
            return Ok(());
        };
        loop {
            if files.reap(&BTreeSet::new()).is_ok_and(|pending| !pending) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn decode_graphics_png(
        &self,
        session: &SessionId,
        source: &SemanticPane,
        png: &[u8],
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, Failure> {
        let selector = SemanticEventSource {
            panel_id: source.panel_id.clone(),
            presentation_id: Some(source.presentation_id.clone()),
        };
        let before = self
            .event_source(session, &selector)
            .map_err(source_failure)?;
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
        let after = self
            .event_source(session, &selector)
            .map_err(source_failure)?;
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
        SemanticEventsFailure::Denied
        | SemanticEventsFailure::InvalidIntent
        | SemanticEventsFailure::InvalidRegex
        | SemanticEventsFailure::SessionMismatch
        | SemanticEventsFailure::Unavailable
        | SemanticEventsFailure::Conflict
        | SemanticEventsFailure::NotFound
        | SemanticEventsFailure::CursorEpoch
        | SemanticEventsFailure::CursorAhead
        | SemanticEventsFailure::SourceUnavailable
        | SemanticEventsFailure::RevisionExhausted => Failure::SourceUnavailable,
    }
}
