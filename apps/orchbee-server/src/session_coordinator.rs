//! Composes committed Workbench membership with retained terminal authorization and physical
//! outcome state under one coordinator owner. It prepares and settles cross-domain operations
//! without performing Worker I/O or owning Client presentation.

use crate::terminal_coordinator::{
    Completion, LaunchCapture, PendingOperation, PreparedOperation,
    TerminalCompletionTokenMismatch, TerminalCoordinator, TerminalPhase, WorkerPlacement,
};
use orchbee_capability_node_protocol::{
    LeaseExpiresAtUnixMilliseconds, TerminalOperationAuthorization,
};
use orchbee_client_server_protocol::{ClientMessage, RequestId, ServerMessage};
use orchbee_server::{
    WorkbenchAuthority, WorkbenchGrant, WorkbenchReadOrNavigationRequest, WorktreeListResolution,
};
use orchbee_workbench_protocol::{
    PresentationBinding, TerminalId, WorkbenchDirectoryId, WorkbenchResponse, WorkerId,
};

#[cfg(any(windows, test))]
mod checkpoint_restore;
mod configured_command;
mod editor_popup;
#[cfg(windows)]
mod held_restore;
mod independent_popup;
#[cfg(windows)]
mod layout_reopen;
mod plugin_popup;
pub(crate) use editor_popup::EditorPopupLaunch;
mod exit_close;
mod tab_close;
pub(crate) use tab_close::IdleCloseMember;
mod tab_set_close;
pub(crate) use tab_set_close::TabSetCloseStart;
mod tab_create;
#[cfg(test)]
pub(crate) use tab_create::PendingTerminalTabCreate;
pub(crate) use tab_create::{PostcommitCoordinationError, PreparedTerminalTabCreate};
mod panel_create;
#[cfg(test)]
pub(crate) use panel_create::PendingTerminalPanelCreate;
pub(crate) use panel_create::PreparedTerminalPanelCreate;
mod terminal_selection;
mod workspace_directory;
mod worktree_restore;
pub(crate) use terminal_selection::{
    BindingViewerPreparationError, PreparedBindingViewer, PreparedSendTarget,
};
mod target_lifecycle;
pub(crate) use target_lifecycle::TargetLifecycle;
mod agent_names;
mod agent_registration;
#[cfg(any(windows, test))]
mod agent_resume;
#[cfg(windows)]
pub(crate) use agent_resume::{ResumeTarget, RunTableEntry, read_run_table};
mod directory_label;
mod pane_graphics;
mod presentation_titles;
mod process_observation;
mod semantic_events;
#[cfg(all(test, windows))]
pub(crate) use process_observation::SETTLE as PROCESS_IDENTITY_SETTLE;
pub(crate) use process_observation::{
    ProcessObservationTarget, decoded_argv, host_time_as_creation, identify as identify_host_agent,
};

pub(crate) struct SessionCoordinator {
    terminals: TerminalCoordinator,
    workbench: Option<WorkbenchAuthority>,
    process_samples: process_observation::Samples,
    followed_directories: directory_label::FollowedDirectories,
    terminal_observations: semantic_events::Observations,
    /// The one Agent cell, shared by managed names and resume capture.
    agents: Option<std::sync::Weak<crate::agent_service::AgentService>>,
    /// Delivered Agent session references carried forward to later captures.
    #[cfg(windows)]
    carried_agent_sessions: agent_resume::CarriedSessions,
}

#[cfg(test)]
mod tests;

impl SessionCoordinator {
    pub(crate) const fn new(workbench: Option<WorkbenchAuthority>) -> Self {
        Self {
            terminals: TerminalCoordinator::new(),
            workbench,
            process_samples: process_observation::Samples::new(),
            followed_directories: directory_label::FollowedDirectories::new(),
            terminal_observations: semantic_events::Observations::new(),
            agents: None,
            #[cfg(windows)]
            carried_agent_sessions: agent_resume::CarriedSessions::new(),
        }
    }

    /// The Session's started terminals, as a `query` reply counts them.
    pub(crate) fn terminal_count(&self) -> usize {
        self.terminals.started_count()
    }

    pub(crate) fn workbench_session_matches(
        &self,
        session: &orchbee_workbench_protocol::SessionId,
    ) -> Option<bool> {
        Some(self.workbench.as_ref()?.session_id() == session)
    }

    pub(crate) fn plugin_pane(
        &mut self,
        panel_id: &orchbee_workbench_protocol::WorkbenchPanelId,
    ) -> Option<orchbee_workbench_protocol::PluginOutcome> {
        Some(self.workbench.as_mut()?.plugin_pane(panel_id))
    }
    pub(crate) fn workspace_metadata(
        &mut self,
        request: orchbee_workbench_protocol::WorkspaceMetadataRequest,
        now: std::time::Instant,
    ) -> Option<orchbee_workbench_protocol::WorkspaceMetadataResult> {
        Some(self.workbench.as_mut()?.workspace_metadata(request, now))
    }

    pub(crate) fn acknowledge_terminal_input(
        &mut self,
        target: crate::terminal_coordinator::BindingViewerTarget,
    ) {
        let Ok(facts) = self.terminal_agent_bindings() else {
            return;
        };
        let terminal = format!(
            "{:032x}",
            u128::from_be_bytes(target.terminal_id.as_bytes())
        );
        let worker = format!(
            "{:032x}",
            u128::from_be_bytes(target.worker.worker_id.as_bytes())
        );
        if let Some(fact) = facts.iter().find(|fact| {
            fact.identity == target.identity
                && fact.worker_generation == target.worker.generation
                && fact.binding.terminal_id.as_str() == terminal
                && fact.binding.worker_id.as_str() == worker
        }) && let Some(workbench) = self.workbench.as_mut()
        {
            let _ = workbench.acknowledge_terminal_agent_input(fact);
        }
    }

    fn terminal_agent_bindings(
        &self,
    ) -> Result<
        Vec<orchbee_server::TerminalAgentBinding>,
        orchbee_server::WorkbenchTerminalInvariantError,
    > {
        Ok(self
            .agent_registration_snapshot()?
            .into_iter()
            .map(|fact| orchbee_server::TerminalAgentBinding {
                binding: fact.binding().clone(),
                worker_generation: fact.worker_generation(),
                identity: fact.identity(),
            })
            .collect())
    }

    pub(crate) fn panel_agent(
        &mut self,
        request: orchbee_workbench_protocol::PanelAgentRequest,
        now: std::time::Instant,
    ) -> Option<orchbee_workbench_protocol::PanelAgentResult> {
        let facts = match self.terminal_agent_bindings() {
            Ok(facts) => facts,
            Err(_) => {
                return Some(orchbee_workbench_protocol::PanelAgentResult {
                    protocol_version: orchbee_workbench_protocol::ProtocolVersion,
                    request_id: request.request_id,
                    session_id: request.session_id,
                    outcome: orchbee_workbench_protocol::PanelAgentOutcome::Failed {
                        reason: orchbee_workbench_protocol::PanelAgentFailure::Unavailable,
                    },
                });
            }
        };
        let release = matches!(&request.action, orchbee_workbench_protocol::PanelAgentAction::Report { report, .. }
            if matches!(report.as_ref(), orchbee_workbench_protocol::PanelAgentReport::Release { .. }));
        let result = self.workbench.as_mut()?.panel_agent(request, &facts, now);
        if let orchbee_workbench_protocol::PanelAgentOutcome::Reported {
            binding,
            disposition,
            view,
            ..
        } = &result.outcome
        {
            let released = release
                && *disposition == orchbee_workbench_protocol::PanelAgentDisposition::Applied
                && view.agent.is_none();
            self.observe_name_owner(binding, released);
        }
        Some(result)
    }

    pub(crate) fn prepare_agent_explanation(
        &mut self,
        request: &orchbee_workbench_protocol::PanelAgentRequest,
        now: std::time::Instant,
    ) -> Result<
        (
            orchbee_server::TerminalAgentExplainContext,
            ProcessObservationTarget,
        ),
        orchbee_workbench_protocol::PanelAgentFailure,
    > {
        use orchbee_workbench_protocol::PanelAgentFailure as Failure;
        let facts = self
            .terminal_agent_bindings()
            .map_err(|_| Failure::Unavailable)?;
        let context = self
            .workbench
            .as_mut()
            .ok_or(Failure::Unavailable)?
            .prepare_terminal_agent_explain(request, &facts, now)?;
        let target = self
            .process_observation_targets()
            .map_err(|_| Failure::Unavailable)?
            .into_iter()
            .find(|target| {
                target.binding == context.physical.binding
                    && target.target.identity == context.physical.identity
                    && target.target.worker.generation == context.physical.worker_generation
            })
            .ok_or(Failure::BindingChanged)?;
        Ok((context, target))
    }

    pub(crate) fn panel_metadata(
        &mut self,
        request: orchbee_workbench_protocol::PanelMetadataRequest,
        now: std::time::Instant,
    ) -> Option<orchbee_workbench_protocol::PanelMetadataResult> {
        let reconciled = self.terminal_agent_bindings().ok().and_then(|facts| {
            self.workbench
                .as_mut()?
                .reconcile_terminal_agent_bindings(&facts)
                .ok()
        });
        if reconciled.is_none() {
            return Some(orchbee_workbench_protocol::PanelMetadataResult {
                protocol_version: orchbee_workbench_protocol::ProtocolVersion,
                request_id: request.request_id,
                session_id: request.session_id,
                outcome: orchbee_workbench_protocol::PanelMetadataOutcome::Failed {
                    reason: orchbee_workbench_protocol::PanelMetadataFailure::Unavailable,
                },
            });
        }
        Some(self.workbench.as_mut()?.panel_metadata(request, now))
    }

    pub(crate) fn worktree_authority(&mut self) -> Option<&mut WorkbenchAuthority> {
        self.workbench.as_mut()
    }

    /// Resolves committed Worktree List membership; `Unavailable` covers an
    /// absent authority so callers need no second poisoning path.
    pub(crate) fn worktree_list_target(
        &self,
        directory_id: &WorkbenchDirectoryId,
    ) -> WorktreeListResolution {
        match self.workbench.as_ref() {
            Some(authority) => authority.worktree_list_target(directory_id),
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
        let session = authority.session_id().clone();
        let grant = if manage_navigation {
            WorkbenchGrant::manage_navigation(session)
        } else {
            WorkbenchGrant::read_only(session)
        };
        Some(authority.execute(Some(&grant), request))
    }

    pub(crate) fn prepare(
        &mut self,
        request: ClientMessage,
        worker: Option<WorkerPlacement>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
        capture: &LaunchCapture,
    ) -> PreparedOperation {
        let start_fenced = match &request {
            ClientMessage::TerminalStart(request) => self
                .terminals
                .retained_authorization(request.terminal_create_request_id())
                .and_then(presentation_binding)
                .is_some_and(|binding| self.binding_has_close(&binding)),
            ClientMessage::TerminalControl(..)
            | ClientMessage::TerminalCreate(..)
            | ClientMessage::TerminalViewer(..)
            | ClientMessage::TerminalSend(..)
            | ClientMessage::TerminalLaunchRead(..) => false,
        };
        self.terminals
            .prepare(start_fenced, request, worker, expiry, capture)
    }

    pub(crate) fn execute_terminal_tab_move(
        &mut self,
        command: orchbee_workbench_protocol::WorkbenchCommand,
    ) -> Option<orchbee_workbench_protocol::WorkbenchCommandResult> {
        use orchbee_server::{WorkbenchTabMoveBegin, WorkbenchTabMovePreparation};
        let Self { workbench, .. } = self;
        let workbench = workbench.as_mut()?;
        let grant = WorkbenchGrant::manage_navigation(workbench.session_id().clone());
        let admission = match workbench.begin_terminal_tab_move(Some(&grant), command) {
            WorkbenchTabMoveBegin::Ready(result) => return Some(result),
            WorkbenchTabMoveBegin::Admitted(admission) => admission,
        };
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
        let Self { workbench, .. } = self;
        let workbench = workbench.as_mut()?;
        let grant = WorkbenchGrant::manage_navigation(workbench.session_id().clone());
        let admission = match workbench.begin_terminal_panel_placement(Some(&grant), command) {
            WorkbenchPanelPlacementBegin::Ready(result) => return Some(result),
            WorkbenchPanelPlacementBegin::Admitted(admission) => admission,
        };
        Some(match admission.finish() {
            WorkbenchPanelPlacementPreparation::Ready(result) => result,
            WorkbenchPanelPlacementPreparation::Prepared(prepared) => prepared.commit(),
        })
    }

    /// Commit retained-Panel topology without creating or disposing a terminal.
    pub(crate) fn execute_terminal_panel_relocate(
        &mut self,
        command: orchbee_workbench_protocol::WorkbenchCommand,
    ) -> Option<orchbee_workbench_protocol::WorkbenchCommandResult> {
        self.execute_terminal_panel_relocate_validated(command, None)
    }

    pub(crate) fn execute_terminal_panel_relocate_validated(
        &mut self,
        command: orchbee_workbench_protocol::WorkbenchCommand,
        directory_worker: Option<orchbee_workbench_protocol::WorkerId>,
    ) -> Option<orchbee_workbench_protocol::WorkbenchCommandResult> {
        use orchbee_server::{WorkbenchPanelRelocateBegin, WorkbenchPanelRelocatePreparation};
        let Self { workbench, .. } = self;
        let workbench = workbench.as_mut()?;
        let grant = WorkbenchGrant::manage_navigation(workbench.session_id().clone())
            .with_directory_association();
        let admission = match workbench.begin_terminal_panel_relocate(Some(&grant), command) {
            WorkbenchPanelRelocateBegin::Ready(result) => return Some(result),
            WorkbenchPanelRelocateBegin::Admitted(admission) => admission,
        };
        let prepared = match directory_worker {
            Some(worker) => admission.finish_with_directory(worker),
            None => admission.finish(),
        };
        Some(match prepared {
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
        let prepared = self
            .terminals
            .prepare_completion(pending, completion, current_worker)?;
        let result = prepared.result().clone();
        prepared.commit();
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
        let (launch, launch_profile, launch_selection) =
            retained_launch_fact(&self.terminals, &binding)
                .unwrap_or_else(|| (not_tracked_fact(), None, None));
        let Some(session_id) =
            orchbee_client_server_protocol::SessionId::parse(snapshot.session_id.as_str()).ok()
        else {
            return Outcome::Unavailable {
                reason:
                    orchbee_client_server_protocol::TerminalLaunchReadUnavailableReason::ServerBusy,
            };
        };
        Outcome::Found {
            session_id,
            workbench_revision: snapshot.revision.value(),
            terminal: Box::new(route),
            launch: Box::new(launch),
            launch_profile,
            launch_selection: launch_selection.map(Box::new),
        }
    }

    pub(crate) fn allocate_identity(&mut self) -> Option<[u8; 16]> {
        self.terminals.allocate_identity()
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
/// The profile and explicit program remain independent optional captured facts;
/// absence of a unique retained record leaves the entire join unavailable.
fn retained_launch_fact(
    terminals: &TerminalCoordinator,
    binding: &PresentationBinding,
) -> Option<(
    orchbee_client_server_protocol::TerminalLaunchFact,
    Option<orchbee_client_server_protocol::LaunchProfileProvenance>,
    Option<orchbee_client_server_protocol::LaunchSelection>,
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
    Some((fact, retained.profile_provenance, retained.launch_selection))
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
