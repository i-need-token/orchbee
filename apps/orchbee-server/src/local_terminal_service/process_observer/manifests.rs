//! One observer-owned install point and bounded, non-evicting Site-lifetime receipts.
use super::{LocalTerminalService, ScreenDetector, Wake};
use orchbee_workbench_protocol::{
    AgentManifestsAction as Action, AgentManifestsFailure as Failure,
    AgentManifestsOperation as Operation, AgentManifestsOutcome as Outcome, AgentManifestsRequest,
    AgentManifestsResult, AgentManifestsSnapshot, ProtocolVersion, WorkbenchRequestId,
    WorkbenchRevision,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Mutex};
pub(super) struct Lifetime<'a>(pub(super) &'a Registry);
impl Drop for Lifetime<'_> {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
const RECEIPTS: usize = 1024;
#[derive(Default)]
pub(super) struct Registry(Mutex<Option<State>>);
struct State {
    root: Option<PathBuf>,
    snapshot: AgentManifestsSnapshot,
    operations: BTreeMap<WorkbenchRequestId, Operation>,
    active: Option<WorkbenchRequestId>,
    taken: bool,
    stopped: bool,
}
impl Registry {
    pub(super) fn initialize(
        &self,
        root: Option<PathBuf>,
        detector: &ScreenDetector,
    ) -> std::io::Result<()> {
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
        };
        *self
            .0
            .lock()
            .map_err(|_| std::io::Error::other("Manifest registry poisoned"))? = Some(State {
            root,
            snapshot,
            operations: BTreeMap::new(),
            active: None,
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
            if let Some(id) = state.active.take() {
                state.operations.insert(
                    id,
                    Operation::Failed {
                        reason: Failure::Cancelled,
                    },
                );
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
        match action {
            Action::Current {} if !state.stopped => Outcome::Current {
                snapshot: state.snapshot.clone(),
            },
            Action::Reload { operation_id } | Action::Operation { operation_id } => {
                if let Some(status) = state.operations.get(operation_id) {
                    return Outcome::Operation {
                        operation_id: operation_id.clone(),
                        status: status.clone(),
                    };
                }
                if matches!(action, Action::Operation { .. }) {
                    return failed(Failure::NotFound);
                }
                if state.stopped {
                    return failed(Failure::Unavailable);
                }
                if state.active.is_some() {
                    return failed(Failure::Busy);
                }
                if state.operations.len() == RECEIPTS {
                    return failed(Failure::ResourceLimit);
                }
                state.active = Some(operation_id.clone());
                state.taken = false;
                state
                    .operations
                    .insert(operation_id.clone(), Operation::Pending {});
                Outcome::Operation {
                    operation_id: operation_id.clone(),
                    status: Operation::Pending {},
                }
            }
            Action::Current {} => failed(Failure::Unavailable),
        }
    }
    fn take(&self) -> Option<(WorkbenchRequestId, Option<PathBuf>)> {
        let mut lock = self.0.lock().ok()?;
        let state = lock.as_mut()?;
        if state.stopped || state.taken {
            return None;
        }
        let id = state.active.clone()?;
        state.taken = true;
        Some((id, state.root.clone()))
    }
    fn complete(
        &self,
        id: WorkbenchRequestId,
        loaded: Result<ScreenDetector, String>,
        install: impl FnOnce(ScreenDetector) -> Result<(), Failure>,
    ) {
        let Ok(mut lock) = self.0.lock() else { return };
        let Some(state) = lock.as_mut() else { return };
        if state.stopped || state.active.as_ref() != Some(&id) {
            return;
        }
        let result = loaded.map_err(|_| Failure::LoadFailed).and_then(|next| {
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
        });
        state.active = None;
        state.operations.insert(
            id,
            match result {
                Ok(revision) => Operation::Applied { revision },
                Err(reason) => Operation::Failed { reason },
            },
        );
    }
}
fn failed(reason: Failure) -> Outcome {
    Outcome::Failed { reason }
}
pub(super) fn serve(service: &LocalTerminalService, wake: &Wake, detector: &mut ScreenDetector) {
    let Some((id, root)) = wake.manifests.take() else {
        return;
    };
    let next = ScreenDetector::load(root.as_deref());
    wake.manifests.complete(id, next, |next| {
        let mut coordinator = service
            .coordinator
            .lock()
            .map_err(|_| Failure::Unavailable)?;
        coordinator.reset_screen_stabilization();
        *detector = next;
        Ok(())
    });
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
        if matches!(&request.action, Action::Reload { .. }) {
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
                    Operation::Failed {
                        reason: Failure::LoadFailed,
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
        let (operation, original_root) = registry.take().unwrap();
        registry.complete(
            operation,
            ScreenDetector::load(original_root.as_deref()),
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
