//! Existing Windows startup effects. The command adapter retains its existing execution helpers.
use super::Receipt;
pub(crate) use crate::cli::agent_start_runtime::{
    read_agent_observation, read_terminal_processes_cancellable, snapshot_from_origin,
    stream_snapshot,
};
use crate::{cli::Failure, invocation::Target, local_client::LocalClient};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::Snapshot;
use std::time::Instant;
pub(crate) fn submit_prompt_on_session(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    target: &Target,
    text: &str,
    codex_boundary: bool,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<Receipt, Failure> {
    let receipt = crate::cli::agent_start_runtime::submit_prompt_on_session(
        client,
        snapshot,
        target,
        text,
        codex_boundary,
        deadline,
        wake,
    )?;
    Ok(Receipt {
        status: receipt.exit_status(),
        verdict: receipt.verdict(),
        accepted: receipt.accepted(),
        document: receipt.document(),
    })
}
