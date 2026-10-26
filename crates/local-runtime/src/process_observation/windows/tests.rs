use super::*;

#[test]
fn current_identity_is_exact_and_a_different_birth_is_not_the_same_process() {
    let expected = current_identity().unwrap();
    assert_eq!(expected.pid(), std::process::id());
    assert_ne!(expected.creation_time(), 0);
    let (observed, _handle) = observe(expected);
    assert_eq!(observed, ProcessObservation::Present(expected));
    let different = ProcessIdentity::new(expected.pid(), expected.creation_time() + 1).unwrap();
    let (observed, _handle) = observe(different);
    assert_eq!(observed, ProcessObservation::Present(expected));
    assert_ne!(observed, ProcessObservation::Present(different));
    assert_eq!(
        observe(ProcessIdentity::new(4, 1).unwrap()).0,
        ProcessObservation::Unknown
    );
}
