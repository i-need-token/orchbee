use std::{
    io::{self, Read, Write},
    num::NonZeroU64,
    os::{
        fd::AsFd,
        unix::net::{UnixListener, UnixStream},
    },
    thread,
    time::{Duration, Instant},
};

use nix::{
    poll::{PollFd, PollFlags, poll},
    sys::socket::{Backlog, listen},
};
use orchbee_local_client_protocol::{
    ClientHello, ClientSessionConnection, ConnectClientError, LocalClientMessage, ProtocolError,
    RejectionReason, read_message, write_message,
};

use super::TestDirectory;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn invalid_cohort_and_expired_deadline_do_not_connect() -> TestResult {
    let fixture = TestDirectory::new()?;
    let endpoint = fixture.path.join("client.sock");
    let listener = UnixListener::bind(&endpoint)?;
    listener.set_nonblocking(true)?;
    assert!(matches!(
        ClientSessionConnection::connect_before(&endpoint, "", Instant::now()),
        Err(ConnectClientError::Protocol(ProtocolError::InvalidCohort))
    ));
    assert!(matches!(
        ClientSessionConnection::connect_before(&endpoint, "test", Instant::now()),
        Err(ConnectClientError::Io(error)) if error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    Ok(())
}

#[test]
fn accepted_deadline_connection_preserves_next_phase_and_blocking_mode() -> TestResult {
    let fixture = TestDirectory::new()?;
    let endpoint = fixture.path.join("client.sock");
    let listener = UnixListener::bind(&endpoint)?;
    let server = thread::spawn(move || -> io::Result<()> {
        let mut stream = accept_candidate(&listener)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        assert!(matches!(
            read_message(&mut stream),
            Ok(LocalClientMessage::ClientHello(_))
        ));
        let mut bytes = Vec::new();
        let generation = NonZeroU64::new(9).ok_or_else(|| io::Error::other("generation"))?;
        write_message(
            &mut bytes,
            &LocalClientMessage::ClientAccepted { generation },
        )
        .map_err(io::Error::other)?;
        bytes.extend_from_slice(b"next-phase");
        stream.write_all(&bytes)?;
        let mut next = [0; 4];
        stream.read_exact(&mut next)?;
        assert_eq!(&next, b"next");
        Ok(())
    });
    let mut client = ClientSessionConnection::connect_before(
        &endpoint,
        "test",
        Instant::now() + Duration::from_secs(2),
    )?;
    assert_eq!(client.generation().get(), 9);
    // Borrowing the fd does not create another reader or duplicate the connection.
    assert!(
        !nix::fcntl::OFlag::from_bits_truncate(nix::fcntl::fcntl(
            &client,
            nix::fcntl::FcntlArg::F_GETFL
        )?)
        .contains(nix::fcntl::OFlag::O_NONBLOCK)
    );
    assert!(
        nix::fcntl::FdFlag::from_bits_truncate(nix::fcntl::fcntl(
            &client,
            nix::fcntl::FcntlArg::F_GETFD
        )?)
        .contains(nix::fcntl::FdFlag::FD_CLOEXEC)
    );
    let mut bytes = [0; 10];
    client.read_exact(&mut bytes)?;
    assert_eq!(&bytes, b"next-phase");
    client.write_all(b"next")?;
    server.join().map_err(|_| "server panic")??;
    Ok(())
}

#[test]
fn silent_server_observes_candidate_eof_after_total_timeout() -> TestResult {
    let fixture = TestDirectory::new()?;
    let endpoint = fixture.path.join("client.sock");
    let listener = UnixListener::bind(&endpoint)?;
    let server = thread::spawn(move || -> io::Result<()> {
        let mut stream = accept_candidate(&listener)?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        assert!(matches!(
            read_message(&mut stream),
            Ok(LocalClientMessage::ClientHello(_))
        ));
        assert_eq!(stream.read(&mut [0])?, 0);
        listener.set_nonblocking(true)?;
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        Ok(())
    });
    let result = ClientSessionConnection::connect_before(
        &endpoint,
        "test",
        Instant::now() + Duration::from_millis(100),
    );
    assert!(
        matches!(result, Err(ConnectClientError::Io(error)) if error.kind() == io::ErrorKind::TimedOut)
    );
    server.join().map_err(|_| "server panic")??;
    Ok(())
}

#[test]
fn invalid_responses_remain_typed_and_close_the_candidate() -> TestResult {
    for response in [
        LocalClientMessage::Rejected {
            reason: RejectionReason::IncompatibleCohort,
        },
        LocalClientMessage::ClientHello(ClientHello::new("test")?),
        LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::new(9).ok_or("generation")?,
        },
    ] {
        let fixture = TestDirectory::new()?;
        let endpoint = fixture.path.join("client.sock");
        let listener = UnixListener::bind(&endpoint)?;
        let mut bytes = Vec::new();
        write_message(&mut bytes, &response)?;
        // The accepted-response case is the malformed frame negative control.
        if matches!(response, LocalClientMessage::ClientAccepted { .. }) {
            bytes[0] = b'X';
        }
        let server = thread::spawn(move || -> io::Result<()> {
            let mut stream = accept_candidate(&listener)?;
            stream.set_read_timeout(Some(Duration::from_secs(2)))?;
            read_message(&mut stream).map_err(io::Error::other)?;
            stream.write_all(&bytes)?;
            match stream.read(&mut [0]) {
                Ok(0) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::ConnectionReset => Ok(()),
                result => Err(io::Error::other(format!(
                    "candidate remained open: {result:?}"
                ))),
            }
        });
        let result = ClientSessionConnection::connect_before(
            &endpoint,
            "test",
            Instant::now() + Duration::from_secs(1),
        );
        match response {
            LocalClientMessage::Rejected { reason } => {
                assert!(
                    matches!(result, Err(ConnectClientError::Rejected(actual)) if actual == reason)
                );
            }
            LocalClientMessage::ClientHello(_) => assert!(matches!(
                result,
                Err(ConnectClientError::UnexpectedResponse)
            )),
            LocalClientMessage::ClientAccepted { .. } => assert!(matches!(
                result,
                Err(ConnectClientError::Protocol(ProtocolError::InvalidMagic))
            )),
        }
        server.join().map_err(|_| "server panic")??;
    }
    Ok(())
}

#[test]
fn full_linux_unix_backlog_is_rejected_without_a_second_connection() -> TestResult {
    let fixture = TestDirectory::new()?;
    let endpoint = fixture.path.join("client.sock");
    let listener = UnixListener::bind(&endpoint)?;
    listen(&listener, Backlog::new(0)?)?;
    let _occupant = UnixStream::connect(&endpoint)?;
    let result = ClientSessionConnection::connect_before(
        &endpoint,
        "test",
        Instant::now() + Duration::from_secs(1),
    );
    assert!(
        matches!(result, Err(ConnectClientError::ConnectFailed(error)) if error.kind() == io::ErrorKind::WouldBlock)
    );
    let _existing = listener.accept()?;
    listener.set_nonblocking(true)?;
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    Ok(())
}

#[test]
fn borrowed_wake_ends_a_held_admission_and_is_never_drained() -> TestResult {
    let fixture = TestDirectory::new()?;
    let endpoint = fixture.path.join("client.sock");
    let listener = UnixListener::bind(&endpoint)?;
    let server = thread::spawn(move || -> io::Result<()> {
        let mut stream = accept_candidate(&listener)?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        // The wake may fire before or after the hello is written; either way
        // the client drops its candidate and this side observes EOF.
        let _ = read_message(&mut stream);
        let mut sink = Vec::new();
        let _ = stream.read_to_end(&mut sink);
        Ok(())
    });
    let (read_end, write_end) = nix::unistd::pipe()?;
    let writer = thread::spawn(move || -> io::Result<()> {
        // Land the wake while the hello response is held.
        thread::sleep(Duration::from_millis(100));
        nix::unistd::write(&write_end, b"x")?;
        Ok(())
    });
    let result = ClientSessionConnection::connect_before_cancellable(
        &endpoint,
        "test",
        Instant::now() + Duration::from_secs(5),
        Some(read_end.as_fd()),
    );
    assert!(
        matches!(result, Err(ConnectClientError::CancellationReady)),
        "unexpected result: {result:?}"
    );
    // The protocol observed but never consumed the descriptor: the byte the
    // caller's owner wrote is still there for the sole arbitration read.
    let mut byte = [0_u8; 1];
    assert_eq!(nix::unistd::read(&read_end, &mut byte)?, 1);
    assert_eq!(&byte, b"x");
    writer.join().map_err(|_| "writer panic")??;
    server.join().map_err(|_| "server panic")??;
    Ok(())
}

#[test]
fn a_preloaded_wake_wins_even_when_the_socket_is_ready() -> TestResult {
    let fixture = TestDirectory::new()?;
    let endpoint = fixture.path.join("client.sock");
    let listener = UnixListener::bind(&endpoint)?;
    let server = thread::spawn(move || -> io::Result<()> {
        let mut stream = accept_candidate(&listener)?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        let _ = read_message(&mut stream);
        let mut sink = Vec::new();
        let _ = stream.read_to_end(&mut sink);
        Ok(())
    });
    let (read_end, write_end) = nix::unistd::pipe()?;
    nix::unistd::write(&write_end, b"x")?;
    let result = ClientSessionConnection::connect_before_cancellable(
        &endpoint,
        "test",
        Instant::now() + Duration::from_secs(5),
        Some(read_end.as_fd()),
    );
    // The wake is decided before simultaneous socket readiness — a fact for
    // the caller's owner, not a committed cancellation.
    assert!(matches!(result, Err(ConnectClientError::CancellationReady)));
    server.join().map_err(|_| "server panic")??;
    Ok(())
}

#[test]
fn a_faulted_wake_descriptor_is_a_source_fault_not_a_wake() -> TestResult {
    let fixture = TestDirectory::new()?;
    let endpoint = fixture.path.join("client.sock");
    let listener = UnixListener::bind(&endpoint)?;
    let server = thread::spawn(move || -> io::Result<()> {
        let mut stream = accept_candidate(&listener)?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        let _ = read_message(&mut stream);
        let mut sink = Vec::new();
        let _ = stream.read_to_end(&mut sink);
        Ok(())
    });
    // A pipe write end with no readers reports POLLERR.
    let (read_end, write_end) = nix::unistd::pipe()?;
    drop(read_end);
    let result = ClientSessionConnection::connect_before_cancellable(
        &endpoint,
        "test",
        Instant::now() + Duration::from_secs(5),
        Some(write_end.as_fd()),
    );
    assert!(matches!(
        result,
        Err(ConnectClientError::CancellationSourceFault(_))
    ));
    server.join().map_err(|_| "server panic")??;
    Ok(())
}

#[test]
fn a_none_wake_is_exactly_the_existing_admission() -> TestResult {
    let fixture = TestDirectory::new()?;
    let endpoint = fixture.path.join("client.sock");
    let listener = UnixListener::bind(&endpoint)?;
    let server = thread::spawn(move || -> io::Result<()> {
        let mut stream = accept_candidate(&listener)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        assert!(matches!(
            read_message(&mut stream),
            Ok(LocalClientMessage::ClientHello(_))
        ));
        let mut bytes = Vec::new();
        write_message(
            &mut bytes,
            &LocalClientMessage::ClientAccepted {
                generation: NonZeroU64::new(9).ok_or_else(|| io::Error::other("generation"))?,
            },
        )
        .map_err(io::Error::other)?;
        stream.write_all(&bytes)?;
        Ok(())
    });
    let client = ClientSessionConnection::connect_before_cancellable(
        &endpoint,
        "test",
        Instant::now() + Duration::from_secs(2),
        None,
    )?;
    assert_eq!(client.generation().get(), 9);
    server.join().map_err(|_| "server panic")??;
    Ok(())
}

fn accept_candidate(listener: &UnixListener) -> io::Result<UnixStream> {
    // A deadline may expire before the Client opens a socket. The fixture must
    // fail within its own bound instead of leaving a Server thread in accept.
    listener.set_nonblocking(true)?;
    let mut descriptors = [PollFd::new(listener.as_fd(), PollFlags::POLLIN)];
    if poll(&mut descriptors, 3_000_u16)? == 0 {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "no Client candidate arrived",
        ));
    }
    listener.accept().map(|(stream, _)| stream)
}

#[test]
fn only_pre_connection_absence_and_refusal_have_dial_failure_provenance() -> TestResult {
    let fixture = TestDirectory::new()?;
    let endpoint = fixture.path.join("client.sock");
    assert!(
        matches!(ClientSessionConnection::connect_before(&endpoint, "test",
        Instant::now() + Duration::from_secs(1)),
        Err(ConnectClientError::ConnectFailed(error)) if error.kind() == io::ErrorKind::NotFound)
    );
    drop(UnixListener::bind(&endpoint)?);
    assert!(
        matches!(ClientSessionConnection::connect_before(&endpoint, "test",
        Instant::now() + Duration::from_secs(1)),
        Err(ConnectClientError::ConnectFailed(error)) if error.kind() == io::ErrorKind::ConnectionRefused)
    );
    Ok(())
}
