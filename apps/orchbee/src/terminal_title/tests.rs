use super::*;
#[test]
fn windows_title_stripping_matches_one_elevation_and_one_bounded_activity_prefix() {
    for raw in [
        "⠋ task",
        "✳ task",
        "  ⠙   task  ",
        "✢ task",
        "✻ task",
        "◐ task",
        "◓ task",
        "◑ task",
        "◒ task",
        "Administrator:   ⠋ task",
    ] {
        assert_eq!(stripped(raw), Some("task"));
    }
    for (raw, expected) in [
        ("⠋ ⠙ task", "⠙ task"),
        ("Administrator: Administrator: task", "Administrator: task"),
        ("★task", "★task"),
        ("✨ task", "✨ task"),
        ("@ task", "@ task"),
        ("task ⠋ detail", "task ⠋ detail"),
        ("⠋task", "⠋task"),
        (" ⠋ 修复🙂标题 ", "修复🙂标题"),
    ] {
        assert_eq!(stripped(raw), Some(expected));
    }
    for raw in ["", "  ", "⠋   ", "Administrator: "] {
        assert_eq!(stripped(raw), None);
    }
}
