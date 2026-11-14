use super::Locale;

#[test]
fn blank_workspace_names_have_recoverable_localized_display_names() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for blank in ["", "   ", "\u{a0}\u{2003}\u{3000}"] {
            assert_eq!(
                locale.workspace_display_label(blank),
                locale.unnamed_workspace()
            );
        }
        assert_eq!(
            locale.workspace_display_label("  资料 Notes  "),
            "  资料 Notes  "
        );
    }
    assert_ne!(
        Locale::EnUs.unnamed_workspace(),
        Locale::ZhCn.unnamed_workspace()
    );
}
