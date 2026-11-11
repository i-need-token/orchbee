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
fn automatic_real_update_avoids_the_manual_ledger_and_only_configuration_changes_policy() {
    let root = Root::new();
    let path = root.0.join("config.toml");
    std::fs::write(&path, "[agent_detection]\nauto_update=true").unwrap();
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
        .initialize_settings(
            Some(root.0.clone()),
            &settings,
            &detector,
            Ok(Some(http.url.clone())),
            true,
            true,
        )
        .unwrap();
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
    let disabled = orchbee_local_runtime::UserSettings {
        manifest_check: false,
        ..settings.settings.clone()
    };
    assert!(wake.manifests.apply_configuration(&disabled));
    // Rule reload never reads config.toml, so an edited file cannot bypass configuration publication.
    std::fs::write(&path, "[agent_detection\nauto_update=true").unwrap();
    wake.manifests.execute(&Action::Reload {
        operation_id: id(2),
    });
    worker.serve(Some(&service), wake, &mut detector);
    assert!(matches!(
        wake.manifests.execute(&Action::Operation {
            operation_id: id(2)
        }),
        Outcome::Operation {
            status: Operation::Applied { .. },
            ..
        }
    ));
    assert!(
        matches!(wake.manifests.execute(&Action::Current {}),Outcome::Current {snapshot} if !snapshot.maintenance.as_ref().unwrap().automatic.as_ref().unwrap().enabled)
    );
    assert!(wake.manifests.apply_configuration(&settings));
    // Re-enable becomes due immediately; admission itself performs no network or cache write.
    wake.manifests.schedule(Instant::now());
    assert!(matches!(
        wake.manifests.0.lock().unwrap().as_ref().unwrap().active,
        Some(Owner::Automatic)
    ));
    wake.manifests.cancel();
}
#[test]
fn unconfigured_source_reads_no_remote_state_and_admits_no_update() {
    let root = Root::new();
    let path = root.0.join("config.toml");
    std::fs::write(&path, "[agent_detection]\nauto_update=true").unwrap();
    let remote = orchbee_local_runtime::DataRoot::at(root.0.clone()).agent_detection_cache();
    std::fs::create_dir_all(&remote).unwrap();
    // Any read of this unconfirmed status would surface as a cache status error.
    std::fs::write(remote.join("status.json"), "unconfirmed").unwrap();
    let settings =
        orchbee_local_runtime::read_user_settings_at(Some(&path), true, |_| None).unwrap();
    let registry = Registry::default();
    registry
        .initialize_settings(
            Some(root.0.clone()),
            &settings,
            &ScreenDetector::bundled().unwrap(),
            Ok(None),
            true,
            true,
        )
        .unwrap();
    registry.schedule(Instant::now());
    assert!(registry.take().is_none(), "no automatic check is admitted");
    assert_eq!(
        registry.execute(&Action::Update {
            operation_id: id(1)
        }),
        failed(Failure::UpdateUnavailable)
    );
    let Outcome::Current { snapshot } = registry.execute(&Action::Current {}) else {
        panic!("current snapshot")
    };
    let maintenance = snapshot.maintenance.unwrap();
    maintenance.validate().unwrap();
    assert_eq!(
        (
            maintenance.source_url,
            maintenance.source_error,
            maintenance.last_update,
            maintenance.status_error
        ),
        (None, None, None, None)
    );
    let automatic = maintenance.automatic.unwrap();
    assert!(automatic.manifest_check && !automatic.update_available && !automatic.enabled);
}
