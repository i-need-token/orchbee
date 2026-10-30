//! Bridges admitted Client intents to the Session coordinator and pinned Worker exchanges,
//! retaining viewer, controller, and close-maintenance ownership. It coordinates logical and
//! physical outcomes without implementing Host PTY behavior or owning Client presentation.

mod configuration;
pub(crate) mod configured_commands;
mod independent_popup;
pub(crate) use independent_popup::PopupOwner;
mod observation;
#[cfg(windows)]
mod pane_graphics;
mod plugins;
#[cfg(windows)]
pub(crate) mod semantic_events;
mod user_preferences;
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
    session_coordinator::SessionCoordinator,
    terminal_coordinator::{Completion, LaunchCapture, PreparedOperation, WorkerPlacement},
    worker_obcn_carrier::{
        WorkerOperationRequest, WorkerOperationResult, WorkerRequestError, WorkerRequestHandle,
    },
    worker_registry::WorkerRegistry,
};

mod agent_drain_read;
#[cfg(windows)]
mod agent_names;
#[cfg(windows)]
mod checkpoint;
#[cfg(windows)]
pub(crate) use checkpoint::CheckpointRestore;
mod close_maintenance;
mod controller_connection;
mod operation_maintenance;
mod panel_agent;
mod panel_create;
mod panel_metadata;
mod process_observer;
mod projection;
mod send;
mod tab_close;
mod tab_create;
mod viewer_connection;
pub(crate) use viewer_connection::LocalTerminalConnection;
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
    preferences: std::sync::OnceLock<orchbee_local_runtime::UserPreferencesStore>,
    configuration: configuration::Registry,
    observer: observation::Observer,
    #[cfg(windows)]
    graphics_preparation: Mutex<()>,
    #[cfg(windows)]
    graphics_files: Option<pane_graphics::GraphicsFiles>,
    #[cfg(windows)]
    graphics_stopping: std::sync::atomic::AtomicBool,
    popups: independent_popup::Popups,
    process_observer: process_observer::ProcessObserver,
    close_maintenance: close_maintenance::CloseMaintenance,
    worktree_maintenance: operation_maintenance::OperationMaintenance,
    group_close_maintenance: operation_maintenance::OperationMaintenance,
    worktree_discovery: worktree_list::DiscoveryGate,
    workers: Arc<Mutex<WorkerRegistry>>,
    coordinator: Mutex<SessionCoordinator>,
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
        Self {
            preferences: std::sync::OnceLock::new(),
            configuration: configuration::Registry::default(),
            observer: observation::Observer::default(),
            #[cfg(windows)]
            graphics_preparation: Mutex::default(),
            #[cfg(windows)]
            graphics_files: None,
            #[cfg(windows)]
            graphics_stopping: std::sync::atomic::AtomicBool::new(false),
            popups: independent_popup::Popups::default(),
            close_maintenance: close_maintenance::CloseMaintenance::default(),
            process_observer: process_observer::ProcessObserver::default(),
            worktree_maintenance: operation_maintenance::OperationMaintenance::new(
                "orchbee-worktree-operations",
                Self::worktree_turn,
            ),
            group_close_maintenance: operation_maintenance::OperationMaintenance::new(
                "orchbee-workspace-group-close",
                Self::workspace_group_close_turn,
            ),
            worktree_discovery: worktree_list::DiscoveryGate::default(),
            workers,
            coordinator: Mutex::new(SessionCoordinator::new(workbench)),
            controllers: Mutex::new(std::collections::BTreeMap::new()),
        }
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
        // Only a creation reads the preference store, and it does so before either lock. A raw
        // terminal creation binds no Presentation, so it needs no reported System shell.
        let capture = match &request {
            ClientMessage::TerminalCreate(_) => self.launch_capture(),
            _ => LaunchCapture::unavailable(),
        };
        let (prepared, handle) = {
            let Ok(workers) = self.workers.lock() else {
                return busy;
            };
            let handle = workers.request_handle();
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return busy;
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
        match coordinator.complete(
            pending,
            completion,
            placement(workers.request_handle().as_ref()),
        ) {
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
    let executable = format!("orchbee{}", std::env::consts::EXE_SUFFIX);
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
        #[cfg(any(windows, test))]
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
            | WorkerOperationResult::WorktreeMutation(_),
        ) => Completion::Unknown,
        // The drain variant can never come back from the Launch/Start
        // requests submitted here; it exists for the Agent drain pump.
        Ok(WorkerOperationResult::EditorCommand(_))
        | Ok(WorkerOperationResult::Titles(_))
        | Ok(WorkerOperationResult::AgentDrain(_)) => Completion::Unknown,
        Err(WorkerRequestError::Backpressure) => Completion::Busy,
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(_)) => Completion::Unknown,
        Err(
            WorkerRequestError::Unavailable
            | WorkerRequestError::WrongWorker
            | WorkerRequestError::WrongGeneration
            | WorkerRequestError::InvalidRequest(_)
            | WorkerRequestError::NotDelivered(_),
        ) => Completion::Unavailable,
    }
}
fn placement(handle: Option<&WorkerRequestHandle>) -> Option<WorkerPlacement> {
    handle.and_then(WorkerPlacement::of)
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
