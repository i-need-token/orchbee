//! The retry decision rules: bounded fresh reads, the unchanged-identity condition, and no retry
//! after an unknown outcome or for a caller-owned identity.

use super::*;

#[test]
fn a_stale_refusal_earns_at_most_three_fresh_reads() {
    let mut retry = StaleRetry::<u8>::new(true);
    for _ in 0..RETRIES {
        assert!(retry.reread(Ended::Stale));
    }
    assert!(!retry.reread(Ended::Stale));
    assert!(!retry.reread(Ended::Changed));
}

#[test]
fn changed_reads_and_stale_refusals_share_one_budget() {
    let mut retry = StaleRetry::<u8>::new(true);
    assert!(retry.reread(Ended::Changed));
    assert!(retry.reread(Ended::Stale));
    assert!(retry.reread(Ended::Changed));
    assert!(!retry.reread(Ended::Stale));
}

#[test]
fn only_the_identity_submitted_first_is_submitted_again() {
    let mut retry = StaleRetry::new(true);
    assert!(retry.submit(1));
    assert!(retry.submit(1));
    assert!(!retry.submit(2));
    assert!(retry.submit(1));
}

#[test]
fn a_caller_identity_is_never_resubmitted() {
    let mut retry = StaleRetry::<u8>::new(false);
    assert!(!retry.reread(Ended::Stale));
    assert!(retry.reread(Ended::Changed));
}

#[test]
fn only_a_marked_definite_refusal_earns_a_fresh_read() {
    let marked = |kind| Failure::new(kind).mark_stale_view();
    assert_eq!(
        Ended::submission(&marked(FailureKind::Rejected)),
        Ended::Stale
    );
    assert_eq!(
        Ended::resolution(&marked(FailureKind::Rejected)),
        Ended::Changed
    );
    for failure in [
        marked(FailureKind::OutcomeUnknown),
        marked(FailureKind::TransportLoss),
        marked(FailureKind::Timeout),
        Failure::new(FailureKind::Rejected),
    ] {
        assert_eq!(Ended::submission(&failure), Ended::Settled);
        assert_eq!(Ended::resolution(&failure), Ended::Settled);
    }
    let mut retry = StaleRetry::<u8>::new(true);
    assert!(!retry.reread(Ended::Settled));
    assert!(retry.reread(Ended::Stale));
}

#[test]
fn a_creation_is_submitted_again_only_when_every_derived_value_matches() {
    // A creation's identity pairs its resolved request with the Client and directory it derived.
    let mut retry = StaleRetry::new(true);
    assert!(retry.submit(("tab.create 2", Some(7), "C:/repo")));
    assert!(retry.submit(("tab.create 2", Some(7), "C:/repo")));
    assert!(!retry.submit(("tab.create 2", Some(7), "C:/other")));
    assert!(!retry.submit(("tab.create 2", None, "C:/repo")));
    assert!(!retry.submit(("tab.create 3", Some(7), "C:/repo")));
}
