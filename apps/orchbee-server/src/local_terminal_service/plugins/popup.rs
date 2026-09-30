//! One connection owns a popup; EOF transfers accepted physical ownership to Site maintenance.
use super::super::{LocalTerminalService, lease_expiry, placement, submit};
use crate::{
    site_coordinator::ProcessObservationTarget,
    terminal_coordinator::{BindingViewerTarget, PreparedOperation},
    worker_obcn_carrier::{WorkerOperationRequest, WorkerOperationResult},
};
use orchbee_capability_node_protocol as node;
use orchbee_client_server_protocol as client;
use orchbee_workbench_protocol::{
    PluginAction, PluginFailure as Failure, PluginOutcome as Outcome, PluginRequest, PluginResult,
    PresentationBinding, ProtocolVersion, TerminalId, WorkbenchRequestId, WorkerId,
};
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    sync::Mutex,
    time::{Duration, Instant},
};

const MAXIMUM_OWNERS: usize = 32;
const SETTLEMENT_WINDOW: Duration = Duration::from_secs(5);
const OBSERVATION_INTERVAL: Duration = Duration::from_millis(100);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Opening,
    Active,
    Queued,
    Waiting,
    Dispose,
    Working,
    Unknown,
    Closed,
}
#[derive(Clone)]
struct Popup {
    id: WorkbenchRequestId,
    reference: client::RequestId,
    authorization: Option<node::TerminalOperationAuthorization>,
    target: Option<BindingViewerTarget>,
    phase: Phase,
    connected: bool,
    ordinal: u8,
    cursor: Option<node::LifecycleInventoryCursor>,
    pages: u8,
    deadline: Option<Instant>,
    next: Instant,
}
#[derive(Default)]
pub(in crate::local_terminal_service) struct Popups(Mutex<BTreeMap<NonZeroU64, Popup>>);

impl LocalTerminalService {
    pub(crate) fn execute_popup(&self, owner: NonZeroU64, request: PluginRequest) -> PluginResult {
        let outcome = self
            .popup_operation(owner, &request)
            .unwrap_or_else(|reason| {
                if let PluginAction::PopupOpen { popup_id, .. } = &request.action
                    && let Ok(mut owners) = self.popups.0.lock()
                    && let Some(popup) = owners.get_mut(&owner)
                    && popup.id == *popup_id
                    && (popup.phase == Phase::Unknown
                        || popup.phase == Phase::Opening && popup.authorization.is_some())
                {
                    popup.phase = Phase::Unknown;
                    return Outcome::PopupUnknown {
                        popup_id: popup_id.clone(),
                    };
                }
                Outcome::Failed { reason }
            });
        PluginResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome,
        }
    }
    fn popup_operation(
        &self,
        owner: NonZeroU64,
        request: &PluginRequest,
    ) -> Result<Outcome, Failure> {
        request.validate().map_err(|_| Failure::InvalidIntent)?;
        if self.workbench_site_matches(&request.site_id) != Some(true) {
            return Err(Failure::SiteMismatch);
        }
        match &request.action {
            PluginAction::PopupOpen {
                popup_id,
                spec,
                geometry,
            } => {
                // Admission is recorded before any possible physical side effect.
                if !self.close_maintenance.available() {
                    return Err(Failure::Unavailable);
                }
                let reference = client::RequestId::parse(popup_id.as_str())
                    .map_err(|_| Failure::InvalidIntent)?;
                {
                    let mut owners = self.popups.0.lock().map_err(|_| Failure::Unavailable)?;
                    if owners.contains_key(&owner) || owners.len() >= MAXIMUM_OWNERS {
                        return Err(Failure::Busy);
                    }
                    // A popup ID cannot authorize another owner's retained resource.
                    if owners.values().any(|popup| popup.id == *popup_id) {
                        return Err(Failure::Conflict);
                    }
                    owners.insert(
                        owner,
                        Popup {
                            id: popup_id.clone(),
                            reference: reference.clone(),
                            authorization: None,
                            target: None,
                            phase: Phase::Opening,
                            connected: true,
                            ordinal: 1,
                            cursor: None,
                            pages: 0,
                            deadline: None,
                            next: Instant::now(),
                        },
                    );
                }
                let prepared = (|| {
                    let workers = self.workers.lock().map_err(|_| Failure::Unavailable)?;
                    let handle = workers.request_handle();
                    let mut coordinator =
                        self.coordinator.lock().map_err(|_| Failure::Unavailable)?;
                    let (prepared, title, authorization) = coordinator.prepare_plugin_popup(
                        popup_id,
                        spec,
                        *geometry,
                        placement(handle.as_ref()),
                        lease_expiry(),
                    )?;
                    let mut owners = self.popups.0.lock().map_err(|_| Failure::Unavailable)?;
                    let popup = owners.get_mut(&owner).ok_or(Failure::Unavailable)?;
                    popup.authorization = authorization;
                    Ok((prepared, title, handle))
                })();
                let (prepared, title, handle) = match prepared {
                    Ok(value) => value,
                    Err(reason) => {
                        // No dispatch occurred. A reserved binding, if any, remains owned.
                        let mut owners = self.popups.0.lock().map_err(|_| Failure::Unavailable)?;
                        if owners
                            .get(&owner)
                            .is_some_and(|popup| popup.authorization.is_none())
                        {
                            owners.remove(&owner);
                        } else if let Some(popup) = owners.get_mut(&owner) {
                            popup.phase = Phase::Unknown;
                        }
                        return Err(reason);
                    }
                };
                let result = match prepared {
                    PreparedOperation::Ready(result) => result,
                    PreparedOperation::Dispatch(pending) => {
                        let completion = handle.as_ref().map_or(
                            crate::terminal_coordinator::Completion::Unavailable,
                            |handle| submit(handle, pending.command()),
                        );
                        let workers = self.workers.lock().map_err(|_| Failure::PhysicalUnknown)?;
                        self.coordinator
                            .lock()
                            .map_err(|_| Failure::PhysicalUnknown)?
                            .complete(
                                pending,
                                completion,
                                placement(workers.request_handle().as_ref()),
                            )
                            .map_err(|_| Failure::PhysicalUnknown)?
                    }
                };
                let workers = self.workers.lock().map_err(|_| Failure::PhysicalUnknown)?;
                let mut coordinator = self
                    .coordinator
                    .lock()
                    .map_err(|_| Failure::PhysicalUnknown)?;
                let target = placement(workers.request_handle().as_ref())
                    .and_then(|worker| coordinator.viewer_target(&reference, worker));
                let mut owners = self.popups.0.lock().map_err(|_| Failure::PhysicalUnknown)?;
                let popup = owners.get_mut(&owner).ok_or(Failure::PhysicalUnknown)?;
                match result {
                    client::ServerMessage::TerminalCreateResult(result)
                        if matches!(
                            result.outcome(),
                            client::CreateOutcome::Accepted {
                                state: client::CreateAcceptedState::Started { .. }
                            }
                        ) && target.is_some() =>
                    {
                        popup.target = target;
                        popup.phase = Phase::Active;
                        Ok(Outcome::PopupOpened {
                            popup_id: popup_id.clone(),
                            title,
                        })
                    }
                    _ if popup.authorization.is_none()
                        || popup
                            .authorization
                            .is_some_and(|auth| coordinator.popup_definitively_uncreated(auth)) =>
                    {
                        if let Some(auth) = popup.authorization {
                            coordinator.release_plugin_popup(&reference, auth);
                        }
                        owners.remove(&owner);
                        Err(Failure::Unavailable)
                    }
                    _ => {
                        popup.phase = Phase::Unknown;
                        Ok(Outcome::PopupUnknown {
                            popup_id: popup_id.clone(),
                        })
                    }
                }
            }
            PluginAction::PopupClose { popup_id } => {
                let mut owners = self.popups.0.lock().map_err(|_| Failure::Unavailable)?;
                let popup = owners
                    .get_mut(&owner)
                    .filter(|popup| popup.id == *popup_id)
                    .ok_or(Failure::NotFound)?;
                let outcome = match popup.phase {
                    Phase::Closed => Outcome::PopupClosed {
                        popup_id: popup_id.clone(),
                    },
                    Phase::Unknown => Outcome::PopupUnknown {
                        popup_id: popup_id.clone(),
                    },
                    Phase::Opening => return Err(Failure::Busy),
                    _ => {
                        if popup.phase == Phase::Active {
                            popup.phase = Phase::Queued;
                            popup.next = Instant::now();
                        }
                        Outcome::PopupClosing {
                            popup_id: popup_id.clone(),
                        }
                    }
                };
                let fence = popup
                    .authorization
                    .map(|auth| (popup.reference.clone(), auth));
                drop(owners);
                if let Some((reference, authorization)) = fence {
                    self.coordinator
                        .lock()
                        .map_err(|_| Failure::Unavailable)?
                        .fence_plugin_popup(&reference, authorization);
                }
                self.close_maintenance.notify();
                Ok(outcome)
            }
            PluginAction::PopupRead { popup_id } => {
                let popup = self
                    .popups
                    .0
                    .lock()
                    .map_err(|_| Failure::Unavailable)?
                    .get(&owner)
                    .filter(|popup| popup.id == *popup_id)
                    .cloned()
                    .ok_or(Failure::NotFound)?;
                match popup.phase {
                    Phase::Closed => Ok(Outcome::PopupClosed {
                        popup_id: popup_id.clone(),
                    }),
                    Phase::Unknown => Ok(Outcome::PopupUnknown {
                        popup_id: popup_id.clone(),
                    }),
                    Phase::Active => {
                        let target = popup.target.ok_or(Failure::Unavailable)?;
                        let observation = ProcessObservationTarget {
                            site_id: request.site_id.clone(),
                            binding: PresentationBinding {
                                worker_id: WorkerId::parse(&hex(target
                                    .worker
                                    .worker_id
                                    .as_bytes()))
                                .map_err(|_| Failure::Unavailable)?,
                                terminal_id: TerminalId::parse(&hex(target.terminal_id.as_bytes()))
                                    .map_err(|_| Failure::Unavailable)?,
                            },
                            target,
                        };
                        let settled =
                            self.observe_plugin_runtime(&observation, Some(&popup.reference))?;
                        let owners = self.popups.0.lock().map_err(|_| Failure::Unavailable)?;
                        if !owners.get(&owner).is_some_and(|current| {
                            current.id == popup.id
                                && current.phase == Phase::Active
                                && current.target == popup.target
                        }) {
                            return Err(Failure::Conflict);
                        }
                        Ok(Outcome::PopupState {
                            popup_id: popup_id.clone(),
                            settled,
                        })
                    }
                    _ => Ok(Outcome::PopupState {
                        popup_id: popup_id.clone(),
                        settled: false,
                    }),
                }
            }
            _ => Err(Failure::InvalidIntent),
        }
    }
    pub(crate) fn disconnect_popup(&self, owner: NonZeroU64) {
        if let Ok(mut owners) = self.popups.0.lock()
            && let Some(popup) = owners.get_mut(&owner)
        {
            popup.connected = false;
            match popup.phase {
                Phase::Closed => {
                    owners.remove(&owner);
                }
                Phase::Active => {
                    popup.phase = Phase::Queued;
                    popup.next = Instant::now();
                }
                // An interrupted Open remains retained, never silently released.
                Phase::Opening => {
                    popup.phase = Phase::Unknown;
                }
                _ => {}
            }
        }
        self.close_maintenance.notify();
    }

    /// One exact lifecycle step per turn. Unknown mutation is parked, never resubmitted.
    pub(crate) fn popup_maintenance_turn(&self) -> Option<Instant> {
        let selected = {
            let mut owners = self.popups.0.lock().ok()?;
            owners
                .iter_mut()
                .find(|(_, popup)| {
                    matches!(popup.phase, Phase::Queued | Phase::Waiting | Phase::Dispose)
                        && popup.next <= Instant::now()
                })
                .map(|(owner, popup)| {
                    let before = popup.clone();
                    popup.phase = Phase::Working;
                    (*owner, before)
                })
        };
        if let Some((owner, mut popup)) = selected {
            let previous = popup.phase;
            let action = match previous {
                Phase::Queued => {
                    popup.deadline = Instant::now().checked_add(SETTLEMENT_WINDOW);
                    node::TerminalLifecycleAction::Force
                }
                Phase::Waiting => node::TerminalLifecycleAction::InventoryPage {
                    cursor: popup.cursor,
                },
                Phase::Dispose => node::TerminalLifecycleAction::Dispose,
                _ => unreachable!("only actionable phases are selected"),
            };
            let result = (|| {
                if previous != Phase::Queued
                    && popup
                        .deadline
                        .is_none_or(|deadline| Instant::now() >= deadline)
                {
                    return None;
                }
                let target = popup.target?;
                let authorization = popup.authorization?;
                let dispatch = {
                    let workers = self.workers.lock().ok()?;
                    let handle = workers.request_handle()?;
                    if placement(Some(&handle)) != Some(target.worker) {
                        return None;
                    }
                    let mut coordinator = self.coordinator.lock().ok()?;
                    if !coordinator.fence_plugin_popup(&popup.reference, authorization) {
                        return None;
                    }
                    let request = coordinator.popup_lifecycle_request(
                        &popup.id,
                        authorization,
                        target,
                        popup.ordinal,
                        action,
                        lease_expiry()?,
                    )?;
                    (handle, request)
                };
                let (handle, request) = dispatch;
                match handle.request(WorkerOperationRequest::Lifecycle(request)) {
                    Ok(WorkerOperationResult::Lifecycle(result)) if result.request() == request => {
                        Some(result.outcome())
                    }
                    _ => None,
                }
            })();
            popup.ordinal = popup.ordinal.saturating_add(1);
            popup.phase = match (previous, result) {
                (
                    Phase::Queued,
                    Some(
                        node::TerminalLifecycleOutcome::ForceAccepted
                        | node::TerminalLifecycleOutcome::HostFailure(
                            node::LifecycleHostFailure::HostRefused,
                        ),
                    ),
                ) => Phase::Waiting,
                (Phase::Waiting, Some(node::TerminalLifecycleOutcome::InventoryPage(page))) => {
                    popup.pages += 1;
                    match page.target {
                        Some(observed)
                            if matches!(
                                observed.phase,
                                node::LifecycleTerminalPhase::Exited { .. }
                                    | node::LifecycleTerminalPhase::Failed
                            ) =>
                        {
                            Phase::Dispose
                        }
                        Some(_) => {
                            popup.cursor = None;
                            popup.pages = 0;
                            Phase::Waiting
                        }
                        None if page.next_cursor.is_some() && popup.pages < 8 => {
                            popup.cursor = page.next_cursor;
                            Phase::Waiting
                        }
                        None => Phase::Unknown,
                    }
                }
                (Phase::Dispose, Some(node::TerminalLifecycleOutcome::Disposed)) => Phase::Closed,
                _ => Phase::Unknown,
            };
            if popup.ordinal > 32 && popup.phase != Phase::Closed {
                popup.phase = Phase::Unknown;
            }
            popup.next = Instant::now()
                + if popup.phase == Phase::Waiting && popup.cursor.is_none() {
                    OBSERVATION_INTERVAL
                } else {
                    Duration::ZERO
                };
            // A known Dispose ACK remains authoritative even if the current Worker was replaced.
            if popup.phase == Phase::Closed {
                if let Some(target) = popup.target {
                    self.settle_plugin_observer_disposed(target);
                }
                if let (Some(authorization), Ok(mut coordinator)) =
                    (popup.authorization, self.coordinator.lock())
                {
                    if !coordinator.release_plugin_popup(&popup.reference, authorization) {
                        popup.phase = Phase::Unknown;
                    }
                } else {
                    popup.phase = Phase::Unknown;
                }
            }
            if let Ok(mut owners) = self.popups.0.lock()
                && let Some(current) = owners.get_mut(&owner)
                && current.id == popup.id
                && current.phase == Phase::Working
            {
                popup.connected = current.connected;
                *current = popup;
                if current.phase == Phase::Closed && !current.connected {
                    owners.remove(&owner);
                }
            }
        }
        self.popups
            .0
            .lock()
            .ok()?
            .values()
            .filter(|popup| matches!(popup.phase, Phase::Queued | Phase::Waiting | Phase::Dispose))
            .map(|popup| popup.next)
            .min()
    }
}
fn hex(bytes: [u8; 16]) -> String {
    format!("{:032x}", u128::from_be_bytes(bytes))
}
