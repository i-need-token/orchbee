//! Bridges admitted Client intents to the Session coordinator and pinned Worker exchanges,
//! retaining viewer, controller, and close-maintenance ownership. It coordinates logical and
//! physical outcomes without implementing Host PTY behavior or owning Client presentation.

mod configuration;
pub(crate) mod configured_commands;
mod independent_popup;
pub(crate) use independent_popup::PopupOwner;
mod observation;
mod pane_graphics;
mod plugins;
pub(crate) mod semantic_events;
mod settings;
use std::{
    num::NonZeroU64,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use orchbee_capability_node_protocol::{CapabilityNodeMessage, LeaseExpiresAtUnixMilliseconds};
use orchbee_client_server_protocol::{
    ClientMessage, CreateOutcome, CreateUnavailableReason, RequestId, ServerMessage, StartOutcome,
    StartUnavailableReason, TerminalCreateResult, TerminalStartResult,
};

use crate::{
    agent::AgentRuntimeSnapshot,
    agent_service::{AgentService, PreparedUnavailable},
    fault_records::{GuardedLock, RecordedLock},
    session_coordinator::SessionCoordinator,
    terminal_coordinator::{Completion, LaunchCapture, PreparedOperation, WorkerPlacement},
    worker_obcn_carrier::{
        WorkerOperationRequest, WorkerOperationResult, WorkerRequestError, WorkerRequestHandle,
    },
    worker_registry::WorkerRegistry,
};

mod agent_catalog;
mod agent_confirmation;
mod agent_drain_read;
mod agent_integration;
mod agent_names;
mod agent_quota_resume;
mod agent_resume;
mod agent_start_delivery;
mod checkpoint;
pub(crate) use checkpoint::CheckpointRestore;
pub(crate) use checkpoint::Offered;
mod close_maintenance;
mod connected_hosts;
mod controller_connection;
mod controller_focus;
mod current_directory;
mod file_bar;
mod moves_stopped;
mod operation_maintenance;
mod panel_agent;
mod panel_create;
mod panel_metadata;
mod panel_restore;
mod process_observer;
mod projection;
mod prompt_line;
mod prompt_readiness;
mod send;
mod tab_close;
mod tab_create;
mod tab_duplicate;
mod tab_set_close;
mod terminal_output;
mod terminal_runtime;
mod viewer_connection;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
mod worker_replacement;
mod workspace_directory;
mod workspace_git;
mod workspace_group_close;
mod workspace_metadata;
mod worktree_list;
mod worktree_operation;
mod worktree_query;

// One queued plus one in-flight exchange, each with a two-second write and twenty-second read.
const LEASE_LIFETIME_MILLISECONDS: u64 = 46_000;

// The private prepared-read budget for one Agent OBAP read: minted once per
// provider call before any preparation lock is taken, so a denied request
// never observes it and an eligible request never re-mints it. It is an
// internal budget only — lock and scheduler delay can overrun it, and its
// expiry surfaces the existing `RuntimeUnavailable` arm, never a client-
// facing guarantee.
const AGENT_DRAIN_READ_WINDOW: Duration = Duration::from_secs(4);
// Cooperative pacing for the `Backpressure` admission arm only: the pump
// sleeps at most this private interval inside the remaining window. It is
// never a no-progress-page delay — an empty page is complete coverage.
const AGENT_DRAIN_READ_RETRY: Duration = Duration::from_millis(25);

pub(crate) struct LocalTerminalService {
    controller_focus: Mutex<controller_focus::FocusRegistry>,
    configuration: configuration::Registry,
    observer: observation::Observer,
    graphics_preparation: Mutex<()>,
    graphics_files: Option<pane_graphics::GraphicsFiles>,
    graphics_stopping: std::sync::atomic::AtomicBool,
    popups: independent_popup::Popups,
    process_observer: process_observer::ProcessObserver,
    close_maintenance: close_maintenance::CloseMaintenance,
    agent_resume: agent_resume::AgentResume,
    agent_start_delivery: agent_start_delivery::AgentStartDelivery,
    agent_confirmations: agent_confirmation::AgentConfirmations,
    moves_stopped: moves_stopped::MovesStopped,
    #[cfg(windows)]
    plugin_hooks: plugins::PluginHooks,
    worktree_maintenance: operation_maintenance::OperationMaintenance,
    group_close_maintenance: operation_maintenance::OperationMaintenance,
    tab_set_maintenance: operation_maintenance::OperationMaintenance,
    tab_duplicate_maintenance: operation_maintenance::OperationMaintenance,
    worktree_discovery: worktree_list::DiscoveryGate,
    terminal_output: std::sync::OnceLock<Arc<crate::terminal_output::TerminalOutput>>,
    terminal_runtime: Arc<crate::terminal_runtime::TerminalRuntime>,
    file_bar: std::sync::OnceLock<Arc<crate::file_bar::FileBar>>,
    workers: Arc<Mutex<WorkerRegistry>>,
    coordinator: RecordedLock<SessionCoordinator>,
    controllers: Mutex<
        std::collections::BTreeMap<
            controller_connection::ControlTarget,
            controller_connection::ControllerReservation,
        >,
    >,
}

impl LocalTerminalService {
    pub(crate) fn new(workers: Arc<Mutex<WorkerRegistry>>) -> Self {
        Self::from_identity(workers, random_session_id())
    }

    fn from_identity(
        workers: Arc<Mutex<WorkerRegistry>>,
        identity: std::io::Result<orchbee_workbench_protocol::SessionId>,
    ) -> Self {
        let workbench = identity
            .ok()
            .and_then(|session| orchbee_server::WorkbenchAuthority::new(session).ok());
        Self::from_workbench(workers, workbench)
    }

    fn from_workbench(
        workers: Arc<Mutex<WorkerRegistry>>,
        workbench: Option<orchbee_server::WorkbenchAuthority>,
    ) -> Self {
        let catalog = workers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .agent_catalog();
        Self {
            controller_focus: Mutex::default(),
            configuration: configuration::Registry::default(),
            observer: observation::Observer::default(),
            graphics_preparation: Mutex::default(),
            graphics_files: None,
            graphics_stopping: std::sync::atomic::AtomicBool::new(false),
            popups: independent_popup::Popups::default(),
            close_maintenance: close_maintenance::CloseMaintenance::default(),
            process_observer: process_observer::ProcessObserver::default(),
            agent_resume: agent_resume::AgentResume::default(),
            agent_start_delivery: agent_start_delivery::AgentStartDelivery::default(),
            agent_confirmations: agent_confirmation::AgentConfirmations::default(),
            moves_stopped: moves_stopped::MovesStopped::default(),
            #[cfg(windows)]
            plugin_hooks: plugins::PluginHooks::default(),
            worktree_maintenance: operation_maintenance::OperationMaintenance::new(
                "orchbee-worktree-operations",
                Self::worktree_turn,
            ),
            group_close_maintenance: operation_maintenance::OperationMaintenance::new(
                "orchbee-workspace-group-close",
                Self::workspace_group_close_turn,
            ),
            tab_set_maintenance: operation_maintenance::OperationMaintenance::new(
                "orchbee-tab-set-close",
                Self::tab_set_close_turn,
            ),
            tab_duplicate_maintenance: operation_maintenance::OperationMaintenance::new(
                "orchbee-tab-duplicate",
                Self::tab_duplicate_turn,
            ),
            worktree_discovery: worktree_list::DiscoveryGate::default(),
            terminal_output: std::sync::OnceLock::new(),
            terminal_runtime: Arc::default(),
            file_bar: std::sync::OnceLock::new(),
            workers,
            coordinator: RecordedLock::new(
                SessionCoordinator::new(workbench).with_agent_catalog(catalog),
                GuardedLock::Coordinator,
            ),
            controllers: Mutex::new(std::collections::BTreeMap::new()),
        }
    }

    /// The Session's terminal-runtime state, which the Worker replacement sets and the
    /// `workbench.terminal-runtime` read answers from.
    pub(crate) fn terminal_runtime(&self) -> &Arc<crate::terminal_runtime::TerminalRuntime> {
        &self.terminal_runtime
    }

    /// The Session's started terminals, saturating at `u32::MAX`.
    pub(crate) fn terminal_count(&self) -> u32 {
        let coordinator = self
            .coordinator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        u32::try_from(coordinator.terminal_count()).unwrap_or(u32::MAX)
    }

    /// Whether a terminal runs, or a launch has not settled. A poisoned coordinator reads as one.
    pub(crate) fn terminal_activity(&self) -> bool {
        self.coordinator
            .lock()
            .map_or(true, |coordinator| coordinator.terminal_activity())
    }

    /// Whether a Workbench operation is pending. A poisoned coordinator reads as one.
    pub(crate) fn operation_pending(&self) -> bool {
        self.coordinator
            .lock()
            .map_or(true, |coordinator| coordinator.operation_pending())
    }

    pub(crate) fn workbench_session_matches(
        &self,
        session: &orchbee_workbench_protocol::SessionId,
    ) -> Option<bool> {
        self.coordinator
            .lock()
            .ok()?
            .workbench_session_matches(session)
    }

    /// The production prepared-read provider for Agent OBAP reads. One
    /// private four-second window is minted here — after the executor's
    /// policy gate has already admitted this request and before any
    /// preparation lock is taken — then the drain pump runs under it with
    /// the private 25 ms backpressure interval. Every failure arm is the
    /// typed `PreparedUnavailable` the executor maps to
    /// `RuntimeUnavailable`; nothing here fabricates a last-good read.
    pub(crate) fn prepare_agent_drain_read(
        &self,
        agents: &AgentService,
    ) -> Result<AgentRuntimeSnapshot, PreparedUnavailable> {
        let deadline = Instant::now() + AGENT_DRAIN_READ_WINDOW;
        agent_drain_read::agent_drain_prepare(
            self.workers.as_ref(),
            &self.coordinator,
            agents,
            deadline,
            AGENT_DRAIN_READ_RETRY,
        )
    }

    pub(crate) fn execute_workbench(
        &self,
        request: orchbee_server::WorkbenchReadOrNavigationRequest,
        manage_navigation: bool,
    ) -> Option<orchbee_workbench_protocol::WorkbenchResponse> {
        self.coordinator
            .lock()
            .ok()?
            .execute_workbench(request, manage_navigation)
    }

    pub(crate) fn execute(&self, request: ClientMessage) -> ServerMessage {
        if let ClientMessage::TerminalSend(request) = request {
            return self.execute_send(&request);
        }
        if let ClientMessage::TerminalLaunchRead(request) = request {
            // A read-only coordinator join: no worker handle, no pending
            // operation, no dispatch. The busy answer covers only a poisoned
            // coordinator lock; an uncontended wait still blocks normally.
            let outcome = self.coordinator.lock().map_or(
                orchbee_client_server_protocol::TerminalLaunchReadOutcome::Unavailable {
                    reason:
                        orchbee_client_server_protocol::TerminalLaunchReadUnavailableReason::ServerBusy,
                },
                |coordinator| {
                    coordinator.terminal_launch_read(request.terminal_id().as_str())
                },
            );
            return ServerMessage::TerminalLaunchReadResult(
                orchbee_client_server_protocol::TerminalLaunchReadResult::new(
                    request.request_id().clone(),
                    outcome,
                ),
            );
        }
        let busy = busy_result(&request);
        // Only a creation reads the Session's settings, and it does so before either lock. A raw
        // terminal creation binds no Presentation, so it needs no reported System shell.
        let capture = match &request {
            ClientMessage::TerminalCreate(_) => self.launch_capture(),
            _ => LaunchCapture::unavailable(),
        };
        let (prepared, handle) = {
            let Ok(workers) = self.workers.lock() else {
                return busy;
            };
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return busy;
            };
            // A creation an exact constraint names, and the start of a deferred terminal, go to
            // that terminal's Worker; any other creation goes to the local Worker.
            let addressed = match &request {
                ClientMessage::TerminalCreate(create) => match create.worker_constraint() {
                    Some(orchbee_client_server_protocol::WorkerConstraint::Exact { worker_id }) => {
                        Some(worker_named(worker_id.as_str()))
                    }
                    _ => None,
                },
                ClientMessage::TerminalStart(start) => coordinator
                    .reference_worker(start.terminal_create_request_id())
                    .map(Some),
                _ => None,
            };
            let handle = match addressed {
                Some(worker) => worker.and_then(|worker| workers.handle_for_worker(worker)),
                None => workers.request_handle(),
            };
            (
                coordinator.prepare(
                    request,
                    placement(handle.as_ref()),
                    lease_expiry(),
                    &capture,
                ),
                handle,
            )
        };
        let pending = match prepared {
            PreparedOperation::Ready(result) => return result,
            PreparedOperation::Dispatch(pending) => pending,
        };
        let dispatched = placement(handle.as_ref());
        let completion = handle.map_or(Completion::Unavailable, |handle| {
            self.submit(&handle, pending.command())
        });
        // Retain registry ownership through commit, so a replacement cannot race this snapshot.
        let Ok(workers) = self.workers.lock() else {
            return unknown_result(&busy);
        };
        let Ok(mut coordinator) = self.coordinator.lock() else {
            return unknown_result(&busy);
        };
        match coordinator.complete(pending, completion, current_placement(&workers, dispatched)) {
            Ok(result) => result,
            Err(_mismatch) => unknown_result(&busy),
        }
    }

    pub(crate) fn execute_terminal_tab_move(
        &self,
        command: orchbee_workbench_protocol::WorkbenchCommand,
    ) -> Option<orchbee_workbench_protocol::WorkbenchCommandResult> {
        self.coordinator
            .lock()
            .ok()?
            .execute_terminal_tab_move(command)
    }

    pub(crate) fn execute_terminal_panel_placement(
        &self,
        command: orchbee_workbench_protocol::WorkbenchCommand,
    ) -> Option<orchbee_workbench_protocol::WorkbenchCommandResult> {
        self.coordinator
            .lock()
            .ok()?
            .execute_terminal_panel_placement(command)
    }

    pub(crate) fn execute_terminal_panel_relocate(
        &self,
        command: orchbee_workbench_protocol::WorkbenchCommand,
    ) -> Option<orchbee_workbench_protocol::WorkbenchCommandResult> {
        self.coordinator
            .lock()
            .ok()?
            .execute_terminal_panel_relocate(command)
    }
}

fn random_session_id() -> std::io::Result<orchbee_workbench_protocol::SessionId> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|error| std::io::Error::other(error.to_string()))?;
    orchbee_workbench_protocol::SessionId::parse(&format!("{:032x}", u128::from_be_bytes(bytes)))
        .map_err(std::io::Error::other)
}

fn create_reference() -> Option<RequestId> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    RequestId::parse(&format!("{:032x}", u128::from_be_bytes(bytes))).ok()
}

#[cfg(test)]
mod tests;

/// The `orchbee` CLI shipped beside this Server, which configured commands and plugins invoke.
fn cli_executable() -> Option<std::path::PathBuf> {
    let executable = orchbee_local_runtime::CohortArtifact::Client.file_name();
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(executable)))
}

impl LocalTerminalService {
    fn submit(&self, handle: &WorkerRequestHandle, message: &CapabilityNodeMessage) -> Completion {
        let request = match message {
            // Planning attached the capture the launch dispatches with.
            CapabilityNodeMessage::AuthorizedTerminalLaunch(request) => {
                WorkerOperationRequest::Launch(request.clone())
            }
            CapabilityNodeMessage::AuthorizedTerminalStart(request) => {
                WorkerOperationRequest::Start(*request)
            }
            // This coordinator path handles only authorized Create/Start operations; other OBCN families use their dedicated paths.
            _ => return Completion::Unavailable,
        };
        submit_request(handle, request)
    }
}

fn submit_request(handle: &WorkerRequestHandle, request: WorkerOperationRequest) -> Completion {
    match handle.request(request) {
        #[cfg(any(windows, test))]
        Ok(WorkerOperationResult::Handoff(_)) => Completion::Unknown,
        Ok(WorkerOperationResult::MoveTurn(_)) => Completion::Unknown,
        Ok(WorkerOperationResult::Shutdown(_)) => Completion::Unknown,
        Ok(WorkerOperationResult::Launch(result)) => Completion::Launch(result),
        Ok(WorkerOperationResult::Start(result)) => Completion::Start(result),
        Ok(
            WorkerOperationResult::CwdValidation(_)
            | WorkerOperationResult::Viewer(_)
            | WorkerOperationResult::Control(_)
            | WorkerOperationResult::Lifecycle(_)
            | WorkerOperationResult::Send(_)
            | WorkerOperationResult::WorktreeJob(_)
            | WorkerOperationResult::PluginJob(_)
            | WorkerOperationResult::AgentIntegrationJob(_)
            | WorkerOperationResult::AgentProbeJob(_)
            | WorkerOperationResult::FilesProcess(_)
            | WorkerOperationResult::WorktreeMutation(_),
        ) => Completion::Unknown,
        // The drain variant can never come back from the Launch/Start
        // requests submitted here; it exists for the Agent drain pump.
        Ok(WorkerOperationResult::Titles(_))
        | Ok(WorkerOperationResult::TitleChanges(_))
        | Ok(WorkerOperationResult::HostQuery(_))
        | Ok(WorkerOperationResult::AdoptHost(_))
        | Ok(WorkerOperationResult::AgentDrain(_)) => Completion::Unknown,
        Err(WorkerRequestError::Backpressure) => Completion::Busy,
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(_)) => Completion::Unknown,
        Err(
            WorkerRequestError::Unavailable
            | WorkerRequestError::WrongWorker
            | WorkerRequestError::WrongGeneration
            | WorkerRequestError::InvalidRequest(_)
            | WorkerRequestError::NotDelivered(_)
            | WorkerRequestError::Declined,
        ) => Completion::Unavailable,
    }
}
fn placement(handle: Option<&WorkerRequestHandle>) -> Option<WorkerPlacement> {
    handle.and_then(WorkerPlacement::of)
}
/// The carrier a new terminal of a Tab or Panel command is created on: that of the Worker of its
/// Workspace's primary Directory, on whichever node it runs. A Workspace without one names the
/// local Worker, which admission then refuses.
fn creation_handle(
    workers: &WorkerRegistry,
    coordinator: &crate::session_coordinator::SessionCoordinator,
    command: &orchbee_workbench_protocol::WorkbenchCommand,
) -> Option<WorkerRequestHandle> {
    use orchbee_workbench_protocol::WorkbenchAction;
    let context = match &command.action {
        WorkbenchAction::TabCreate {
            terminal_context_id,
            ..
        }
        | WorkbenchAction::PanelSplit {
            terminal_context_id,
            ..
        }
        | WorkbenchAction::PanelCreateTerminal {
            terminal_context_id,
            ..
        }
        | WorkbenchAction::PanelRestoreTerminal {
            terminal_context_id,
            ..
        } => Some(terminal_context_id),
        // A Workspace launch validates its new Directory on the Worker that will hold it.
        _ => None,
    };
    match context.and_then(|context| coordinator.context_worker(context)) {
        Some(worker) => workers.handle_for_worker(worker),
        None => workers.request_handle(),
    }
}
/// The placement of the generation that serves `worker`'s Worker now, on whichever node it runs,
/// against which a completion of an operation dispatched to `worker` settles.
fn current_placement(
    workers: &WorkerRegistry,
    worker: Option<WorkerPlacement>,
) -> Option<WorkerPlacement> {
    placement(workers.handle_for_worker(worker?.worker_id).as_ref())
}
/// The serving placement of the Worker a wire id names, on whichever node it runs.
fn current_placement_of(workers: &WorkerRegistry, worker_id: &str) -> Option<WorkerPlacement> {
    placement(
        worker_named(worker_id)
            .and_then(|worker| workers.handle_for_worker(worker))
            .as_ref(),
    )
}
/// The placement of each Worker serving the Session now, local or remote, once per Worker.
fn serving_placements(workers: &WorkerRegistry) -> Vec<WorkerPlacement> {
    workers
        .serving_handles()
        .iter()
        .filter_map(WorkerPlacement::of)
        .collect()
}
/// The carrier among `serving` through which `worker_id`'s Worker serves as exactly `generation`.
fn target_handle(
    serving: &[WorkerRequestHandle],
    worker_id: orchbee_capability_node_protocol::WorkerId,
    generation: orchbee_capability_node_protocol::WorkerGeneration,
) -> Option<&WorkerRequestHandle> {
    let target = WorkerPlacement {
        worker_id,
        generation,
    };
    serving
        .iter()
        .find(|handle| WorkerPlacement::of(handle) == Some(target))
}
/// The Worker a wire id names: 32 lowercase hexadecimal digits, not all zero.
fn worker_named(text: &str) -> Option<orchbee_capability_node_protocol::WorkerId> {
    if text.len() != 32
        || !text
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return None;
    }
    let value = u128::from_str_radix(text, 16).ok()?;
    orchbee_capability_node_protocol::WorkerId::from_bytes(value.to_be_bytes()).ok()
}
/// The Worker incarnation a retained operation was admitted under, in Workbench terms.
fn worker_identity(
    handle: Option<&WorkerRequestHandle>,
) -> Option<(orchbee_workbench_protocol::WorkerId, NonZeroU64)> {
    let handle = handle?;
    let id = orchbee_workbench_protocol::WorkerId::parse(&format!(
        "{:032x}",
        u128::from_be_bytes(handle.worker_id().as_bytes())
    ))
    .ok()?;
    Some((id, handle.generation()))
}
fn lease_expiry() -> Option<LeaseExpiresAtUnixMilliseconds> {
    let now = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis(),
    )
    .ok()?;
    Some(LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::new(
        now.checked_add(LEASE_LIFETIME_MILLISECONDS)?,
    )?))
}
fn busy_result(request: &ClientMessage) -> ServerMessage {
    match request {
        ClientMessage::TerminalControl(request) => ServerMessage::TerminalControlResult(
            orchbee_client_server_protocol::TerminalControlResult::new(
                request.request_id().clone(),
                orchbee_client_server_protocol::TerminalControlOutcome::Unavailable {
                    reason: orchbee_client_server_protocol::ControlUnavailableReason::ServerBusy,
                },
            ),
        ),
        ClientMessage::TerminalViewer(request) => ServerMessage::TerminalViewerResult(
            orchbee_client_server_protocol::TerminalViewerResult::new(
                request.request_id().clone(),
                orchbee_client_server_protocol::TerminalViewerOutcome::Unavailable {
                    reason: orchbee_client_server_protocol::ViewerUnavailableReason::ServerBusy,
                },
            ),
        ),
        ClientMessage::TerminalCreate(request) => {
            ServerMessage::TerminalCreateResult(TerminalCreateResult::new(
                request.request_id().clone(),
                CreateOutcome::Unavailable {
                    reason: CreateUnavailableReason::ServerBusy,
                },
            ))
        }
        ClientMessage::TerminalStart(request) => {
            ServerMessage::TerminalStartResult(TerminalStartResult::new(
                request.request_id().clone(),
                StartOutcome::Unavailable {
                    reason: StartUnavailableReason::ServerBusy,
                },
            ))
        }
        ClientMessage::TerminalSend(request) => ServerMessage::TerminalSendResult(
            match orchbee_client_server_protocol::TerminalSendResult::new(
                request.request_id().clone(),
                None,
                orchbee_client_server_protocol::TerminalSendOutcome::Unavailable {
                    reason: orchbee_client_server_protocol::SendUnavailableReason::ServerBusy,
                    progress: orchbee_client_server_protocol::SendProgress::none(),
                    cleanup: orchbee_client_server_protocol::SendCleanupStatus::NotNeeded,
                },
            ) {
                Ok(result) => result,
                Err(_) => unreachable!("validated send outcomes are encodable"),
            },
        ),
        ClientMessage::TerminalLaunchRead(request) => ServerMessage::TerminalLaunchReadResult(
            orchbee_client_server_protocol::TerminalLaunchReadResult::new(
                request.request_id().clone(),
                orchbee_client_server_protocol::TerminalLaunchReadOutcome::Unavailable {
                    reason:
                        orchbee_client_server_protocol::TerminalLaunchReadUnavailableReason::ServerBusy,
                },
            ),
        ),
    }
}
fn unknown_result(result: &ServerMessage) -> ServerMessage {
    match result {
        ServerMessage::TerminalControlResult(result) => ServerMessage::TerminalControlResult(orchbee_client_server_protocol::TerminalControlResult::new(result.request_id().clone(), orchbee_client_server_protocol::TerminalControlOutcome::OutcomeUnknown {})),
        ServerMessage::TerminalViewerResult(result) => ServerMessage::TerminalViewerResult(orchbee_client_server_protocol::TerminalViewerResult::new(
            result.request_id().clone(), orchbee_client_server_protocol::TerminalViewerOutcome::Unavailable {
                reason: orchbee_client_server_protocol::ViewerUnavailableReason::TerminalRuntimeUnavailable,
            })),
        ServerMessage::TerminalCreateResult(result) => ServerMessage::TerminalCreateResult(
            TerminalCreateResult::new(result.request_id().clone(), CreateOutcome::OutcomeUnknown),
        ),
        ServerMessage::TerminalStartResult(result) => ServerMessage::TerminalStartResult(
            TerminalStartResult::new(result.request_id().clone(), StartOutcome::OutcomeUnknown),
        ),
        ServerMessage::TerminalSendResult(result) => ServerMessage::TerminalSendResult(
            match orchbee_client_server_protocol::TerminalSendResult::new(
                result.request_id().clone(),
                result.target().cloned(),
                orchbee_client_server_protocol::TerminalSendOutcome::OutcomeUnknown {
                    progress: orchbee_client_server_protocol::SendProgress::none(),
                    cleanup: orchbee_client_server_protocol::SendCleanupStatus::Unconfirmed,
                },
            )
            .or_else(|_| {
                orchbee_client_server_protocol::TerminalSendResult::new(
                    result.request_id().clone(),
                    None,
                    orchbee_client_server_protocol::TerminalSendOutcome::Unavailable {
                        reason: orchbee_client_server_protocol::SendUnavailableReason::ServerBusy,
                        progress: orchbee_client_server_protocol::SendProgress::none(),
                        cleanup: orchbee_client_server_protocol::SendCleanupStatus::NotNeeded,
                    },
                )
            }) {
                Ok(result) => result,
                Err(_) => unreachable!("validated send outcomes are encodable"),
            },
        ),
        ServerMessage::TerminalLaunchReadResult(result) => {
            ServerMessage::TerminalLaunchReadResult(
                orchbee_client_server_protocol::TerminalLaunchReadResult::new(
                    result.request_id().clone(),
                    orchbee_client_server_protocol::TerminalLaunchReadOutcome::Unavailable {
                        reason:
                            orchbee_client_server_protocol::TerminalLaunchReadUnavailableReason::ServerBusy,
                    },
                ),
            )
        }
    }
}
