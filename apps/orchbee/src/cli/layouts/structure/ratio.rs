//! A canonical CAS with its request identity and known outcome retained for composition.
use crate::{
    cli::{Failure, FailureKind, query},
    local_client::LocalClient,
};
use orchbee_workbench_protocol::{
    ClientSplitIdentity, ProtocolVersion, Snapshot, WorkbenchAction, WorkbenchCommand,
    WorkbenchRequestId, WorkbenchRevision, WorkbenchSplitRatio, WorkbenchTabId,
    WorkbenchTerminalContextId,
};
use serde_json::{Value, json};
use std::time::Instant;

#[allow(clippy::too_many_arguments)]
pub(in crate::cli) fn submit(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    context: &WorkbenchTerminalContextId,
    tab: &WorkbenchTabId,
    split: ClientSplitIdentity,
    previous_ratio: WorkbenchSplitRatio,
    ratio: WorkbenchSplitRatio,
    request_id: Option<WorkbenchRequestId>,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
) -> Result<(Value, Result<WorkbenchRevision, Failure>), Failure> {
    let request_id = request_id.map(Ok).unwrap_or_else(query::request_id)?;
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: request_id.clone(),
        site_id: snapshot.site_id.clone(),
        expected_revision: snapshot.revision,
        action: WorkbenchAction::SplitRatioSet {
            terminal_context_id: context.clone(),
            tab_id: tab.clone(),
            split: split.clone(),
            previous_ratio,
            ratio,
        },
    };
    let result = client.set_split_ratio(command, deadline, wake);
    let outcome = match &result {
        Ok(revision) => {
            json!({"kind":if ratio == previous_ratio { "unchanged" } else { "configured" }, "revision":revision})
        }
        Err(failure) => json!({
            "kind":if failure.may_have_committed { "outcome-unknown" } else { "not-committed" },
            "reason":failure.reason,
        }),
    };
    let effect = json!({
        "action":"layout.set-split-ratio","siteId":snapshot.site_id,"requestId":request_id,
        "baseRevision":snapshot.revision,"terminalContextId":context,"tabId":tab,
        "split":split,"previousRatio":previous_ratio,"ratio":ratio,"result":outcome,
    });
    Ok((
        effect,
        result.map_err(|failure| {
            Failure::new(if failure.may_have_committed {
                FailureKind::OutcomeUnknown
            } else {
                FailureKind::Rejected
            })
        }),
    ))
}
