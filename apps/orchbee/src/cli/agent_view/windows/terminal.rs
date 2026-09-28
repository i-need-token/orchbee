//! Inspect a named physical terminal without synthesizing a canonical session.
use super::*;
use crate::cli::terminal_observation::Observation;
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use serde_json::json;

pub(super) fn read(
    client: &mut crate::local_client::LocalClient,
    snapshot: &Snapshot,
    binding: AgentLaunchBinding,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<(serde_json::Value, results::Projection), Failure> {
    let row = query::named_terminal::terminal_row(snapshot, &binding)?;
    let read = query::terminal_metadata::read(
        client,
        snapshot,
        &crate::invocation::Target::Id(row.terminal_id.clone()),
        deadline,
        signals.poll_fd(),
    )?;
    check(deadline, signals)?;
    if read.frame.route.host_epoch.to_text() != binding.host_epoch.as_str() {
        return Err(view_failure(FailureKind::ServiceUnavailable));
    }
    let mut fields = Observation::Observed(Box::new(read)).fields();
    fields["terminalId"] = json!(row.terminal_id);
    fields["presentationId"] = json!(row.presentation_id);
    fields["panelId"] = json!(row.panel_id);
    fields["source"] = results::snapshot_source(snapshot);
    // Panel reports describe the primary presentation only. Never publish the
    // parent's report as a child's provider identity.
    let results::Projection::PanelList(panels) =
        results::project(&QueryCommand::PanelList { tab: None }, snapshot)?
    else {
        return Err(view_failure(FailureKind::InternalFailure));
    };
    if let Some(panel) = panels
        .iter()
        .find(|p| p.primary_presentation_id == row.presentation_id)
    {
        let panel_id = orchbee_workbench_protocol::WorkbenchPanelId::parse(&panel.panel_id)
            .map_err(|_| view_failure(FailureKind::ProtocolInvalid))?;
        let mut metadata = crate::cli::panel_metadata_query::read(
            client,
            snapshot,
            std::slice::from_ref(&panel_id),
            deadline,
            signals,
        )?;
        metadata.verify(client, snapshot, deadline, signals)?;
        fields["metadata"] = metadata.value(&panel_id);
        fields["reportedAgent"] = metadata.report_value(&panel_id);
    }
    query::named_terminal::verify_named_binding(client, &binding, deadline, signals.poll_fd())?;
    let latest = query::refresh_snapshot(
        client,
        &snapshot.site_id,
        snapshot.revision,
        deadline,
        signals,
    )?;
    if latest.revision != snapshot.revision {
        return Err(crate::cli::workbench_agents::failure("workbench-changed"));
    }
    check(deadline, signals)?;
    Ok((
        results::snapshot_source(snapshot),
        results::Projection::AgentTerminalInspection {
            binding,
            terminal: fields,
        },
    ))
}
