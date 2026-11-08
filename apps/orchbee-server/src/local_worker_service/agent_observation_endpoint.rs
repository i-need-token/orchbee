//! Owns the managed Session's private same-user hook-ingress endpoint and bounded connection
//! loop, reading complete envelopes before shared Agent-service admission and acknowledgement.
//! One absolute connection deadline applies across the exchange; an accepted acknowledgement
//! confirms join bookkeeping, not that a canonical Agent session exists.

#[cfg(unix)]
use std::fs;
use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[cfg(unix)]
use nix::unistd::geteuid;
use orchbee_local_ipc::{
    AsLocalIo, LocalListener as UnixListener, LocalStream as UnixStream, PollFd, PollFlags,
    PollTimeout, poll,
};
#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
#[cfg(unix)]
type BoundIdentity = (u64, u64);
#[cfg(windows)]
#[derive(Clone, Copy)]
struct BoundIdentity;

use crate::agent::HookGeneration;
use crate::agent_service::AgentService;

#[cfg(unix)]
const OBSERVATION_SOCKET_NAME: &str = "agent-observation-v1.sock";
/// `server.maxConnections` — oracle `MAX_ACTIVE_CONNECTIONS`.
const MAX_ACTIVE_CONNECTIONS: usize = 64;
/// `MAX_OBSERVATION_BODY_BYTES` — also re-checked inside the admit fold.
const MAX_OBSERVATION_BODY_BYTES: usize = 16 * 1024;
/// Absolute per-connection deadline — deliberate deviation from the
/// oracle's 500 ms inactivity timer: it additionally bounds the
/// blocking admit fold and the ACK progress.
const CONNECTION_DEADLINE: Duration = Duration::from_millis(500);
/// Finite poll wake cap — the liveness fix for timers created outside
/// this thread: a native Instant seam can emit `ScheduleSettlement`
/// inside the shared cell (event-before-hint) while the poll loop has
/// no connection work to wake it. Without a cap the poll would block
/// forever on `PollTimeout::NONE` and the timer would never fire.
/// 25 ms keeps an externally created timer visible within one settle
/// quantum; it bounds WAKE latency only — connection and settlement
/// deadlines stay exact. No notify bus, no extra thread.
const POLL_LIVENESS_CAP: Duration = Duration::from_millis(25);

/// The bound-but-not-started phase: owns the listener, the stop-pipe
/// pair, and the pathname identity recorded at our own bind. Dropping
/// it unlinks the pathname iff its current metadata still matches that
/// recorded identity — nothing else is ever unlinked. `bound_identity`
/// is the ownership token: `start` disarms it exactly when the
/// pathname's cleanup ownership transfers to the running endpoint.
pub(crate) struct BoundEndpoint {
    path: PathBuf,
    bound_identity: Option<BoundIdentity>,
    listener: Option<UnixListener>,
    stop_reader: Option<UnixStream>,
    stop_writer: Option<UnixStream>,
}

/// The running endpoint: owns the stop writer and the single thread
/// handle. The thread owns the listener fd and every live connection.
pub(crate) struct AgentObservationEndpoint {
    path: PathBuf,
    bound_identity: BoundIdentity,
    stop_writer: UnixStream,
    thread: Option<JoinHandle<()>>,
}

/// Unlinks `path` iff its current pathname metadata still matches the
/// identity recorded at our own bind. `None` means the bind-time
/// identity capture itself failed — a mismatch, a concurrent removal,
/// a metadata error, or an unverifiable identity leaves the path
/// untouched: an unverified pathname is never unlinked. The residual
/// same-euid swap window between this check and the unlink is bounded
/// by the private mode-0700 runtime directory plus the Session lifetime
/// contract and sits inside the established same-euid trust boundary.
#[cfg(unix)]
fn unlink_if_identity_matches(path: &Path, bound_identity: Option<BoundIdentity>) {
    let Some(identity) = bound_identity else {
        return;
    };
    if let Ok(metadata) = fs::symlink_metadata(path)
        && (metadata.dev(), metadata.ino()) == identity
    {
        let _ = fs::remove_file(path);
    }
}

/// Binds the observation socket under the caller's existing private
/// contract (the caller holds the Session lifetime lock — the only thing
/// that makes the same-euid-socket reclaim below sound).
///
/// Pre-bind staleness gate — the oracle `removeStaleSocket` port:
/// absent -> proceed; non-socket or foreign-owner -> fail loudly;
/// same-euid socket -> the private directory's own stale file -> unlink.
///
/// Pathname identity is captured immediately after our own bind, and
/// EVERY later unlink — rollback or teardown — is gated on it. An
/// identity-capture failure must not unlink anything unverified: the
/// just-bound path is left and the error surfaces loudly; the next
/// bind's pre-gate reclaims a leftover own socket.
#[cfg(unix)]
pub(crate) fn bind(runtime_directory: &Path) -> io::Result<BoundEndpoint> {
    let path = runtime_directory.join(OBSERVATION_SOCKET_NAME);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
        Ok(metadata) if !metadata.file_type().is_socket() => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "agent observation path exists and is not a socket",
            ));
        }
        Ok(metadata) if metadata.uid() != geteuid().as_raw() => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "agent observation socket is foreign-owned",
            ));
        }
        Ok(_) => fs::remove_file(&path)?,
    }
    let listener = UnixListener::bind(&path)?;
    let bound_identity = fs::symlink_metadata(&path)
        .map(|metadata| (metadata.dev(), metadata.ino()))
        .map_err(io::Error::other)?;
    // Everything after this point rolls back through the identity gate.
    let prepared = (|| {
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        let (stop_reader, stop_writer) = UnixStream::pair()?;
        stop_reader.set_nonblocking(true)?;
        io::Result::Ok((stop_reader, stop_writer))
    })();
    let (stop_reader, stop_writer) = match prepared {
        Ok(pair) => pair,
        Err(error) => {
            unlink_if_identity_matches(&path, Some(bound_identity));
            return Err(error);
        }
    };
    Ok(BoundEndpoint {
        path,
        bound_identity: Some(bound_identity),
        listener: Some(listener),
        stop_reader: Some(stop_reader),
        stop_writer: Some(stop_writer),
    })
}

impl BoundEndpoint {
    /// The bound pathname retained by the managed Session for launch environment injection.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Moves the listener and stop reader into the single owned thread
    /// — exactly one owner of each field at every instant. The
    /// pathname's cleanup ownership transfers by disarming this
    /// wrapper's identity AFTER the spawn succeeds: a spawn panic
    /// unwinds `self` with the identity still armed and Drop unlinks
    /// the just-bound path — the bind-rollback shape preserved. On the
    /// success path every owned field is taken and `self` then drops
    /// normally: no `mem::forget`, no leaked allocation.
    pub(crate) fn start(mut self, agents: Arc<AgentService>) -> AgentObservationEndpoint {
        let listener = self.listener.take().expect("bound listener present");
        let stop_reader = self.stop_reader.take().expect("stop reader present");
        let thread = thread::spawn(move || endpoint_loop(listener, stop_reader, agents));
        AgentObservationEndpoint {
            path: std::mem::take(&mut self.path),
            bound_identity: self.bound_identity.take().expect("bound identity present"),
            stop_writer: self.stop_writer.take().expect("stop writer present"),
            thread: Some(thread),
        }
    }
}

impl Drop for BoundEndpoint {
    fn drop(&mut self) {
        unlink_if_identity_matches(&self.path, self.bound_identity);
    }
}

impl AgentObservationEndpoint {
    /// The bound pathname — informational.
    #[cfg(test)]
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for AgentObservationEndpoint {
    /// stop byte -> unconditional join -> identity-gated unlink. The
    /// join is cooperative, not a hard bound: the thread's only
    /// unbounded call is the shared-state fold (bounded CPU today),
    /// and join mirrors exactly that bound. No kill, no supervisor —
    /// the thread is reaped before the pathname is touched, so a
    /// dropped `JoinHandle` can never detach.
    fn drop(&mut self) {
        let _ = self.stop_writer.write_all(&[0u8]);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        unlink_if_identity_matches(&self.path, Some(self.bound_identity));
    }
}

#[cfg(windows)]
impl AgentObservationEndpoint {
    /// Signals the owned ingress and joins it without renewing the Session stop deadline.
    pub(crate) fn stop_before(&mut self, deadline: std::time::Instant) -> io::Result<()> {
        if self
            .thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
        {
            self.stop_writer.write_all(&[0])?;
        }
        while self
            .thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
        {
            if std::time::Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            thread::sleep(std::time::Duration::from_millis(10));
        }
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| io::Error::other("Agent ingress panicked"))?;
        }
        Ok(())
    }
}

enum ConnState {
    Reading {
        body: Vec<u8>,
    },
    /// `acknowledgement` is the accepted envelope generation's reply, whose
    /// single wire definition lives in `hook_contract`.
    AckPending {
        acknowledgement: &'static [u8],
        written: usize,
    },
}

struct Conn {
    stream: UnixStream,
    deadline: Instant,
    state: ConnState,
}

/// One ACK progress step — the private controllable writer seam. The
/// production path calls this with the connection's real nonblocking
/// stream once per POLLOUT wake; unit tests inject a scripted writer to
/// cover the partial/WouldBlock/elapsed arms without
/// environment-dependent socket backpressure.
enum AckProgress {
    Pending,
    Done,
    DeadlineExpired,
    Failed,
}

fn advance_ack_write(
    writer: &mut impl Write,
    acknowledgement: &[u8],
    written: &mut usize,
    deadline: Instant,
    now: Instant,
) -> AckProgress {
    if now >= deadline {
        return AckProgress::DeadlineExpired;
    }
    let Some(remaining) = acknowledgement.get(*written..) else {
        return AckProgress::Failed;
    };
    match writer.write(remaining) {
        Ok(0) => AckProgress::Failed,
        Ok(n) => {
            *written += n;
            if *written == acknowledgement.len() {
                AckProgress::Done
            } else {
                AckProgress::Pending
            }
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => AckProgress::Pending,
        Err(_) => AckProgress::Failed,
    }
}

/// `min(every live connection deadline, earliest settlement deadline,
/// now + POLL_LIVENESS_CAP)`. The cap makes the poll finite even with
/// zero connections and zero timers — the only way a settlement timer
/// created by a different caller (a native Instant seam sharing the
/// cell mutex) is ever observed. A poisoned `next_deadline` fold
/// surfaces `Err` here and simply contributes no deadline — the loop
/// keeps serving (connection handling decides each connection's own
/// honest close).
fn poll_timeout(conns: &[Conn], agents: &AgentService, now: Instant) -> PollTimeout {
    let mut remaining = POLL_LIVENESS_CAP;
    let mut tighten = |deadline: Instant| {
        remaining = remaining.min(deadline.saturating_duration_since(now));
    };
    for conn in conns {
        tighten(conn.deadline);
    }
    if let Ok(Some(deadline)) = agents.next_deadline() {
        tighten(deadline);
    }
    PollTimeout::try_from(remaining).unwrap_or(PollTimeout::MAX)
}

/// The post-admit transition — the injectable deadline-recheck seam.
/// `Some(AckPending)` with the accepted generation's acknowledgement only
/// when the fold accepted AND the shared
/// absolute deadline still holds at the re-check instant; `None`
/// destroys the connection with zero ACK bytes. The fold's outcome is
/// bound, its duration is not — an admitted-but-late connection is
/// destroyed without its ACK (the fold already applied what it
/// applied; the receipt is withheld honestly, never fabricated).
fn state_after_admit(
    acknowledgement: Option<&'static [u8]>,
    now: Instant,
    deadline: Instant,
) -> Option<ConnState> {
    let acknowledgement = acknowledgement.filter(|_| now < deadline)?;
    Some(ConnState::AckPending {
        acknowledgement,
        written: 0,
    })
}

/// The bytes that acknowledge an admission outcome, if it was accepted.
fn acknowledgement_for(accepted: Option<HookGeneration>) -> Option<&'static [u8]> {
    accepted.map(|generation| generation.acknowledgement().as_bytes())
}

/// One connection step; returns true when the connection is destroyed.
/// The admit fold is the loop's only blocking call — stated, not
/// hard-bounded; the same absolute deadline is re-checked after the
/// fold (via `state_after_admit`) and again before every POLLOUT write
/// attempt (inside `advance_ack_write` with a fresh instant).
fn conn_step(conn: &mut Conn, ready: PollFlags, agents: &AgentService) -> bool {
    if ready.intersects(PollFlags::POLLERR | PollFlags::POLLHUP | PollFlags::POLLNVAL) {
        return true;
    }
    match &mut conn.state {
        ConnState::Reading { body } => {
            if !ready.contains(PollFlags::POLLIN) {
                return false;
            }
            let mut chunk = [0u8; 8192];
            match conn.stream.read(&mut chunk) {
                Ok(0) => match agents.admit_envelope(body, Instant::now()) {
                    Ok(accepted) => {
                        match state_after_admit(
                            acknowledgement_for(accepted),
                            Instant::now(),
                            conn.deadline,
                        ) {
                            Some(state) => {
                                conn.state = state;
                                conn_step(conn, PollFlags::POLLOUT, agents)
                            }
                            None => true,
                        }
                    }
                    // Fold unavailability (poisoned cell) closes this
                    // connection honestly; the loop itself keeps serving.
                    Err(_) => true,
                },
                Ok(n) => {
                    if body.len() + n > MAX_OBSERVATION_BODY_BYTES {
                        return true;
                    }
                    body.extend_from_slice(&chunk[..n]);
                    #[cfg(windows)]
                    match serde_json::from_slice::<serde_json::Value>(body) {
                        Ok(_) => {
                            let accepted =
                                agents.admit_envelope(body, Instant::now()).ok().flatten();
                            if let Some(state) = state_after_admit(
                                acknowledgement_for(accepted),
                                Instant::now(),
                                conn.deadline,
                            ) {
                                conn.state = state;
                                return conn_step(conn, PollFlags::POLLOUT, agents);
                            }
                            return true;
                        }
                        Err(error) if !error.is_eof() => return true,
                        Err(_) => {}
                    }
                    false
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => false,
                Err(_) => true,
            }
        }
        ConnState::AckPending {
            acknowledgement,
            written,
        } => {
            if !ready.intersects(PollFlags::POLLOUT | PollFlags::POLLIN) {
                return false;
            }
            match advance_ack_write(
                &mut conn.stream,
                acknowledgement,
                written,
                conn.deadline,
                Instant::now(),
            ) {
                AckProgress::Pending => false,
                AckProgress::Done | AckProgress::DeadlineExpired | AckProgress::Failed => true,
            }
        }
    }
}

/// Drains the accept queue as a bounded batch — `budget` attempts per
/// call, never an unbounded pull. A continuously queued backlog must
/// not starve the stop pipe, the settlement tick, or live connection
/// work: whatever remains queued keeps the listener readable and is
/// drained on the next bounded iteration. The production budget is
/// `MAX_ACTIVE_CONNECTIONS` — the existing cap doubles as the private
/// batch bound, no new setting.
///
/// An accepted socket that cannot be made nonblocking is dropped,
/// never retained: `conn_step` issues reads and ACK writes off
/// readiness and synthetic POLLOUT, so a possibly-blocking fd could
/// wedge the whole loop. Over-cap accepts are likewise dropped — the
/// oracle `maxConnections` shape. `WouldBlock` means the queue is
/// drained; any other accept error is a listener failure and surfaces
/// to the caller, which owns the exit decision.
fn accept_batch(listener: &UnixListener, conns: &mut Vec<Conn>, budget: usize) -> io::Result<()> {
    for _ in 0..budget {
        match listener.accept() {
            Ok((stream, _)) => {
                if stream.set_nonblocking(true).is_err() {
                    continue;
                }
                if conns.len() < MAX_ACTIVE_CONNECTIONS {
                    conns.push(Conn {
                        stream,
                        deadline: Instant::now() + CONNECTION_DEADLINE,
                        state: ConnState::Reading { body: Vec::new() },
                    });
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn endpoint_loop(listener: UnixListener, stop_reader: UnixStream, agents: Arc<AgentService>) {
    let mut conns: Vec<Conn> = Vec::new();
    'poll: loop {
        // Absolute-deadline sweep — every state, including AckPending.
        let now = Instant::now();
        conns.retain(|conn| conn.deadline > now);

        let revents = {
            let mut fds = Vec::with_capacity(conns.len() + 2);
            fds.push(PollFd::new(listener.as_local_io(), PollFlags::POLLIN));
            fds.push(PollFd::new(stop_reader.as_local_io(), PollFlags::POLLIN));
            for conn in &conns {
                let flags = match conn.state {
                    ConnState::Reading { .. } => PollFlags::POLLIN,
                    ConnState::AckPending { .. } => PollFlags::POLLOUT | PollFlags::POLLIN,
                };
                fds.push(PollFd::new(conn.stream.as_local_io(), flags));
            }
            match poll(&mut fds, poll_timeout(&conns, &agents, now)) {
                Ok(_) => {}
                // Signal-interrupted wait — retry the bounded poll.
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue 'poll,
                // A permanent poll failure can never serve again —
                // stop the owner rather than hot-spinning on revents
                // that stay empty forever. Per-connection and fold
                // failures below still never kill the loop; this is
                // the loop's own liveness contract.
                Err(_) => break 'poll,
            }
            fds.iter()
                .map(|fd| fd.revents().unwrap_or(PollFlags::empty()))
                .collect::<Vec<_>>()
        };

        // Stop-pipe wake wins over everything else; a dead listener fd
        // is the only other exit — the thread never dies on a
        // connection- or service-fold failure.
        if revents[1].intersects(PollFlags::POLLIN | PollFlags::POLLERR | PollFlags::POLLHUP)
            || revents[0].intersects(PollFlags::POLLERR | PollFlags::POLLHUP | PollFlags::POLLNVAL)
        {
            break;
        }

        if revents[0].contains(PollFlags::POLLIN)
            && accept_batch(&listener, &mut conns, MAX_ACTIVE_CONNECTIONS).is_err()
        {
            // A non-transient accept failure is a listener failure —
            // the oracle `server.on('error') -> close` shape; the
            // thread exits rather than hot-spinning on an fd that can
            // never serve again.
            break 'poll;
        }

        // The endpoint-owned clock drives settlement: fire the tick
        // when the earliest scheduled timer has passed.
        if let Ok(Some(deadline)) = agents.next_deadline()
            && deadline <= Instant::now()
        {
            let _ = agents.tick(Instant::now());
        }

        // `revents` is the pre-accept snapshot: connections accepted in
        // this same iteration have no polled readiness yet — they take
        // their first step on the next wake under their own deadline.
        // Destruction is deferred until the scan completes: removing a
        // conn mid-scan would shift `conns[]` out of alignment with the
        // fixed `revents[]` slots.
        let stepped = revents.len().saturating_sub(2).min(conns.len());
        let mut destroy: Vec<usize> = Vec::new();
        for index in 0..stepped {
            if revents[2 + index].is_empty() {
                continue;
            }
            if conn_step(&mut conns[index], revents[2 + index], &agents) {
                destroy.push(index);
            }
        }
        for index in destroy.into_iter().rev() {
            conns.remove(index);
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(windows)]
fn unlink_if_identity_matches(_path: &Path, _bound_identity: Option<BoundIdentity>) {}
#[cfg(windows)]
pub(crate) fn bind(runtime_directory: &Path) -> io::Result<BoundEndpoint> {
    let path = orchbee_local_runtime::ModeCheckedLocalSessionRendezvous::observation_endpoint_path(
        runtime_directory,
    );
    let listener = UnixListener::bind(&path)?;
    listener.set_nonblocking(true)?;
    let (stop_reader, stop_writer) = UnixStream::pair()?;
    stop_reader.set_nonblocking(true)?;
    Ok(BoundEndpoint {
        path,
        bound_identity: Some(BoundIdentity),
        listener: Some(listener),
        stop_reader: Some(stop_reader),
        stop_writer: Some(stop_writer),
    })
}
