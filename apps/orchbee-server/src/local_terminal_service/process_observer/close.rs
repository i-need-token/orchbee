//! The evidence a conditional Tab or Panel Close reads through the sampler thread: for each live
//! terminal, the shell's report in one fresh frame and then one process sample.
use super::{LocalTerminalService, ProcessObservationTarget, Wake, on_demand};
use crate::local_terminal_service::tab_close::evidence::CloseFacts;
use std::{
    sync::{Arc, Mutex, PoisonError, atomic::Ordering},
    time::Instant,
};

/// The live terminals of one close, in membership order, and what was read of each so far. Facts
/// are published as they arrive, so a read the budget cuts short still keeps what it learned.
pub(super) struct CloseRead {
    targets: Vec<ProcessObservationTarget>,
    facts: Arc<Mutex<Vec<CloseFacts>>>,
}

pub(super) type Requests = on_demand::Requests<CloseRead, ()>;

impl LocalTerminalService {
    /// What was read of each target by `deadline`, one entry per target in order. A refused,
    /// unavailable or late read leaves its facts missing, which only ever asks.
    pub(in crate::local_terminal_service) fn request_close_facts(
        &self,
        targets: Vec<ProcessObservationTarget>,
        deadline: Instant,
    ) -> Vec<CloseFacts> {
        let facts = Arc::new(Mutex::new(vec![CloseFacts::default(); targets.len()]));
        let read = CloseRead {
            targets,
            facts: Arc::clone(&facts),
        };
        // The answer itself carries nothing: every fact the read reached is already published.
        let _ = self.request_on_demand(
            |wake| &wake.closes,
            read,
            deadline.saturating_duration_since(Instant::now()),
        );
        facts.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

impl CloseRead {
    fn publish(&self, index: usize, facts: CloseFacts) {
        if let Some(slot) = self
            .facts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_mut(index)
        {
            *slot = facts;
        }
    }
    fn runs_nothing(&self, index: usize) -> bool {
        self.facts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(index)
            .is_some_and(|facts| facts.runs_nothing())
    }
}

/// Reads the waiting close's targets in order and stops at the first terminal that asks: the
/// close then asks whatever the others would say.
pub(super) fn serve(service: &LocalTerminalService, wake: &Wake) {
    on_demand::serve(service, wake, &wake.closes, |job| {
        let read = &job.parameters;
        for (index, target) in read.targets.iter().enumerate() {
            if job.cancelled.load(Ordering::Acquire) || Instant::now() >= job.deadline {
                break;
            }
            let pending = service.sample_close_facts(target, &job.cancelled, &mut |facts| {
                read.publish(index, facts);
            });
            if pending.is_some() {
                // As with every on-demand read, a viewer whose cleanup is uncertain voids its read.
                read.publish(index, CloseFacts::default());
                return (None, pending);
            }
            if !read.runs_nothing(index) {
                break;
            }
        }
        (Some(()), None)
    });
}
