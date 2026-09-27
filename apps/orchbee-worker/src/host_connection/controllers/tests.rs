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

#[test]
fn takeover_requires_exact_committed_incumbent_and_fences_old_token() -> TestResult {
    let mut state = HostControllers::default();
    let viewer = OperationId::from_bytes([1; 16])?;
    let old = OperationId::from_bytes([2; 16])?;
    let new = OperationId::from_bytes([3; 16])?;
    let next_viewer = OperationId::from_bytes([4; 16])?;
    let terminal = HostTerminalId::from_bytes([5; 16])?;
    let other = HostTerminalId::from_bytes([6; 16])?;
    state
        .reserve(viewer, old, terminal)
        .map_err(|_| "reserve")?;
    assert!(matches!(
        state.takeover_incumbent(next_viewer, new, terminal, old),
        Err(ControlRejection::ControllerBusy)
    ));
    state.commit(old, NonZeroU64::MIN);
    assert!(matches!(
        state.takeover_incumbent(next_viewer, new, other, old),
        Err(ControlRejection::NotController)
    ));
    assert!(matches!(
        state.takeover_incumbent(next_viewer, new, terminal, next_viewer),
        Err(ControlRejection::NotController)
    ));
    assert_eq!(
        state
            .takeover_incumbent(next_viewer, new, terminal, old)
            .map_err(|_| "incumbent")?
            .map(|g| g.controller_id),
        Some(old)
    );
    state.remove(old); // Represents an acknowledged Host detach.
    assert!(
        state
            .takeover_incumbent(next_viewer, new, terminal, old)
            .map_err(|_| "already released")?
            .is_none()
    );
    state
        .reserve(next_viewer, new, terminal)
        .map_err(|_| "new reserve")?;
    state.commit(new, NonZeroU64::MIN);
    assert!(matches!(
        state.admit(viewer, old, NonZeroU64::MAX),
        Err(ControlRejection::NotController)
    ));
    assert!(state.admit(next_viewer, new, NonZeroU64::MIN).is_ok());
    state.remove(old);
    assert!(state.contains(new));
    Ok(())
}
