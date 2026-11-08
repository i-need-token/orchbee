use super::*;
use crate::{
    ActivationState, RetainedSessionRecord as Retained, SessionCatalog as Catalog, SessionName,
    SessionStore,
    durable_sessions::{Directory, Lifecycle},
    process_registration::{Creator, ProcessIdentity, ProcessRole},
};
use std::io;

fn setup(base: &Fixture, local: &Fixture, text: &str) -> (Catalog, Rendezvous, String) {
    let catalog = Catalog::try_lock(&base.0, &local.root()).unwrap();
    let name = SessionName::parse(text).unwrap();
    catalog.prepare_activation(&name).unwrap();
    let session = base.open(name.as_str());
    let LockAttempt::Acquired(_activation) = session.try_acquire_activation_lock().unwrap() else {
        panic!("activation");
    };
    catalog.begin_activation(&session, [1; 16]).unwrap();
    session.commit_activation().unwrap();
    (catalog, session, name.as_str().to_owned())
}
fn directory(local: &Fixture) -> Directory {
    Directory::open(&local.root()).unwrap().unwrap()
}
fn record_version(local: &Fixture, scope: &str) -> u64 {
    let path = local.0.join("sessions").join(scope).join("session.json");
    let value: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    value["version"].as_u64().unwrap()
}
fn guards(session: &Rendezvous) -> (crate::ModeCheckedSessionLock, crate::ModeCheckedSessionLock) {
    let LockAttempt::Acquired(activation) = session.try_acquire_activation_lock().unwrap() else {
        panic!("activation");
    };
    let LockAttempt::Acquired(lifetime) = session.try_acquire_session_lifetime_lock().unwrap()
    else {
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
                let (catalog, session, scope) = setup(&base, &local, text);
                assert_eq!(record_version(&local, &scope), 1);
                if saved {
                    let mut store = SessionStore::open(&local.root(), &session, 64).unwrap();
                    store.publish(b"last valid workbench").unwrap();
                }
                let (activation, lifetime) = guards(&session);
                assert!(
                    catalog
                        .recover_inactive(&session, &activation, &lifetime)
                        .unwrap()
                );
                assert_eq!(
                    session.activation_state().unwrap(),
                    ActivationState::Crashed {
                        activation_id: [1; 16]
                    }
                );
                assert_eq!(
                    directory(&local).read(&scope).unwrap().unwrap().lifecycle,
                    Lifecycle::Crashed
                );
                // A repeated reconciler retains the committed old generation.
                assert!(
                    catalog
                        .recover_inactive(&session, &activation, &lifetime)
                        .unwrap()
                );
                assert!(catalog.begin_activation(&session, [1; 16]).is_err());
                catalog.begin_activation(&session, [2; 16]).unwrap();
                session.commit_activation().unwrap();
                assert_eq!(record_version(&local, &scope), 1);
                let record = directory(&local).read(&scope).unwrap().unwrap();
                assert_eq!(record.activation_id, Some([2; 16]));
                assert_eq!(record.lifecycle, Lifecycle::Active);
                assert_eq!(session.activation_state().unwrap(), ActivationState::Active);
                let store = SessionStore::open(&local.root(), &session, 64).unwrap();
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
            let (catalog, session, scope) = setup(&base, &local, text);
            {
                let mut store = SessionStore::open(&local.root(), &session, 64).unwrap();
                store.publish(b"retained workbench").unwrap();
            }
            let directory = directory(&local);
            let (activation, lifetime) = guards(&session);
            if receipt == "crashed" {
                catalog
                    .recover_inactive(&session, &activation, &lifetime)
                    .unwrap();
            } else {
                Retained::retain(&local.root(), &session)
                    .unwrap()
                    .complete([7; 16])
                    .unwrap();
                session
                    .retain_active_record()
                    .unwrap()
                    .complete([7; 16])
                    .unwrap();
            }
            let previous_marker =
                fs::read(session.runtime_directory().join("activation.started")).unwrap();
            // The activator publishes the new catalog and is lost before marker consumption.
            catalog.begin_activation(&session, [2; 16]).unwrap();
            let mut interrupted = directory.read(&scope).unwrap().unwrap();
            assert_eq!(interrupted.activation_id, Some([2; 16]));
            assert!(
                catalog
                    .recover_inactive(&session, &activation, &lifetime)
                    .unwrap(),
                "{text}/{receipt}"
            );
            assert_eq!(
                session.activation_state().unwrap(),
                ActivationState::Crashed {
                    activation_id: [2; 16]
                }
            );

            // Interruption immediately after Crashed publication still carries the prior receipt.
            fs::write(
                session.runtime_directory().join("activation.started"),
                &previous_marker,
            )
            .unwrap();
            assert!(
                catalog
                    .recover_inactive(&session, &activation, &lifetime)
                    .unwrap()
            );
            assert_eq!(
                session.activation_state().unwrap(),
                ActivationState::Crashed {
                    activation_id: [2; 16]
                }
            );
            assert_eq!(
                SessionStore::open(&local.root(), &session, 64)
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
                session.runtime_directory().join("activation.started"),
                &previous_marker,
            )
            .unwrap();
            assert!(
                catalog
                    .recover_inactive(&session, &activation, &lifetime)
                    .is_err()
            );
            assert_eq!(directory.read(&scope).unwrap().unwrap(), interrupted);
            assert_eq!(
                fs::read(session.runtime_directory().join("activation.started")).unwrap(),
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
        let (catalog, session, scope) = setup(&base, &local, "refused");
        let directory = directory(&local);
        let mut record = directory.read(&scope).unwrap().unwrap();
        {
            let processes = &mut record.processes;
            processes
                .reserve(ProcessRole::Server, Creator::Activator)
                .unwrap();
            if case != "spawning" {
                let server = if case == "live" {
                    crate::process_observation::current_identity().unwrap()
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
                            crate::process_observation::current_identity().unwrap(),
                        )
                        .unwrap();
                }
            }
        }
        directory.publish(&scope, &record).unwrap();
        let (activation, lifetime) = guards(&session);
        assert_eq!(
            catalog
                .recover_inactive(&session, &activation, &lifetime)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData,
            "{case}"
        );
        assert_eq!(directory.read(&scope).unwrap().unwrap(), record, "{case}");
        assert_eq!(
            session.activation_state().unwrap(),
            ActivationState::Active,
            "{case}"
        );
    }
}

#[test]
fn endpoint_checkpoint_writer_and_record_writer_obstruction_refuse() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (catalog, session, scope) = setup(&base, &local, "obstructed");
    let directory = directory(&local);
    let original = directory.read(&scope).unwrap().unwrap();
    let (activation, lifetime) = guards(&session);
    let writer = directory.try_writer(&scope).unwrap();
    assert_eq!(
        catalog
            .recover_inactive(&session, &activation, &lifetime)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(writer);
    let store = SessionStore::open(&local.root(), &session, 64).unwrap();
    assert_eq!(
        catalog
            .recover_inactive(&session, &activation, &lifetime)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(store);
    for endpoint in Rendezvous::socket_budget_paths(session.runtime_directory())
        .into_iter()
        .chain([Rendezvous::observation_endpoint_path(
            session.runtime_directory(),
        )])
    {
        let listener = orchbee_local_ipc::LocalListener::bind(endpoint).unwrap();
        assert_eq!(
            catalog
                .recover_inactive(&session, &activation, &lifetime)
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        drop(listener);
        assert_eq!(directory.read(&scope).unwrap().unwrap(), original);
        assert_eq!(session.activation_state().unwrap(), ActivationState::Active);
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
        let (catalog, session, scope) = setup(&base, &local, "partial");
        let directory = directory(&local);
        let mut record = directory.read(&scope).unwrap().unwrap();
        record.lifecycle = Lifecycle::Crashed;
        directory.publish(&scope, &record).unwrap();
        fs::write(
            session.runtime_directory().join("activation.started"),
            bytes,
        )
        .unwrap();
        let (activation, lifetime) = guards(&session);
        assert!(
            catalog
                .recover_inactive(&session, &activation, &lifetime)
                .unwrap()
        );
        assert_eq!(
            session.activation_state().unwrap(),
            ActivationState::Crashed {
                activation_id: [1; 16]
            }
        );
        fs::write(
            session.runtime_directory().join("activation.started"),
            b"orchbee-crashed-v1:02",
        )
        .unwrap();
        assert!(
            catalog
                .recover_inactive(&session, &activation, &lifetime)
                .is_err()
        );
        assert_eq!(
            fs::read(session.runtime_directory().join("activation.started")).unwrap(),
            b"orchbee-crashed-v1:02"
        );
        assert_eq!(directory.read(&scope).unwrap().unwrap(), record);
    }
}

#[test]
fn clean_stop_preserves_children_registered_after_the_server_retained_its_generation() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (_catalog, session, scope) = setup(&base, &local, "later-host");
    let mut retained = Retained::retain(&local.root(), &session).unwrap();
    let directory = directory(&local);
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
        let (catalog, session, scope) = setup(&base, &local, text);
        let runtime = session.runtime_directory().to_owned();
        drop(session);
        assert_eq!(
            runtime.parent(),
            Some(base.0.canonicalize().unwrap().as_path())
        );
        for leaf in [
            "activation.started",
            "activation.lock",
            "session-lifetime.lock",
        ] {
            match fs::remove_file(runtime.join(leaf)) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => panic!("fixture removal: {error}"),
            }
        }
        fs::remove_dir(&runtime).unwrap();
        drop(catalog);
        let entry = Catalog::list(
            &base.0,
            &local.root(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap()
        .into_iter()
        .find(|entry| entry.name.as_str() == text)
        .unwrap();
        assert!(entry.cold_recoverable().unwrap());
        assert!(
            !runtime.exists(),
            "listing creates no runtime or record lock"
        );
        let catalog = Catalog::try_lock(&base.0, &local.root()).unwrap();
        catalog
            .prepare_activation(&SessionName::parse(text).unwrap())
            .unwrap();
        let session = base.open(&scope);
        let (activation, lifetime) = guards(&session);
        assert!(
            catalog
                .recover_inactive(&session, &activation, &lifetime)
                .unwrap()
        );
        catalog.begin_activation(&session, [2; 16]).unwrap();
        assert_eq!(session.runtime_directory(), runtime);
    }
}
