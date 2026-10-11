//! Whole revision-pinned resources are staged off the UI/control connections.
#[cfg(test)]
mod tests;
use super::{
    Arc, AtomicBool, Instant, LaneFailure, LocalClient, LocalClientOrigin, LocalOperationClock,
    Ordering, mint_request_id,
};
use crate::terminal_session::pane_graphics::{BATCH, Canvas, Job, Layer, Round, Source};
use base64::Engine;
use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_CHUNK_BYTES, MAXIMUM_GRAPHICS_LAYERS, MAXIMUM_GRAPHICS_RESIDENT_BYTES,
    PaneGraphicsAction as Action, PaneGraphicsLayer, PaneGraphicsOutcome as Outcome,
    PaneGraphicsRequest, ProtocolVersion, Snapshot, SnapshotReadIntent, WorkbenchRequestId,
    WorkbenchRevision, WorkbenchSnapshotOutcome, graphics_rgba_bytes,
};
use std::collections::BTreeMap;
pub(super) fn read_round(
    origin: &LocalClientOrigin,
    clock: &LocalOperationClock,
    client_slot: &mut Option<LocalClient>,
    job: Job,
    deadline: Instant,
    wake: orchbee_local_ipc::LocalIo<'_>,
    shutdown: &Arc<AtomicBool>,
) -> Result<Round, LaneFailure> {
    if client_slot.is_none() {
        *client_slot = Some(
            origin
                .reconnect_before_cancellable(deadline, clock.clone(), Some(wake))
                .map_err(|_| LaneFailure::Transport)?,
        );
    }
    let client = client_slot.as_mut().ok_or(LaneFailure::Transport)?;
    let result = Reader {
        client,
        clock,
        deadline,
        wake,
        shutdown,
        job: &job,
    }
    .read();
    if result.is_err() {
        *client_slot = None;
    }
    result.map(|rows| Round {
        scope: job.scope,
        rows,
    })
}
struct Reader<'a> {
    client: &'a mut LocalClient,
    clock: &'a LocalOperationClock,
    deadline: Instant,
    wake: orchbee_local_ipc::LocalIo<'a>,
    shutdown: &'a Arc<AtomicBool>,
    job: &'a Job,
}
impl Reader<'_> {
    fn id(&self) -> Result<WorkbenchRequestId, LaneFailure> {
        if self.shutdown.load(Ordering::SeqCst) || self.clock.now() >= self.deadline {
            return Err(LaneFailure::Transport);
        }
        WorkbenchRequestId::parse(mint_request_id()?.as_str()).map_err(|_| LaneFailure::Transport)
    }
    fn snapshot(&mut self) -> Result<Snapshot, LaneFailure> {
        let request = SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: self.id()?,
        };
        let reply = self
            .client
            .submit_workbench_snapshot_read_before_cancellable(
                request,
                self.deadline,
                Some(self.wake),
            )
            .map_err(|_| LaneFailure::Transport)?;
        let WorkbenchSnapshotOutcome::Available { snapshot } = reply.outcome else {
            return Err(LaneFailure::Unavailable);
        };
        if snapshot.site_id != self.job.scope.site
            || snapshot.revision < self.job.scope.revision
            || !self.job.scope.sources.iter().all(|s| s.matches(&snapshot))
        {
            return Err(LaneFailure::Unavailable);
        }
        Ok(snapshot)
    }
    fn exchange(&mut self, action: Action) -> Result<Outcome, LaneFailure> {
        let request = PaneGraphicsRequest {
            protocol_version: ProtocolVersion,
            request_id: self.id()?,
            site_id: self.job.scope.site.clone(),
            action,
        };
        let reply = self
            .client
            .pane_graphics(&request, self.deadline, Some(self.wake))
            .map_err(|_| LaneFailure::Transport)?;
        match reply.outcome {
            Outcome::Failed {
                reason: orchbee_workbench_protocol::PaneGraphicsFailure::Denied,
            } => Err(LaneFailure::Denied),
            Outcome::Failed { .. } => Err(LaneFailure::Unavailable),
            outcome => Ok(outcome),
        }
    }
    fn catalog(
        &mut self,
        source: &Source,
    ) -> Result<(WorkbenchRevision, Vec<PaneGraphicsLayer>), LaneFailure> {
        let Outcome::Catalog {
            target,
            revision,
            layers,
        } = self.exchange(Action::Catalog {
            target: source.target.clone(),
        })?
        else {
            return Err(LaneFailure::Transport);
        };
        if target != source.target {
            return Err(LaneFailure::Transport);
        }
        Ok((revision, layers))
    }
    fn resource(
        &mut self,
        source: &Source,
        metadata: &PaneGraphicsLayer,
    ) -> Result<Layer, LaneFailure> {
        if let Some(layer) = self
            .job
            .previous
            .get(&source.target.panel_id)
            .filter(|canvas| canvas.source == *source)
            .and_then(|canvas| canvas.layers().iter().find(|l| l.metadata() == metadata))
        {
            return Ok(layer.clone());
        }
        let total = graphics_rgba_bytes(metadata.width, metadata.height)
            .map_err(|_| LaneFailure::Transport)?;
        let mut pixels = Vec::with_capacity(total);
        while pixels.len() < total {
            let offset = u32::try_from(pixels.len()).map_err(|_| LaneFailure::Transport)?;
            let Outcome::Pixels {
                target,
                layer_id,
                resource_revision,
                width,
                height,
                offset: returned,
                total_bytes,
                data_base64,
                complete,
            } = self.exchange(Action::Pixels {
                target: source.target.clone(),
                layer_id: metadata.layer_id.clone(),
                resource_revision: metadata.resource_revision,
                offset,
                maximum_bytes: MAXIMUM_GRAPHICS_CHUNK_BYTES as u32,
            })?
            else {
                return Err(LaneFailure::Transport);
            };
            if target != source.target
                || layer_id != metadata.layer_id
                || resource_revision != metadata.resource_revision
                || width != metadata.width
                || height != metadata.height
                || returned != offset
                || total_bytes as usize != total
            {
                return Err(LaneFailure::Transport);
            }
            let chunk = base64::engine::general_purpose::STANDARD
                .decode(data_base64)
                .map_err(|_| LaneFailure::Transport)?;
            if chunk.is_empty()
                || chunk.len() > MAXIMUM_GRAPHICS_CHUNK_BYTES
                || pixels
                    .len()
                    .checked_add(chunk.len())
                    .is_none_or(|end| end > total)
                || complete != (pixels.len() + chunk.len() == total)
            {
                return Err(LaneFailure::Transport);
            }
            pixels.extend(chunk);
        }
        Layer::new(metadata.clone(), pixels).ok_or(LaneFailure::Transport)
    }
    fn read(
        &mut self,
    ) -> Result<BTreeMap<orchbee_workbench_protocol::WorkbenchPanelId, Canvas>, LaneFailure> {
        if self.job.scope.sources.len() > BATCH {
            return Err(LaneFailure::Unavailable);
        }
        self.snapshot()?;
        let mut rows = BTreeMap::new();
        let mut bytes = 0;
        let mut resources = 0;
        for source in &self.job.scope.sources {
            let (_, metadata) = self.catalog(source)?;
            let mut layers = Vec::new();
            for layer in &metadata {
                bytes += graphics_rgba_bytes(layer.width, layer.height)
                    .map_err(|_| LaneFailure::Transport)?;
                resources += 1;
                if bytes > MAXIMUM_GRAPHICS_RESIDENT_BYTES || resources > MAXIMUM_GRAPHICS_LAYERS {
                    return Err(LaneFailure::Unavailable);
                }
                layers.push(self.resource(source, layer)?);
            }
            let (after, after_layers) = self.catalog(source)?;
            if metadata != after_layers {
                return Err(LaneFailure::Unavailable);
            }
            let canvas =
                Canvas::new(source.clone(), after, layers).ok_or(LaneFailure::Transport)?;
            rows.insert(source.target.panel_id.clone(), canvas);
        }
        self.snapshot()?;
        Ok(rows)
    }
}
