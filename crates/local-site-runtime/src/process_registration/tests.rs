use super::*;

fn identity(pid: u32, creation: u64) -> ProcessIdentity {
    ProcessIdentity::new(pid, creation).unwrap()
}
fn enrolled() -> SiteProcesses {
    let mut processes = SiteProcesses::default();
    for (role, creator, id) in [
        (ProcessRole::Server, Creator::Activator, identity(1, 10)),
        (
            ProcessRole::Worker,
            Creator::Process(identity(1, 10)),
            identity(2, 20),
        ),
        (
            ProcessRole::Host,
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
    let mut processes = SiteProcesses::default();
    for (role, creator) in [
        (ProcessRole::Server, Creator::Process(identity(1, 10))),
        (ProcessRole::Worker, Creator::Activator),
        (ProcessRole::Worker, Creator::Process(identity(1, 10))),
        (ProcessRole::Host, Creator::Process(identity(2, 20))),
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
    assert_eq!(
        processes.status([ProcessObservation::Gone; 3]),
        RecoveryVerdict::Uncertain(ProcessRole::Worker)
    );
}

#[test]
fn incomplete_creation_is_uncertain_even_when_no_endpoint_or_known_process_remains() {
    let mut processes = SiteProcesses::default();
    assert_eq!(
        processes.status([ProcessObservation::Gone; 3]),
        RecoveryVerdict::Gone
    );
    processes
        .reserve(ProcessRole::Server, Creator::Activator)
        .unwrap();
    assert_eq!(
        processes.status([ProcessObservation::Gone; 3]),
        RecoveryVerdict::Uncertain(ProcessRole::Server)
    );
    processes
        .identify(ProcessRole::Server, identity(1, 10))
        .unwrap();
    assert_eq!(
        processes.status([ProcessObservation::Gone; 3]),
        RecoveryVerdict::Gone
    );
}

#[test]
fn pid_reuse_proves_old_identity_gone_without_granting_any_termination_capability() {
    let processes = enrolled();
    for (index, expected) in [
        (0, identity(1, 10)),
        (1, identity(2, 20)),
        (2, identity(3, 30)),
    ] {
        let mut observations = [ProcessObservation::Gone; 3];
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
        processes.identify(ProcessRole::Host, identity(5, 50)),
        Err(RegistrationError::NoReservation)
    );
    assert_eq!(processes, before);
    let mut processes = SiteProcesses::default();
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
        serde_json::from_value::<SiteProcesses>(valid.clone()).unwrap(),
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
        assert!(serde_json::from_value::<SiteProcesses>(invalid).is_err());
    }
}
