//! Read display aliases once within the physical observation's deadline and byte budget.
use super::{AtomicU8, Instant, LocalClient, MAXIMUM_BYTES, check, request_id};
use orchbee_agent_observation_protocol::{
    AgentNamedTarget, AgentNamesListOutcome, AgentNamesRequest, AgentNamesResult,
    MAXIMUM_NAME_BINDINGS, ObapRequestId,
};

pub(super) fn read(
    client: &mut LocalClient,
    deadline: Instant,
    cancel: &AtomicU8,
    bytes: &mut usize,
) -> Result<Vec<AgentNamedTarget>, ()> {
    check(deadline, cancel)?;
    let reply = client
        .submit_agent_names_before_cancellable(
            AgentNamesRequest::NamesList {
                request_id: ObapRequestId::parse(request_id()?.as_str()).map_err(|_| ())?,
            },
            deadline,
            Some(orchbee_local_ipc::LocalIo::Wake(cancel, 1)),
        )
        .map_err(|_| ())?;
    *bytes = bytes
        .checked_add(serde_json::to_vec(&reply).map_err(|_| ())?.len())
        .ok_or(())?;
    if *bytes > MAXIMUM_BYTES {
        return Err(());
    }
    check(deadline, cancel)?;
    let AgentNamesResult::NamesList { result, .. } = reply else {
        return Err(());
    };
    match result.outcome {
        AgentNamesListOutcome::ListedTargets { bindings, .. }
            if bindings.len() <= MAXIMUM_NAME_BINDINGS =>
        {
            Ok(bindings)
        }
        AgentNamesListOutcome::Listed { .. }
        | AgentNamesListOutcome::Rejected { .. }
        | AgentNamesListOutcome::Unavailable { .. } => Ok(Vec::new()),
        _ => Err(()),
    }
}
