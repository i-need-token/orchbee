//! Real Windows control exchanges retain submission uncertainty without mutation retry.
#![cfg(windows)]
use orchbee_local_client_protocol::{
    ClientHello, ConnectClientError, LocalClientMessage, SessionStopRequest, SessionStopResult,
    SessionStopStatus, StopAction, StopId, exchange_session_stop_before, read_message,
    write_message,
};
use orchbee_local_ipc::{LocalListener, LocalStream};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

fn endpoint() -> PathBuf {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    format!(
        r"\\.\pipe\orchbee-stop-wire-test-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    )
    .into()
}
fn request() -> SessionStopRequest {
    SessionStopRequest {
        hello: ClientHello::new("test").unwrap(),
        id: StopId::new([1; 16]).unwrap(),
        action: StopAction::Start,
    }
}

#[test]
fn accepted_unknown_wrong_id_and_pre_submission_expiry_remain_distinct() {
    for mode in 0..3 {
        let endpoint = endpoint();
        let listener = LocalListener::bind(&endpoint).unwrap();
        let driver = thread::spawn(move || {
            let (mut peer, _) = listener.accept().unwrap();
            peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            assert_eq!(
                read_message(&mut peer).unwrap(),
                LocalClientMessage::SessionStopRequest(request())
            );
            if mode == 0 || mode == 2 {
                write_message(
                    &mut peer,
                    &LocalClientMessage::SessionStopResult(SessionStopResult {
                        id: if mode == 0 {
                            request().id
                        } else {
                            StopId::new([2; 16]).unwrap()
                        },
                        status: SessionStopStatus::NotRecorded,
                    }),
                )
                .unwrap();
            }
        });
        let result = exchange_session_stop_before(
            &endpoint,
            request(),
            Instant::now() + Duration::from_secs(2),
            None,
        );
        driver.join().unwrap();
        if mode == 0 {
            assert_eq!(result.unwrap().status, SessionStopStatus::NotRecorded);
        } else {
            let error = result.unwrap_err();
            assert!(error.submitted);
            if mode == 2 {
                assert!(matches!(
                    error.failure,
                    ConnectClientError::UnexpectedResponse
                ));
            }
        }
    }
    let endpoint = endpoint();
    let listener = LocalListener::bind(&endpoint).unwrap();
    let error =
        exchange_session_stop_before(&endpoint, request(), Instant::now(), None).unwrap_err();
    assert!(!error.submitted);
    // The unconsumed listener is still available to an unrelated proof connection.
    let connecting = thread::spawn(move || LocalStream::connect(endpoint).unwrap());
    let (peer, _) = listener.accept().unwrap();
    drop(peer);
    drop(connecting.join().unwrap());
}
