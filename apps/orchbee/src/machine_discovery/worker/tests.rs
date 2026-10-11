use super::*;
#[test]
fn mailbox_coalesces_and_failures_distinguish_transport_from_protocol() {
    let worker = Worker {
        cancel: Arc::new(AtomicU8::new(0)),
        latest: Arc::new(Mutex::new(None)),
        task: None,
    };
    for generation in 1..=1000 {
        *worker.latest.lock().unwrap() = Some(Update {
            generation,
            event: Event::Failed {
                permanent: false,
                at: Instant::now(),
            },
        });
    }
    assert_eq!(worker.take().unwrap().generation, 1000);
    assert!(worker.take().is_none());
    for kind in [
        io::ErrorKind::TimedOut,
        io::ErrorKind::UnexpectedEof,
        io::ErrorKind::ConnectionReset,
    ] {
        assert!(!permanent(
            snapshot_error(SnapshotReadError::RequestNotSubmitted(kind.into())).kind()
        ));
    }
    assert!(permanent(
        snapshot_error(SnapshotReadError::RequestEncoding(
            WorkbenchWireError::InvalidHeader
        ))
        .kind()
    ));
    assert!(permanent(io::ErrorKind::PermissionDenied));
}
