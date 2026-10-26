use std::num::NonZeroU64;

use super::{
    ClientConnectionAdmission, ClientConnectionRegistry, MAXIMUM_ACTIVE_CLIENT_CONNECTIONS,
};

#[test]
fn admits_multiple_connections_with_unique_increasing_generations() {
    let mut registry = ClientConnectionRegistry::new();
    let mut generations = Vec::new();
    for _connection in 0..MAXIMUM_ACTIVE_CLIENT_CONNECTIONS {
        let ClientConnectionAdmission::Accepted { generation } = registry.admit() else {
            panic!("Client Connection was not admitted within capacity");
        };
        generations.push(generation);
    }

    assert_eq!(
        registry.admit(),
        ClientConnectionAdmission::CapacityExceeded
    );
    assert!(generations.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn disconnect_removes_only_the_matching_generation() {
    let mut registry = ClientConnectionRegistry::new();
    let ClientConnectionAdmission::Accepted { generation: first } = registry.admit() else {
        panic!("first Client Connection was not admitted");
    };
    let ClientConnectionAdmission::Accepted { generation: second } = registry.admit() else {
        panic!("second Client Connection was not admitted");
    };

    assert!(registry.disconnect(first));
    assert!(!registry.disconnect(first));
    assert!(registry.disconnect(second));
}

#[test]
fn stale_cleanup_cannot_remove_a_replacement_connection() {
    let mut registry = ClientConnectionRegistry::new();
    let ClientConnectionAdmission::Accepted { generation: first } = registry.admit() else {
        panic!("first Client Connection was not admitted");
    };
    assert!(registry.disconnect(first));
    let ClientConnectionAdmission::Accepted {
        generation: replacement,
    } = registry.admit()
    else {
        panic!("replacement Client Connection was not admitted");
    };

    assert!(!registry.disconnect(first));
    assert!(registry.disconnect(replacement));
}

#[test]
fn admits_the_last_nonzero_generation_before_exhaustion() {
    let maximum_generation = NonZeroU64::MAX;
    let mut registry = ClientConnectionRegistry {
        active_generations: Default::default(),
        next_generation: Some(maximum_generation),
    };

    assert_eq!(
        registry.admit(),
        ClientConnectionAdmission::Accepted {
            generation: maximum_generation,
        }
    );
    assert!(registry.disconnect(maximum_generation));
    assert_eq!(
        registry.admit(),
        ClientConnectionAdmission::GenerationExhausted
    );
}
