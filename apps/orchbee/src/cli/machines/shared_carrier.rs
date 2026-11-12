//! What the machine connection carriers share: one lock over their state with one signal, the
//! local `OBHS` acceptor profile, the single-process `query` answer, the `OBHS` answer write, and
//! the frame loop over each read.

use orchbee_connection_handshake::{
    AcceptorMessage, AcceptorProfile, Build, ConnectionEntry, ConnectionKind, Extensions,
    ProcessEntry, ProcessRole, ProcessState, ProductVersion, QueryResult, encode_acceptor_frame,
};
use orchbee_machine_protocol::{Frame, FrameDecoder, FrameError};
use std::{
    io::Write,
    num::NonZeroU32,
    sync::{Condvar, Mutex, MutexGuard, PoisonError},
    time::Instant,
};

/// A carrier's state under its lock, and the signal that wakes every thread waiting to look at it
/// again. No thread holds the lock across I/O.
pub(super) struct Shared<S> {
    state: Mutex<S>,
    signal: Condvar,
}

impl<S> Shared<S> {
    pub(super) const fn new(state: S) -> Self {
        Self {
            state: Mutex::new(state),
            signal: Condvar::new(),
        }
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, S> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Wakes every waiting thread to look at the state again.
    pub(super) fn wake(&self) {
        self.signal.notify_all();
    }

    pub(super) fn wait<'a>(&self, state: MutexGuard<'a, S>) -> MutexGuard<'a, S> {
        self.signal
            .wait(state)
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn wait_until<'a>(
        &self,
        state: MutexGuard<'a, S>,
        deadline: Instant,
    ) -> MutexGuard<'a, S> {
        let timeout = deadline.saturating_duration_since(Instant::now());
        self.signal
            .wait_timeout(state, timeout)
            .unwrap_or_else(PoisonError::into_inner)
            .0
    }
}

/// This build's acceptor of `connection`, in this environment.
pub(super) fn acceptor_profile(connection: ConnectionKind) -> AcceptorProfile {
    AcceptorProfile::local(
        connection,
        ProductVersion::new(env!("CARGO_PKG_VERSION"))
            .expect("the package version is printable ASCII"),
        Build::default(),
        orchbee_local_runtime::environment_name(),
    )
}

/// The one process entry a `query` reports: this process in `role` and `state`, holding no
/// terminal and serving the profile's connection.
pub(super) fn query(
    profile: &AcceptorProfile,
    role: ProcessRole,
    state: ProcessState,
) -> Option<QueryResult> {
    let entry = ProcessEntry {
        role,
        product_version: profile.product_version.clone(),
        build: profile.build.clone(),
        environment: profile.environment.name().clone(),
        pid: NonZeroU32::new(std::process::id())?,
        state,
        terminals: 0,
        connections: vec![ConnectionEntry {
            kind: profile.connection.contract_name(),
            generations: profile.generations,
            capabilities: Some(profile.capabilities.clone()),
        }],
    };
    QueryResult::new(vec![entry], None, Extensions::new()).ok()
}

/// Writes one `OBHS` acceptor frame; false when it could not be written whole.
pub(super) fn answer(output: &mut impl Write, message: &AcceptorMessage) -> bool {
    encode_acceptor_frame(message).is_ok_and(|frame| {
        output
            .write_all(&frame)
            .and_then(|()| output.flush())
            .is_ok()
    })
}

/// Feeds one read's bytes to `decoder` and hands each complete frame to `take`, which returns
/// false to stop. Returns whether to keep reading, or the error that closes the connection.
pub(super) fn frames(
    decoder: &mut FrameDecoder,
    bytes: &[u8],
    mut take: impl FnMut(&Frame) -> bool,
) -> Result<bool, FrameError> {
    let mut offset = 0;
    while let Some(rest) = bytes.get(offset..).filter(|rest| !rest.is_empty()) {
        let (consumed, frame) = decoder.decode(rest)?;
        offset += consumed;
        if let Some(frame) = frame
            && !take(&frame)
        {
            return Ok(false);
        }
        if consumed == 0 {
            break;
        }
    }
    Ok(true)
}
