use std::num::NonZeroU64;

use orchbee_local_site_protocol::WorkerId;

use super::{WorkerAdmission, WorkerRegistry};

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
    assert!(registry.disconnect(first_generation));

    let WorkerAdmission::Accepted {
        generation: second_generation,
    } = registry.admit(SECOND_WORKER)
    else {
        panic!("replacement Worker was not admitted");
    };
    assert!(second_generation > first_generation);
}

#[test]
fn stale_cleanup_cannot_remove_a_replacement() {
    let mut registry = WorkerRegistry::new();
    let WorkerAdmission::Accepted {
        generation: first_generation,
    } = registry.admit(FIRST_WORKER)
    else {
        panic!("first Worker was not admitted");
    };
    assert!(registry.disconnect(first_generation));
    let WorkerAdmission::Accepted {
        generation: replacement_generation,
    } = registry.admit(SECOND_WORKER)
    else {
        panic!("replacement Worker was not admitted");
    };

    assert!(!registry.disconnect(first_generation));
    assert_eq!(
        registry.admit(FIRST_WORKER),
        WorkerAdmission::Duplicate {
            active_generation: replacement_generation,
            active_worker_id: SECOND_WORKER,
        }
    );
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
    assert!(registry.disconnect(maximum_generation));
    assert_eq!(
        registry.admit(SECOND_WORKER),
        WorkerAdmission::GenerationExhausted
    );
}
