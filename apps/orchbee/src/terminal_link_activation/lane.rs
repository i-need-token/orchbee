//! Native TUI single-flight owner for fresh reads and one external effect.

use std::{
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{
    local_client::{LocalClientOrigin, LocalOperationClock},
    machine_observation,
    terminal_link_activation::{
        Activation, Guard, PaintedLinkSource, ReadAttachment, context, plugin,
        windows::SystemBrowser,
    },
};
use orchbee_local_ipc::{AsLocalIo, LocalStream};
use orchbee_workbench_protocol::PluginLinkCatalogGuard;

const JOB_BUDGET: Duration = Duration::from_secs(5);
const NOTICE_FOR: Duration = Duration::from_secs(3);

/// The immediate single-flight admission result returned to the pointer owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Admission {
    Accepted,
    Busy,
    Unavailable,
}

/// The localized terminal outcome that remains visible after one admitted click.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Notice {
    Opened,
    Handled,
    NoLink,
    Failed,
    Unknown,
}

#[derive(Clone)]
struct Job {
    source: PaintedLinkSource,
    column: u16,
    row: u64,
    viewport: Range<u64>,
    deadline: Instant,
}

struct Lane {
    sender: Option<SyncSender<Job>>,
    outcomes: Receiver<Notice>,
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
            .name("orchbee-link-activation".into())
            .spawn(move || {
                let mut browser = SystemBrowser;
                while let Ok(job) = commands.recv() {
                    if stop.load(Ordering::Acquire) != 0 {
                        break;
                    }
                    let result = execute(&origin, &clock, &reader, &stop, &job, &mut browser);
                    if output.send(result).is_err() {
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

/// UI-thread owner for one bounded worker lane, its admission bit, and expiring notice.
pub(crate) struct Driver {
    clock: LocalOperationClock,
    origin: Option<LocalClientOrigin>,
    lane: Option<Lane>,
    inflight: bool,
    notice: Option<(Notice, Instant)>,
}

impl Driver {
    /// Create an inactive owner using the session's monotonic operation clock.
    pub(crate) fn new(clock: LocalOperationClock) -> Self {
        Self {
            clock,
            origin: None,
            lane: None,
            inflight: false,
            notice: None,
        }
    }

    /// Replace the reconnect origin after joining and clearing any older lane.
    pub(crate) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.close();
        self.origin = Some(origin);
    }

    /// Stop admission, interrupt waits, join the owned lane, and clear transient state.
    pub(crate) fn close(&mut self) {
        self.lane.take();
        self.origin = None;
        self.inflight = false;
        self.notice = None;
    }

    /// Admit one exact painted cell without queueing behind an in-flight job.
    pub(crate) fn admit(
        &mut self,
        source: PaintedLinkSource,
        local_column: u16,
        local_row: u16,
    ) -> Admission {
        if self.inflight {
            return Admission::Busy;
        }
        let Some(origin) = self.origin.clone() else {
            return Admission::Unavailable;
        };
        let Some((column, row)) = source.rows.point(local_column, local_row) else {
            return Admission::Unavailable;
        };
        let Some(deadline) = self.clock.now().checked_add(JOB_BUDGET) else {
            return Admission::Unavailable;
        };
        let Some(viewport_end) = source
            .rows
            .start_row
            .checked_add(u64::from(source.rows.row_count))
        else {
            return Admission::Unavailable;
        };
        if self.lane.is_none() {
            let Ok(lane) = Lane::spawn(origin, self.clock.clone()) else {
                return Admission::Unavailable;
            };
            self.lane = Some(lane);
        }
        let job = Job {
            viewport: source.rows.start_row..viewport_end.min(source.rows.total_rows),
            source,
            column,
            row,
            deadline,
        };
        let Some(sender) = self.lane.as_ref().and_then(|lane| lane.sender.as_ref()) else {
            self.lane.take();
            return Admission::Unavailable;
        };
        match sender.try_send(job) {
            Ok(()) => {
                self.inflight = true;
                self.notice = None;
                Admission::Accepted
            }
            Err(TrySendError::Full(_)) => Admission::Busy,
            Err(TrySendError::Disconnected(_)) => {
                self.lane.take();
                Admission::Unavailable
            }
        }
    }

    /// Drain at most one outcome and expire its notice; returns whether the UI must redraw.
    pub(crate) fn tick(&mut self) -> bool {
        let now = self.clock.now();
        let mut changed = false;
        if let Some(result) = self
            .lane
            .as_ref()
            .and_then(|lane| lane.outcomes.try_recv().ok())
        {
            self.inflight = false;
            self.notice = now.checked_add(NOTICE_FOR).map(|until| (result, until));
            changed = true;
        }
        if self
            .lane
            .as_ref()
            .is_some_and(|lane| lane.task.as_ref().is_none_or(JoinHandle::is_finished))
        {
            self.lane.take();
            if self.inflight {
                self.inflight = false;
                self.notice = now
                    .checked_add(NOTICE_FOR)
                    .map(|until| (Notice::Unknown, until));
                changed = true;
            }
        }
        if self.notice.is_some_and(|(_, until)| now >= until) {
            self.notice = None;
            changed = true;
        }
        changed
    }

    /// Return the unexpired outcome notice without changing lane state.
    pub(crate) fn notice(&self) -> Option<Notice> {
        self.notice
            .filter(|(_, until)| self.clock.now() < *until)
            .map(|(notice, _)| notice)
    }

    /// Retire an older completed-job notice before a newer synchronous
    /// admission failure is published by the UI owner.
    pub(crate) fn clear_notice(&mut self) {
        self.notice = None;
    }

    #[cfg(test)]
    pub(crate) fn set_notice_for_test(&mut self, notice: Notice) {
        self.notice = self
            .clock
            .now()
            .checked_add(NOTICE_FOR)
            .map(|until| (notice, until));
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        self.close();
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

/// One-shot external browser submission boundary, replaced by a recording port in tests.
trait BrowserPort {
    /// Submit the URL once and report whether the platform accepted that request.
    fn open(&mut self, url: &str) -> Result<(), ()>;
}

impl BrowserPort for SystemBrowser {
    fn open(&mut self, url: &str) -> Result<(), ()> {
        SystemBrowser::open(self, url)
    }
}

enum EffectHandoff {
    Plugin(Result<Activation, plugin::Error<()>>),
    NoHandler,
}

fn execute(
    origin: &LocalClientOrigin,
    clock: &LocalOperationClock,
    wake: &LocalStream,
    stopping: &AtomicU8,
    job: &Job,
    browser: &mut impl BrowserPort,
) -> Notice {
    let mut guard = LaneGuard {
        clock,
        wake,
        stopping,
    };
    if guard.check(job.deadline).is_err() {
        return Notice::Failed;
    }
    let Ok(mut client) =
        origin.reconnect_before_cancellable(job.deadline, clock.clone(), Some(wake.as_local_io()))
    else {
        return Notice::Failed;
    };
    let Ok(snapshot) = machine_observation::snapshot(&mut client, job.deadline, stopping) else {
        return Notice::Failed;
    };
    if job.source.snapshot_member(&snapshot).is_none() {
        return Notice::Failed;
    }
    let resolved = {
        let Ok(mut reader) = ReadAttachment::open(
            &mut client,
            &snapshot,
            &job.source,
            job.deadline,
            &mut guard,
        ) else {
            return Notice::Failed;
        };
        let result = reader.resolve(job.column, job.row, job.viewport.clone());
        if !reader.detach() {
            return Notice::Failed;
        }
        match result {
            Ok(result) => result,
            Err(_) => return Notice::Failed,
        }
    };
    let Some(url) = resolved.url.as_deref() else {
        return Notice::NoLink;
    };
    if plugin::validate_url::<()>(url).is_err() {
        return Notice::Failed;
    }
    let selection = match plugin::select(url, job.deadline, &mut guard) {
        Ok(selection) => selection,
        Err(_) => return Notice::Failed,
    };
    let initial_catalog = selection.catalog;
    let preflight = match selection.selection {
        Some(selection) => {
            let Ok(preflight) = plugin::Preflight::new::<()>(url, selection) else {
                return Notice::Failed;
            };
            Some(preflight)
        }
        None => None,
    };

    // Effect handoff gets a fresh canonical membership and a fresh attachment
    // read. It validates the same URL/carrier; it never adopts a replacement.
    let Ok(current) = machine_observation::snapshot(&mut client, job.deadline, stopping) else {
        return Notice::Failed;
    };
    if job.source.snapshot_member(&current).is_none() {
        return Notice::Failed;
    }
    let (handoff, observed_context) = {
        let Ok(mut reader) =
            ReadAttachment::open(&mut client, &current, &job.source, job.deadline, &mut guard)
        else {
            return Notice::Failed;
        };
        let context = match preflight.as_ref() {
            Some(preflight) => match reader.observe_context(
                &current,
                &job.source.member,
                preflight.provided_context(),
            ) {
                Ok(context) => Some(context),
                Err(_) => return Notice::Failed,
            },
            None => None,
        };
        let result = reader.resolve(job.column, job.row, job.viewport.clone());
        if !reader.detach() {
            return Notice::Failed;
        }
        (result, context)
    };
    if !matches!(handoff, Ok(ref current) if current == &resolved) {
        return Notice::Failed;
    }
    if context::fence_member(
        &mut client,
        &current,
        &job.source.member,
        job.deadline,
        &mut guard,
    )
    .is_err()
    {
        return Notice::Failed;
    }

    match preflight {
        Some(preflight) => {
            let Some(context) = observed_context else {
                return Notice::Failed;
            };
            let Ok(invocation) = preflight.finish::<()>(context) else {
                return Notice::Failed;
            };
            settle_effect(
                EffectHandoff::Plugin(plugin::submit(
                    &mut client,
                    &current,
                    invocation,
                    job.deadline,
                    &mut guard,
                )),
                url,
                browser,
            )
        }
        None if browser_url(url) => {
            if guard.check(job.deadline).is_err() {
                Notice::Failed
            } else if let Ok(current) = plugin::select(url, job.deadline, &mut guard) {
                // No-handler is an effect-handoff fact. A newly matching
                // entry or any catalog publication refuses the browser route
                // instead of bypassing frozen plugin priority.
                if guard.check(job.deadline).is_err() {
                    Notice::Failed
                } else {
                    settle_browser(&initial_catalog, current, url, browser)
                }
            } else {
                Notice::Failed
            }
        }
        None => Notice::Failed,
    }
}

fn settle_browser(
    initial: &PluginLinkCatalogGuard,
    current: plugin::SelectionSnapshot,
    url: &str,
    browser: &mut impl BrowserPort,
) -> Notice {
    if current.catalog != *initial || current.selection.is_some() {
        Notice::Failed
    } else {
        settle_effect(EffectHandoff::NoHandler, url, browser)
    }
}

fn settle_effect(handoff: EffectHandoff, url: &str, browser: &mut impl BrowserPort) -> Notice {
    match handoff {
        EffectHandoff::Plugin(Ok(Activation::Started { .. })) => Notice::Handled,
        EffectHandoff::Plugin(Ok(Activation::Failed { .. })) => Notice::Failed,
        EffectHandoff::Plugin(Ok(Activation::Unknown { .. })) => Notice::Unknown,
        EffectHandoff::Plugin(Ok(Activation::NoLink | Activation::NoHandler) | Err(_)) => {
            Notice::Failed
        }
        EffectHandoff::NoHandler if browser_url(url) => {
            if browser.open(url).is_ok() {
                Notice::Opened
            } else {
                Notice::Failed
            }
        }
        EffectHandoff::NoHandler => Notice::Failed,
    }
}

fn browser_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

#[cfg(test)]
mod tests;
