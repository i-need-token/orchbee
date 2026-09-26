use crate::terminal_coordinator::{
    Completion, PendingOperation, PreparedOperation, TerminalCompletionTokenMismatch,
    TerminalCoordinator, TerminalPhase, WorkerPlacement,
};
use orchbee_capability_node_protocol::{
    LeaseExpiresAtUnixMilliseconds, TerminalOperationAuthorization,
};
use orchbee_client_server_protocol::{
    ClientMessage, CreateAcceptedState, CreateFailureReason, CreateOutcome,
    CreateUnavailableReason, OpenOutcome, RequestId, ServerMessage,
};
use orchbee_server::{
    WorkbenchAuthority, WorkbenchGrant, WorkbenchReadOrNavigationRequest,
    WorkbenchTerminalMembership, WorktreeListResolution,
};
use orchbee_workbench_protocol::{
    PresentationBinding, TerminalId, WorkbenchDirectoryId, WorkbenchProjectId, WorkbenchResponse,
    WorkerId,
};

mod standalone;
pub(crate) use standalone::PreparedOpen;
mod tab_close;
mod tab_create;
#[cfg(test)]
pub(crate) use tab_create::PendingTerminalTabCreate;
pub(crate) use tab_create::{PostcommitCoordinationError, PreparedTerminalTabCreate};
mod panel_split;
#[cfg(test)]
pub(crate) use panel_split::PendingTerminalPanelSplit;
pub(crate) use panel_split::PreparedTerminalPanelSplit;
mod terminal_selection;
mod workspace_directory;
mod worktree_restore;
pub(crate) use terminal_selection::{
    BindingViewerPreparationError, PreparedBindingViewer, PreparedSendTarget,
};
mod target_lifecycle;
pub(crate) use target_lifecycle::TargetLifecycle;
mod agent_registration;

pub(crate) struct SiteCoordinator {
    terminals: TerminalCoordinator,
    workbench: Option<WorkbenchAuthority>,
    standalone: StandaloneTerminal,
}

#[derive(Debug, Eq, PartialEq)]
enum StandaloneTerminal {
    Empty,
    Creating(RequestId),
    Settled {
        reference: RequestId,
        outcome: OpenOutcome,
    },
}

#[cfg(test)]
mod tests;

impl SiteCoordinator {
    pub(crate) const fn new(workbench: Option<WorkbenchAuthority>) -> Self {
        Self {
            terminals: TerminalCoordinator::new(),
            workbench,
            standalone: StandaloneTerminal::Empty,
        }
    }

    pub(crate) fn workbench_site_matches(
        &self,
        site: &orchbee_workbench_protocol::SiteId,
    ) -> Option<bool> {
        Some(self.workbench.as_ref()?.site_id() == site)
    }

    pub(crate) fn worktree_authority(&mut self) -> Option<&mut WorkbenchAuthority> {
        self.workbench.as_mut()
    }

    /// Resolves committed Worktree List membership; `Unavailable` covers an
    /// absent authority so callers need no second poisoning path.
    pub(crate) fn worktree_list_target(
        &self,
        project_id: &WorkbenchProjectId,
        directory_id: &WorkbenchDirectoryId,
    ) -> WorktreeListResolution {
        match self.workbench.as_ref() {
            Some(authority) => authority.worktree_list_target(project_id, directory_id),
            None => WorktreeListResolution::Unavailable,
        }
    }

    /// Reserves `count` sequential operation identities once, so the Worktree
    /// List Start/Poll exchanges mint fresh IDs without re-entering the lock.
    pub(crate) fn allocate_identity_block(&mut self, count: u128) -> Option<u128> {
        self.terminals.allocate_identity_block(count)
    }

    pub(crate) fn execute_workbench(
        &mut self,
        request: WorkbenchReadOrNavigationRequest,
        manage_navigation: bool,
    ) -> Option<WorkbenchResponse> {
        let authority = self.workbench.as_mut()?;
        let site = authority.site_id().clone();
        let grant = if manage_navigation {
            WorkbenchGrant::manage_navigation(site)
        } else {
            WorkbenchGrant::read_only(site)
        };
        Some(authority.execute(Some(&grant), request))
    }

    pub(crate) fn prepare(
        &mut self,
        request: ClientMessage,
        worker: Option<WorkerPlacement>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
    ) -> PreparedOperation {
        let start_fenced = match &request {
            ClientMessage::TerminalStart(request) => self
                .terminals
                .retained_authorization(request.terminal_create_request_id())
                .and_then(presentation_binding)
                .is_some_and(|binding| self.binding_has_close(&binding)),
            _ => false,
        };
        self.terminals
            .prepare(start_fenced, request, worker, expiry)
    }

    pub(crate) fn execute_terminal_tab_move(
        &mut self,
        command: orchbee_workbench_protocol::WorkbenchCommand,
    ) -> Option<orchbee_workbench_protocol::WorkbenchCommandResult> {
        use orchbee_server::{WorkbenchTabMoveBegin, WorkbenchTabMovePreparation};
        let Self {
            terminals,
            workbench,
            standalone,
        } = self;
        let workbench = workbench.as_mut()?;
        let grant = WorkbenchGrant::manage_navigation(workbench.site_id().clone());
        let admission = match workbench.begin_terminal_tab_move(Some(&grant), command) {
            WorkbenchTabMoveBegin::Ready(result) => return Some(result),
            WorkbenchTabMoveBegin::Admitted(admission) => admission,
        };
        if admission
            .standalone_membership()
            .is_some_and(|membership| !composition_matches(membership, standalone, terminals))
        {
            return Some(admission.retire_composition_failure());
        }
        Some(match admission.finish() {
            WorkbenchTabMovePreparation::Ready(result) => result,
            WorkbenchTabMovePreparation::Prepared(prepared) => prepared.commit(),
        })
    }

    pub(crate) fn execute_terminal_panel_placement(
        &mut self,
        command: orchbee_workbench_protocol::WorkbenchCommand,
    ) -> Option<orchbee_workbench_protocol::WorkbenchCommandResult> {
        use orchbee_server::{WorkbenchPanelPlacementBegin, WorkbenchPanelPlacementPreparation};
        let Self {
            terminals,
            workbench,
            standalone,
        } = self;
        let workbench = workbench.as_mut()?;
        let grant = WorkbenchGrant::manage_navigation(workbench.site_id().clone());
        let admission = match workbench.begin_terminal_panel_placement(Some(&grant), command) {
            WorkbenchPanelPlacementBegin::Ready(result) => return Some(result),
            WorkbenchPanelPlacementBegin::Admitted(admission) => admission,
        };
        if admission
            .standalone_membership()
            .is_some_and(|membership| !composition_matches(membership, standalone, terminals))
        {
            return Some(admission.retire_composition_failure());
        }
        Some(match admission.finish() {
            WorkbenchPanelPlacementPreparation::Ready(result) => result,
            WorkbenchPanelPlacementPreparation::Prepared(prepared) => prepared.commit(),
        })
    }

    /// `panel.relocate`: same-context leaf relocation keeps the context's
    /// binding multiset identical, so the same standalone composition check
    /// `panel move` runs holds through the single atomic commit.
    pub(crate) fn execute_terminal_panel_relocate(
        &mut self,
        command: orchbee_workbench_protocol::WorkbenchCommand,
    ) -> Option<orchbee_workbench_protocol::WorkbenchCommandResult> {
        use orchbee_server::{WorkbenchPanelRelocateBegin, WorkbenchPanelRelocatePreparation};
        let Self {
            terminals,
            workbench,
            standalone,
        } = self;
        let workbench = workbench.as_mut()?;
        let grant = WorkbenchGrant::manage_navigation(workbench.site_id().clone());
        let admission = match workbench.begin_terminal_panel_relocate(Some(&grant), command) {
            WorkbenchPanelRelocateBegin::Ready(result) => return Some(result),
            WorkbenchPanelRelocateBegin::Admitted(admission) => admission,
        };
        if admission
            .standalone_membership()
            .is_some_and(|membership| !composition_matches(membership, standalone, terminals))
        {
            return Some(admission.retire_composition_failure());
        }
        Some(match admission.finish() {
            WorkbenchPanelRelocatePreparation::Ready(result) => result,
            WorkbenchPanelRelocatePreparation::Prepared(prepared) => prepared.commit(),
        })
    }

    pub(crate) fn complete(
        &mut self,
        pending: Box<PendingOperation>,
        completion: Completion,
        current_worker: Option<WorkerPlacement>,
    ) -> Result<ServerMessage, TerminalCompletionTokenMismatch> {
        let reference = pending.create_reference().clone();
        let Self {
            terminals,
            standalone,
            workbench: _,
        } = self;
        let prepared = terminals.prepare_completion(pending, completion, current_worker)?;
        let result = prepared.result().clone();
        let standalone_settlement = if matches!(standalone, StandaloneTerminal::Creating(current) if current == &reference)
        {
            let outcome = open_outcome(&reference, prepared.result());
            Some(StandaloneTerminal::Settled { reference, outcome })
        } else {
            None
        };
        prepared.commit();
        if let Some(settled) = standalone_settlement {
            *standalone = settled;
        }
        Ok(result)
    }

    pub(crate) fn viewer_target(
        &self,
        reference: &RequestId,
        worker: WorkerPlacement,
    ) -> Option<crate::terminal_coordinator::BindingViewerTarget> {
        let target = self.terminals.viewer_target(reference, worker)?;
        (self.target_lifecycle(target) == TargetLifecycle::Unfenced).then_some(target)
    }

    /// `terminal.launch.read`: the visible binding, the committed revision, and
    /// the retained launch fact all resolve under this one coordinator
    /// acquisition, so the answer is a single point-in-time join. The read
    /// touches no Worker or Host RPC and mutates nothing.
    pub(crate) fn terminal_launch_read(
        &self,
        terminal_id: &str,
    ) -> orchbee_client_server_protocol::TerminalLaunchReadOutcome {
        use orchbee_client_server_protocol::TerminalLaunchReadOutcome as Outcome;
        let Some(workbench) = self.workbench.as_ref() else {
            return Outcome::NotFound;
        };
        let snapshot = workbench.snapshot();
        let Some((route, binding)) = launch_route(snapshot, terminal_id) else {
            return Outcome::NotFound;
        };
        let (launch, launch_profile) = retained_launch_fact(&self.terminals, &binding)
            .unwrap_or_else(|| (not_tracked_fact(), None));
        let Some(site_id) =
            orchbee_client_server_protocol::SiteId::parse(snapshot.site_id.as_str()).ok()
        else {
            return Outcome::Unavailable {
                reason:
                    orchbee_client_server_protocol::TerminalLaunchReadUnavailableReason::ServerBusy,
            };
        };
        Outcome::Found {
            site_id,
            workbench_revision: snapshot.revision.value(),
            terminal: Box::new(route),
            launch: Box::new(launch),
            launch_profile,
        }
    }

    pub(crate) fn allocate_identity(&mut self) -> Option<[u8; 16]> {
        self.terminals.allocate_identity()
    }
}

fn composition_matches(
    membership: &WorkbenchTerminalMembership,
    standalone: &StandaloneTerminal,
    terminals: &TerminalCoordinator,
) -> bool {
    if !membership.is_standalone() {
        return false;
    }
    match standalone {
        StandaloneTerminal::Empty => membership.is_empty(),
        StandaloneTerminal::Creating(reference) | StandaloneTerminal::Settled { reference, .. } => {
            let Some(binding) = terminals
                .retained_authorization(reference)
                .and_then(presentation_binding)
            else {
                return false;
            };
            membership
                .members()
                .iter()
                .filter(|member| member.binding() == &binding)
                .count()
                == 1
        }
    }
}

fn tree_composition_matches(
    membership: &WorkbenchTerminalMembership,
    standalone: &StandaloneTerminal,
    terminals: &TerminalCoordinator,
) -> bool {
    if !membership.is_standalone() {
        return false;
    }
    match standalone {
        StandaloneTerminal::Empty => membership.is_empty(),
        StandaloneTerminal::Creating(_) | StandaloneTerminal::Settled { .. } => {
            !membership.is_empty()
                && membership
                    .members()
                    .iter()
                    .all(|member| terminals.has_retained_binding(member.binding()))
        }
    }
}

fn presentation_binding(
    authorization: TerminalOperationAuthorization,
) -> Option<PresentationBinding> {
    Some(PresentationBinding {
        terminal_id: TerminalId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(authorization.terminal_id().as_bytes())
        ))
        .ok()?,
        worker_id: WorkerId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(authorization.worker_id().as_bytes())
        ))
        .ok()?,
    })
}

fn open_outcome(reference: &RequestId, result: &ServerMessage) -> OpenOutcome {
    let ServerMessage::TerminalCreateResult(result) = result else {
        return OpenOutcome::OutcomeUnknown {};
    };
    open_create_outcome(reference, result.outcome())
}

fn open_create_outcome(reference: &RequestId, outcome: CreateOutcome) -> OpenOutcome {
    match outcome {
        CreateOutcome::Accepted {
            state: CreateAcceptedState::Started { .. },
        } => OpenOutcome::Opened {
            terminal_create_request_id: reference.clone(),
        },
        CreateOutcome::Unavailable { reason } => unavailable(reason),
        CreateOutcome::Failed { reason } => OpenOutcome::Failed { reason },
        CreateOutcome::Rejected { .. } => OpenOutcome::Failed {
            reason: CreateFailureReason::TerminalCreateFailed,
        },
        CreateOutcome::Accepted {
            state: CreateAcceptedState::AwaitingStart,
        }
        | CreateOutcome::OutcomeUnknown => OpenOutcome::OutcomeUnknown {},
    }
}

fn unavailable(reason: CreateUnavailableReason) -> OpenOutcome {
    OpenOutcome::Unavailable { reason }
}

/// Walks the committed snapshot in traversal order — the same order `terminal
/// list` rows follow — and returns the routing for the first presentation whose
/// binding targets `terminal_id`.
fn launch_route(
    snapshot: &orchbee_workbench_protocol::Snapshot,
    terminal_id: &str,
) -> Option<(
    orchbee_client_server_protocol::TerminalLaunchRoute,
    PresentationBinding,
)> {
    for context in &snapshot.terminal_contexts {
        for tab in &context.tabs {
            if let Some(found) = launch_route_in_node(&tab.root, context, tab, terminal_id) {
                return Some(found);
            }
        }
    }
    None
}

fn launch_route_in_node(
    node: &orchbee_workbench_protocol::PanelNode,
    context: &orchbee_workbench_protocol::TerminalContext,
    tab: &orchbee_workbench_protocol::Tab,
    terminal_id: &str,
) -> Option<(
    orchbee_client_server_protocol::TerminalLaunchRoute,
    PresentationBinding,
)> {
    use orchbee_workbench_protocol::PanelNode;
    match node {
        PanelNode::Panel {
            right_click: _,
            panel_id,
            primary_presentation,
            child_presentations,
        } => {
            for presentation in
                std::iter::once(primary_presentation).chain(child_presentations.iter())
            {
                let Some(binding) = &presentation.binding else {
                    continue;
                };
                if binding.terminal_id.as_str() != terminal_id {
                    continue;
                }
                let route = launch_route_ids(
                    terminal_id,
                    binding.worker_id.as_str(),
                    presentation.presentation_id.as_str(),
                    panel_id.as_str(),
                    tab.tab_id.as_str(),
                    context.terminal_context_id.as_str(),
                )?;
                return Some((route, binding.clone()));
            }
            None
        }
        PanelNode::Split { children, .. } => children
            .iter()
            .find_map(|slot| launch_route_in_node(&slot.node, context, tab, terminal_id)),
    }
}

/// Converts snapshot identities into OBCS identities. Both sides validate the
/// same 32-hex non-zero shape, so failure is unreachable for a snapshot the
/// authority committed — a `None` propagates as `not-found` rather than a panic.
fn launch_route_ids(
    terminal_id: &str,
    worker_id: &str,
    presentation_id: &str,
    panel_id: &str,
    tab_id: &str,
    terminal_context_id: &str,
) -> Option<orchbee_client_server_protocol::TerminalLaunchRoute> {
    use orchbee_client_server_protocol as obcs;
    Some(obcs::TerminalLaunchRoute::new(
        obcs::TerminalId::parse(terminal_id).ok()?,
        obcs::WorkerId::parse(worker_id).ok()?,
        obcs::TerminalLaunchBoundTo::new(
            obcs::PresentationId::parse(presentation_id).ok()?,
            obcs::PanelId::parse(panel_id).ok()?,
            obcs::TabId::parse(tab_id).ok()?,
            obcs::TerminalContextId::parse(terminal_context_id).ok()?,
        ),
    ))
}

/// The retained coordinator fact for a visible binding: exactly one internally
/// consistent record may answer, otherwise the observation is `not-tracked`.
/// The returned provenance is the Server-side record frozen at prepare —
/// `None` both when no record resolves and when the launch never named a
/// profile.
fn retained_launch_fact(
    terminals: &TerminalCoordinator,
    binding: &PresentationBinding,
) -> Option<(
    orchbee_client_server_protocol::TerminalLaunchFact,
    Option<orchbee_client_server_protocol::LaunchProfileProvenance>,
)> {
    let terminal = u128::from_str_radix(binding.terminal_id.as_str(), 16)
        .ok()
        .and_then(|identity| {
            orchbee_capability_node_protocol::TerminalId::from_bytes(identity.to_be_bytes()).ok()
        })?;
    let worker = u128::from_str_radix(binding.worker_id.as_str(), 16)
        .ok()
        .and_then(|identity| {
            orchbee_capability_node_protocol::WorkerId::from_bytes(identity.to_be_bytes()).ok()
        })?;
    let retained = terminals.retained_create_for_binding(terminal, worker)?;
    let state = launch_state(retained.phase);
    let geometry = match retained.phase {
        TerminalPhase::Started { geometry, .. } => Some(geometry),
        _ => None,
    };
    let fact = orchbee_client_server_protocol::TerminalLaunchFact::new(
        state,
        std::num::NonZeroU64::new(retained.worker_generation.get()),
        retained.outcome,
        geometry,
    )
    .ok()?;
    Some((fact, retained.profile_provenance.clone()))
}

fn launch_state(phase: TerminalPhase) -> orchbee_client_server_protocol::TerminalLaunchState {
    use orchbee_client_server_protocol::TerminalLaunchState as State;
    match phase {
        TerminalPhase::Creating => State::Creating,
        TerminalPhase::AwaitingStart(_) => State::AwaitingStart,
        TerminalPhase::Starting(_) => State::Starting,
        TerminalPhase::Started { .. } => State::Started,
        TerminalPhase::Unknown(_) => State::Unknown,
        TerminalPhase::Unavailable(_) => State::Unavailable,
    }
}

fn not_tracked_fact() -> orchbee_client_server_protocol::TerminalLaunchFact {
    let Ok(fact) = orchbee_client_server_protocol::TerminalLaunchFact::new(
        orchbee_client_server_protocol::TerminalLaunchState::NotTracked,
        None,
        None,
        None,
    ) else {
        unreachable!("not-tracked is the empty fact shape");
    };
    fact
}
