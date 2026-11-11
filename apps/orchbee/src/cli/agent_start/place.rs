//! The new Tab or split an Agent start opens beside its reference Panel, through the same
//! creation operations and stale-view retry as `tab create` and `panel split`. Only the closed
//! catalog id ever names the Agent; the Server resolves the command the new shell runs first.
use crate::{
    cli::{
        Failure, FailureKind, caller_context::PanelTarget, console_interrupt::ArmedSignals,
        mutation, results,
    },
    invocation::{AgentStartPlace, MutationCommand, Resource, TabContext, Target},
    local_client::LocalClient,
};
use orchbee_workbench_protocol::{
    AgentStartSelector, LaunchOptions, Snapshot, WorkbenchLabel, WorkbenchPanelId,
    WorkbenchPanelSplitDirection, WorkbenchTerminalGeometry,
};
use std::time::Instant;

/// A creation that applied nothing, or a new Panel whose shell did not start. The Panel is named
/// whenever it exists, so the outcome can say that it stays.
pub(super) struct Refusal {
    pub(super) failure: Failure,
    pub(super) created: Option<WorkbenchPanelId>,
}

/// The creation operation a place submits, as receipts and causes name it.
pub(super) const fn operation(place: AgentStartPlace) -> &'static str {
    match place {
        AgentStartPlace::SplitRight | AgentStartPlace::SplitDown => "panel.split",
        AgentStartPlace::NewTab | AgentStartPlace::Here => "tab.create",
    }
}

/// Open the new Tab or split for `place` beside `reference` and return its Panel. With an
/// `agent_start` selector the new shell runs that Agent as its first command.
pub(super) fn create(
    client: &mut LocalClient,
    snapshot: &mut Snapshot,
    deadline: Instant,
    signals: &mut ArmedSignals,
    (reference, place): (&PanelTarget, AgentStartPlace),
    agent_start: Option<AgentStartSelector>,
) -> Result<WorkbenchPanelId, Box<Refusal>> {
    let refused = |failure| {
        Box::new(Refusal {
            failure,
            created: None,
        })
    };
    let options = LaunchOptions {
        agent_start,
        ..LaunchOptions::default()
    };
    let geometry = WorkbenchTerminalGeometry::new(80, 24, 0, 0).map_err(|_| refused(internal()))?;
    let ((), receipt) =
        mutation::submit_creation(client, snapshot, deadline, signals, |_, snapshot, _| {
            let command = command(snapshot, reference, place, geometry, &options)?;
            Ok(((), command, ()))
        })
        .map_err(refused)?;
    let key = match place {
        AgentStartPlace::SplitRight | AgentStartPlace::SplitDown => "newPanelId",
        AgentStartPlace::NewTab | AgentStartPlace::Here => "panelId",
    };
    let panel = receipt
        .id_value(key)
        .and_then(|id| WorkbenchPanelId::parse(id).ok())
        .ok_or_else(|| refused(internal()))?;
    // The Panel exists from here on; a shell that did not start leaves it in place.
    if let Some(kind) = receipt.launch_failure() {
        return Err(Box::new(Refusal {
            failure: Failure::new(kind)
                .details(super::super::stage_reason_details(
                    "terminal",
                    "terminal-launch-failed",
                ))
                .source(serde_json::json!({ "requestId": receipt.request_id() })),
            created: Some(panel),
        }));
    }
    Ok(panel)
}

/// Derive the creation from `snapshot`, again on every stale-view attempt: the reference Panel,
/// its Workspace or Tab, and the next free label all come from the read.
fn command(
    snapshot: &Snapshot,
    reference: &PanelTarget,
    place: AgentStartPlace,
    geometry: WorkbenchTerminalGeometry,
    options: &LaunchOptions,
) -> Result<MutationCommand, Failure> {
    let target = reference.resolve(snapshot)?;
    let entries = results::panel_entries(snapshot);
    let (panel, context, tab) = results::resolve_or_fail(
        entries
            .iter()
            .map(|(panel, context, tab)| (panel.as_str(), "", (*panel, *context, *tab))),
        &target,
        Resource::Panel,
    )?;
    let label =
        |count: usize| WorkbenchLabel::parse(&(count + 1).to_string()).map_err(|_| internal());
    let direction = match place {
        AgentStartPlace::SplitRight => WorkbenchPanelSplitDirection::Right,
        AgentStartPlace::SplitDown => WorkbenchPanelSplitDirection::Below,
        AgentStartPlace::NewTab => {
            let workspace = snapshot
                .navigation
                .workspaces
                .iter()
                .find(|workspace| workspace.terminal_context_id == context.terminal_context_id)
                .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
            return Ok(MutationCommand::TabCreate {
                automatic_label: true,
                context: TabContext::Workspace(Target::Id(workspace.workspace_id.to_string())),
                label: label(context.tabs.len())?,
                geometry,
                launch: None,
                profile: None,
                options: options.clone(),
            });
        }
        AgentStartPlace::Here => return Err(internal()),
    };
    let count = entries
        .iter()
        .filter(|(_, _, candidate)| candidate.tab_id == tab.tab_id)
        .count();
    Ok(MutationCommand::PanelSplit {
        right_click: orchbee_workbench_protocol::PanelRightClickTarget::default(),
        target: Target::Id(panel.to_string()),
        direction,
        label: label(count)?,
        custom_label: None,
        geometry,
        launch: None,
        profile: None,
        options: options.clone(),
    })
}

fn internal() -> Failure {
    Failure::new(FailureKind::InternalFailure)
}
