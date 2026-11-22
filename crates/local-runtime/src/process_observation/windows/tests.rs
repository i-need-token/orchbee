use super::*;

#[test]
fn current_identity_is_exact_and_a_different_birth_is_not_the_same_process() {
    let expected = current_identity().unwrap();
    assert_eq!(expected.pid(), std::process::id());
    assert_ne!(expected.creation_time(), 0);
    let (observed, _handle) = observe(expected, CohortArtifact::Server);
    assert_eq!(observed, ProcessObservation::Present(expected));
    let different = ProcessIdentity::new(expected.pid(), expected.creation_time() + 1).unwrap();
    let (observed, _handle) = observe(different, CohortArtifact::Server);
    assert_eq!(observed, ProcessObservation::Present(expected));
    assert_ne!(observed, ProcessObservation::Present(different));
}

#[test]
fn an_id_this_account_may_not_open_stays_unknown_only_while_its_holder_runs_the_program() {
    let held = crate::tests::service_control_manager();
    assert_eq!(unopened(held, "SERVICES.EXE"), ProcessObservation::Unknown);
    assert_eq!(
        unopened(held, &CohortArtifact::Server.file_name()),
        ProcessObservation::Gone
    );
    assert_eq!(
        unopened(u32::MAX - 7, "services.exe"),
        ProcessObservation::Gone
    );
}
