use super::*;
#[test]
fn window_title_template_tokens_escapes_missing_values_and_diagnostics() {
    let t =
        WindowTitleTemplate::parse("{{{ hostname }}}/{workspace}/{tab}/{pane}/{terminal_title}")
            .unwrap()
            .unwrap();
    assert_eq!(
        t.render(["Host", "Work", "Tab", "Pane", "Running"])
            .as_deref(),
        Some("{Host}/Work/Tab/Pane/Running")
    );
    assert_eq!(t.render(["", "", "", "", ""]).as_deref(), Some("{}////"));
    assert_eq!(WindowTitleTemplate::parse(""), Ok(None));
    for (text, expected) in [
        ("{x}", "unknown-token"),
        ("{tab", "unclosed-brace"),
        ("x}", "unmatched-brace"),
    ] {
        assert_eq!(WindowTitleTemplate::parse(text), Err(expected));
    }
}
#[test]
fn window_title_template_bounds_after_substitution_preserve_spaces_and_remove_controls() {
    let t = WindowTitleTemplate::parse("  {pane} {terminal_title}  ")
        .unwrap()
        .unwrap();
    assert_eq!(
        t.render(["", "", "", "a\tb", "⠋  c\x1b"]).as_deref(),
        Some("ab ⠋  c")
    );
    let title = t.render(["", "", "", &"😀".repeat(500), ""]).unwrap();
    assert_eq!(title.chars().count(), 198);
    assert!(WindowTitleTemplate::parse(&"x".repeat(4097)).is_err());
    assert!(
        WindowTitleTemplate::parse("{pane}")
            .unwrap()
            .unwrap()
            .render(["", "", "", "", ""])
            .is_none()
    );
}
