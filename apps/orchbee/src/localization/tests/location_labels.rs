use super::Locale;
use crate::terminal_session::{
    NativeLocationLabelStatus as Status, NativeLocationLabelTarget as Target, NativeSessionNotice,
    NativeSetupStatus,
};

#[test]
fn blank_workspace_names_have_localized_validation_and_recoverable_display_names() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for blank in ["", "   ", "\u{a0}\u{2003}\u{3000}"] {
            assert_eq!(
                locale.workspace_display_label(blank),
                locale.unnamed_workspace()
            );
            assert!(!locale.workspace_name_required().trim().is_empty());
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
    assert_ne!(
        Locale::EnUs.workspace_name_required(),
        Locale::ZhCn.workspace_name_required()
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

#[test]
fn location_rename_recovery_preserves_its_target_without_offering_recovery_after_retirement() {
    for target in [Target::Project, Target::Workspace] {
        let setup = NativeSetupStatus::LocationLabel {
            target,
            status: Status::CommittedNeedRefresh,
        };
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let live =
                locale.native_status_with_setup("live".to_owned(), false, None, Some(setup), false);
            assert!(
                live.contains(locale.location_label_status(target, Status::CommittedNeedRefresh))
            );
            assert!(!live.contains("Ctrl+B"));
            let retired = locale.native_status_with_setup(
                "retired".to_owned(),
                true,
                None,
                Some(setup),
                false,
            );
            assert!(retired.starts_with("retired"));
            assert!(retired.contains(locale.location_rename_committed_fact(target)));
            assert!(!retired.contains("Ctrl+B w"));
            let notice = NativeSessionNotice::LocalBusy;
            let busy = locale.native_status_with_setup(
                "live".to_owned(),
                false,
                Some(notice),
                Some(setup),
                false,
            );
            assert!(busy.starts_with(&locale.native_notice(notice)));
            assert!(
                busy.contains(locale.location_label_status(target, Status::CommittedNeedRefresh))
            );
        }
    }
}

#[test]
fn later_errors_take_precedence_over_location_rename_success() {
    for target in [Target::Project, Target::Workspace] {
        for status in [Status::Renamed, Status::Unchanged] {
            let setup = NativeSetupStatus::LocationLabel { target, status };
            for locale in [Locale::EnUs, Locale::ZhCn] {
                let notice = NativeSessionNotice::CreateNotCreated;
                assert_eq!(
                    locale.native_status_with_setup(
                        "live".to_owned(),
                        false,
                        Some(notice),
                        Some(setup),
                        false
                    ),
                    locale.native_notice(notice)
                );
                assert_eq!(
                    locale.native_status_with_setup(
                        "retired".to_owned(),
                        true,
                        None,
                        Some(setup),
                        false
                    ),
                    "retired"
                );
            }
        }
    }
}
