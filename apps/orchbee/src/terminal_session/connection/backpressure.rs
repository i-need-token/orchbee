use super::{Identities, LocalClient, Result, TerminalStatus, submission};
use orchbee_client_server_protocol::{
    ControlUnavailableReason, TerminalControlAction as Control,
    TerminalControlOutcome as ControlOutcome, TerminalControlRequest,
    TerminalViewerAction as Viewer, TerminalViewerOutcome as ViewerOutcome, TerminalViewerRequest,
    ViewerUnavailableReason,
};
use std::{
    num::NonZeroU64,
    thread,
    time::{Duration, Instant},
};

pub(super) fn viewer(
    client: &mut LocalClient,
    ids: &mut Identities,
    action: Viewer,
) -> Result<ViewerOutcome> {
    with_budget(client, |client, deadline| {
        loop {
            let result = client
                .submit_terminal_viewer(TerminalViewerRequest::new(ids.next()?, action.clone()))
                .map_err(submission)?
                .into_outcome();
            if result
                != (ViewerOutcome::Unavailable {
                    reason: ViewerUnavailableReason::ServerBusy,
                })
            {
                return Ok(result);
            }
            wait(deadline)?;
        }
    })
}

pub(super) fn control(
    client: &mut LocalClient,
    ids: &mut Identities,
    mut action: Control,
    sequence: &mut u64,
) -> Result<ControlOutcome> {
    with_budget(client, |client, deadline| {
        loop {
            if let Control::Input {
                sequence: attempt, ..
            }
            | Control::Resize {
                sequence: attempt, ..
            } = &action
            {
                *sequence = attempt.get();
            }
            let result = client
                .submit_terminal_control(TerminalControlRequest::new(ids.next()?, action.clone()))
                .map_err(submission)?
                .into_outcome();
            match result {
                ControlOutcome::OutcomeUnknown {} => return Err(TerminalStatus::OutcomeUnknown),
                ControlOutcome::Unavailable {
                    reason: ControlUnavailableReason::ServerBusy,
                } => {}
                result => return Ok(result),
            }
            wait(deadline)?;
            // Server may have consumed this sequence before its queue rejected the action.
            // Only a correlated ServerBusy proves that no Host action was dispatched.
            if let Control::Input {
                sequence: attempt, ..
            }
            | Control::Resize {
                sequence: attempt, ..
            } = &mut action
            {
                *attempt = attempt
                    .get()
                    .checked_add(1)
                    .and_then(NonZeroU64::new)
                    .ok_or(TerminalStatus::Unavailable)?;
            }
        }
    })
}

fn with_budget<T>(
    client: &mut LocalClient,
    operation: impl FnOnce(&mut LocalClient, Instant) -> Result<T>,
) -> Result<T> {
    let previous = client.submission_deadline();
    let deadline = client.next_operation_deadline();
    client.set_submission_deadline(Some(deadline));
    let result = operation(client, deadline);
    client.set_submission_deadline(previous);
    result
}
fn wait(deadline: Instant) -> Result<()> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(TerminalStatus::Unavailable);
    }
    thread::sleep(remaining.min(Duration::from_millis(10)));
    if Instant::now() >= deadline {
        return Err(TerminalStatus::Unavailable);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
