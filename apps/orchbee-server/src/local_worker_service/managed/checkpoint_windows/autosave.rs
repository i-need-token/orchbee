//! One managed writer. Disk I/O never runs on a Client request or under the Session gate.
use crate::local_terminal_service::LocalTerminalService;
use orchbee_local_runtime::SessionStore;
use orchbee_server::WorkbenchCheckpoint;
use orchbee_workbench_protocol::{WorkbenchRevision, WorkerId};
use std::{
    io,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant, SystemTime},
};

const DEBOUNCE: Duration = Duration::from_millis(500);
const PERIOD: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(50);
const RETRY: Duration = Duration::from_secs(1);
/// The most one background save spends capturing terminal output before it saves anyway.
const OUTPUT_CAPTURE: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SaveKind {
    Background,
    Final,
}

fn should_publish(kind: SaveKind, matches_suppressed_initial: bool) -> bool {
    kind == SaveKind::Final || !matches_suppressed_initial
}

struct Schedule {
    revision: Option<WorkbenchRevision>,
    dirty: Option<Instant>,
    periodic: Instant,
    retry: Option<Instant>,
}
impl Schedule {
    fn new(now: Instant) -> Self {
        Self {
            revision: None,
            dirty: Some(now + DEBOUNCE),
            periodic: now + PERIOD,
            retry: None,
        }
    }
    fn observe(&mut self, revision: WorkbenchRevision, now: Instant) {
        if self.revision != Some(revision) {
            self.revision = Some(revision);
            self.dirty = Some(now + DEBOUNCE);
        }
    }
    fn ready(&self, now: Instant) -> bool {
        self.retry.is_none_or(|retry| now >= retry)
            && (self.dirty.is_some_and(|due| now >= due) || now >= self.periodic)
    }
    fn captured(&mut self, revision: WorkbenchRevision, now: Instant) {
        self.revision = Some(revision);
        self.dirty = None;
        self.periodic = now + PERIOD;
        self.retry = None;
    }
    fn failed(&mut self, now: Instant) {
        self.retry = Some(now + RETRY);
        self.dirty = Some(now + RETRY);
    }
}

pub(in crate::local_worker_service::managed) struct Saver {
    stop: mpsc::Sender<()>,
    join: Option<thread::JoinHandle<()>>,
    stopping: bool,
    failed_join: bool,
}
impl Saver {
    pub(in crate::local_worker_service::managed) fn start(
        store: Arc<Mutex<SessionStore>>,
        terminals: Arc<LocalTerminalService>,
        worker: WorkerId,
        suppress_initial_empty: bool,
    ) -> io::Result<Self> {
        let mut suppressed_payload = if suppress_initial_empty {
            let (_, bytes) = terminals.cached_checkpoint(&worker)?;
            Some(bytes)
        } else {
            None
        };
        Self::spawn(move |receive| {
            let mut schedule = Schedule::new(Instant::now());
            let mut last_error = None;
            let mut last_history_error = None;
            loop {
                // Cancellation also fences a capture that has not begun. A write in progress must
                // return before join can authorize the final clean-stop capture.
                match receive.try_recv() {
                    Ok(()) | Err(mpsc::TryRecvError::Disconnected) => break,
                    Err(mpsc::TryRecvError::Empty) => {}
                }
                let now = Instant::now();
                let result = (|| -> io::Result<bool> {
                    schedule.observe(terminals.checkpoint_revision()?, now);
                    if !schedule.ready(now) {
                        return Ok(false);
                    }
                    if terminals.output_replay_enabled() {
                        terminals.capture_terminal_output(Instant::now() + OUTPUT_CAPTURE);
                    }
                    let (revision, bytes) = terminals.cached_checkpoint(&worker)?;
                    schedule.captured(revision, now);
                    if !publish_for_kind(
                        &store,
                        &bytes,
                        SaveKind::Background,
                        suppressed_payload.as_deref() == Some(bytes.as_slice()),
                    )? {
                        return Ok(false);
                    }
                    suppressed_payload = None;
                    terminals.terminal_output_after_save(&bytes);
                    record_history(&terminals, &bytes, &mut last_history_error);
                    Ok(true)
                })();
                match result {
                    Err(error) => {
                        if last_error != Some(error.kind()) {
                            eprintln!(
                                "orchbee-server diagnostic component=checkpoint operation=autosave stage=save cause=io:{:?}",
                                error.kind()
                            );
                        }
                        last_error = Some(error.kind());
                        schedule.failed(Instant::now());
                    }
                    Ok(_) => last_error = None,
                }
                match receive.recv_timeout(POLL) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        })
    }

    fn spawn(run: impl FnOnce(mpsc::Receiver<()>) + Send + 'static) -> io::Result<Self> {
        let (stop, receive) = mpsc::channel();
        let join = thread::Builder::new()
            .name("workbench-checkpoint".into())
            .spawn(move || run(receive))?;
        Ok(Self {
            stop,
            join: Some(join),
            stopping: false,
            failed_join: false,
        })
    }

    pub(in crate::local_worker_service::managed) fn stop_before(
        &mut self,
        deadline: Instant,
    ) -> io::Result<()> {
        if !self.stopping {
            self.stopping = true;
            let _ = self.stop.send(());
        }
        if self.failed_join {
            return Err(io::Error::other("checkpoint saver panicked"));
        }
        while self.join.as_ref().is_some_and(|join| !join.is_finished()) {
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            thread::sleep(Duration::from_millis(10));
        }
        if let Some(join) = self.join.take()
            && join.join().is_err()
        {
            self.failed_join = true;
            return Err(io::Error::other("checkpoint saver panicked"));
        }
        Ok(())
    }
}

/// Offers a committed save to the earlier-layout history. A failure is a diagnostic, reported
/// once per kind until a write succeeds; it never fails the save.
pub(in crate::local_worker_service::managed) fn record_history(
    terminals: &LocalTerminalService,
    bytes: &[u8],
    last_error: &mut Option<io::ErrorKind>,
) {
    let Some(history) = terminals.layout_history() else {
        return;
    };
    match history.after_save(bytes, SystemTime::now()) {
        Ok(_) => *last_error = None,
        Err(error) => {
            if *last_error != Some(error.kind()) {
                eprintln!(
                    "orchbee-server diagnostic component=checkpoint operation=layout-history stage=record cause=io:{:?}",
                    error.kind()
                );
            }
            *last_error = Some(error.kind());
        }
    }
}

pub(super) fn publish(store: &Mutex<SessionStore>, bytes: &[u8]) -> io::Result<()> {
    store
        .lock()
        .map_err(|_| io::Error::other("checkpoint store failed"))?
        .publish_validated(bytes, |payload| WorkbenchCheckpoint::parse(payload).is_ok())
}

fn publish_for_kind(
    store: &Mutex<SessionStore>,
    bytes: &[u8],
    kind: SaveKind,
    matches_suppressed_initial: bool,
) -> io::Result<bool> {
    if !should_publish(kind, matches_suppressed_initial) {
        return Ok(false);
    }
    publish(store, bytes)?;
    Ok(true)
}

pub(in crate::local_worker_service::managed) fn publish_final(
    store: &Mutex<SessionStore>,
    bytes: &[u8],
) -> io::Result<()> {
    // Explicit clean stop confirms the current workspace even when it still matches the
    // all-invalid empty fallback that background publication suppresses.
    match publish_for_kind(store, bytes, SaveKind::Final, true)? {
        true => Ok(()),
        false => Err(io::Error::other("final checkpoint was suppressed")),
    }
}

#[cfg(test)]
mod tests;
