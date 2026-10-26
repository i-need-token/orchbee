use super::super::*;
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
        orchbee_local_runtime::read_user_settings_at(Some(&path), true, |_| None).unwrap();
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
    std::fs::write(&path, "[update\nmanifest_check=false").unwrap();
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
