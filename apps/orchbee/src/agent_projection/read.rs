//! Complete canonical pages overlap one physical checkpoint and its final revision fence.
use super::{MAXIMUM_BYTES, Row};
use crate::local_client::Cancellation;
use crate::{local_client::LocalClient, machine_agents, machine_observation, machines::ProfileId};
use orchbee_agent_observation_protocol::{
    AgentListCursor, AgentListOutcome, AgentListRejectionReason, AgentReadRequest, AgentReadResult,
    AgentSafeInteger, AgentSessionSummary, ObapRequestId,
};
use orchbee_workbench_protocol::{Snapshot, WorkbenchRevision};
use std::{collections::BTreeSet, time::Instant};
const MAXIMUM_CANONICAL_ROWS: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Unavailable,
    Denied,
    Changed,
    Invalid,
    TooLarge,
    Timeout,
    Cancelled,
}
#[derive(Debug)]
pub(crate) struct Checkpoint {
    pub(crate) canonical_revision: AgentSafeInteger,
    pub(crate) report_revision: WorkbenchRevision,
    pub(crate) rows: Vec<Row>,
}
fn check(deadline: Instant, cancel: Cancellation<'_>) -> Result<(), Error> {
    if cancel.is_cancelled().map_err(|_| Error::Unavailable)? {
        Err(Error::Cancelled)
    } else if Instant::now() >= deadline {
        Err(Error::Timeout)
    } else {
        Ok(())
    }
}
fn page(
    client: &mut LocalClient,
    include_ended: bool,
    cursor: Option<AgentListCursor>,
    deadline: Instant,
    cancel: Cancellation<'_>,
) -> Result<
    (
        AgentSafeInteger,
        Option<AgentListCursor>,
        Vec<AgentSessionSummary>,
    ),
    Error,
> {
    check(deadline, cancel)?;
    let id = ObapRequestId::parse(
        ProfileId::generate()
            .map_err(|_| Error::Unavailable)?
            .as_str(),
    )
    .map_err(|_| Error::Invalid)?;
    let result = client.submit_agent_read_before_cancellable(
        AgentReadRequest::List {
            request_id: id,
            include_ended,
            limit: AgentSafeInteger::new(100),
            cursor,
        },
        deadline,
        cancel.wake(),
    );
    check(deadline, cancel)?;
    let AgentReadResult::List { result, .. } = result.map_err(|_| Error::Unavailable)? else {
        return Err(Error::Invalid);
    };
    match result.outcome {
        AgentListOutcome::Listed {
            revision,
            next_cursor,
            items,
        } => Ok((revision, next_cursor, items)),
        AgentListOutcome::Rejected {
            reason: AgentListRejectionReason::ReadDenied,
        } => Err(Error::Denied),
        AgentListOutcome::Rejected { .. } => Err(Error::Changed),
        AgentListOutcome::Unavailable { .. } => Err(Error::Unavailable),
    }
}

pub(crate) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    include_ended: bool,
    deadline: Instant,
    cancel: Cancellation<'_>,
) -> Result<Checkpoint, Error> {
    let mut cursor = None;
    let mut cursors = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut revision = None;
    let mut items = Vec::new();
    let mut bytes = 0usize;
    let mut pages = 0;
    loop {
        pages += 1;
        let (head, next, page) = page(client, include_ended, cursor, deadline, cancel)?;
        if page.len() > 100
            || (page.is_empty() && next.is_some())
            || (!include_ended
                && page.iter().any(|r| {
                    r.lifecycle != orchbee_agent_observation_protocol::AgentLifecycle::Live
                }))
        {
            return Err(Error::Invalid);
        }
        if revision.is_some_and(|old| old != head) {
            return Err(Error::Changed);
        }
        revision = Some(head);
        for row in page {
            bytes = bytes
                .checked_add(serde_json::to_vec(&row).map_err(|_| Error::Invalid)?.len())
                .ok_or(Error::TooLarge)?;
            if items.len() == MAXIMUM_CANONICAL_ROWS || bytes > MAXIMUM_BYTES {
                return Err(Error::TooLarge);
            }
            if !ids.insert(row.session_id.clone()) {
                return Err(Error::Invalid);
            }
            items.push(row);
        }
        let Some(next) = next else { break };
        if items.len() == MAXIMUM_CANONICAL_ROWS
            || pages >= MAXIMUM_CANONICAL_ROWS.div_ceil(100)
            || !cursors.insert(next.as_str().to_owned())
        {
            return Err(Error::TooLarge);
        }
        cursor = Some(next);
    }
    let revision = revision.ok_or(Error::Invalid)?;
    let physical = machine_agents::read_states_at(client, snapshot, deadline, cancel);
    check(deadline, cancel)?;
    let physical = match physical {
        Ok(physical) => physical,
        Err(()) => return Err(outran(client, snapshot, deadline, cancel)),
    };
    let rows = super::project(items, &physical).map_err(|_| Error::TooLarge)?;
    let mut total = 0usize;
    for (i, row) in rows.iter().enumerate() {
        total = total
            .checked_add(
                serde_json::to_vec(&row.document(i + 1, None))
                    .map_err(|_| Error::Invalid)?
                    .len(),
            )
            .ok_or(Error::TooLarge)?;
        if total > MAXIMUM_BYTES {
            return Err(Error::TooLarge);
        }
    }
    check(deadline, cancel)?;
    let result = Checkpoint {
        canonical_revision: revision,
        report_revision: physical.revision,
        rows,
    };
    confirm(client, snapshot, &result, include_ended, deadline, cancel)?;
    Ok(result)
}

/// Classify a failed physical read. A Workbench that moved past the snapshot during the read is a
/// changed view, which a fresh snapshot can resolve; anything else leaves the source unavailable.
fn outran(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    deadline: Instant,
    cancel: Cancellation<'_>,
) -> Error {
    let current = machine_observation::snapshot(client, deadline, cancel);
    if let Err(error) = check(deadline, cancel) {
        return error;
    }
    match current {
        Ok(current) if current.revision != snapshot.revision => Error::Changed,
        Ok(_) | Err(_) => Error::Unavailable,
    }
}

/// Canonical records cannot change around the physical span. A final physical
/// fence then proves source overlap; topology changes never establish uniqueness.
pub(crate) fn confirm(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    read: &Checkpoint,
    include_ended: bool,
    deadline: Instant,
    cancel: Cancellation<'_>,
) -> Result<(), Error> {
    let (last, _, _) = page(client, include_ended, None, deadline, cancel)?;
    if last != read.canonical_revision {
        return Err(Error::Changed);
    }
    let fence = machine_agents::confirm(client, snapshot, read.report_revision, deadline, cancel);
    check(deadline, cancel)?;
    fence.map_err(|_| Error::Changed)?;
    let final_snapshot = machine_observation::snapshot(client, deadline, cancel);
    check(deadline, cancel)?;
    if final_snapshot.map_err(|_| Error::Unavailable)?.revision != snapshot.revision {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
