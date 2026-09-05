use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn controller_is_exclusive_and_shared_input_resize_sequence_cannot_replay() -> TestResult {
    let mut state = HostControllers::default();
    let viewer = OperationId::from_bytes([1; 16])?;
    let grant = OperationId::from_bytes([2; 16])?;
    let other = OperationId::from_bytes([3; 16])?;
    let terminal = HostTerminalId::from_bytes([4; 16])?;
    state
        .reserve(viewer, grant, terminal)
        .map_err(|_| "reserve failed")?;
    assert!(matches!(
        state.reserve(other, other, terminal),
        Err(ControlRejection::ControllerBusy)
    ));
    assert!(matches!(
        state.admit(viewer, grant, NonZeroU64::MIN),
        Err(ControlRejection::NotController)
    ));
    state.commit(grant, NonZeroU64::MIN);
    assert!(matches!(
        state.admit(other, grant, NonZeroU64::MIN),
        Err(ControlRejection::NotController)
    ));
    state
        .admit(viewer, grant, NonZeroU64::MIN)
        .map_err(|_| "first sequence rejected")?;
    assert!(matches!(
        state.admit(viewer, grant, NonZeroU64::MIN),
        Err(ControlRejection::SequenceMismatch)
    ));
    state
        .admit(viewer, grant, NonZeroU64::MAX)
        .map_err(|_| "higher sequence rejected")?;
    state.remove(grant);
    assert!(state.for_viewer(other).is_none());
    state
        .reserve(other, other, terminal)
        .map_err(|_| "explicit acquisition failed")?;
    state.commit(other, NonZeroU64::MIN);
    assert!(matches!(
        state.admit(viewer, grant, NonZeroU64::MAX),
        Err(ControlRejection::NotController)
    ));
    Ok(())
}
