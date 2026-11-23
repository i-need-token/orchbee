use super::*;

fn identity(pid: u32, creation: u64) -> ProcessIdentity {
    ProcessIdentity::new(pid, creation).unwrap()
}
fn enrolled() -> SessionProcesses {
    let mut processes = SessionProcesses::default();
    for (role, creator, id) in [
        (ProcessRole::Server, Creator::Activator, identity(1, 10)),
        (
            ProcessRole::Worker,
            Creator::Process(identity(1, 10)),
            identity(2, 20),
        ),
        (
            ProcessRole::Host(crate::HostSlot::FIRST),
            Creator::Process(identity(2, 20)),
            identity(3, 30),
        ),
    ] {
        processes.reserve(role, creator).unwrap();
        processes.identify(role, id).unwrap();
    }
    processes
}

#[test]
fn only_the_exact_registered_parent_can_reserve_its_direct_child() {
    let mut processes = SessionProcesses::default();
    for (role, creator) in [
        (ProcessRole::Server, Creator::Process(identity(1, 10))),
        (ProcessRole::Worker, Creator::Activator),
        (ProcessRole::Worker, Creator::Process(identity(1, 10))),
        (
            ProcessRole::Host(crate::HostSlot::FIRST),
            Creator::Process(identity(2, 20)),
        ),
    ] {
        let before = processes.clone();
        assert_eq!(
            processes.reserve(role, creator),
            Err(RegistrationError::WrongCreator)
        );
        assert_eq!(processes, before);
    }
    processes
        .reserve(ProcessRole::Server, Creator::Activator)
        .unwrap();
    processes
        .identify(ProcessRole::Server, identity(1, 10))
        .unwrap();
    for creator in [
        Creator::Process(identity(1, 11)),
        Creator::Process(identity(4, 10)),
    ] {
        let before = processes.clone();
        assert_eq!(
            processes.reserve(ProcessRole::Worker, creator),
            Err(RegistrationError::WrongCreator)
        );
        assert_eq!(processes, before);
    }
    processes
        .reserve(ProcessRole::Worker, Creator::Process(identity(1, 10)))
        .unwrap();
}

#[test]
fn an_ended_reservation_never_refuses_recovery_and_only_identified_roles_decide() {
    let server = Creator::Process(identity(1, 10));
    let mut fresh = SessionProcesses::default();
    fresh
        .reserve(ProcessRole::Server, Creator::Activator)
        .unwrap();
    let mut worker = fresh.clone();
    worker
        .identify(ProcessRole::Server, identity(1, 10))
        .unwrap();
    worker.reserve(ProcessRole::Worker, server).unwrap();
    let mut host = worker.clone();
    host.identify(ProcessRole::Worker, identity(2, 20)).unwrap();
    host.reserve(
        ProcessRole::Host(crate::HostSlot::FIRST),
        Creator::Process(identity(2, 20)),
    )
    .unwrap();
    let mut replacement = enrolled();
    replacement.reserve_replacement_worker(server).unwrap();
    for processes in [fresh, worker, host, replacement] {
        let mut observations = [ProcessObservation::Gone; ROLES.len()];
        for (index, role) in ROLES.into_iter().enumerate() {
            if processes.registration(role) == Registration::Spawning {
                // No identity names the slot's process, so no observation of it decides.
                observations[index] = ProcessObservation::Unknown;
            }
        }
        assert_eq!(processes.status(observations), RecoveryVerdict::Gone);
        for (index, role) in ROLES.into_iter().enumerate() {
            let Some(identity) = processes.registration(role).identity() else {
                continue;
            };
            let mut live = observations;
            live[index] = ProcessObservation::Present(identity);
            assert_eq!(processes.status(live), RecoveryVerdict::Live(role));
            let mut unknown = observations;
            unknown[index] = ProcessObservation::Unknown;
            assert_eq!(processes.status(unknown), RecoveryVerdict::Uncertain(role));
        }
    }
}

#[test]
fn pid_reuse_proves_old_identity_gone_without_granting_any_termination_capability() {
    let processes = enrolled();
    for (index, expected) in [
        (0, identity(1, 10)),
        (1, identity(2, 20)),
        (2, identity(3, 30)),
    ] {
        let mut observations = [ProcessObservation::Gone; ROLES.len()];
        observations[index] = ProcessObservation::Present(expected);
        assert!(matches!(
            processes.status(observations),
            RecoveryVerdict::Live(_)
        ));
        observations[index] =
            ProcessObservation::Present(identity(expected.pid(), expected.creation_time() + 1));
        assert_eq!(processes.status(observations), RecoveryVerdict::Gone);
        observations[index] = ProcessObservation::Unknown;
        assert!(matches!(
            processes.status(observations),
            RecoveryVerdict::Uncertain(_)
        ));
    }
}

#[test]
fn repeated_registration_and_aliasing_cannot_replace_an_owner() {
    let mut processes = enrolled();
    let before = processes.clone();
    assert_eq!(
        processes.reserve(ProcessRole::Server, Creator::Activator),
        Err(RegistrationError::AlreadyStarted)
    );
    assert_eq!(
        processes.identify(ProcessRole::Host(crate::HostSlot::FIRST), identity(5, 50)),
        Err(RegistrationError::NoReservation)
    );
    assert_eq!(processes, before);
    let mut processes = SessionProcesses::default();
    processes
        .reserve(ProcessRole::Server, Creator::Activator)
        .unwrap();
    processes
        .identify(ProcessRole::Server, identity(1, 10))
        .unwrap();
    processes
        .reserve(ProcessRole::Worker, Creator::Process(identity(1, 10)))
        .unwrap();
    let before = processes.clone();
    assert_eq!(
        processes.identify(ProcessRole::Worker, identity(1, 10)),
        Err(RegistrationError::DuplicateIdentity)
    );
    assert_eq!(processes, before);
}

#[test]
fn strict_deserialization_admits_only_nonzero_distinct_identities_with_registered_ancestors() {
    let valid = serde_json::to_value(enrolled()).unwrap();
    assert_eq!(
        serde_json::from_value::<SessionProcesses>(valid.clone()).unwrap(),
        enrolled()
    );
    for invalid in [
        {
            let mut value = valid.clone();
            value["server"]["identity"]["pid"] = 0.into();
            value
        },
        {
            let mut value = valid.clone();
            value["worker"]["identity"]["creation_time"] = 0.into();
            value
        },
        {
            let mut value = valid.clone();
            value["host"]["identity"] = value["worker"]["identity"].clone();
            value
        },
        {
            let mut value = valid.clone();
            value["server"] = serde_json::json!({"state":"not-started"});
            value
        },
        {
            let mut value = valid.clone();
            value["host"]["future"] = true.into();
            value
        },
        {
            let mut value = valid;
            value.as_object_mut().unwrap().remove("worker");
            value
        },
    ] {
        assert!(serde_json::from_value::<SessionProcesses>(invalid).is_err());
    }
}

#[test]
fn only_the_recorded_server_returns_a_gone_worker_to_spawning_and_the_host_keeps_its_identity() {
    let mut processes = enrolled();
    let before = processes.clone();
    for creator in [
        Creator::Activator,
        Creator::Process(identity(2, 20)),
        Creator::Process(identity(1, 11)),
    ] {
        assert_eq!(
            processes.reserve_replacement_worker(creator),
            Err(RegistrationError::WrongCreator)
        );
        assert_eq!(processes, before);
    }
    let server = Creator::Process(identity(1, 10));
    processes.reserve_replacement_worker(server).unwrap();
    assert_eq!(
        processes.registration(ProcessRole::Worker),
        Registration::Spawning
    );
    assert_eq!(
        processes.registration(ProcessRole::Host(crate::HostSlot::FIRST)),
        before.registration(ProcessRole::Host(crate::HostSlot::FIRST))
    );
    let reread: SessionProcesses =
        serde_json::from_value(serde_json::to_value(&processes).unwrap()).unwrap();
    assert_eq!(reread, processes);
    // A failed replacement that left no process may be replaced again.
    processes.reserve_replacement_worker(server).unwrap();
    processes
        .identify(ProcessRole::Worker, identity(4, 40))
        .unwrap();
    assert_eq!(
        processes
            .registration(ProcessRole::Host(crate::HostSlot::FIRST))
            .identity(),
        Some(identity(3, 30))
    );
    // The adopted Host's identity is never reused, and only the new Worker may reserve a Host.
    assert_eq!(
        processes.reserve(
            ProcessRole::Host(crate::HostSlot::FIRST),
            Creator::Process(identity(4, 40))
        ),
        Err(RegistrationError::AlreadyStarted)
    );
}

#[test]
fn a_replacement_needs_a_worker_and_a_settled_host() {
    let server = Creator::Process(identity(1, 10));
    let mut processes = SessionProcesses::default();
    processes
        .reserve(ProcessRole::Server, Creator::Activator)
        .unwrap();
    processes
        .identify(ProcessRole::Server, identity(1, 10))
        .unwrap();
    assert_eq!(
        processes.reserve_replacement_worker(server),
        Err(RegistrationError::NoWorker)
    );
    processes.reserve(ProcessRole::Worker, server).unwrap();
    processes
        .identify(ProcessRole::Worker, identity(2, 20))
        .unwrap();
    processes
        .reserve(
            ProcessRole::Host(crate::HostSlot::FIRST),
            Creator::Process(identity(2, 20)),
        )
        .unwrap();
    let before = processes.clone();
    assert_eq!(
        processes.reserve_replacement_worker(server),
        Err(RegistrationError::HostSpawning)
    );
    assert_eq!(processes, before);
}

#[test]
fn a_spawning_host_needs_an_identified_worker_and_an_identified_host_none() {
    let mut replaced = enrolled();
    replaced
        .reserve_replacement_worker(Creator::Process(identity(1, 10)))
        .unwrap();
    let valid = serde_json::to_value(&replaced).unwrap();
    assert!(serde_json::from_value::<SessionProcesses>(valid.clone()).is_ok());
    let mut spawning = valid;
    spawning["host"] = serde_json::json!({"state":"spawning"});
    assert!(serde_json::from_value::<SessionProcesses>(spawning).is_err());
    // A generation adopting its predecessor's Host starts with that Host alone, and has started
    // nothing of its own.
    let adopting = SessionProcesses::adopting(
        &[KeptHost {
            slot: crate::HostSlot::FIRST,
            identity: identity(3, 30),
            epoch: None,
        }],
        None,
    );
    let read: SessionProcesses =
        serde_json::from_value(serde_json::to_value(&adopting).unwrap()).unwrap();
    assert_eq!(read, adopting);
    assert!(read.is_unstarted());
}
