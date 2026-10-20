use super::{OutputAfter, OutputCursor};

#[test]
fn cursor_round_trip_preserves_full_width_coordinates() {
    let cursor = OutputCursor {
        terminal_id: "a".repeat(32),
        host_epoch: "b".repeat(32),
        generation: u64::MAX,
        output_epoch: 1,
        revision: u64::MAX,
        row: u64::MAX,
    };
    assert_eq!(
        OutputAfter::parse(&cursor.encode()),
        Some(OutputAfter::Cursor(cursor))
    );
    assert_eq!(OutputAfter::parse("now"), Some(OutputAfter::Now));
}

#[test]
fn malformed_and_cross_version_cursors_fail_before_discovery() {
    let id = "a".repeat(32);
    for value in [
        String::new(),
        "NOW".to_owned(),
        format!("o2:{id}:{id}:1:1:0:0"),
        format!("o1:{id}:{id}:0:1:0:0"),
        format!("o1:{id}:{id}:1:0:0:0"),
        format!("o1:{id}:{id}:1:1:-1:0"),
        format!("o1:{id}:{id}:1:1:0:18446744073709551616"),
        format!("o1:{id}:{id}:1:1:0:0:extra"),
        format!("o1:{}:{id}:1:1:0:0", "0".repeat(32)),
    ] {
        assert!(OutputAfter::parse(&value).is_none(), "{value}");
    }
}
