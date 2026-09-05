use std::{
    num::NonZeroU64,
    sync::{Arc, Mutex, Weak},
};

#[cfg(test)]
use std::{
    io::{Read, Write},
    os::fd::AsFd,
};

use orchbee_local_site_protocol::WorkerId;

use crate::worker_obcn_carrier::{
    WorkerCarrier, WorkerCarrierShutdown, WorkerRequestHandle, WorkerWireExit, WorkerWireOwner,
};

#[derive(Debug)]
struct ActiveCarrier {
    _shutdown: WorkerCarrierShutdown,
    request_handle: WorkerRequestHandle,
}

#[derive(Debug)]
struct ActiveWorker {
    carrier: Option<ActiveCarrier>,
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

pub(crate) struct RegisteredWorkerWireOwner {
    registration: Option<WorkerGenerationRegistration>,
    wire_owner: Option<WorkerWireOwner>,
}

impl RegisteredWorkerWireOwner {
    pub(crate) fn run(mut self, stream: std::os::unix::net::UnixStream) -> WorkerWireExit {
        let wire_owner = self
            .wire_owner
            .take()
            .unwrap_or_else(|| unreachable!("registered Worker must own its wire"));
        wire_owner.run(stream)
    }

    #[cfg(test)]
    pub(crate) fn run_configured_for_test<Stream>(mut self, stream: Stream) -> WorkerWireExit
    where
        Stream: AsFd + Read + Write,
    {
        let wire_owner = self
            .wire_owner
            .take()
            .unwrap_or_else(|| unreachable!("registered Worker must own its wire"));
        wire_owner.run_configured_for_test(stream)
    }
}

impl Drop for RegisteredWorkerWireOwner {
    fn drop(&mut self) {
        // Wire settlement must precede registry retirement so a replacement can never overlap an
        // old write or result publication, including during unwinding and thread-spawn failure.
        drop(self.wire_owner.take());
        drop(self.registration.take());
    }
}

struct WorkerGenerationRegistration {
    generation: NonZeroU64,
    registry: Weak<Mutex<WorkerRegistry>>,
}

impl Drop for WorkerGenerationRegistration {
    fn drop(&mut self) {
        let Some(registry) = self.registry.upgrade() else {
            return;
        };
        if let Ok(mut registry) = registry.lock() {
            // The generation check makes delayed guard destruction harmless to a replacement.
            let _retired = registry.retire_carrier(self.generation);
        }
    }
}

#[derive(Debug)]
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
        if let Some(active) = self.active.as_ref() {
            return WorkerAdmission::Duplicate {
                active_generation: active.generation,
                active_worker_id: active.worker_id,
            };
        }
        let Some(generation) = self.next_generation else {
            return WorkerAdmission::GenerationExhausted;
        };
        self.active = Some(ActiveWorker {
            carrier: None,
            generation,
            worker_id,
        });
        self.next_generation = generation.get().checked_add(1).and_then(NonZeroU64::new);
        WorkerAdmission::Accepted { generation }
    }

    pub(crate) fn register_carrier(
        registry: &Arc<Mutex<Self>>,
        generation: NonZeroU64,
        carrier: WorkerCarrier,
    ) -> Option<RegisteredWorkerWireOwner> {
        let mut registry_guard = registry.lock().ok()?;
        let active = registry_guard.active.as_mut()?;
        if active.generation != generation
            || carrier.generation() != generation
            || carrier.worker_id() != active.worker_id
            || active.carrier.is_some()
            || !carrier.is_active()
        {
            return None;
        }
        let (request_handle, shutdown, wire_owner) = carrier.into_parts();
        active.carrier = Some(ActiveCarrier {
            _shutdown: shutdown,
            request_handle,
        });
        drop(registry_guard);
        Some(RegisteredWorkerWireOwner {
            registration: Some(WorkerGenerationRegistration {
                generation,
                registry: Arc::downgrade(registry),
            }),
            wire_owner: Some(wire_owner),
        })
    }

    pub(crate) fn request_handle(&self) -> Option<WorkerRequestHandle> {
        self.active
            .as_ref()
            .and_then(|active| active.carrier.as_ref())
            .map(|carrier| &carrier.request_handle)
            .filter(|request_handle| request_handle.is_active())
            .cloned()
    }

    pub(crate) fn cancel_pending_admission(&mut self, generation: NonZeroU64) -> bool {
        if self
            .active
            .as_ref()
            .is_none_or(|active| active.generation != generation || active.carrier.is_some())
        {
            return false;
        }
        self.active = None;
        true
    }

    fn retire_carrier(&mut self, generation: NonZeroU64) -> bool {
        if self
            .active
            .as_ref()
            .is_none_or(|active| active.generation != generation || active.carrier.is_none())
        {
            return false;
        }
        self.active = None;
        true
    }
}

#[cfg(test)]
mod tests;
