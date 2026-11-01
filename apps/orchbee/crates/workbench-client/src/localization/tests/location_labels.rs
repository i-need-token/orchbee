use super::Locale;
use crate::workbench_state::workspace_setup::{
    LocationLabelStatus as Status, LocationLabelTarget as Target,
};

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

#[test]
fn location_label_results_name_the_target_and_distinguish_unchanged_from_renamed() {
    for (target, name) in [
        (Target::Project, "Project"),
        (Target::Workspace, "Workspace"),
    ] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let unchanged = locale.location_label_status(target, Status::Unchanged);
            let renamed = locale.location_label_status(target, Status::Renamed);
            let unavailable = locale.location_label_status(target, Status::Unavailable);
            assert!(unchanged.contains(name));
            assert!(renamed.contains(name));
            assert!(unavailable.contains(name));
            assert_ne!(unchanged, renamed);
            assert!(!unchanged.contains("Ctrl+B e"));
            assert!(!renamed.contains("Ctrl+B e"));
        }
        assert!(
            Locale::EnUs
                .location_label_status(target, Status::Unchanged)
                .ends_with("label unchanged")
        );
        assert!(
            Locale::EnUs
                .location_label_status(target, Status::Unavailable)
                .contains("no command was sent")
        );
        assert!(
            Locale::ZhCn
                .location_label_status(target, Status::Unavailable)
                .contains("未发送命令")
        );
    }
}
