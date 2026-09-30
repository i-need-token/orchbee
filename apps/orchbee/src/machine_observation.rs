//! Common bounded, cancellable observation lifecycle for Agent and Workspace consumers.
use crate::{
    local_client::LocalClient, machine_discovery::Connection, machine_picker::Choice,
    machines::ProfileId,
};
use orchbee_workbench_protocol::*;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
const FRESH_FOR: Duration = Duration::from_secs(10);
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Observation<T> {
    Loading,
    Ready(Arc<T>),
    Unavailable,
}
#[derive(Clone)]
pub(crate) struct Source {
    pub(crate) choice: Choice,
    pub(crate) connection: Connection,
}
type Reader<T> = fn(&mut LocalClient, Instant, &AtomicU8) -> Result<Arc<T>, ()>;
pub(crate) fn request_id() -> Result<WorkbenchRequestId, ()> {
    WorkbenchRequestId::parse(ProfileId::generate().map_err(|_| ())?.as_str()).map_err(|_| ())
}
pub(crate) fn check(deadline: Instant, cancel: &AtomicU8) -> Result<(), ()> {
    if Instant::now() >= deadline || cancel.load(Ordering::Acquire) != 0 {
        Err(())
    } else {
        Ok(())
    }
}
pub(crate) fn snapshot(
    client: &mut LocalClient,
    deadline: Instant,
    cancel: &AtomicU8,
) -> Result<Snapshot, ()> {
    check(deadline, cancel)?;
    match client
        .submit_workbench_snapshot_read_before_cancellable(
            SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: request_id()?,
            },
            deadline,
            Some(orchbee_local_ipc::LocalIo::Wake(cancel, 1)),
        )
        .map_err(|_| ())?
        .outcome
    {
        WorkbenchSnapshotOutcome::Available { snapshot } => Ok(snapshot),
        _ => Err(()),
    }
}
struct Completed<T> {
    at: Instant,
    rows: Result<Arc<T>, ()>,
}
struct Job<T> {
    cancel: Arc<AtomicU8>,
    task: Option<JoinHandle<Completed<T>>>,
}
impl<T: Send + Sync + 'static> Job<T> {
    fn start(source: Connection, deadline: Instant, read: Reader<T>) -> Result<Self, ()> {
        let cancel = Arc::new(AtomicU8::new(0));
        let flag = cancel.clone();
        let task = thread::Builder::new()
            .name("orchbee-machine-observation".into())
            .spawn(move || {
                let result = (|| {
                    let mut client = source.reconnect(deadline, &flag).map_err(|_| ())?;
                    read(&mut client, deadline, &flag)
                })();
                Completed {
                    at: Instant::now(),
                    rows: result,
                }
            })
            .map_err(|_| ())?;
        Ok(Self {
            cancel,
            task: Some(task),
        })
    }
    fn stop(&self) {
        self.cancel.store(1, Ordering::Release);
    }
    fn finished(&self) -> bool {
        self.task.as_ref().is_none_or(JoinHandle::is_finished)
    }
}
impl<T> Drop for Job<T> {
    fn drop(&mut self) {
        self.cancel.store(1, Ordering::Release);
        if let Some(task) = self.task.take() {
            let _settled = task.join();
        }
    }
}
struct Entry<T> {
    choice: Choice,
    job: Option<Job<T>>,
    observation: Observation<T>,
    observed: Option<Instant>,
    next: Instant,
}
pub(crate) struct Collector<T> {
    entries: Vec<Entry<T>>,
    retired: Vec<Job<T>>,
    cursor: usize,
}
impl<T: Clone + Send + Sync + 'static> Collector<T> {
    pub(crate) fn clear(&mut self) {
        for mut entry in self.entries.drain(..) {
            if let Some(job) = entry.job.take() {
                job.stop();
                self.retired.push(job);
            }
        }
    }
    pub(crate) fn tick(
        &mut self,
        sources: Vec<Source>,
        now: Instant,
        read: Reader<T>,
    ) -> Vec<(Choice, Observation<T>)> {
        self.tick_with_start(sources, now, |source, deadline| {
            Job::start(source, deadline, read)
        })
    }
    fn tick_with_start(
        &mut self,
        sources: Vec<Source>,
        now: Instant,
        mut start: impl FnMut(Connection, Instant) -> Result<Job<T>, ()>,
    ) -> Vec<(Choice, Observation<T>)> {
        self.retired.retain(|job| !job.finished());
        let mut old = std::mem::take(&mut self.entries);
        for source in &sources {
            let entry = old
                .iter()
                .position(|e| e.choice == source.choice)
                .map(|i| old.remove(i))
                .unwrap_or_else(|| Entry {
                    choice: source.choice.clone(),
                    job: None,
                    observation: Observation::Loading,
                    observed: None,
                    next: now,
                });
            self.entries.push(entry);
        }
        for mut entry in old {
            if let Some(job) = entry.job.take() {
                job.stop();
                self.retired.push(job);
            }
        }
        for entry in &mut self.entries {
            if entry.job.as_ref().is_some_and(Job::finished) {
                let mut job = entry.job.take().expect("finished job");
                entry.observation = match job.task.take().expect("owned job").join() {
                    Ok(Completed { at, rows: Ok(rows) }) => {
                        entry.observed = Some(at);
                        Observation::Ready(rows)
                    }
                    _ => {
                        entry.observed = None;
                        Observation::Unavailable
                    }
                };
                entry.next = now + Duration::from_secs(2);
            }
        }
        let mut active =
            self.retired.len() + self.entries.iter().filter(|e| e.job.is_some()).count();
        let len = self.entries.len();
        for offset in 0..len {
            let index = (self.cursor + offset) % len;
            let entry = &mut self.entries[index];
            if active >= 4 {
                break;
            }
            if entry.job.is_none() && now >= entry.next {
                match start(
                    sources[index].connection.clone(),
                    now + Duration::from_secs(5),
                ) {
                    Ok(job) => {
                        entry.job = Some(job);
                        active += 1;
                    }
                    Err(()) => {
                        entry.observation = Observation::Unavailable;
                        entry.next = now + Duration::from_secs(2);
                    }
                }
            }
        }
        if len > 0 {
            self.cursor = (self.cursor + 1) % len;
        }
        self.entries
            .iter()
            .map(|entry| {
                (
                    entry.choice.clone(),
                    if entry
                        .observed
                        .is_some_and(|at| now.saturating_duration_since(at) > FRESH_FOR)
                    {
                        Observation::Unavailable
                    } else {
                        entry.observation.clone()
                    },
                )
            })
            .collect()
    }
}
impl<T> Drop for Collector<T> {
    fn drop(&mut self) {
        for entry in &self.entries {
            if let Some(job) = &entry.job {
                job.cancel.store(1, Ordering::Release);
            }
        }
        for job in &self.retired {
            job.cancel.store(1, Ordering::Release);
        }
        // Job::drop joins every writer after the cancellation pass.
    }
}
impl<T> Default for Collector<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            retired: Vec::new(),
            cursor: 0,
        }
    }
}

#[cfg(test)]
mod tests;
