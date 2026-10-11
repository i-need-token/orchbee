use super::super::*;
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
