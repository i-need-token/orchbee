use std::{
    cell::Cell,
    io::{self, Read, Write},
    num::NonZeroU64,
    os::{
        fd::{AsFd, BorrowedFd},
        unix::net::UnixStream,
    },
    time::{Duration, Instant},
};

use nix::errno::Errno;
use nix::poll::PollFlags;

use super::{
    Deadline, DeadlineIo, WakeFault, WakeReady, complete_connect, finish_admission, is_wake,
    translate_wake, wake_observation_error,
};
use crate::{ConnectClientError, LocalClientMessage, RejectionReason, write_message};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The Client phase over the deadline transport, as admission runs it after the answer: a decoded
/// acceptance still counts only before the deadline.
fn exchange_hello(
    stream: &mut (impl Read + Write + AsFd),
    deadline: Deadline<'_>,
) -> Result<NonZeroU64, ConnectClientError> {
    let generation =
        super::super::handshake::enter_client_phase(&mut DeadlineIo { stream, deadline })?;
    deadline.remaining()?;
    Ok(generation)
}

#[test]
fn partial_reads_and_writes_share_the_original_deadline() -> TestResult {
    for writing in [true, false] {
        let (stream, mut peer) = UnixStream::pair()?;
        stream.set_nonblocking(true)?;
        if !writing {
            peer.write_all(b"abcdef")?;
        }
        let start = Instant::now();
        let time = Cell::new(start);
        let clock = || time.get();
        let deadline = Deadline {
            expires_at: start + Duration::from_millis(3),
            now: &clock,
            wake: None,
        };
        let mut chunked = ChunkedStream {
            stream,
            time: &time,
        };
        let mut transport = DeadlineIo {
            stream: &mut chunked,
            deadline,
        };
        let failure = if writing {
            transport.write_all(b"abcdef").unwrap_err()
        } else {
            transport.read_exact(&mut [0; 6]).unwrap_err()
        };
        assert_eq!(failure.kind(), io::ErrorKind::TimedOut);
        assert_eq!(time.get().duration_since(start), Duration::from_millis(3));
        drop(chunked);
        if writing {
            let mut written = Vec::new();
            peer.read_to_end(&mut written)?;
            assert_eq!(written, b"abc");
        }
    }
    Ok(())
}

#[test]
fn interrupted_and_spurious_ready_io_cannot_renew_the_deadline() -> TestResult {
    for failure in [io::ErrorKind::Interrupted, io::ErrorKind::WouldBlock] {
        let (stream, mut peer) = UnixStream::pair()?;
        let start = Instant::now();
        let time = Cell::new(start);
        let clock = || time.get();
        let deadline = Deadline {
            expires_at: start + Duration::from_millis(3),
            now: &clock,
            wake: None,
        };
        let mut stream = FailingStream {
            stream,
            time: &time,
            failure,
        };
        let error = DeadlineIo {
            stream: &mut stream,
            deadline,
        }
        .write_all(b"hello")
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert_eq!(time.get(), deadline.expires_at);
        drop(stream);
        assert_eq!(peer.read(&mut [0])?, 0);
    }
    Ok(())
}

#[test]
fn absolute_deadline_checks_before_at_and_after_the_boundary() -> TestResult {
    let cutoff = Instant::now() + Duration::from_secs(1);
    for (time, expired) in [
        (cutoff - Duration::from_nanos(1), false),
        (cutoff, true),
        (cutoff + Duration::from_nanos(1), true),
    ] {
        let clock = || time;
        assert_eq!(
            Deadline {
                expires_at: cutoff,
                now: &clock,
                wake: None,
            }
            .remaining()
            .is_err(),
            expired
        );
    }
    Ok(())
}

#[test]
fn decoded_acceptance_at_deadline_does_not_complete() -> TestResult {
    let (stream, mut peer) = UnixStream::pair()?;
    stream.set_nonblocking(true)?;
    let generation = NonZeroU64::new(7).ok_or("generation")?;
    let mut request = Vec::new();
    write_message(&mut request, &LocalClientMessage::ClientHello)?;
    let mut response = Vec::new();
    write_message(
        &mut response,
        &LocalClientMessage::ClientAccepted { generation },
    )?;
    peer.write_all(&response)?;
    let start = Instant::now();
    let time = Cell::new(start);
    let clock = || time.get();
    let deadline = Deadline {
        expires_at: start + Duration::from_millis((request.len() + response.len()) as u64),
        now: &clock,
        wake: None,
    };
    let mut chunked = ChunkedStream {
        stream,
        time: &time,
    };
    assert!(matches!(
        exchange_hello(&mut chunked, deadline),
        Err(ConnectClientError::Io(error)) if error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(time.get(), deadline.expires_at);
    Ok(())
}

#[test]
fn expiration_after_blocking_restoration_closes_the_candidate() -> TestResult {
    let (stream, mut peer) = UnixStream::pair()?;
    stream.set_nonblocking(true)?;
    peer.set_read_timeout(Some(Duration::from_secs(1)))?;
    let generation = NonZeroU64::new(7).ok_or("generation")?;
    let start = Instant::now();
    let time = Cell::new(start);
    let clock = || {
        let value = time.get();
        time.set(value + Duration::from_millis(1));
        value
    };
    let deadline = Deadline {
        expires_at: start + Duration::from_millis(1),
        now: &clock,
        wake: None,
    };
    assert!(matches!(
        finish_admission(
            stream,
            generation,
            orchbee_connection_handshake::CapabilitySet::default(),
            deadline
        ),
        Err(ConnectClientError::Io(error)) if error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(peer.read(&mut [0])?, 0);
    Ok(())
}

#[test]
fn pending_connect_checks_kernel_readiness_and_errors_without_reconnecting() -> TestResult {
    // AF_UNIX normally completes immediately or returns backlog EAGAIN on Linux.
    // Inject only EINPROGRESS; poll and SO_ERROR still use a real connected socket.
    let (stream, _peer) = UnixStream::pair()?;
    let deadline = Deadline {
        expires_at: Instant::now() + Duration::from_secs(1),
        now: &Instant::now,
        wake: None,
    };
    complete_connect(&stream, Err(Errno::EINPROGRESS), deadline)?;
    assert_eq!(
        complete_connect(&stream, Err(Errno::ECONNREFUSED), deadline)
            .unwrap_err()
            .raw_os_error(),
        Some(Errno::ECONNREFUSED as i32)
    );
    assert_eq!(
        complete_connect(&stream, Err(Errno::EAGAIN), deadline)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    let expired = Deadline {
        expires_at: Instant::now(),
        now: &Instant::now,
        wake: None,
    };
    assert_eq!(
        complete_connect(&stream, Err(Errno::EINPROGRESS), expired)
            .unwrap_err()
            .kind(),
        io::ErrorKind::TimedOut
    );
    Ok(())
}

#[test]
fn wake_ready_wins_over_simultaneous_socket_readiness() -> TestResult {
    // The pending-connect poll sees POLLOUT on the socket and POLLIN on the
    // wake pipe together; the wake observation is decided first.
    let (stream, _peer) = UnixStream::pair()?;
    let (read_end, write_end) = nix::unistd::pipe()?;
    nix::unistd::write(&write_end, b"x")?;
    let deadline = Deadline {
        expires_at: Instant::now() + Duration::from_secs(1),
        now: &Instant::now,
        wake: Some(read_end.as_fd()),
    };
    let error = complete_connect(&stream, Err(Errno::EINPROGRESS), deadline).unwrap_err();
    assert!(is_wake::<WakeReady>(&error));
    Ok(())
}

#[test]
fn wake_observation_survives_the_hello_io_wrapper_and_translates_once() -> TestResult {
    // The marker rides inside the Io wrapper like any io::Error — it is not
    // reinterpreted, and the retry loop cannot confuse it for Interrupted or
    // WouldBlock. Translation happens once at the public boundary.
    let (stream, _peer) = UnixStream::pair()?;
    stream.set_nonblocking(true)?;
    let (read_end, write_end) = nix::unistd::pipe()?;
    nix::unistd::write(&write_end, b"x")?;
    let deadline = Deadline {
        expires_at: Instant::now() + Duration::from_secs(1),
        now: &Instant::now,
        wake: Some(read_end.as_fd()),
    };
    let mut stream = stream;
    let error = exchange_hello(&mut stream, deadline).unwrap_err();
    assert!(matches!(
        &error,
        ConnectClientError::Io(inner) if is_wake::<WakeReady>(inner)
    ));
    assert!(matches!(
        translate_wake(error),
        ConnectClientError::CancellationReady
    ));
    Ok(())
}

#[test]
fn a_faulted_wake_descriptor_is_distinct_from_ready() -> TestResult {
    // EOF is readable, so the owner distinguishes the broken pipe during its
    // arbitration read. Unix poll implementations may report POLLIN with HUP.
    let (stream, _peer) = UnixStream::pair()?;
    stream.set_nonblocking(true)?;
    let (read_end, write_end) = nix::unistd::pipe()?;
    drop(write_end);
    let deadline = Deadline {
        expires_at: Instant::now() + Duration::from_secs(1),
        now: &Instant::now,
        wake: Some(read_end.as_fd()),
    };
    let mut stream = stream;
    let error = DeadlineIo {
        stream: &mut stream,
        deadline,
    }
    .write_all(b"x")
    .unwrap_err();
    let translated = translate_wake(ConnectClientError::Io(error));
    assert!(matches!(
        translated,
        ConnectClientError::CancellationReady | ConnectClientError::CancellationSourceFault(_)
    ));
    Ok(())
}

#[test]
fn wake_fault_flags_are_distinct_unless_readable() {
    for fault in [PollFlags::POLLERR, PollFlags::POLLHUP, PollFlags::POLLNVAL] {
        let error = wake_observation_error(fault).unwrap();
        assert!(is_wake::<WakeFault>(&error));
        assert!(!is_wake::<WakeReady>(&error));
    }

    let readable_and_faulted =
        wake_observation_error(PollFlags::POLLIN | PollFlags::POLLHUP).unwrap();
    assert!(is_wake::<WakeReady>(&readable_and_faulted));
    assert!(!is_wake::<WakeFault>(&readable_and_faulted));
    assert!(wake_observation_error(PollFlags::empty()).is_none());
}

#[test]
fn an_untriggered_wake_descriptor_preserves_the_deadline() -> TestResult {
    let (stream, _peer) = UnixStream::pair()?;
    stream.set_nonblocking(true)?;
    let (read_end, _write_end) = nix::unistd::pipe()?;
    let start = Instant::now();
    let time = Cell::new(start);
    let clock = || {
        let value = time.get();
        time.set(value + Duration::from_millis(1));
        value
    };
    let deadline = Deadline {
        expires_at: start + Duration::from_millis(1),
        now: &clock,
        wake: Some(read_end.as_fd()),
    };
    let mut stream = stream;
    let error = DeadlineIo {
        stream: &mut stream,
        deadline,
    }
    .read_exact(&mut [0])
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    Ok(())
}

#[test]
fn translate_wake_leaves_ordinary_errors_untouched() {
    let timeout = io::Error::new(io::ErrorKind::TimedOut, "deadline");
    assert!(matches!(
        translate_wake(ConnectClientError::Io(timeout)),
        ConnectClientError::Io(error) if error.kind() == io::ErrorKind::TimedOut
    ));
    assert!(matches!(
        translate_wake(ConnectClientError::Rejected(
            RejectionReason::CapacityExceeded
        )),
        ConnectClientError::Rejected(RejectionReason::CapacityExceeded)
    ));
    assert!(matches!(
        translate_wake(ConnectClientError::UnexpectedResponse),
        ConnectClientError::UnexpectedResponse
    ));
}

struct ChunkedStream<'a> {
    stream: UnixStream,
    time: &'a Cell<Instant>,
}

struct FailingStream<'a> {
    stream: UnixStream,
    time: &'a Cell<Instant>,
    failure: io::ErrorKind,
}

impl AsFd for FailingStream<'_> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.stream.as_fd()
    }
}

impl Read for FailingStream<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buffer)
    }
}

impl Write for FailingStream<'_> {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        self.time.set(self.time.get() + Duration::from_millis(1));
        Err(io::Error::from(self.failure))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

impl AsFd for ChunkedStream<'_> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.stream.as_fd()
    }
}

impl Read for ChunkedStream<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let length = buffer.len().min(1);
        let count = self.stream.read(&mut buffer[..length])?;
        self.time.set(self.time.get() + Duration::from_millis(1));
        Ok(count)
    }
}

impl Write for ChunkedStream<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let length = buffer.len().min(1);
        let count = self.stream.write(&buffer[..length])?;
        self.time.set(self.time.get() + Duration::from_millis(1));
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}
