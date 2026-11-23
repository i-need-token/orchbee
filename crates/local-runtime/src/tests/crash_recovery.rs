use super::*;
use crate::{
    ActivationState, RetainedSessionRecord as Retained, SessionCatalog as Catalog, SessionName,
    SessionStore,
    durable_sessions::{Directory, Lifecycle},
    process_registration::{Creator, ProcessIdentity, ProcessRole, Registration},
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
                // Recovery answers the generation it proved dead, which a fallback counts.
                assert_eq!(
                    catalog
                        .recover_inactive(&session, &activation, &lifetime)
                        .unwrap(),
                    Some([1; 16])
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
                        .is_some()
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
                    .unwrap()
                    .is_some(),
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
                    .is_some()
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
fn a_server_that_cannot_be_created_leaves_a_reservation_the_next_start_recovers() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (catalog, session, scope) = setup(&base, &local, "failed-start");
    let directory = directory(&local);
    let (activation, lifetime) = guards(&session);
    assert!(
        catalog
            .spawn_server(
                &session,
                &activation,
                &base.0.join("missing-server.exe"),
                &[],
                Instant::now() + Duration::from_secs(5),
            )
            .is_err()
    );
    assert_eq!(
        directory
            .read(&scope)
            .unwrap()
            .unwrap()
            .processes
            .registration(ProcessRole::Server),
        Registration::Spawning
    );
    assert_eq!(
        catalog
            .recover_inactive(&session, &activation, &lifetime)
            .unwrap(),
        Some([1; 16])
    );
    catalog.begin_activation(&session, [2; 16]).unwrap();
    assert!(
        directory
            .read(&scope)
            .unwrap()
            .unwrap()
            .processes
            .is_unstarted()
    );
}

#[test]
fn a_child_slot_an_ended_attempt_left_spawning_never_blocks_recovery() {
    // Every identified process of the generation is gone.
    let server = ProcessIdentity::new(u32::MAX - 7, 1).unwrap();
    let worker = ProcessIdentity::new(u32::MAX - 11, 2).unwrap();
    let host = ProcessIdentity::new(u32::MAX - 15, 3).unwrap();
    for case in ["spawning-worker", "spawning-host", "spawning-replacement"] {
        let base = Fixture::new();
        let local = Fixture::new();
        let (catalog, session, scope) = setup(&base, &local, case);
        let directory = directory(&local);
        let mut record = directory.read(&scope).unwrap().unwrap();
        {
            let processes = &mut record.processes;
            processes
                .reserve(ProcessRole::Server, Creator::Activator)
                .unwrap();
            processes.identify(ProcessRole::Server, server).unwrap();
            processes
                .reserve(ProcessRole::Worker, Creator::Process(server))
                .unwrap();
            if case != "spawning-worker" {
                processes.identify(ProcessRole::Worker, worker).unwrap();
                processes
                    .reserve(
                        ProcessRole::Host(crate::HostSlot::FIRST),
                        Creator::Process(worker),
                    )
                    .unwrap();
            }
            if case == "spawning-replacement" {
                processes
                    .identify(ProcessRole::Host(crate::HostSlot::FIRST), host)
                    .unwrap();
                processes
                    .reserve_replacement_worker(Creator::Process(server))
                    .unwrap();
            }
        }
        directory.publish(&scope, &record).unwrap();
        let (activation, lifetime) = guards(&session);
        assert_eq!(
            catalog
                .recover_inactive(&session, &activation, &lifetime)
                .unwrap(),
            Some([1; 16]),
            "{case}"
        );
        assert_eq!(
            session.activation_state().unwrap(),
            ActivationState::Crashed {
                activation_id: [1; 16]
            },
            "{case}"
        );
    }
}

#[test]
fn exact_live_registration_refuses_without_publication() {
    for case in ["live", "host-live"] {
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
            let server = if case == "live" {
                crate::process_observation::current_identity().unwrap()
            } else {
                ProcessIdentity::new(u32::MAX - 7, 1).unwrap()
            };
            processes.identify(ProcessRole::Server, server).unwrap();
            if case == "host-live" {
                let worker = ProcessIdentity::new(u32::MAX - 11, 2).unwrap();
                processes
                    .reserve(ProcessRole::Worker, Creator::Process(server))
                    .unwrap();
                processes.identify(ProcessRole::Worker, worker).unwrap();
                processes
                    .reserve(
                        ProcessRole::Host(crate::HostSlot::FIRST),
                        Creator::Process(worker),
                    )
                    .unwrap();
                processes
                    .identify(
                        ProcessRole::Host(crate::HostSlot::FIRST),
                        crate::process_observation::current_identity().unwrap(),
                    )
                    .unwrap();
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
fn an_id_now_held_by_a_process_this_account_may_not_open_or_end_never_blocks_recovery() {
    // The registered Server and the successor pair ended, and their ID now belongs to the Service
    // Control Manager, created at another time.
    let held =
        |creation_time| ProcessIdentity::new(super::service_control_manager(), creation_time);
    let (server, successor, worker) = (held(1).unwrap(), held(2).unwrap(), held(3).unwrap());
    let base = Fixture::new();
    let local = Fixture::new();
    let (catalog, session, scope) = setup(&base, &local, "reused");
    let directory = directory(&local);
    let mut record = directory.read(&scope).unwrap().unwrap();
    record
        .processes
        .reserve(ProcessRole::Server, Creator::Activator)
        .unwrap();
    record
        .processes
        .identify(ProcessRole::Server, server)
        .unwrap();
    crate::durable_sessions::reserve_successor(&mut record, [7; 16], successor, [9; 16]).unwrap();
    crate::durable_sessions::record_successor_worker(&mut record, [7; 16], successor, worker)
        .unwrap();
    directory.publish(&scope, &record).unwrap();
    let (activation, lifetime) = guards(&session);
    assert_eq!(
        catalog
            .recover_inactive(&session, &activation, &lifetime)
            .unwrap(),
        Some([1; 16])
    );
}

#[test]
fn a_live_host_is_kept_for_adoption_only_when_the_binding_record_names_it() {
    // The Server and Worker are gone; this test process stands in for the running Host, and a
    // child it starts for the Session's Keeper.
    let server = ProcessIdentity::new(u32::MAX - 7, 1).unwrap();
    let worker = ProcessIdentity::new(u32::MAX - 11, 2).unwrap();
    let host = crate::process_observation::current_identity().unwrap();
    let mut stand_in = std::process::Command::new(
        std::path::Path::new(&std::env::var_os("SystemRoot").unwrap()).join("System32\\PING.EXE"),
    )
    .args(["-n", "120", "127.0.0.1"])
    .stdout(std::process::Stdio::null())
    .spawn()
    .unwrap();
    let keeper = crate::process_observation::identity(
        &winsafe::HPROCESS::OpenProcess(
            winsafe::co::PROCESS::QUERY_LIMITED_INFORMATION,
            false,
            stand_in.id(),
        )
        .unwrap(),
    )
    .unwrap();
    let another_host = ProcessIdentity::new(host.pid(), host.creation_time() + 1).unwrap();
    for case in ["another-host", "this-host"] {
        let base = Fixture::new();
        let local = Fixture::new();
        let (catalog, session, scope) = setup(&base, &local, case);
        let directory = directory(&local);
        let mut record = directory.read(&scope).unwrap().unwrap();
        {
            let processes = &mut record.processes;
            processes
                .reserve(ProcessRole::Server, Creator::Activator)
                .unwrap();
            processes.identify(ProcessRole::Server, server).unwrap();
            processes
                .reserve(ProcessRole::Worker, Creator::Process(server))
                .unwrap();
            processes.identify(ProcessRole::Worker, worker).unwrap();
            processes
                .reserve(
                    ProcessRole::Host(crate::HostSlot::FIRST),
                    Creator::Process(worker),
                )
                .unwrap();
            processes
                .identify(ProcessRole::Host(crate::HostSlot::FIRST), host)
                .unwrap();
            processes.reserve_keeper(Creator::Process(worker)).unwrap();
            processes.identify(ProcessRole::Keeper, keeper).unwrap();
        }
        directory.publish(&scope, &record).unwrap();
        let named = if case == "this-host" {
            host
        } else {
            another_host
        };
        write_private(
            local.0.join("sessions").join(&scope).join("bindings.json"),
            serde_json::to_vec(&serde_json::json!({
                "version": 1,
                "hosts": [{ "process": {
                    "pid": named.pid(),
                    "creation_time": named.creation_time(),
                } }],
            }))
            .unwrap(),
        )
        .unwrap();
        let (activation, lifetime) = guards(&session);
        let recovered = catalog.recover_inactive(&session, &activation, &lifetime);
        if case == "another-host" {
            // The running Host still blocks recovery, and the refusal names it.
            let error = recovered.unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert_eq!(
                crate::RecoveryBlocker::of(&error),
                Some(crate::RecoveryBlocker {
                    role: crate::BlockerRole::TerminalHost,
                    process_id: host.pid(),
                })
            );
            assert_eq!(directory.read(&scope).unwrap().unwrap(), record);
            continue;
        }
        // The ended generation counts as a crash, and the next one starts with that Host and the
        // Keeper still running beside it, so its Worker adopts that Keeper.
        assert_eq!(recovered.unwrap(), Some([1; 16]));
        catalog.begin_activation(&session, [2; 16]).unwrap();
        session.commit_activation().unwrap();
        let next = directory.read(&scope).unwrap().unwrap();
        assert_eq!(next.activation_id, Some([2; 16]));
        assert_eq!(
            next.processes,
            crate::process_registration::SessionProcesses::adopting(
                &[crate::process_registration::KeptHost {
                    slot: crate::HostSlot::FIRST,
                    identity: host,
                    epoch: None,
                }],
                Some(keeper),
            )
        );
    }
    stand_in.kill().unwrap();
    stand_in.wait().unwrap();
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
                .is_some()
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
            ProcessRole::Host(crate::HostSlot::FIRST),
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
                .is_some()
        );
        catalog.begin_activation(&session, [2; 16]).unwrap();
        assert_eq!(session.runtime_directory(), runtime);
    }
}
