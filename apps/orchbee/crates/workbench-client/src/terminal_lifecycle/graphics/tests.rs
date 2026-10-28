use super::*;
#[test]
fn deletion_is_owned_bounded_and_unsettled_until_the_flush() {
    let lease = ImageLease::new();
    assert!(!lease.register(0));
    for id in 1..=128 {
        assert!(lease.register(id));
    }
    assert!(!lease.register(129));
    assert!(!lease.register(1));
    lease.control_started();
    let before = lease.cleanup_sequence();
    assert!(before.starts_with("\x18\x1b\\"));
    assert!(before.contains(&delete(1)));
    assert!(!before.contains("d=A"));
    assert_eq!(lease.capacity(), 0);
    lease.flushed(&[1]);
    assert_eq!(lease.capacity(), 1);
    assert!(!lease.cleanup_sequence().contains(&delete(1)));
    assert!(lease.obligation.restoration_required());
    lease.flushed(&(2..=128).collect::<Vec<_>>());
    assert_eq!(lease.cleanup_sequence(), "");
    assert!(!lease.obligation.restoration_required());
}
