//! One exact request, with transport handoff retained for report uncertainty.
use super::metadata_exchange::{MetadataExchangeError, MetadataExchangeReason};
use crate::local_client::{LocalClient, exchange_once};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{
    SemanticEventsRequest, SemanticEventsResult, WorkbenchRequest, WorkbenchResponse,
    read_workbench_response, write_workbench_request,
};
use std::time::Instant;
impl LocalClient {
    pub fn semantic_events(
        &mut self,
        request: SemanticEventsRequest,
        deadline: Instant,
        wake: Option<LocalIo<'_>>,
    ) -> Result<SemanticEventsResult, MetadataExchangeError> {
        let before = |reason| MetadataExchangeError {
            submitted: false,
            reason,
        };
        let mut frame = Vec::new();
        write_workbench_request(&mut frame, &WorkbenchRequest::Events(request.clone()))
            .map_err(|_| before(MetadataExchangeReason::Encoding))?;
        let deadline = self
            .submission_deadline_before(deadline)
            .map_err(|_| before(MetadataExchangeReason::Deadline))?;
        let connection = self
            .connection
            .as_mut()
            .ok_or_else(|| before(MetadataExchangeReason::ConnectionUnavailable))?;
        let attempt = exchange_once(
            connection,
            &self.clock,
            deadline,
            self.io_chunk_limits,
            wake,
            &frame,
            |io| read_workbench_response(io),
        );
        self.finish_metadata_exchange(attempt, |response| match response {
            WorkbenchResponse::Events(result) if result.answers(&request) => Some(result),
            _ => None,
        })
    }
}
