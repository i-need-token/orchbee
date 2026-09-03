use std::num::NonZeroU64;

use super::{ClientSessionAdmission, ClientSessionRegistry, MAXIMUM_ACTIVE_CLIENT_SESSIONS};

#[test]
fn admits_multiple_sessions_with_unique_increasing_generations() {
    let mut registry = ClientSessionRegistry::new();
    let mut generations = Vec::new();
    for _session in 0..MAXIMUM_ACTIVE_CLIENT_SESSIONS {
        let ClientSessionAdmission::Accepted { generation } = registry.admit() else {
            panic!("Client Session was not admitted within capacity");
        };
        generations.push(generation);
    }

    assert_eq!(registry.admit(), ClientSessionAdmission::CapacityExceeded);
    assert!(generations.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn disconnect_removes_only_the_matching_generation() {
    let mut registry = ClientSessionRegistry::new();
    let ClientSessionAdmission::Accepted { generation: first } = registry.admit() else {
        panic!("first Client Session was not admitted");
    };
    let ClientSessionAdmission::Accepted { generation: second } = registry.admit() else {
        panic!("second Client Session was not admitted");
    };

    assert!(registry.disconnect(first));
    assert!(!registry.disconnect(first));
    assert!(registry.disconnect(second));
}

#[test]
fn stale_cleanup_cannot_remove_a_replacement_session() {
    let mut registry = ClientSessionRegistry::new();
    let ClientSessionAdmission::Accepted { generation: first } = registry.admit() else {
        panic!("first Client Session was not admitted");
    };
    assert!(registry.disconnect(first));
    let ClientSessionAdmission::Accepted {
        generation: replacement,
    } = registry.admit()
    else {
        panic!("replacement Client Session was not admitted");
    };

    assert!(!registry.disconnect(first));
    assert!(registry.disconnect(replacement));
}

#[test]
fn admits_the_last_nonzero_generation_before_exhaustion() {
    let maximum_generation = NonZeroU64::MAX;
    let mut registry = ClientSessionRegistry {
        active_generations: Default::default(),
        next_generation: Some(maximum_generation),
    };

    assert_eq!(
        registry.admit(),
        ClientSessionAdmission::Accepted {
            generation: maximum_generation,
        }
    );
    assert!(registry.disconnect(maximum_generation));
    assert_eq!(
        registry.admit(),
        ClientSessionAdmission::GenerationExhausted
    );
}
