use std::num::NonZeroU64;

use orchbee_local_site_protocol::WorkerId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActiveWorker {
    generation: NonZeroU64,
    worker_id: WorkerId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerAdmission {
    Accepted {
        generation: NonZeroU64,
    },
    Duplicate {
        active_generation: NonZeroU64,
        active_worker_id: WorkerId,
    },
    GenerationExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkerRegistry {
    active: Option<ActiveWorker>,
    next_generation: Option<NonZeroU64>,
}

impl WorkerRegistry {
    pub(crate) const fn new() -> Self {
        Self {
            active: None,
            next_generation: Some(NonZeroU64::MIN),
        }
    }

    pub(crate) fn admit(&mut self, worker_id: WorkerId) -> WorkerAdmission {
        if let Some(active) = self.active {
            return WorkerAdmission::Duplicate {
                active_generation: active.generation,
                active_worker_id: active.worker_id,
            };
        }
        let Some(generation) = self.next_generation else {
            return WorkerAdmission::GenerationExhausted;
        };
        self.active = Some(ActiveWorker {
            generation,
            worker_id,
        });
        self.next_generation = generation.get().checked_add(1).and_then(NonZeroU64::new);
        WorkerAdmission::Accepted { generation }
    }

    pub(crate) fn disconnect(&mut self, generation: NonZeroU64) -> bool {
        if self
            .active
            .is_some_and(|active| active.generation == generation)
        {
            self.active = None;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests;
