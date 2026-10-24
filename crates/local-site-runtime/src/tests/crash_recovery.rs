use super::*;
use crate::{
    ActivationState, WindowsNamedSiteCatalog as Catalog, WindowsRetainedSessionRecord as Retained,
    WindowsSessionStore, WindowsSiteName,
    durable_sessions_windows::{Directory, Lifecycle},
    process_registration::{Creator, ProcessIdentity, ProcessRole},
};
use std::io;

fn setup(base: &Fixture, local: &Fixture, text: &str) -> (Catalog, Rendezvous, String) {
    let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
    let name = WindowsSiteName::parse(text).unwrap();
    let scope = name.scope_for_user(&orchbee_local_ipc::current_user_sid().unwrap());
    catalog.prepare_activation(&scope, Some(&name)).unwrap();
    let site = base.open(&scope);
    let LockAttempt::Acquired(activation) = site.try_acquire_activation_lock().unwrap() else {
        panic!("activation");
    };
    site.register_session_name(&activation, &name).unwrap();
    catalog.begin_activation(&site, [1; 16]).unwrap();
    site.commit_activation().unwrap();
    (catalog, site, scope)
}
fn directory(base: &Fixture, local: &Fixture) -> Directory {
    Directory::open(&local.0, &base.0.canonicalize().unwrap())
        .unwrap()
        .unwrap()
}
fn record_path(base: &Fixture, local: &Fixture, scope: &str) -> std::path::PathBuf {
    local
        .0
        .join("Orchbee/session-catalog")
        .join(Rendezvous::host_endpoint_id(
            &base.0.canonicalize().unwrap(),
        ))
        .join(format!("{scope}.json"))
}
fn record_version(base: &Fixture, local: &Fixture, scope: &str) -> u64 {
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(record_path(base, local, scope)).unwrap()).unwrap();
    value["version"].as_u64().unwrap()
}
fn guards(site: &Rendezvous) -> (crate::ModeCheckedSiteLock, crate::ModeCheckedSiteLock) {
    let LockAttempt::Acquired(activation) = site.try_acquire_activation_lock().unwrap() else {
        panic!("activation");
    };
    let LockAttempt::Acquired(lifetime) = site.try_acquire_site_lifetime_lock().unwrap() else {
        panic!("lifetime");
    };
    (activation, lifetime)
}

#[test]
fn dead_default_and_named_generations_recover_without_a_checkpoint_or_erasing_one() {
    for text in ["default", "cold-work"] {
        for saved in [false, true] {
            {
                let base = Fixture::new();
                let local = Fixture::new();
                let (catalog, site, scope) = setup(&base, &local, text);
                assert_eq!(record_version(&base, &local, &scope), 1);
                if saved {
                    let mut store = WindowsSessionStore::open(&local.0, &site, 64).unwrap();
                    store.publish(b"last valid workbench").unwrap();
                }
                let (activation, lifetime) = guards(&site);
                assert!(
                    catalog
                        .recover_inactive(&site, &activation, &lifetime)
                        .unwrap()
                );
                assert_eq!(
                    site.activation_state().unwrap(),
                    ActivationState::Crashed {
                        activation_id: [1; 16]
                    }
                );
                assert_eq!(
                    directory(&base, &local)
                        .read(&scope)
                        .unwrap()
                        .unwrap()
                        .lifecycle,
                    Lifecycle::Crashed
                );
                // A repeated reconciler retains the committed old generation.
                assert!(
                    catalog
                        .recover_inactive(&site, &activation, &lifetime)
                        .unwrap()
                );
                assert!(catalog.begin_activation(&site, [1; 16]).is_err());
                catalog.begin_activation(&site, [2; 16]).unwrap();
                site.commit_activation().unwrap();
                assert_eq!(record_version(&base, &local, &scope), 1);
                let record = directory(&base, &local).read(&scope).unwrap().unwrap();
                assert_eq!(record.activation_id, Some([2; 16]));
                assert_eq!(record.lifecycle, Lifecycle::Active);
                assert_eq!(site.activation_state().unwrap(), ActivationState::Active);
                let store = WindowsSessionStore::open(&local.0, &site, 64).unwrap();
                assert_eq!(
                    store.read().unwrap(),
                    saved.then(|| b"last valid workbench".to_vec())
                );
            }
        }
    }
}

#[test]
fn interrupted_new_generation_recovers_only_while_every_child_is_not_started() {
    for text in ["default", "interrupted"] {
        for receipt in ["crashed", "stopped"] {
            let base = Fixture::new();
            let local = Fixture::new();
            let (catalog, site, scope) = setup(&base, &local, text);
            {
                let mut store = WindowsSessionStore::open(&local.0, &site, 64).unwrap();
                store.publish(b"retained workbench").unwrap();
            }
            let directory = directory(&base, &local);
            let (activation, lifetime) = guards(&site);
            if receipt == "crashed" {
                catalog
                    .recover_inactive(&site, &activation, &lifetime)
                    .unwrap();
            } else {
                Retained::retain(&local.0, &site)
                    .unwrap()
                    .unwrap()
                    .complete([7; 16])
                    .unwrap();
                site.retain_active_record()
                    .unwrap()
                    .complete([7; 16])
                    .unwrap();
            }
            let previous_marker =
                fs::read(site.runtime_directory().join("activation.started")).unwrap();
            // The activator publishes the new catalog and is lost before marker consumption.
            catalog.begin_activation(&site, [2; 16]).unwrap();
            let mut interrupted = directory.read(&scope).unwrap().unwrap();
            assert_eq!(interrupted.activation_id, Some([2; 16]));
            assert!(
                catalog
                    .recover_inactive(&site, &activation, &lifetime)
                    .unwrap(),
                "{text}/{receipt}"
            );
            assert_eq!(
                site.activation_state().unwrap(),
                ActivationState::Crashed {
                    activation_id: [2; 16]
                }
            );

            // Interruption immediately after Crashed publication still carries the prior receipt.
            fs::write(
                site.runtime_directory().join("activation.started"),
                &previous_marker,
            )
            .unwrap();
            assert!(
                catalog
                    .recover_inactive(&site, &activation, &lifetime)
                    .unwrap()
            );
            assert_eq!(
                site.activation_state().unwrap(),
                ActivationState::Crashed {
                    activation_id: [2; 16]
                }
            );
            assert_eq!(
                WindowsSessionStore::open(&local.0, &site, 64)
                    .unwrap()
                    .read()
                    .unwrap(),
                Some(b"retained workbench".to_vec())
            );

            // A reserved/identified role forbids treating an unrelated receipt as a begin boundary.
            interrupted
                .processes
                .reserve(ProcessRole::Server, Creator::Activator)
                .unwrap();
            interrupted
                .processes
                .identify(
                    ProcessRole::Server,
                    ProcessIdentity::new(u32::MAX - 7, 1).unwrap(),
                )
                .unwrap();
            directory.publish(&scope, &interrupted).unwrap();
            fs::write(
                site.runtime_directory().join("activation.started"),
                &previous_marker,
            )
            .unwrap();
            assert!(
                catalog
                    .recover_inactive(&site, &activation, &lifetime)
                    .is_err()
            );
            assert_eq!(directory.read(&scope).unwrap().unwrap(), interrupted);
            assert_eq!(
                fs::read(site.runtime_directory().join("activation.started")).unwrap(),
                previous_marker
            );
        }
    }
}

#[test]
fn incomplete_unknown_and_exact_live_registration_refuse_without_publication() {
    for case in ["spawning", "unknown", "live", "host-live"] {
        let base = Fixture::new();
        let local = Fixture::new();
        let (catalog, site, scope) = setup(&base, &local, "refused");
        let directory = directory(&base, &local);
        let mut record = directory.read(&scope).unwrap().unwrap();
        {
            let processes = &mut record.processes;
            processes
                .reserve(ProcessRole::Server, Creator::Activator)
                .unwrap();
            if case != "spawning" {
                let server = if case == "live" {
                    crate::process_observation_windows::current_identity().unwrap()
                } else {
                    ProcessIdentity::new(if case == "unknown" { 4 } else { u32::MAX - 7 }, 1)
                        .unwrap()
                };
                processes.identify(ProcessRole::Server, server).unwrap();
                if case == "host-live" {
                    let worker = ProcessIdentity::new(u32::MAX - 11, 2).unwrap();
                    processes
                        .reserve(ProcessRole::Worker, Creator::Process(server))
                        .unwrap();
                    processes.identify(ProcessRole::Worker, worker).unwrap();
                    processes
                        .reserve(ProcessRole::Host, Creator::Process(worker))
                        .unwrap();
                    processes
                        .identify(
                            ProcessRole::Host,
                            crate::process_observation_windows::current_identity().unwrap(),
                        )
                        .unwrap();
                }
            }
        }
        directory.publish(&scope, &record).unwrap();
        let (activation, lifetime) = guards(&site);
        assert_eq!(
            catalog
                .recover_inactive(&site, &activation, &lifetime)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData,
            "{case}"
        );
        assert_eq!(directory.read(&scope).unwrap().unwrap(), record, "{case}");
        assert_eq!(
            site.activation_state().unwrap(),
            ActivationState::Active,
            "{case}"
        );
    }
}

#[test]
fn endpoint_checkpoint_writer_and_record_writer_obstruction_refuse() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (catalog, site, scope) = setup(&base, &local, "obstructed");
    let directory = directory(&base, &local);
    let original = directory.read(&scope).unwrap().unwrap();
    let (activation, lifetime) = guards(&site);
    let writer = directory.try_writer(&scope).unwrap();
    assert_eq!(
        catalog
            .recover_inactive(&site, &activation, &lifetime)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(writer);
    let store = WindowsSessionStore::open(&local.0, &site, 64).unwrap();
    assert_eq!(
        catalog
            .recover_inactive(&site, &activation, &lifetime)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(store);
    for endpoint in Rendezvous::socket_budget_paths(site.runtime_directory())
        .into_iter()
        .chain([Rendezvous::observation_endpoint_path(
            site.runtime_directory(),
        )])
    {
        let listener = orchbee_local_ipc::LocalListener::bind(endpoint).unwrap();
        assert_eq!(
            catalog
                .recover_inactive(&site, &activation, &lifetime)
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        drop(listener);
        assert_eq!(directory.read(&scope).unwrap().unwrap(), original);
        assert_eq!(site.activation_state().unwrap(), ActivationState::Active);
    }
}

#[test]
fn committed_crash_publication_retries_only_its_matching_interrupted_marker() {
    for bytes in [
        b"".as_slice(),
        b"orchbee-crashed-v1:",
        b"orchbee-crashed-v1:010101",
    ] {
        let base = Fixture::new();
        let local = Fixture::new();
        let (catalog, site, scope) = setup(&base, &local, "partial");
        let directory = directory(&base, &local);
        let mut record = directory.read(&scope).unwrap().unwrap();
        record.lifecycle = Lifecycle::Crashed;
        directory.publish(&scope, &record).unwrap();
        fs::write(site.runtime_directory().join("activation.started"), bytes).unwrap();
        let (activation, lifetime) = guards(&site);
        assert!(
            catalog
                .recover_inactive(&site, &activation, &lifetime)
                .unwrap()
        );
        assert_eq!(
            site.activation_state().unwrap(),
            ActivationState::Crashed {
                activation_id: [1; 16]
            }
        );
        fs::write(
            site.runtime_directory().join("activation.started"),
            b"orchbee-crashed-v1:02",
        )
        .unwrap();
        assert!(
            catalog
                .recover_inactive(&site, &activation, &lifetime)
                .is_err()
        );
        assert_eq!(
            fs::read(site.runtime_directory().join("activation.started")).unwrap(),
            b"orchbee-crashed-v1:02"
        );
        assert_eq!(directory.read(&scope).unwrap().unwrap(), record);
    }
}

#[test]
fn clean_stop_preserves_children_registered_after_the_server_retained_its_generation() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (_catalog, site, scope) = setup(&base, &local, "later-host");
    let mut retained = Retained::retain(&local.0, &site).unwrap().unwrap();
    let directory = directory(&base, &local);
    let mut record = directory.read(&scope).unwrap().unwrap();
    let processes = &mut record.processes;
    for (role, creator, identity) in [
        (
            ProcessRole::Server,
            Creator::Activator,
            ProcessIdentity::new(8, 10).unwrap(),
        ),
        (
            ProcessRole::Worker,
            Creator::Process(ProcessIdentity::new(8, 10).unwrap()),
            ProcessIdentity::new(12, 20).unwrap(),
        ),
        (
            ProcessRole::Host,
            Creator::Process(ProcessIdentity::new(12, 20).unwrap()),
            ProcessIdentity::new(16, 30).unwrap(),
        ),
    ] {
        processes.reserve(role, creator).unwrap();
        processes.identify(role, identity).unwrap();
    }
    directory.publish(&scope, &record).unwrap();
    retained.complete([3; 16]).unwrap();
    record.lifecycle = Lifecycle::Stopped {
        request_id: [3; 16],
    };
    assert_eq!(directory.read(&scope).unwrap().unwrap(), record);
    retained.complete([3; 16]).unwrap();
    assert!(retained.complete([4; 16]).is_err());
    assert_eq!(directory.read(&scope).unwrap().unwrap(), record);
}

#[test]
fn registered_dead_runtime_loss_remains_discoverable_and_recreates_the_same_route() {
    for text in ["default", "lost-runtime"] {
        let base = Fixture::new();
        let local = Fixture::new();
        let (catalog, site, scope) = setup(&base, &local, text);
        let runtime = site.runtime_directory().to_owned();
        drop(site);
        assert_eq!(
            runtime.parent(),
            Some(base.0.canonicalize().unwrap().as_path())
        );
        for leaf in [
            "session-name.json",
            "activation.started",
            "activation.lock",
            "site-lifetime.lock",
        ] {
            match fs::remove_file(runtime.join(leaf)) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => panic!("fixture removal: {error}"),
            }
        }
        fs::remove_dir(&runtime).unwrap();
        drop(catalog);
        let entry = Catalog::list(&base.0, &local.0, Instant::now() + Duration::from_secs(5))
            .unwrap()
            .into_iter()
            .find(|entry| entry.name.as_str() == text)
            .unwrap();
        assert!(entry.cold_recoverable().unwrap());
        assert!(
            !runtime.exists(),
            "listing creates no runtime or record lock"
        );
        let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
        let name = WindowsSiteName::parse(text).unwrap();
        assert_eq!(
            catalog.prepare_activation(&scope, Some(&name)).unwrap(),
            Some(name.clone())
        );
        let site = base.open(&scope);
        let (activation, lifetime) = guards(&site);
        site.register_session_name(&activation, &name).unwrap();
        assert!(
            catalog
                .recover_inactive(&site, &activation, &lifetime)
                .unwrap()
        );
        catalog.begin_activation(&site, [2; 16]).unwrap();
        assert_eq!(site.runtime_directory(), runtime);
    }
}
