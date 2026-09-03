use std::sync::Arc;

use super::{ConnectionSlots, MAXIMUM_CLIENT_CONNECTIONS};

#[test]
fn bounds_connection_threads_and_reuses_a_released_slot() {
    let slots = Arc::new(ConnectionSlots::new());
    let mut reservations = Vec::new();
    for _connection in 0..MAXIMUM_CLIENT_CONNECTIONS {
        let Some(reservation) = slots.acquire() else {
            panic!("connection slot was unavailable within the bound");
        };
        reservations.push(reservation);
    }

    assert!(slots.acquire().is_none());
    let released = reservations.pop();
    assert!(released.is_some());
    drop(released);
    assert!(slots.acquire().is_some());
}
