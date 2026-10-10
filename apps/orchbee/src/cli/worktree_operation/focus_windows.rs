//! Post-open Client control is fenced separately from the retained Git operation.
use super::post_open::Receipt;
use crate::{
    cli::{
        Failure,
        source::ArmedSignals,
        workspace_focus::{self, Navigation, Target},
    },
    local_client::LocalClient,
};
use orchbee_workbench_protocol::{ClientViewRecord, Snapshot, WorkbenchWorkspaceId};
use std::time::Instant;

pub(super) fn execute(
    client: &mut LocalClient,
    original: &Snapshot,
    target: &WorkbenchWorkspaceId,
    selected: &ClientViewRecord,
    waiting: (Instant, &mut ArmedSignals),
    receipt: &mut Receipt,
) -> Result<(), Failure> {
    receipt.stage = "focus";
    receipt.record(workspace_focus::execute(
        client,
        original,
        Target {
            workspace: target,
            minimum: original.revision,
            navigation: Navigation::Unchanged,
        },
        selected,
        waiting.0,
        waiting.1,
    )?);
    Ok(())
}
