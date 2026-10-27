//! One observer-owned install point and bounded, non-evicting Session-lifetime receipts.
use super::{LocalTerminalService, ScreenDetector, Wake};
use crate::agent_screen::updating::{self, CatalogUrl, Failure as UpdateFailure, UpdateReport};
use orchbee_workbench_protocol::{
    AgentManifestActivation as Activation, AgentManifestMaintenance,
    AgentManifestsAction as Action, AgentManifestsFailure as Failure,
    AgentManifestsOperation as Operation, AgentManifestsOutcome as Outcome, AgentManifestsRequest,
    AgentManifestsResult, AgentManifestsSnapshot, ProtocolVersion, WorkbenchRequestId,
    WorkbenchRevision,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Mutex};
mod scheduling;
mod worker;
use orchbee_local_runtime::UserSettings;
use scheduling::Schedule;
use std::time::Instant;
pub(super) use worker::Worker;
pub(super) struct Lifetime<'a>(pub(super) &'a Registry);
impl Drop for Lifetime<'_> {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
const RECEIPTS: usize = 1024;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Reload,
    Update,
}
struct Receipt {
    kind: Kind,
    status: Operation,
}
#[derive(Clone, PartialEq, Eq)]
enum Owner {
    Manual(WorkbenchRequestId),
    Automatic,
}
struct Work {
    id: Owner,
    kind: Kind,
    root: Option<PathBuf>,
    source: Result<CatalogUrl, UpdateFailure>,
}
#[derive(Default)]
pub(super) struct Registry(Mutex<Option<State>>);
struct State {
    root: Option<PathBuf>,
    source: Result<CatalogUrl, UpdateFailure>,
    update_disabled: bool,
    snapshot: AgentManifestsSnapshot,
    operations: BTreeMap<WorkbenchRequestId, Receipt>,
    active: Option<Owner>,
    schedule: Option<Schedule>,
    taken: bool,
    stopped: bool,
}
impl Registry {
    #[cfg(test)]
    pub(super) fn initialization_for_test(&self) -> (bool, bool) {
        let state = self.0.lock().unwrap();
        (
            state.is_some(),
            state.as_ref().is_some_and(|state| state.schedule.is_some()),
        )
    }
    pub(super) fn apply_configuration(&self, settings: &UserSettings) -> bool {
        // Configuration publication already owns the coordinator. Manifest activation owns
        // this registry before installing into that coordinator, so waiting here would invert
        // the lock order. A busy maintenance transition refuses this reload atomically.
        let Ok(mut lock) = self.0.try_lock() else {
            return false;
        };
        let Some(state) = lock.as_mut() else {
            return false;
        };
        if state.stopped || state.active.is_some() {
            return false;
        }
        let Some(schedule) = state.schedule.as_mut() else {
            return false;
        };
        schedule.apply(settings, Instant::now());
        true
    }

    #[cfg(test)]
    pub(super) fn initialize(
        &self,
        root: Option<PathBuf>,
        detector: &ScreenDetector,
    ) -> std::io::Result<()> {
        self.initialize_source(root, detector, updating::configured_source())
    }
    pub(super) fn initialize_settings(
        &self,
        settings: &UserSettings,
        detector: &ScreenDetector,
        background: bool,
        release: bool,
    ) -> std::io::Result<()> {
        self.initialize_source(
            settings
                .path
                .as_deref()
                .and_then(std::path::Path::parent)
                .map(std::path::Path::to_owned),
            detector,
            updating::configured_source(),
        )?;
        if let Some(state) = self
            .0
            .lock()
            .map_err(|_| std::io::Error::other("Manifest registry poisoned"))?
            .as_mut()
        {
            state.schedule = Some(Schedule::new(settings, background, release, Instant::now()));
        }
        Ok(())
    }
    pub(super) fn schedule(&self, now: Instant) {
        let Ok(mut lock) = self.0.lock() else { return };
        let Some(state) = lock.as_mut() else { return };
        if state.stopped || state.active.is_some() {
            return;
        }
        let available = available(state);
        if state
            .schedule
            .as_mut()
            .is_some_and(|s| s.admit(now, available))
        {
            state.active = Some(Owner::Automatic);
            state.taken = false;
        }
    }
    fn initialize_source(
        &self,
        root: Option<PathBuf>,
        detector: &ScreenDetector,
        source: Result<CatalogUrl, UpdateFailure>,
    ) -> std::io::Result<()> {
        let maintenance = updating::maintenance(root.as_deref(), &source);
        let snapshot = AgentManifestsSnapshot {
            revision: WorkbenchRevision::new(1),
            engine_version: 3,
            configuration_root: root
                .as_ref()
                .map(|p| {
                    p.to_str()
                        .map(String::from)
                        .ok_or_else(|| std::io::Error::other("Invalid rule configuration path"))
                })
                .transpose()?,
            manifests: detector.summaries(),
            maintenance: Some(maintenance),
        };
        *self
            .0
            .lock()
            .map_err(|_| std::io::Error::other("Manifest registry poisoned"))? = Some(State {
            root,
            source,
            update_disabled: false,
            snapshot,
            operations: BTreeMap::new(),
            active: None,
            schedule: None,
            taken: false,
            stopped: false,
        });
        Ok(())
    }
    pub(super) fn cancel(&self) {
        if let Ok(mut lock) = self.0.lock()
            && let Some(state) = lock.as_mut()
        {
            state.stopped = true;
            if let Some(owner) = state.active.clone() {
                let kind = match &owner {
                    Owner::Automatic => Kind::Update,
                    Owner::Manual(id) => match state.operations.get(id) {
                        Some(receipt) => receipt.kind,
                        None => return,
                    },
                };
                if !(state.taken && kind == Kind::Update) {
                    settle(
                        state,
                        owner,
                        Operation::Failed {
                            reason: Failure::Cancelled,
                        },
                    );
                }
            }
        }
    }
    fn execute(&self, action: &Action) -> Outcome {
        let Ok(mut lock) = self.0.lock() else {
            return failed(Failure::Unavailable);
        };
        let Some(state) = lock.as_mut() else {
            return failed(Failure::Unavailable);
        };
        let (operation_id, kind) = match action {
            Action::Current {} => {
                return if state.stopped {
                    failed(Failure::Unavailable)
                } else {
                    Outcome::Current {
                        snapshot: snapshot(state),
                    }
                };
            }
            Action::Reload { operation_id } => (operation_id, Some(Kind::Reload)),
            Action::Update { operation_id } => (operation_id, Some(Kind::Update)),
            Action::Operation { operation_id } => (operation_id, None),
        };
        if let Some(receipt) = state.operations.get(operation_id) {
            if kind.is_some_and(|k| k != receipt.kind) {
                return failed(Failure::Conflict);
            }
            return Outcome::Operation {
                operation_id: operation_id.clone(),
                status: receipt.status.clone(),
            };
        }
        let Some(kind) = kind else {
            return failed(Failure::NotFound);
        };
        if state.stopped {
            return failed(Failure::Unavailable);
        }
        if state.active.is_some() {
            return failed(Failure::Busy);
        }
        if state.operations.len() == RECEIPTS {
            return failed(Failure::ResourceLimit);
        }
        if kind == Kind::Update
            && (state.source.is_err() || state.root.is_none() || state.update_disabled)
        {
            return failed(Failure::UpdateUnavailable);
        }
        state.active = Some(Owner::Manual(operation_id.clone()));
        state.taken = false;
        state.operations.insert(
            operation_id.clone(),
            Receipt {
                kind,
                status: Operation::Pending {},
            },
        );
        Outcome::Operation {
            operation_id: operation_id.clone(),
            status: Operation::Pending {},
        }
    }
    fn take(&self) -> Option<Work> {
        let mut lock = self.0.lock().ok()?;
        let state = lock.as_mut()?;
        if state.stopped || state.taken {
            return None;
        }
        let id = state.active.clone()?;
        let kind = match &id {
            Owner::Manual(id) => state.operations.get(id)?.kind,
            Owner::Automatic => Kind::Update,
        };
        state.taken = true;
        Some(Work {
            id,
            kind,
            root: state.root.clone(),
            source: state.source.clone(),
        })
    }
    #[cfg(test)]
    fn complete(
        &self,
        id: WorkbenchRequestId,
        loaded: Result<ScreenDetector, String>,
        install: impl FnOnce(ScreenDetector) -> Result<(), Failure>,
    ) {
        self.complete_reload(Owner::Manual(id), loaded, None, install);
    }
    fn complete_reload(
        &self,
        id: Owner,
        loaded: Result<ScreenDetector, String>,
        maintenance: Option<AgentManifestMaintenance>,
        install: impl FnOnce(ScreenDetector) -> Result<(), Failure>,
    ) {
        let Ok(mut lock) = self.0.lock() else { return };
        let Some(state) = lock.as_mut() else { return };
        if state.stopped || state.active.as_ref() != Some(&id) {
            return;
        }
        // Rule reload never reads `config.toml`; only configuration publication changes the schedule.
        let status = match activate(state, loaded, install) {
            Ok(revision) => Operation::Applied { revision },
            Err(reason) => Operation::Failed { reason },
        };
        if let Some(maintenance) = maintenance {
            state.snapshot.maintenance = Some(maintenance)
        }
        settle(state, id, status);
    }
    fn refuse_work(&self, id: Owner, reason: Failure) {
        let Ok(mut lock) = self.0.lock() else { return };
        let Some(state) = lock.as_mut() else { return };
        if state.active.as_ref() == Some(&id) {
            settle(state, id, Operation::Failed { reason });
        }
    }
    fn complete_update(
        &self,
        id: Owner,
        report: Result<UpdateReport, ()>,
        loaded: Option<Result<ScreenDetector, String>>,
        install: impl FnOnce(ScreenDetector) -> Result<(), Failure>,
    ) {
        let Ok(mut lock) = self.0.lock() else { return };
        let Some(state) = lock.as_mut() else { return };
        if state.active.as_ref() != Some(&id) {
            return;
        }
        let status = match report {
            Err(()) => {
                state.update_disabled = true;
                Operation::Unknown {
                    reason: Failure::Unavailable,
                }
            }
            Ok(report) => {
                let activation = if state.stopped {
                    Activation::NotAttempted {}
                } else if let Some(loaded) = loaded {
                    match activate(state, loaded, install) {
                        Ok(revision) => Activation::Applied { revision },
                        Err(reason) => Activation::Failed { reason },
                    }
                } else {
                    Activation::NotAttempted {}
                };
                if let Some(maintenance) = &mut state.snapshot.maintenance {
                    maintenance.last_update = Some(report.clone());
                    maintenance.status_error = if report.status_saved {
                        None
                    } else {
                        report.failure
                    };
                }
                Operation::Updated {
                    report: Box::new(report),
                    activation,
                }
            }
        };
        settle(state, id, status);
    }
}
fn available(state: &State) -> bool {
    state.source.is_ok() && state.root.is_some() && !state.update_disabled
}
fn snapshot(state: &State) -> AgentManifestsSnapshot {
    let mut snapshot = state.snapshot.clone();
    if let Some(maintenance) = &mut snapshot.maintenance {
        maintenance.automatic = state.schedule.as_ref().map(|s| s.project(available(state)));
    }
    snapshot
}
fn settle(state: &mut State, id: Owner, status: Operation) {
    state.active = None;
    match id {
        Owner::Manual(id) => {
            if let Some(receipt) = state.operations.get_mut(&id) {
                receipt.status = status;
            }
        }
        Owner::Automatic => {
            if let Some(schedule) = &mut state.schedule {
                schedule.last = Some(status);
            }
        }
    }
}
fn activate(
    state: &mut State,
    loaded: Result<ScreenDetector, String>,
    install: impl FnOnce(ScreenDetector) -> Result<(), Failure>,
) -> Result<WorkbenchRevision, Failure> {
    let next = loaded.map_err(|_| Failure::LoadFailed)?;
    let revision = state
        .snapshot
        .revision
        .value()
        .checked_add(1)
        .ok_or(Failure::RevisionExhausted)?;
    let summaries = next.summaries();
    install(next)?;
    state.snapshot.revision = WorkbenchRevision::new(revision);
    state.snapshot.manifests = summaries;
    Ok(state.snapshot.revision)
}
fn failed(reason: Failure) -> Outcome {
    Outcome::Failed { reason }
}
fn install(
    service: Option<&LocalTerminalService>,
    detector: &mut ScreenDetector,
    next: ScreenDetector,
) -> Result<(), Failure> {
    let service = service.ok_or(Failure::Unavailable)?;
    let mut coordinator = service
        .coordinator
        .lock()
        .map_err(|_| Failure::Unavailable)?;
    coordinator.reset_screen_stabilization();
    *detector = next;
    Ok(())
}
impl LocalTerminalService {
    pub(crate) fn execute_agent_manifests(
        &self,
        request: AgentManifestsRequest,
    ) -> AgentManifestsResult {
        let outcome = match self.workbench_session_matches(&request.session_id) {
            Some(true) => self
                .process_observer
                .wake
                .manifests
                .execute(&request.action),
            Some(false) => failed(Failure::SessionMismatch),
            None => failed(Failure::Unavailable),
        };
        if matches!(
            &request.action,
            Action::Reload { .. } | Action::Update { .. }
        ) {
            self.process_observer.wake.changed.notify_all();
        }
        AgentManifestsResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }
    }
}

#[cfg(test)]
mod tests;
