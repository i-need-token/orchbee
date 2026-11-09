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
        "管理员: ⠋ task",
        "管理员：task",
        "  task  ",
    ] {
        assert_eq!(strip_terminal_title(raw), Some("task"));
    }
    for (raw, expected) in [
        ("⠋ ⠙ task", "⠙ task"),
        ("Administrator: Administrator: task", "Administrator: task"),
        ("管理员: Administrator: task", "Administrator: task"),
        ("管理员:task", "管理员:task"),
        ("管理员 task", "管理员 task"),
        ("★task", "★task"),
        ("✨ task", "✨ task"),
        ("@ task", "@ task"),
        ("task ⠋ detail", "task ⠋ detail"),
        ("⠋task", "⠋task"),
        (" ⠋ 修复🙂标题 ", "修复🙂标题"),
    ] {
        assert_eq!(strip_terminal_title(raw), Some(expected));
    }
    for raw in ["", "  ", "◐", "⠋   ", "Administrator: ", "管理员： "] {
        assert_eq!(strip_terminal_title(raw), None);
    }
}

#[test]
fn published_titles_are_stripped_visible_and_bounded_at_a_character_boundary() {
    assert_eq!(
        project_terminal_title("⠋ a\u{7}b\u{1b}c\u{9b} \u{7f}").as_deref(),
        Some("abc")
    );
    for raw in ["⠋ \u{7}", "Administrator: \u{1b}\u{9c}", "  "] {
        assert_eq!(project_terminal_title(raw), None, "{raw:?}");
    }
    let exact = "a".repeat(MAXIMUM_PRESENTATION_TITLE_BYTES);
    assert_eq!(
        project_terminal_title(&exact).as_deref(),
        Some(exact.as_str())
    );
    for raw in [
        "a".repeat(MAXIMUM_PRESENTATION_TITLE_BYTES + 1),
        format!("✳ {}", "🙂".repeat(300)),
        format!("ab{}", "修".repeat(400)),
    ] {
        let projected = project_terminal_title(&raw).unwrap();
        let kept = projected.strip_suffix(ELLIPSIS).unwrap();
        let visible = strip_terminal_title(&raw).unwrap();
        assert!(projected.len() <= MAXIMUM_PRESENTATION_TITLE_BYTES, "{raw}");
        assert!(visible.starts_with(kept), "{raw}");
        let longer = visible[kept.len()..].chars().next().unwrap().len_utf8();
        assert!(
            projected.len() + longer > MAXIMUM_PRESENTATION_TITLE_BYTES,
            "{raw}"
        );
    }
    let long = "\u{7}🙂".repeat(1000);
    for raw in ["\u{85}⠋ x", "管理员：\u{0}", "✳ \u{202e}rtl\u{200b}", &long] {
        assert!(project_terminal_title(raw).is_none_or(|title| publishable(&title)));
    }
}
