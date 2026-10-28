use super::{aggregate, unique};

#[test]
fn aggregate_omits_unknown_and_preserves_attention_priority() {
    assert_eq!(aggregate([].into_iter()), None);
    assert_eq!(
        aggregate([Some("working"), Some("idle")].into_iter()),
        Some("working")
    );
    assert_eq!(aggregate([Some("working"), None].into_iter()), None);
    assert_eq!(
        aggregate([None, Some("blocked")].into_iter()),
        Some("blocked")
    );
}

#[test]
fn provider_is_reported_only_when_the_observed_value_is_unique() {
    assert_eq!(unique(["codex", "codex"].into_iter()), Some("codex".into()));
    assert_eq!(unique(["codex", "claude"].into_iter()), None);
    assert_eq!(unique([].into_iter()), None);
}
