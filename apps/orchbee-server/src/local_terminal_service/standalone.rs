use super::{LocalTerminalService, create_reference, lease_expiry, placement, submit};
use crate::site_coordinator::PreparedOpen;
use orchbee_client_server_protocol::{CreateUnavailableReason, OpenOutcome, TerminalOpenRequest};
use std::time::Instant;

impl LocalTerminalService {
    pub(super) fn open_standalone(&self, request: &TerminalOpenRequest) -> OpenOutcome {
        self.open_standalone_before(request, Instant::now() + self.standalone_wait)
    }

    fn open_standalone_before(
        &self,
        request: &TerminalOpenRequest,
        deadline: Instant,
    ) -> OpenOutcome {
        // Entropy is an adapter input, never read while holding coordinator or registry locks.
        // Existing shared results remain usable even when the entropy source fails.
        let mut candidate = create_reference();
        loop {
            if Instant::now() >= deadline {
                return busy();
            }
            let Ok(workers) = self.workers.lock() else {
                return busy();
            };
            let handle = workers.request_handle();
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return busy();
            };
            let prepared = coordinator.prepare_open(
                request,
                candidate.take(),
                placement(handle.as_ref()),
                lease_expiry(),
            );
            drop(workers);
            match prepared {
                PreparedOpen::Ready(outcome) => return outcome,
                PreparedOpen::Wait => {
                    // wait_timeout releases the coordinator mutex while waiting; every wake
                    // returns to the same absolute deadline and rechecks authoritative state.
                    let Some(remaining) = deadline
                        .checked_duration_since(Instant::now())
                        .filter(|duration| !duration.is_zero())
                    else {
                        return busy();
                    };
                    let Ok((guard, _)) =
                        self.standalone_changed.wait_timeout(coordinator, remaining)
                    else {
                        return busy();
                    };
                    drop(guard);
                }
                PreparedOpen::Dispatch(pending) => {
                    drop(coordinator);
                    let completion = handle.map_or(super::Completion::Unavailable, |handle| {
                        submit(&handle, pending.command())
                    });
                    let Ok(workers) = self.workers.lock() else {
                        return OpenOutcome::OutcomeUnknown {};
                    };
                    let Ok(mut coordinator) = self.coordinator.lock() else {
                        return OpenOutcome::OutcomeUnknown {};
                    };
                    let current = placement(workers.request_handle().as_ref());
                    if coordinator.complete(pending, completion, current).is_err() {
                        return OpenOutcome::OutcomeUnknown {};
                    }
                    let outcome = match coordinator.prepare_open(request, None, current, None) {
                        PreparedOpen::Ready(outcome) => outcome,
                        PreparedOpen::Wait | PreparedOpen::Dispatch(_) => {
                            OpenOutcome::OutcomeUnknown {}
                        }
                    };
                    drop(coordinator);
                    drop(workers);
                    self.standalone_changed.notify_all();
                    return outcome;
                }
            }
        }
    }
}

fn busy() -> OpenOutcome {
    OpenOutcome::Unavailable {
        reason: CreateUnavailableReason::ServerBusy,
    }
}

#[cfg(test)]
mod tests;
