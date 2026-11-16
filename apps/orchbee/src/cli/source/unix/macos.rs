//! macOS captures through a kqueue: its `EVFILT_SIGNAL` filters wake the poll for the eligible
//! set, while the signals themselves stay blocked and pending on the owner's thread until a
//! nonblocking dequeue consumes one. macOS has no signalfd, and nothing reads the inherited
//! dispositions without unsafe code, so eligibility comes from the kernel's own rule: a blocked
//! signal stays pending unless its disposition is ignore, which discards it. Arming blocks the
//! candidates (the watched members this thread does not already block), raises each on this
//! thread and dequeues it again; a candidate that never became pending is ignored, leaves the
//! kqueue and is unblocked again. A kqueue records only signals sent to the process, never one
//! raised on a thread, so classification never wakes the poll; an ignored signal would wake it,
//! which is why ignored candidates must leave the kqueue.
//!
//! The nonblocking dequeue uses `SIGCONT` as a sentinel, because XNU never discards `SIGCONT`,
//! even when it is ignored: with `SIGCONT` blocked, one raised `SIGCONT` makes `sigwait` over the
//! probed set plus `SIGCONT` return at once, with a pending probed member first, since every
//! watched signal has a lower number. A returned member leaves the sentinel pending, and a second
//! `sigwait` consumes it. A `SIGCONT` already pending under an inherited block merges with the
//! sentinel and is consumed with it; its continue action happened when it was sent.
//!
//! XNU's `sigwait` takes a pending signal from any thread of the process, scanning from the first.
//! The capture therefore keeps the single-owner model Linux states: no other thread holds a
//! pending watched signal, and a process-directed signal that every thread blocks lands on the
//! first thread, the owner's.

use std::os::fd::{AsFd, BorrowedFd};

use nix::{
    errno::Errno,
    libc::timespec,
    sys::{
        event::{EvFlags, EventFilter, FilterFlag, KEvent, Kqueue},
        signal::{SigSet, SigmaskHow, Signal, raise},
    },
};

/// Arming fails only on a signal or kqueue call. A failure before the first probe is raised
/// restores the inherited mask before it is reported; a later one leaves the candidates blocked
/// for the rest of the invocation, so a raised probe is never delivered.
pub(crate) type ArmError = Errno;

/// The user event that marks the kqueue readable for signals raised again at arm.
const REPENDED: usize = 0;

/// The kqueue over the eligible set, and that set for the dequeue.
pub(super) struct Capture {
    queue: Kqueue,
    eligible: SigSet,
}

impl Capture {
    /// Block the candidates, classify them, and keep the inherited mask plus the eligible set
    /// blocked. Returns the capture with the exact prior mask.
    pub(super) fn arm(watched: &[Signal]) -> Result<(Self, SigSet), Errno> {
        let inherited = SigSet::thread_get_mask()?;
        let candidates: SigSet = watched
            .iter()
            .copied()
            .filter(|signal| !inherited.contains(*signal))
            .collect();
        let queue = Kqueue::new()?;
        let mut blocked = inherited;
        blocked.extend(candidates.iter());
        let old_mask = blocked.thread_swap_mask(SigmaskHow::SIG_SETMASK)?;
        let (eligible, sent) = match classify(&queue, candidates) {
            Ok(classified) => classified,
            Err(Unarmed::Restorable(errno)) => {
                old_mask.thread_set_mask()?;
                return Err(errno);
            }
            Err(Unarmed::Held(errno)) => return Err(errno),
        };
        let mut armed = old_mask;
        armed.extend(eligible.iter());
        armed.thread_set_mask()?;
        if sent.iter().next().is_some() {
            for signal in sent.iter() {
                raise(signal)?;
            }
            queue.kevent(
                &[KEvent::new(
                    REPENDED,
                    EventFilter::EVFILT_USER,
                    EvFlags::EV_ADD | EvFlags::EV_CLEAR,
                    FilterFlag::NOTE_TRIGGER,
                    0,
                    0,
                )],
                &mut [],
                Some(NO_WAIT),
            )?;
        }
        Ok((Self { queue, eligible }, old_mask))
    }

    pub(super) fn poll_fd(&self) -> BorrowedFd<'_> {
        self.queue.as_fd()
    }

    /// Clear the queue's readiness, then dequeue the lowest pending eligible signal. A signal
    /// sent after the drain marks the queue again.
    pub(super) fn take(&mut self) -> Result<Option<i32>, Errno> {
        drain(&self.queue)?;
        dequeue(self.eligible).map(|signal| signal.map(|signal| signal as i32))
    }
}

/// Why arming stopped: before any probe was raised, the inherited mask can be restored;
/// afterwards a probe may still be pending, so the candidates stay blocked.
enum Unarmed {
    Restorable(Errno),
    Held(Errno),
}

/// Split the blocked candidates into the eligible set and the signals sent to the process while
/// arming. Each sent signal was dequeued here and must be raised again, exactly once.
fn classify(queue: &Kqueue, candidates: SigSet) -> Result<(SigSet, SigSet), Unarmed> {
    let watch: Vec<KEvent> = candidates
        .iter()
        .map(|signal| signal_event(signal, EvFlags::EV_ADD))
        .collect();
    queue
        .kevent(&watch, &mut [], Some(NO_WAIT))
        .map_err(Unarmed::Restorable)?;
    // Pending already: sent between the mask swap and the registration. An ignored signal never
    // becomes pending, so each is eligible.
    let mut sent = dequeue_all(candidates).map_err(Unarmed::Restorable)?;
    let mut eligible = sent;
    for signal in candidates.iter().filter(|signal| !sent.contains(*signal)) {
        raise(signal).map_err(Unarmed::Held)?;
        let mut probe = SigSet::empty();
        probe.add(signal);
        if dequeue(probe).map_err(Unarmed::Held)?.is_some() {
            eligible.add(signal);
        }
    }
    let unwatch: Vec<KEvent> = candidates
        .iter()
        .filter(|signal| !eligible.contains(*signal))
        .map(|signal| signal_event(signal, EvFlags::EV_DELETE))
        .collect();
    queue
        .kevent(&unwatch, &mut [], Some(NO_WAIT))
        .map_err(Unarmed::Held)?;
    // A recorded signal was sent after the registration: a probe may have merged with it, so it
    // counts as sent. Dequeuing what is still pending leaves each sent signal pending once after
    // it is raised again.
    let recorded = drain(queue).map_err(Unarmed::Held)?;
    let late = dequeue_all(eligible).map_err(Unarmed::Held)?;
    sent.extend(
        recorded
            .iter()
            .chain(late.iter())
            .filter(|signal| eligible.contains(*signal)),
    );
    Ok((eligible, sent))
}

const NO_WAIT: timespec = timespec {
    tv_sec: 0,
    tv_nsec: 0,
};

fn signal_event(signal: Signal, flags: EvFlags) -> KEvent {
    KEvent::new(
        signal as usize,
        EventFilter::EVFILT_SIGNAL,
        flags,
        FilterFlag::empty(),
        0,
        0,
    )
}

/// Retrieve every queued event without waiting, returning the signals recorded since the last
/// drain. Retrieval clears each event.
fn drain(queue: &Kqueue) -> Result<SigSet, Errno> {
    let mut recorded = SigSet::empty();
    let mut events = [signal_event(Signal::SIGHUP, EvFlags::empty()); 8];
    loop {
        let count = queue.kevent(&[], &mut events, Some(NO_WAIT))?;
        for event in &events[..count] {
            if event.filter() == Ok(EventFilter::EVFILT_SIGNAL)
                && let Some(signal) = i32::try_from(event.ident())
                    .ok()
                    .and_then(|signo| Signal::try_from(signo).ok())
            {
                recorded.add(signal);
            }
        }
        if count < events.len() {
            return Ok(recorded);
        }
    }
}

/// Dequeue every pending member of `set` without waiting.
fn dequeue_all(set: SigSet) -> Result<SigSet, Errno> {
    let mut dequeued = SigSet::empty();
    while let Some(signal) = dequeue(set)? {
        dequeued.add(signal);
    }
    Ok(dequeued)
}

/// Dequeue the lowest pending member of `set` on this thread without waiting, or none. Every
/// member must already be blocked here.
fn dequeue(set: SigSet) -> Result<Option<Signal>, Errno> {
    if set.iter().next().is_none() {
        return Ok(None);
    }
    let mut sentinel = SigSet::empty();
    sentinel.add(Signal::SIGCONT);
    let previous = sentinel.thread_swap_mask(SigmaskHow::SIG_BLOCK)?;
    let dequeued = dequeue_before_sentinel(set, sentinel);
    previous.thread_set_mask()?;
    dequeued
}

fn dequeue_before_sentinel(set: SigSet, sentinel: SigSet) -> Result<Option<Signal>, Errno> {
    raise(Signal::SIGCONT)?;
    let mut probe = set;
    probe.add(Signal::SIGCONT);
    let first = probe.wait()?;
    if first == Signal::SIGCONT {
        return Ok(None);
    }
    sentinel.wait()?;
    Ok(Some(first))
}

/// The watched members this process could capture now: neither blocked on this thread nor
/// inherited as ignored, found by arming and disarming once.
#[cfg(test)]
pub(super) fn eligible_now(watched: &[Signal]) -> Vec<Signal> {
    let (capture, old_mask) = Capture::arm(watched).expect("arm to classify");
    old_mask.thread_set_mask().expect("restore thread mask");
    capture.eligible.iter().collect()
}

/// Dequeue a pending member of `set` without waiting, so a test can prove a signal stayed pending
/// and leave nothing behind that could fire later.
#[cfg(test)]
pub(super) fn pending_signo(set: SigSet) -> Option<i32> {
    dequeue(set)
        .expect("probe pending signals")
        .map(|signal| signal as i32)
}
