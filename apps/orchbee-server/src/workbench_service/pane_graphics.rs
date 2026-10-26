use super::WorkbenchClientConnection;
use orchbee_workbench_protocol::{
    PaneGraphicsAction as Action, PaneGraphicsFailure as Failure, PaneGraphicsOutcome as Outcome,
    PaneGraphicsRequest, PaneGraphicsResult,
};

impl WorkbenchClientConnection<'_> {
    pub(super) fn pane_graphics(&self, request: PaneGraphicsRequest) -> PaneGraphicsResult {
        let outcome = self
            .graphics_outcome(&request)
            .unwrap_or_else(|reason| Outcome::Failed { reason });
        PaneGraphicsResult {
            protocol_version: request.protocol_version,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome,
        }
    }
    fn graphics_outcome(&self, request: &PaneGraphicsRequest) -> Result<Outcome, Failure> {
        request.validate().map_err(|_| Failure::InvalidIntent)?;
        match &request.action {
            Action::Observe {
                client_id,
                heartbeat,
                observation,
            } => {
                self.service
                    .terminals
                    .validate_graphics_observation(&request.site_id, observation)?;
                self.service
                    .client_views
                    .lock()
                    .map_err(|_| Failure::Unavailable)?
                    .observe_graphics(
                        self.generation,
                        client_id,
                        *heartbeat,
                        (**observation).clone(),
                        std::time::Instant::now(),
                    )?;
                Ok(Outcome::Observed {
                    client_id: client_id.clone(),
                    heartbeat: *heartbeat,
                })
            }
            Action::Info {
                client_id: requested,
                ..
            } => {
                let mut outcome = self
                    .service
                    .terminals
                    .execute_graphics(self.generation, request.clone())
                    .outcome;
                if let Outcome::Info {
                    target,
                    revision,
                    client_id,
                    capabilities,
                    pane_visible,
                    drawn_revision,
                    ..
                } = &mut outcome
                {
                    let source = self
                        .service
                        .terminals
                        .graphics_source(&request.site_id, target)?;
                    let info = self
                        .service
                        .client_views
                        .lock()
                        .map_err(|_| Failure::Unavailable)?
                        .graphics_info(
                            requested.as_ref(),
                            &source,
                            *revision,
                            std::time::Instant::now(),
                        )?;
                    if let Some(info) = info {
                        *client_id = Some(info.client_id);
                        *capabilities = info.capabilities;
                        *pane_visible = info.visible;
                        *drawn_revision = info.drawn;
                    }
                }
                Ok(outcome)
            }
            Action::Set { .. }
            | Action::Clear { .. }
            | Action::Catalog { .. }
            | Action::Pixels { .. }
            | Action::StreamOpen { .. }
            | Action::StreamFrame { .. }
            | Action::StreamClose { .. } => Ok(self
                .service
                .terminals
                .execute_graphics(self.generation, request.clone())
                .outcome),
        }
    }
}
