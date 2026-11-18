//! The new Tab or split an Agent start opens beside its reference Panel, through the same
//! creation operations and stale-view retry as `tab create` and `panel split`. Only the closed
//! catalog id ever names the Agent; the Server resolves the command the new shell runs first.
use super::super::creation::workflow::{FocusHalt, FocusMiss, focus_new_panel};
use crate::{
    cli::{
        Failure, FailureKind, caller_context::PanelTarget, command_interrupt::ArmedSignals,
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

/// A creation that applied nothing, a new Panel whose shell did not start, or a focus that halted
/// the start. The Panel is named whenever it exists, so the outcome can say that it stays; `focus`
/// keeps the Client's answer to a halting focus.
pub(super) struct Refusal {
    pub(super) failure: Failure,
    pub(super) created: Option<WorkbenchPanelId>,
    pub(super) focus: Option<FocusMiss>,
}

/// The creation operation a place submits, as receipts and causes name it.
pub(super) const fn operation(place: AgentStartPlace) -> &'static str {
    match place {
        AgentStartPlace::SplitRight | AgentStartPlace::SplitDown => "panel.split",
        AgentStartPlace::NewTab | AgentStartPlace::Here => "tab.create",
    }
}

pub(super) struct Placement<'a> {
    pub(super) reference: &'a PanelTarget,
    pub(super) caller: Option<&'a PanelTarget>,
    pub(super) place: AgentStartPlace,
    pub(super) directory: crate::invocation::CreationDirectory,
    pub(super) cwd: Option<&'a orchbee_workbench_protocol::WorkbenchDirectoryPath>,
    pub(super) focus: bool,
    pub(super) client: Option<&'a orchbee_workbench_protocol::WorkbenchClientId>,
    pub(super) agent_start: Option<AgentStartSelector>,
}

/// Open the new Tab or split for `place` beside `reference` and return its Panel. With an
/// `agent_start` selector the new shell runs that Agent as its first command.
pub(super) fn create(
    client: &mut LocalClient,
    snapshot: &mut Snapshot,
    deadline: Instant,
    signals: &mut ArmedSignals,
    placement: Placement<'_>,
) -> Result<(WorkbenchPanelId, Option<FocusMiss>), Box<Refusal>> {
    let refused = |failure| {
        Box::new(Refusal {
            failure,
            created: None,
            focus: None,
        })
    };
    let resolved_reference = placement.reference.resolve(snapshot).map_err(refused)?;
    let mut options = LaunchOptions {
        agent_start: placement.agent_start,
        cwd: placement.cwd.cloned(),
        ..LaunchOptions::default()
    };
    if placement.directory != crate::invocation::CreationDirectory::Default {
        super::super::creation::directory::resolve_options(
            placement.directory,
            &mut options,
            snapshot,
            Some(&resolved_reference),
            client,
            deadline,
            signals,
        )
        .map_err(refused)?;
    }
    let selected = if placement.focus {
        let caller = placement
            .caller
            .and_then(|caller| caller.resolve(snapshot).ok());
        Some(
            super::super::creation::workflow::select_ready_client(
                client,
                &snapshot.session_id,
                placement.client,
                caller.as_ref(),
                deadline,
                signals,
            )
            .map_err(refused)?,
        )
    } else {
        None
    };
    let geometry = WorkbenchTerminalGeometry::new(80, 24, 0, 0).map_err(|_| refused(internal()))?;
    let ((), receipt) =
        mutation::submit_creation(client, snapshot, deadline, signals, |_, snapshot, _| {
            let command = command(
                snapshot,
                placement.reference,
                placement.place,
                geometry,
                &options,
            )?;
            Ok(((), command, ()))
        })
        .map_err(refused)?;
    let split = matches!(
        placement.place,
        AgentStartPlace::SplitRight | AgentStartPlace::SplitDown
    );
    let key = if split { "newPanelId" } else { "panelId" };
    let panel = receipt
        .id_value(key)
        .and_then(|id| WorkbenchPanelId::parse(id).ok())
        .ok_or_else(|| refused(internal()))?;
    // The Panel exists from here on; a shell that did not start leaves it in place. A focus that
    // misses leaves the Agent running there too, so the miss is returned beside the Panel.
    if let Some(kind) = receipt.launch_failure() {
        return Err(Box::new(Refusal {
            failure: Failure::new(kind)
                .details(super::super::stage_reason_details(
                    "terminal",
                    "terminal-launch-failed",
                ))
                .source(serde_json::json!({ "requestId": receipt.request_id() })),
            created: Some(panel),
            focus: None,
        }));
    }
    let focus = match selected.as_ref() {
        Some(selected) => focus_new_panel(
            client,
            &snapshot.session_id,
            selected,
            &receipt,
            split,
            deadline,
            signals,
        )
        .map_err(|halt| {
            let FocusHalt { failure, miss } = *halt;
            Box::new(Refusal {
                failure,
                created: Some(panel.clone()),
                focus: miss,
            })
        })?,
        None => None,
    };
    Ok((panel, focus))
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
