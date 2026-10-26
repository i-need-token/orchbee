//! One real Windows metadata connection never becomes a Client Connection or reconnects.
#![cfg(windows)]
use orchbee_local_client_protocol::{
    ClientHello, ConnectClientError, LocalClientMessage, RejectionReason, ServerMetadata,
    ServerMode, exchange_server_metadata_before, read_message, write_message,
};
use orchbee_local_ipc::{AsLocalIo, LocalListener, LocalStream};
use std::{
    io::{self, Read, Write},
    num::{NonZeroU8, NonZeroU32, NonZeroU64},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

fn endpoint() -> PathBuf {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    format!(
        r"\\.\pipe\orchbee-metadata-wire-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    )
    .into()
}

fn sample() -> ServerMetadata {
    ServerMetadata::new(
        "server-test",
        ClientHello::new("different-cohort").unwrap(),
        NonZeroU32::MIN,
        NonZeroU8::new(2).unwrap(),
        ServerMode::ManagedReady,
        u32::MAX,
    )
    .unwrap()
}

#[test]
fn real_metadata_closes_after_one_response_and_keeps_wrong_reply_eof_and_timeout_distinct() {
    for mode in 0..5 {
        let endpoint = endpoint();
        let listener = LocalListener::bind(&endpoint).unwrap();
        let driver = thread::spawn(move || {
            let (mut peer, _) = listener.accept().unwrap();
            peer.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
            assert_eq!(
                read_message(&mut peer).unwrap(),
                LocalClientMessage::ServerMetadataRequest
            );
            match mode {
                0 => {
                    write_message(&mut peer, &LocalClientMessage::ServerMetadata(sample())).unwrap()
                }
                1 => write_message(
                    &mut peer,
                    &LocalClientMessage::ClientAccepted {
                        generation: NonZeroU64::MIN,
                    },
                )
                .unwrap(),
                2 => return,
                3 => peer.write_all(b"OBLC\0").unwrap(),
                4 => write_message(
                    &mut peer,
                    &LocalClientMessage::Rejected {
                        reason: RejectionReason::WrongMessage,
                    },
                )
                .unwrap(),
                _ => unreachable!(),
            }
            assert_eq!(peer.read(&mut [0]).unwrap(), 0, "candidate must close");
        });
        let result = exchange_server_metadata_before(
            &endpoint,
            Instant::now() + Duration::from_millis(if mode == 3 { 250 } else { 2000 }),
            None,
        );
        driver.join().unwrap();
        match mode {
            0 => assert_eq!(result.unwrap(), sample()),
            1 => assert!(matches!(
                result,
                Err(ConnectClientError::UnexpectedResponse)
            )),
            2 => assert!(
                matches!(result,Err(ConnectClientError::Io(error)) if error.kind()==io::ErrorKind::UnexpectedEof)
            ),
            3 => assert!(
                matches!(result,Err(ConnectClientError::Io(error)) if error.kind()==io::ErrorKind::TimedOut)
            ),
            4 => assert!(matches!(
                result,
                Err(ConnectClientError::Rejected(RejectionReason::WrongMessage))
            )),
            _ => unreachable!(),
        }
    }
}

#[test]
fn expired_metadata_attempt_never_consumes_a_listener_and_absence_is_preconnect_only() {
    let endpoint = endpoint();
    assert!(matches!(
        exchange_server_metadata_before(&endpoint, Instant::now() + Duration::from_secs(1), None),
        Err(ConnectClientError::ConnectFailed(_))
    ));
    let listener = LocalListener::bind(&endpoint).unwrap();
    assert!(
        matches!(exchange_server_metadata_before(&endpoint,Instant::now(),None),Err(ConnectClientError::Io(error)) if error.kind()==io::ErrorKind::TimedOut)
    );
    let unrelated = thread::spawn(move || LocalStream::connect(endpoint).unwrap());
    drop(listener.accept().unwrap());
    drop(unrelated.join().unwrap());
}

#[test]
fn metadata_cancellation_releases_the_candidate_and_leaves_the_borrowed_wake_unread() {
    let endpoint = endpoint();
    let listener = LocalListener::bind(&endpoint).unwrap();
    let (mut wake_writer, mut wake_reader) = LocalStream::pair().unwrap();
    let driver = thread::spawn(move || {
        let (mut peer, _) = listener.accept().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        assert_eq!(
            read_message(&mut peer).unwrap(),
            LocalClientMessage::ServerMetadataRequest
        );
        peer.write_all(b"OBLC\0").unwrap();
        wake_writer.write_all(&[1]).unwrap();
        assert_eq!(peer.read(&mut [0]).unwrap(), 0);
        wake_writer
    });
    let result = exchange_server_metadata_before(
        &endpoint,
        Instant::now() + Duration::from_secs(2),
        Some(wake_reader.as_local_io()),
    );
    let _writer = driver.join().unwrap();
    assert!(matches!(result, Err(ConnectClientError::CancellationReady)));
    let mut byte = [0];
    wake_reader.read_exact(&mut byte).unwrap();
    assert_eq!(byte, [1]);
}
