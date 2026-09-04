use std::{
    num::NonZeroU64,
    sync::{Arc, Mutex},
};

use orchbee_local_site_protocol::WorkerId;

use crate::worker_obcn_carrier::create_worker_carrier;

use super::{WorkerAdmission, WorkerGenerationRegistration, WorkerRegistry};

const FIRST_WORKER: WorkerId = WorkerId::from_bytes([1; 16]);
const SECOND_WORKER: WorkerId = WorkerId::from_bytes([2; 16]);

#[test]
fn admits_only_one_worker_and_assigns_increasing_generations() {
    let mut registry = WorkerRegistry::new();
    let WorkerAdmission::Accepted {
        generation: first_generation,
    } = registry.admit(FIRST_WORKER)
    else {
        panic!("first Worker was not admitted");
    };
    assert_eq!(first_generation, NonZeroU64::MIN);
    assert_eq!(
        registry.admit(SECOND_WORKER),
        WorkerAdmission::Duplicate {
            active_generation: first_generation,
            active_worker_id: FIRST_WORKER,
        }
    );
    assert!(registry.cancel_pending_admission(first_generation));

    let WorkerAdmission::Accepted {
        generation: second_generation,
    } = registry.admit(SECOND_WORKER)
    else {
        panic!("replacement Worker was not admitted");
    };
    assert!(second_generation > first_generation);
}

#[test]
fn stale_registration_cleanup_cannot_remove_a_replacement() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let first_generation = {
        let mut registry = registry.lock().unwrap_or_else(|error| error.into_inner());
        let WorkerAdmission::Accepted { generation } = registry.admit(FIRST_WORKER) else {
            panic!("first Worker was not admitted");
        };
        assert!(registry.cancel_pending_admission(generation));
        generation
    };
    let replacement_generation = {
        let mut registry = registry.lock().unwrap_or_else(|error| error.into_inner());
        let WorkerAdmission::Accepted { generation } = registry.admit(SECOND_WORKER) else {
            panic!("replacement Worker was not admitted");
        };
        generation
    };
    let replacement_carrier = create_worker_carrier(SECOND_WORKER, replacement_generation);
    let replacement_observer = replacement_carrier.request_handle();
    let replacement_owner =
        WorkerRegistry::register_carrier(&registry, replacement_generation, replacement_carrier)
            .unwrap_or_else(|| panic!("replacement carrier was not registered"));

    drop(WorkerGenerationRegistration {
        generation: first_generation,
        registry: Arc::downgrade(&registry),
    });

    let registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
    assert!(replacement_observer.is_active());
    assert_eq!(
        registry_guard
            .request_handle()
            .map(|handle| handle.generation()),
        Some(replacement_generation)
    );
    drop(registry_guard);
    drop(replacement_owner);
}

#[test]
fn exposes_only_the_matching_active_generation_carrier() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let generation = {
        let mut registry = registry.lock().unwrap_or_else(|error| error.into_inner());
        let WorkerAdmission::Accepted { generation } = registry.admit(FIRST_WORKER) else {
            panic!("Worker was not admitted");
        };
        assert!(registry.request_handle().is_none());
        generation
    };

    let wrong_carrier = create_worker_carrier(SECOND_WORKER, generation);
    let wrong_observer = wrong_carrier.request_handle();
    assert!(WorkerRegistry::register_carrier(&registry, generation, wrong_carrier).is_none());
    assert!(!wrong_observer.is_active());
    assert!(
        registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .request_handle()
            .is_none()
    );

    let carrier = create_worker_carrier(FIRST_WORKER, generation);
    let observer = carrier.request_handle();
    let registered_owner = WorkerRegistry::register_carrier(&registry, generation, carrier)
        .unwrap_or_else(|| panic!("matching carrier was not registered"));
    let exposed = registry
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .request_handle()
        .unwrap_or_else(|| panic!("active Worker carrier was not exposed"));
    assert_eq!(exposed.generation(), generation);
    assert_eq!(exposed.worker_id(), FIRST_WORKER);

    let duplicate_carrier = create_worker_carrier(FIRST_WORKER, generation);
    let duplicate_observer = duplicate_carrier.request_handle();
    assert!(WorkerRegistry::register_carrier(&registry, generation, duplicate_carrier).is_none());
    assert!(!duplicate_observer.is_active());
    assert!(observer.is_active());
    assert!(
        !registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .cancel_pending_admission(generation)
    );

    drop(registered_owner);

    assert!(!observer.is_active());
    assert!(!exposed.is_active());
    assert!(
        registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .request_handle()
            .is_none()
    );
}

#[test]
fn stale_carrier_registration_preserves_the_replacement() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let first_generation = {
        let mut registry = registry.lock().unwrap_or_else(|error| error.into_inner());
        let WorkerAdmission::Accepted { generation } = registry.admit(FIRST_WORKER) else {
            panic!("first Worker was not admitted");
        };
        assert!(registry.cancel_pending_admission(generation));
        generation
    };
    let replacement_generation = {
        let mut registry = registry.lock().unwrap_or_else(|error| error.into_inner());
        let WorkerAdmission::Accepted { generation } = registry.admit(SECOND_WORKER) else {
            panic!("replacement Worker was not admitted");
        };
        generation
    };

    let stale_carrier = create_worker_carrier(FIRST_WORKER, first_generation);
    let stale_observer = stale_carrier.request_handle();
    assert!(WorkerRegistry::register_carrier(&registry, first_generation, stale_carrier).is_none());
    assert!(!stale_observer.is_active());

    let replacement_carrier = create_worker_carrier(SECOND_WORKER, replacement_generation);
    let replacement_observer = replacement_carrier.request_handle();
    let replacement_owner =
        WorkerRegistry::register_carrier(&registry, replacement_generation, replacement_carrier)
            .unwrap_or_else(|| panic!("replacement carrier was not registered"));

    assert!(replacement_observer.is_active());
    assert_eq!(
        registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .request_handle()
            .map(|handle| handle.generation()),
        Some(replacement_generation)
    );
    drop(replacement_owner);
}

#[test]
fn admits_the_last_nonzero_generation_before_exhaustion() {
    let maximum_generation = NonZeroU64::MAX;
    let mut registry = WorkerRegistry {
        active: None,
        next_generation: Some(maximum_generation),
    };

    assert_eq!(
        registry.admit(FIRST_WORKER),
        WorkerAdmission::Accepted {
            generation: maximum_generation,
        }
    );
    assert!(registry.cancel_pending_admission(maximum_generation));
    assert_eq!(
        registry.admit(SECOND_WORKER),
        WorkerAdmission::GenerationExhausted
    );
}
