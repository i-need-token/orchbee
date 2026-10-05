//! Owned connection, cancellation, worker, and outcome lane for copy queries.

use std::{
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use orchbee_local_ipc::{AsLocalIo, LocalStream};

use super::{Action, Admission, Failure, Outcome, QueryIdentity, Request, Value};
use crate::{
    local_client::{LocalClientOrigin, LocalOperationClock},
    machine_observation,
    terminal_content::{ContentSource, Guard, ReadSession},
    terminal_copy::{self, CopyError},
};

const JOB_BUDGET: Duration = Duration::from_secs(5);
const MAXIMUM_QUERY_BYTES: usize = 4096;

struct Job {
    request: Request,
    deadline: Instant,
}

#[derive(Clone)]
struct Pending {
    identity: QueryIdentity,
    source: ContentSource,
}

struct Lane {
    sender: Option<SyncSender<Job>>,
    outcomes: Receiver<Outcome>,
    wake: LocalStream,
    stopping: Arc<AtomicU8>,
    task: Option<JoinHandle<()>>,
}

impl Lane {
    fn spawn(origin: LocalClientOrigin, clock: LocalOperationClock) -> std::io::Result<Self> {
        let (sender, commands) = mpsc::sync_channel::<Job>(1);
        let (output, outcomes) = mpsc::sync_channel(1);
        let (reader, wake) = LocalStream::pair()?;
        let stopping = Arc::new(AtomicU8::new(0));
        let stop = Arc::clone(&stopping);
        let task = thread::Builder::new()
            .name("orchbee-copy-query".into())
            .spawn(move || {
                while let Ok(job) = commands.recv() {
                    if stop.load(Ordering::Acquire) != 0 {
                        break;
                    }
                    let result = execute(&origin, &clock, &reader, &stop, &job);
                    let outcome = Outcome {
                        identity: job.request.identity,
                        source: job.request.source,
                        result,
                    };
                    if output.send(outcome).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            sender: Some(sender),
            outcomes,
            wake,
            stopping,
            task: Some(task),
        })
    }

    fn interrupt(&mut self) {
        self.stopping.store(1, Ordering::Release);
        self.sender.take();
        let _ = self.wake.shutdown(std::net::Shutdown::Both);
    }
}

impl Drop for Lane {
    fn drop(&mut self) {
        self.interrupt();
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}

/// UI-thread owner for one independent, bounded, non-queueing read lane.
pub(crate) struct Driver {
    clock: LocalOperationClock,
    origin: Option<LocalClientOrigin>,
    lane: Option<Lane>,
    inflight: Option<Pending>,
}

impl Driver {
    pub(crate) fn new(clock: LocalOperationClock) -> Self {
        Self {
            clock,
            origin: None,
            lane: None,
            inflight: None,
        }
    }

    /// Replace the admitted Client-session reconnect origin after joining any older lane.
    pub(crate) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.close();
        self.origin = Some(origin);
    }

    /// Cancel one obsolete episode while retaining the reconnect origin for a later episode.
    pub(crate) fn cancel(&mut self) {
        self.lane.take();
        self.inflight = None;
    }

    /// Retire the Client-session lane and its reconnect origin.
    pub(crate) fn close(&mut self) {
        self.cancel();
        self.origin = None;
    }

    /// Admit one exact query without queueing behind an in-flight job.
    pub(crate) fn admit(&mut self, request: Request) -> Admission {
        if self.inflight.is_some() {
            return Admission::Busy;
        }
        if matches!(&request.action, Action::Search { query, .. } if query.len() > MAXIMUM_QUERY_BYTES)
        {
            return Admission::LimitExceeded;
        }
        let Some(origin) = self.origin.clone() else {
            return Admission::Unavailable;
        };
        let Some(deadline) = self.clock.now().checked_add(JOB_BUDGET) else {
            return Admission::Unavailable;
        };
        if self.lane.is_none() {
            let Ok(lane) = Lane::spawn(origin, self.clock.clone()) else {
                return Admission::Unavailable;
            };
            self.lane = Some(lane);
        }
        let pending = Pending {
            identity: request.identity,
            source: request.source.clone(),
        };
        let Some(sender) = self.lane.as_ref().and_then(|lane| lane.sender.as_ref()) else {
            self.lane.take();
            return Admission::Unavailable;
        };
        match sender.try_send(Job { request, deadline }) {
            Ok(()) => {
                self.inflight = Some(pending);
                Admission::Accepted
            }
            Err(TrySendError::Full(_)) => Admission::Busy,
            Err(TrySendError::Disconnected(_)) => {
                self.lane.take();
                Admission::Unavailable
            }
        }
    }

    /// Drain at most one exact outcome. Worker loss becomes a keyed unexpected failure.
    pub(crate) fn take_outcome(&mut self) -> Option<Outcome> {
        if let Some(outcome) = self
            .lane
            .as_ref()
            .and_then(|lane| lane.outcomes.try_recv().ok())
        {
            let Some(pending) = self.inflight.take() else {
                self.lane.take();
                return Some(Outcome {
                    identity: outcome.identity,
                    source: outcome.source,
                    result: Err(Failure::Unexpected),
                });
            };
            if outcome.identity == pending.identity && outcome.source == pending.source {
                return Some(outcome);
            }
            self.lane.take();
            return Some(unexpected(pending));
        }
        if self
            .lane
            .as_ref()
            .is_some_and(|lane| lane.task.as_ref().is_none_or(JoinHandle::is_finished))
        {
            self.lane.take();
            return self.inflight.take().map(unexpected);
        }
        None
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        self.close();
    }
}

fn unexpected(pending: Pending) -> Outcome {
    Outcome {
        identity: pending.identity,
        source: pending.source,
        result: Err(Failure::Unexpected),
    }
}

struct LaneGuard<'a> {
    clock: &'a LocalOperationClock,
    wake: &'a LocalStream,
    stopping: &'a AtomicU8,
}

impl Guard for LaneGuard<'_> {
    type Error = ();

    fn check(&mut self, deadline: Instant) -> Result<(), Self::Error> {
        if self.stopping.load(Ordering::Acquire) != 0 || self.clock.now() >= deadline {
            Err(())
        } else {
            Ok(())
        }
    }

    fn wake(&self) -> Option<orchbee_local_ipc::LocalIo<'_>> {
        Some(self.wake.as_local_io())
    }
}

fn execute(
    origin: &LocalClientOrigin,
    clock: &LocalOperationClock,
    wake: &LocalStream,
    stopping: &AtomicU8,
    job: &Job,
) -> Result<Value, Failure> {
    let mut guard = LaneGuard {
        clock,
        wake,
        stopping,
    };
    guard
        .check(job.deadline)
        .map_err(|()| Failure::TransportUnavailable)?;
    let mut client = origin
        .reconnect_before_cancellable(job.deadline, clock.clone(), Some(wake.as_local_io()))
        .map_err(|_| Failure::TransportUnavailable)?;
    let snapshot = machine_observation::snapshot(&mut client, job.deadline, stopping)
        .map_err(|()| Failure::TransportUnavailable)?;
    if job.request.source.snapshot_member(&snapshot).is_none() {
        return Err(Failure::SourceChanged);
    }
    let value = {
        let mut reader = ReadSession::open(
            &mut client,
            &snapshot,
            &job.request.source,
            job.deadline,
            &mut guard,
        )
        .map_err(Failure::from)?;
        let result = evaluate(&mut reader, &job.request.action);
        if !reader.detach() {
            return Err(Failure::DetachUnknown);
        }
        result?
    };
    let current = machine_observation::snapshot(&mut client, job.deadline, stopping)
        .map_err(|()| Failure::TransportUnavailable)?;
    if job.request.source.snapshot_member(&current).is_none() {
        return Err(Failure::SourceChanged);
    }
    guard
        .check(job.deadline)
        .map_err(|()| Failure::TransportUnavailable)?;
    Ok(value)
}

fn evaluate(
    reader: &mut ReadSession<'_, LaneGuard<'_>>,
    action: &Action,
) -> Result<Value, Failure> {
    if reader.total_rows() > u64::from(u32::MAX) + 1 {
        return Err(Failure::LimitExceeded);
    }
    match action {
        Action::Motion { cursor, motion } => {
            reader.validate_point(*cursor).map_err(Failure::from)?;
            let columns = reader.columns();
            let total = reader.total_rows();
            terminal_copy::read_motion_with(
                columns,
                total,
                *cursor,
                *motion,
                |range| reader.rows(range),
                || crate::terminal_content::Error::LimitExceeded,
                || crate::terminal_content::Error::ProtocolInvalid,
            )
            .map(Value::Motion)
            .map_err(Failure::from)
        }
        Action::Search {
            cursor,
            query,
            direction,
            previous,
        } => {
            reader.validate_point(*cursor).map_err(Failure::from)?;
            if let Some(previous) = previous {
                reader
                    .validate_point(previous.start)
                    .map_err(Failure::from)?;
                reader.validate_point(previous.end).map_err(Failure::from)?;
            }
            let rows = if query.is_empty() {
                Vec::new()
            } else {
                let total = reader.total_rows();
                reader.rows(0..total).map_err(Failure::from)?
            };
            terminal_copy::search(
                &rows,
                reader.columns(),
                query,
                *direction,
                *cursor,
                *previous,
            )
            .map(Value::Search)
            .map_err(|error| match error {
                CopyError::LimitExceeded => Failure::LimitExceeded,
                CopyError::PatternTooComplex => Failure::PatternTooComplex,
            })
        }
    }
}

#[cfg(test)]
mod tests;
