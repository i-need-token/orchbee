use super::*;
#[test]
fn mailbox_coalesces_and_failures_distinguish_transport_from_protocol() {
    let worker = Worker {
        cancel: Arc::new(OwnedCancellation::new().unwrap()),
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
        assert!(!permanent(kind));
    }
    assert!(permanent(io::ErrorKind::InvalidData));
    assert!(permanent(io::ErrorKind::PermissionDenied));
}
