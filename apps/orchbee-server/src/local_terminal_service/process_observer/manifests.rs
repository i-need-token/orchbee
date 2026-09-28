//! One observer-owned install point and bounded, non-evicting Site-lifetime receipts.
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
use orchbee_local_site_runtime::WindowsUserSettings;
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
    configuration: Option<(Option<PathBuf>, bool)>,
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
    pub(super) fn initialize(
        &self,
        root: Option<PathBuf>,
        detector: &ScreenDetector,
    ) -> std::io::Result<()> {
        self.initialize_source(root, detector, updating::configured_source())
    }
    pub(super) fn initialize_settings(
        &self,
        settings: &WindowsUserSettings,
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
            configuration: state
                .schedule
                .as_ref()
                .map(|s| (s.path.clone(), s.explicit)),
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
        self.complete_reload(Owner::Manual(id), loaded, None, None, install);
    }
    fn complete_reload(
        &self,
        id: Owner,
        loaded: Result<ScreenDetector, String>,
        maintenance: Option<AgentManifestMaintenance>,
        settings: Option<WindowsUserSettings>,
        install: impl FnOnce(ScreenDetector) -> Result<(), Failure>,
    ) {
        let Ok(mut lock) = self.0.lock() else { return };
        let Some(state) = lock.as_mut() else { return };
        if state.stopped || state.active.as_ref() != Some(&id) {
            return;
        }
        let status = match activate(state, loaded, install) {
            Ok(revision) => {
                if let Some(settings) = settings
                    && let Some(schedule) = &mut state.schedule
                {
                    schedule.apply(&settings, Instant::now());
                }
                Operation::Applied { revision }
            }
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
        let outcome = match self.workbench_site_matches(&request.site_id) {
            Some(true) => self
                .process_observer
                .wake
                .manifests
                .execute(&request.action),
            Some(false) => failed(Failure::SiteMismatch),
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
            site_id: request.site_id,
            outcome,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(n: u64) -> WorkbenchRequestId {
        WorkbenchRequestId::parse(&format!("{n:032x}")).unwrap()
    }
    #[test]
    fn reload_receipts_survive_duplicates_failures_and_stop_without_replaying() {
        let registry = Registry::default();
        registry
            .initialize(None, &ScreenDetector::bundled().unwrap())
            .unwrap();
        let action = Action::Reload {
            operation_id: id(1),
        };
        let pending = registry.execute(&action);
        assert_eq!(registry.execute(&action), pending);
        assert_eq!(
            registry.execute(&Action::Reload {
                operation_id: id(2)
            }),
            failed(Failure::Busy)
        );
        assert!(registry.take().is_some());
        assert!(registry.take().is_none());
        registry.complete(id(1), Ok(ScreenDetector::bundled().unwrap()), |_| Ok(()));
        let applied = registry.execute(&action);
        assert!(
            matches!(applied, Outcome::Operation { status: Operation::Applied { revision }, .. } if revision.value() == 2)
        );
        assert_eq!(registry.execute(&action), applied);
        assert!(registry.take().is_none());
        registry.execute(&Action::Reload {
            operation_id: id(2),
        });
        registry.complete(id(2), Err("bad bundle".into()), |_| {
            panic!("must retain old engine")
        });
        assert!(
            matches!(registry.execute(&Action::Current {}), Outcome::Current { snapshot } if snapshot.revision.value() == 2 && snapshot.manifests.len() == 22)
        );
        registry.execute(&Action::Reload {
            operation_id: id(3),
        });
        registry.take().unwrap();
        registry.cancel();
        registry.complete(id(3), Ok(ScreenDetector::bundled().unwrap()), |_| {
            panic!("stop forbids install")
        });
        assert!(matches!(
            registry.execute(&Action::Operation {
                operation_id: id(3)
            }),
            Outcome::Operation {
                status: Operation::Failed {
                    reason: Failure::Cancelled
                },
                ..
            }
        ));
        assert_eq!(
            registry.execute(&Action::Operation {
                operation_id: id(4)
            }),
            failed(Failure::NotFound)
        );
    }
    #[test]
    fn failed_install_and_full_receipt_ledger_preserve_the_installed_snapshot() {
        let registry = Registry::default();
        registry
            .initialize(None, &ScreenDetector::bundled().unwrap())
            .unwrap();
        let before = registry.execute(&Action::Current {});
        registry.execute(&Action::Reload {
            operation_id: id(1),
        });
        registry.take().unwrap();
        registry.complete(id(1), Ok(ScreenDetector::bundled().unwrap()), |_| {
            Err(Failure::Unavailable)
        });
        assert_eq!(registry.execute(&Action::Current {}), before);
        let settled = registry.execute(&Action::Operation {
            operation_id: id(1),
        });
        assert!(matches!(
            settled,
            Outcome::Operation {
                status: Operation::Failed {
                    reason: Failure::Unavailable
                },
                ..
            }
        ));
        {
            let mut lock = registry.0.lock().unwrap();
            let state = lock.as_mut().unwrap();
            for n in 2..=RECEIPTS as u64 {
                state.operations.insert(
                    id(n),
                    Receipt {
                        kind: Kind::Reload,
                        status: Operation::Failed {
                            reason: Failure::LoadFailed,
                        },
                    },
                );
            }
        }
        assert_eq!(
            registry.execute(&Action::Reload {
                operation_id: id(RECEIPTS as u64 + 1)
            }),
            failed(Failure::ResourceLimit)
        );
        assert_eq!(
            registry.execute(&Action::Reload {
                operation_id: id(1)
            }),
            settled
        );
        assert!(registry.take().is_none());
    }
    #[test]
    fn reload_installs_changed_rules_once_from_the_original_root() {
        struct Root(PathBuf);
        impl Drop for Root {
            fn drop(&mut self) {
                let path = self.0.canonicalize().unwrap();
                let parent = std::env::temp_dir().canonicalize().unwrap();
                assert_eq!(path.parent(), Some(parent.as_path()));
                assert!(
                    path.file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("orchbee-manifest-reload-")
                );
                std::fs::remove_dir_all(path).unwrap();
            }
        }
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let root = Root(std::env::temp_dir().join(format!(
            "orchbee-manifest-reload-{:032x}",
            u128::from_le_bytes(nonce)
        )));
        std::fs::create_dir(&root.0).unwrap();
        let mut installed = ScreenDetector::load(Some(&root.0)).unwrap();
        let registry = Registry::default();
        registry
            .initialize(Some(root.0.clone()), &installed)
            .unwrap();
        let rules = root.0.join("agent-detection");
        std::fs::create_dir(&rules).unwrap();
        std::fs::write(
            rules.join("codex.toml"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/agent-detection/codex.toml"
            ))
            .replace("version = \"2026.09.14.1\"", "version = \"0\""),
        )
        .unwrap();
        assert!(installed.summaries().iter().any(|m| m.agent == "codex"
            && m.manifest.source == orchbee_workbench_protocol::AgentManifestSource::Bundled));
        registry.execute(&Action::Reload {
            operation_id: id(1),
        });
        let work = registry.take().unwrap();
        registry.complete(
            match work.id {
                Owner::Manual(id) => id,
                Owner::Automatic => panic!("manual reload"),
            },
            ScreenDetector::load(work.root.as_deref()),
            |next| {
                installed = next;
                Ok(())
            },
        );
        let codex = installed
            .summaries()
            .into_iter()
            .find(|m| m.agent == "codex")
            .unwrap();
        assert_eq!(codex.version.as_deref(), Some("0"));
        assert_eq!(
            codex.manifest.source,
            orchbee_workbench_protocol::AgentManifestSource::LocalOverride
        );
        assert!(
            matches!(registry.execute(&Action::Current {}), Outcome::Current { snapshot } if snapshot.revision.value()==2 && snapshot.manifests.contains(&codex))
        );
        std::fs::write(rules.join("codex.toml"), "invalid").unwrap();
        registry.execute(&Action::Reload {
            operation_id: id(1),
        });
        assert!(registry.take().is_none());
        assert!(installed.summaries().contains(&codex));
    }
}

#[cfg(test)]
mod update_tests {
    use super::*;
    use crate::{
        agent_screen::updating::tests::{Http, INDEX, Root, rules},
        worker_registry::WorkerRegistry,
    };
    use std::{
        sync::{Arc, atomic::Ordering},
        thread,
        time::{Duration, Instant},
    };
    fn id(n: u64) -> WorkbenchRequestId {
        WorkbenchRequestId::parse(&format!("{n:032x}")).unwrap()
    }
    #[test]
    fn live_update_commits_and_activates_without_repeating_downloads() {
        let root = Root::new();
        let http = Http::start(vec![
            INDEX.as_bytes().to_vec(),
            rules("codex", "2099.1").into_bytes(),
        ]);
        let service = LocalTerminalService::new(Arc::new(Mutex::new(WorkerRegistry::new())));
        let wake = &service.process_observer.wake;
        let mut detector = ScreenDetector::bundled().unwrap();
        wake.manifests
            .initialize_source(Some(root.0.clone()), &detector, Ok(http.url.clone()))
            .unwrap();
        assert!(
            !root.0.join("agent-detection").exists(),
            "initial status read creates nothing"
        );
        let action = Action::Update {
            operation_id: id(1),
        };
        let pending = wake.manifests.execute(&action);
        assert_eq!(
            wake.manifests.execute(&Action::Reload {
                operation_id: id(1)
            }),
            failed(Failure::Conflict)
        );
        let mut worker = Worker::default();
        worker.serve(Some(&service), wake, &mut detector);
        assert!(worker.active());
        assert_eq!(wake.manifests.execute(&action), pending);
        let until = Instant::now() + Duration::from_secs(10);
        while worker.active() && Instant::now() < until {
            worker.serve(Some(&service), wake, &mut detector);
            thread::sleep(Duration::from_millis(5));
        }
        assert!(!worker.active());
        let receipt = wake.manifests.execute(&action);
        assert!(
            matches!(&receipt,Outcome::Operation {status:Operation::Updated {report,activation:Activation::Applied {revision}},..} if report.successful() && revision.value()==2),
            "{receipt:?}"
        );
        assert!(
            detector
                .summaries()
                .iter()
                .any(|m| m.agent == "codex" && m.version.as_deref() == Some("2099.1"))
        );
        assert_eq!(wake.manifests.execute(&action), receipt);
        worker.serve(Some(&service), wake, &mut detector);
        assert!(!worker.active());
        let current = wake.manifests.execute(&Action::Current {});
        assert!(
            matches!(current,Outcome::Current {snapshot} if snapshot.maintenance.as_ref().unwrap().last_update.as_ref().unwrap().successful())
        );
    }
    #[test]
    fn active_download_keeps_queries_responsive_and_stop_retains_pending_until_join() {
        let root = Root::new();
        let http = Http::start(vec![vec![]]);
        let wake = Arc::new(Wake::default());
        let mut detector = ScreenDetector::bundled().unwrap();
        wake.manifests
            .initialize_source(Some(root.0.clone()), &detector, Ok(http.url.clone()))
            .unwrap();
        let action = Action::Update {
            operation_id: id(1),
        };
        let pending = wake.manifests.execute(&action);
        let mut worker = Worker::default();
        worker.serve(None, &wake, &mut detector);
        let until = Instant::now() + Duration::from_secs(5);
        while !http.ready.load(Ordering::Acquire) && Instant::now() < until {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(http.ready.load(Ordering::Acquire));
        let query_at = Instant::now();
        assert!(matches!(
            wake.manifests.execute(&Action::Current {}),
            Outcome::Current { .. }
        ));
        worker.serve(None, &wake, &mut detector);
        assert!(query_at.elapsed() < Duration::from_secs(1));
        wake.stop.store(true, Ordering::Release);
        wake.manifests.cancel();
        assert_eq!(wake.manifests.execute(&action), pending);
        let until = Instant::now() + Duration::from_secs(3);
        while worker.active() && Instant::now() < until {
            worker.serve(None, &wake, &mut detector);
            thread::sleep(Duration::from_millis(5));
        }
        assert!(!worker.active());
        assert!(
            matches!(wake.manifests.execute(&action),Outcome::Operation {status:Operation::Updated {report,activation:Activation::NotAttempted {}},..} if report.failure==Some(UpdateFailure::Cancelled))
        );
    }
    #[test]
    fn invalid_source_and_unknown_update_effects_preserve_local_reload() {
        let root = Root::new();
        let detector = ScreenDetector::bundled().unwrap();
        let registry = Registry::default();
        registry
            .initialize_source(
                Some(root.0.clone()),
                &detector,
                Err(UpdateFailure::InvalidSource),
            )
            .unwrap();
        assert_eq!(
            registry.execute(&Action::Update {
                operation_id: id(1)
            }),
            failed(Failure::UpdateUnavailable)
        );
        registry.execute(&Action::Reload {
            operation_id: id(1),
        });
        registry.take().unwrap();
        registry.complete(id(1), Ok(ScreenDetector::bundled().unwrap()), |_| Ok(()));
        registry
            .initialize_source(
                Some(root.0.clone()),
                &detector,
                CatalogUrl::parse(updating::DEFAULT_CATALOG),
            )
            .unwrap();
        registry.execute(&Action::Update {
            operation_id: id(1),
        });
        registry.take().unwrap();
        registry.complete_update(Owner::Manual(id(1)), Err(()), None, |_| {
            panic!("unknown cannot install")
        });
        assert!(matches!(
            registry.execute(&Action::Operation {
                operation_id: id(1)
            }),
            Outcome::Operation {
                status: Operation::Unknown { .. },
                ..
            }
        ));
        assert_eq!(
            registry.execute(&Action::Update {
                operation_id: id(2)
            }),
            failed(Failure::UpdateUnavailable)
        );
        assert!(matches!(
            registry.execute(&Action::Reload {
                operation_id: id(2)
            }),
            Outcome::Operation {
                status: Operation::Pending {},
                ..
            }
        ));
    }
}

#[cfg(test)]
mod automatic_tests {
    use super::*;
    use crate::{
        agent_screen::updating::tests::{Http, INDEX, Root, rules},
        worker_registry::WorkerRegistry,
    };
    use std::{sync::Arc, thread, time::Duration};
    fn id(n: u64) -> WorkbenchRequestId {
        WorkbenchRequestId::parse(&format!("{n:032x}")).unwrap()
    }
    #[test]
    fn automatic_real_update_avoids_the_manual_ledger_and_reload_changes_policy_atomically() {
        let root = Root::new();
        let path = root.0.join("config.toml");
        std::fs::write(&path, "[update]\nmanifest_check=true").unwrap();
        let settings =
            orchbee_local_site_runtime::read_windows_user_settings_at(Some(&path), true, |_| None)
                .unwrap();
        let http = Http::start(vec![
            INDEX.as_bytes().to_vec(),
            rules("codex", "2099.2").into_bytes(),
            INDEX.as_bytes().to_vec(),
            rules("codex", "2099.2").into_bytes(),
        ]);
        let service = LocalTerminalService::new(Arc::new(Mutex::new(WorkerRegistry::new())));
        let wake = &service.process_observer.wake;
        let mut detector = ScreenDetector::bundled().unwrap();
        wake.manifests
            .initialize_settings(&settings, &detector, true, true)
            .unwrap();
        {
            let mut lock = wake.manifests.0.lock().unwrap();
            let state = lock.as_mut().unwrap();
            state.source = Ok(http.url.clone());
            state.snapshot.maintenance = Some(updating::maintenance(Some(&root.0), &state.source));
        }
        let reload = Action::Reload {
            operation_id: id(1),
        };
        wake.manifests.execute(&reload);
        wake.manifests.schedule(Instant::now());
        assert!(
            matches!(
                wake.manifests.0.lock().unwrap().as_ref().unwrap().active,
                Some(Owner::Manual(_))
            ),
            "due automatic work waits for manual work"
        );
        let mut worker = Worker::default();
        worker.serve(Some(&service), wake, &mut detector);
        wake.manifests.schedule(Instant::now());
        worker.serve(Some(&service), wake, &mut detector);
        assert!(worker.active());
        let until = Instant::now() + Duration::from_secs(10);
        while worker.active() && Instant::now() < until {
            worker.serve(Some(&service), wake, &mut detector);
            thread::sleep(Duration::from_millis(5));
        }
        assert!(!worker.active());
        let current = wake.manifests.execute(&Action::Current {});
        assert!(
            matches!(current,Outcome::Current {ref snapshot} if matches!(snapshot.maintenance.as_ref().unwrap().automatic.as_ref().unwrap().last_operation.as_deref(),Some(Operation::Updated {report,activation:Activation::Applied {..}}) if report.successful()))
        );
        assert_eq!(
            wake.manifests
                .0
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .operations
                .len(),
            1
        );
        let revision = match &current {
            Outcome::Current { snapshot } => snapshot.revision,
            _ => panic!("current snapshot"),
        };
        wake.manifests
            .schedule(Instant::now() + Duration::from_secs(1801));
        worker.serve(Some(&service), wake, &mut detector);
        let until = Instant::now() + Duration::from_secs(10);
        while worker.active() && Instant::now() < until {
            worker.serve(Some(&service), wake, &mut detector);
            thread::sleep(Duration::from_millis(5));
        }
        assert!(!worker.active());
        assert!(
            matches!(wake.manifests.execute(&Action::Current {}),Outcome::Current {snapshot} if snapshot.revision==revision && matches!(snapshot.maintenance.as_ref().unwrap().automatic.as_ref().unwrap().last_operation.as_deref(),Some(Operation::Updated {report,activation:Activation::NotAttempted {}}) if report.successful()))
        );
        std::fs::write(&path, "[update]\nmanifest_check=false").unwrap();
        wake.manifests.execute(&Action::Reload {
            operation_id: id(2),
        });
        worker.serve(Some(&service), wake, &mut detector);
        assert!(
            matches!(wake.manifests.execute(&Action::Current {}),Outcome::Current {snapshot} if !snapshot.maintenance.as_ref().unwrap().automatic.as_ref().unwrap().enabled)
        );
        let before = wake.manifests.execute(&Action::Current {});
        std::fs::write(&path, "[update]\nmanifest_check='bad'").unwrap();
        wake.manifests.execute(&Action::Reload {
            operation_id: id(3),
        });
        worker.serve(Some(&service), wake, &mut detector);
        assert_eq!(wake.manifests.execute(&Action::Current {}), before);
        assert!(matches!(
            wake.manifests.execute(&Action::Operation {
                operation_id: id(3)
            }),
            Outcome::Operation {
                status: Operation::Failed {
                    reason: Failure::LoadFailed
                },
                ..
            }
        ));
        std::fs::write(&path, "[update]\nmanifest_check=true").unwrap();
        wake.manifests.execute(&Action::Reload {
            operation_id: id(4),
        });
        worker.serve(Some(&service), wake, &mut detector);
        // Re-enable becomes due immediately; admission itself performs no network or cache write.
        wake.manifests.schedule(Instant::now());
        assert!(matches!(
            wake.manifests.0.lock().unwrap().as_ref().unwrap().active,
            Some(Owner::Automatic)
        ));
        wake.manifests.cancel();
    }
}
