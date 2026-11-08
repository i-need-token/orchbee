//! Shared bounded OBLC/Workbench peer for native process fixtures and observations.
use super::native_cohort::Fixture;
use orchbee_local_client_protocol::ClientConnection;
use orchbee_local_runtime::Cohort;
use orchbee_workbench_protocol::{
    ProtocolVersion, PublicationReadIntent, Snapshot, SnapshotReadIntent, WorkbenchAction,
    WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchPublicationOutcome, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchResponse, WorkbenchRevision, WorkbenchSnapshotOutcome,
    read_workbench_response, write_workbench_request,
};
use std::{
    cell::Cell,
    error::Error,
    io::{self, Read, Write},
    os::{fd::AsFd, unix::net::UnixStream},
    rc::Rc,
    time::{Duration, Instant},
};

/// One absolute deadline re-armed on every underlying syscall: each read and
/// write recomputes the remaining budget rather than inheriting the whole
/// exchange window, so a frame dripping byte-by-byte cannot outlive the
/// intended deadline. Buffered readers keep any prefetched bytes — only real
/// syscalls are metered. The deadline cell is shared by a session's read and
/// write halves, which stay on one thread.
pub(super) struct DeadlineStream {
    stream: UnixStream,
    deadline: Rc<Cell<Instant>>,
}

impl DeadlineStream {
    pub(super) fn new(stream: UnixStream, deadline: Rc<Cell<Instant>>) -> Self {
        Self { stream, deadline }
    }

    fn remaining(&self) -> io::Result<Duration> {
        self.deadline
            .get()
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "absolute exchange deadline expired",
                )
            })
    }
}

impl Read for DeadlineStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let remaining = self.remaining()?;
        self.stream.set_read_timeout(Some(remaining))?;
        self.stream.read(buffer)
    }
}

impl Write for DeadlineStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let remaining = self.remaining()?;
        self.stream.set_write_timeout(Some(remaining))?;
        self.stream.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

// Fixture preparation and canonical observation use the public OBLC/OBWP connection.
// Only the PTY-driven native Client selects contexts or creates terminal Tabs.
pub(super) struct WorkbenchPeer {
    stream: DeadlineStream,
    deadline: Rc<Cell<Instant>>,
    next: u128,
}

impl WorkbenchPeer {
    pub(super) fn connect(fixture: &Fixture, deadline: Instant) -> Result<Self, Box<dyn Error>> {
        let cohort = Cohort::load(&fixture.cohort, deadline)?;
        let admitted = ClientConnection::connect_before(
            fixture.session().client_socket_path(),
            &orchbee_workbench_client::application::local_client::client_offer(Some(
                cohort.identity(),
            )),
            deadline,
        )?;
        let stream = UnixStream::from(admitted.as_fd().try_clone_to_owned()?);
        let deadline = Rc::new(Cell::new(Instant::now()));
        Ok(Self {
            stream: DeadlineStream::new(stream, deadline.clone()),
            deadline,
            next: 1,
        })
    }

    pub(super) fn reserve_fixture_request_range(&mut self) {
        // Fixture bootstrap must not collide with later independent test commands.
        self.next = 1_u128 << 64;
    }

    fn request_id(&mut self) -> Result<WorkbenchRequestId, Box<dyn Error>> {
        let id = WorkbenchRequestId::parse(&format!("{:032x}", self.next))?;
        self.next += 1;
        Ok(id)
    }

    fn exchange(
        &mut self,
        request: WorkbenchRequest,
        deadline: Instant,
    ) -> Result<WorkbenchResponse, Box<dyn Error>> {
        self.deadline.set(deadline);
        write_workbench_request(&mut self.stream, &request)?;
        let response = read_workbench_response(&mut self.stream)?;
        assert!(Instant::now() < deadline, "Workbench response arrived late");
        Ok(response)
    }

    pub(super) fn snapshot(&mut self, deadline: Instant) -> Result<Snapshot, Box<dyn Error>> {
        let request_id = self.request_id()?;
        let request = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
        });
        let WorkbenchResponse::Snapshot(result) = self.exchange(request, deadline)? else {
            return Err("expected Snapshot response".into());
        };
        assert_eq!(result.request_id, request_id);
        let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
            return Err("Snapshot unavailable".into());
        };
        Ok(snapshot)
    }

    pub(super) fn prepare(
        &mut self,
        action: WorkbenchAction,
        deadline: Instant,
    ) -> Result<WorkbenchCommandOutcome, Box<dyn Error>> {
        let snapshot = self.snapshot(deadline)?;
        let request_id = self.request_id()?;
        let request = WorkbenchRequest::Command(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
            session_id: snapshot.session_id,
            expected_revision: snapshot.revision,
            action,
        });
        let WorkbenchResponse::Command(result) = self.exchange(request, deadline)? else {
            return Err("expected preparation Command response".into());
        };
        assert_eq!(result.request_id, request_id);
        Ok(result.outcome)
    }

    pub(super) fn publication(
        &mut self,
        known_revision: WorkbenchRevision,
        deadline: Instant,
    ) -> Result<WorkbenchPublicationOutcome, Box<dyn Error>> {
        let request_id = self.request_id()?;
        let request = WorkbenchRequest::PublicationRead(PublicationReadIntent {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
            known_revision,
        });
        let WorkbenchResponse::Publication(result) = self.exchange(request, deadline)? else {
            return Err("expected Publication response".into());
        };
        assert_eq!(result.request_id, request_id);
        Ok(result.outcome)
    }
}
