use super::*;

fn attachment(value: u8) -> OperationId {
    OperationId::from_bytes([value; 16]).unwrap()
}

fn viewer(value: u8) -> MultiplexViewerId {
    MultiplexViewerId::new(u128::from_be_bytes([value; 16])).unwrap()
}

#[test]
fn a_hint_reaches_only_the_owner_and_stops_with_its_route() {
    let routes = Arc::new(ViewerReadyRoutes::default());
    let owner = Arc::new(ConnectionViewerReady::default());
    let other = Arc::new(ConnectionViewerReady::default());
    let signal = Arc::new(SessionChangeSignal::default());
    owner.bind(&signal);
    let observed = signal.wake_revision();
    let route = routes.route(attachment(1), Arc::clone(&owner));
    let _other_route = routes.route(attachment(2), Arc::clone(&other));

    routes.deliver(attachment(1));
    routes.deliver(attachment(1));
    // An observation viewer has no route; its hints go nowhere.
    routes.deliver(attachment(3));

    assert_ne!(signal.wake_revision(), observed);
    assert_eq!(owner.take(), [viewer(1)]);
    assert!(other.take().is_empty());

    drop(route);
    routes.deliver(attachment(1));
    assert!(owner.take().is_empty());
}
