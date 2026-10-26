use super::*;

#[test]
fn one_shot_cancellation_stays_ready_for_independent_borrowers() -> io::Result<()> {
    let owner = OwnedCancellation::new()?;
    let first = owner.borrow();
    let second = owner.borrow();
    assert!(!first.is_cancelled()?);
    owner.cancel();
    assert!(owner.is_cancelled());
    assert!(first.is_cancelled()?);
    owner.cancel();
    assert!(second.is_cancelled()?);
    assert!(first.is_cancelled()?);
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_closed_borrowed_wake_is_a_fault_not_a_cancellation() -> io::Result<()> {
    use std::os::fd::AsFd;
    let (reader, writer) = io::pipe()?;
    let cancel = Cancellation::new(Some(reader.as_fd()));
    assert!(!cancel.is_cancelled()?);
    drop(writer);
    assert!(cancel.is_cancelled().is_err());
    Ok(())
}

#[cfg(windows)]
#[test]
fn borrowed_windows_wake_keeps_its_exact_signalled_byte() -> io::Result<()> {
    let state = AtomicU8::new(2);
    let cancel = Cancellation::new(Some(LocalIo::Wake(&state, 1)));
    assert!(!cancel.is_cancelled()?);
    state.store(1, Ordering::Release);
    assert!(cancel.is_cancelled()?);
    state.store(0, Ordering::Release);
    assert!(!cancel.is_cancelled()?);
    Ok(())
}
