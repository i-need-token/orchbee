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

use super::{Deadline, DeadlineIo, complete_connect, exchange_hello, finish_admission};
use crate::{ClientHello, ConnectClientError, LocalClientMessage, write_message};

type TestResult = Result<(), Box<dyn std::error::Error>>;

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
                now: &clock
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
    let hello = ClientHello::new("test")?;
    let mut request = Vec::new();
    write_message(
        &mut request,
        &LocalClientMessage::ClientHello(hello.clone()),
    )?;
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
    };
    let mut chunked = ChunkedStream {
        stream,
        time: &time,
    };
    assert!(matches!(
        exchange_hello(&mut chunked, hello, deadline),
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
    };
    assert!(matches!(
        finish_admission(stream, generation, deadline),
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
    };
    assert_eq!(
        complete_connect(&stream, Err(Errno::EINPROGRESS), expired)
            .unwrap_err()
            .kind(),
        io::ErrorKind::TimedOut
    );
    Ok(())
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
