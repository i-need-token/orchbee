//! A repeated identity answers its first outcome until ten minutes or 256 later outcomes pass.
use super::*;

#[test]
fn a_repeated_identity_answers_its_outcome_until_it_is_forgotten() {
    let base = Instant::now();
    let id = |n: u128| RequestId::from_bytes(n.to_be_bytes()).unwrap();
    assert_eq!(RequestId::from_bytes([0; 16]), None);

    let mut requests = Requests::new();
    assert_eq!(requests.begin(base, id(1)), Begin::Run);
    assert_eq!(requests.begin(base, id(1)), Begin::Running);
    assert_eq!(requests.outcome(base, id(1)), Outcome::Running);
    requests.complete(base, id(1), "first");
    assert_eq!(requests.begin(base, id(1)), Begin::Completed("first"));
    let late = base + KEPT_FOR;
    assert_eq!(requests.outcome(late, id(1)), Outcome::Unknown);
    assert_eq!(requests.begin(late, id(1)), Begin::Run);
    requests.complete(late, id(1), "again");

    for n in 2..=KEPT as u128 + 1 {
        assert_eq!(requests.begin(late, id(n)), Begin::Run);
        requests.complete(late, id(n), "later");
    }
    assert_eq!(requests.outcome(late, id(1)), Outcome::Unknown);
    assert_eq!(requests.outcome(late, id(2)), Outcome::Completed("later"));
}
