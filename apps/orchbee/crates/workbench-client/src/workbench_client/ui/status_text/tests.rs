//! Existing status-precedence witnesses owned by the feature selection policy.
use super::{Locale, status_with_setup};
use crate::workbench_client::{
    LocationLabelStatus as Status, LocationLabelTarget as Target, SetupStatus,
    WorkbenchClientNotice,
};

#[cfg(unix)]
#[test]
fn settled_setup_notices_never_hide_later_failures_or_retirement() {
    use crate::workbench_client::{SetupStatus, WorkbenchClientNotice};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for setup in [
            SetupStatus::ProjectCreated,
            SetupStatus::WorkspaceCreated,
            SetupStatus::DirectoryAccepted,
            SetupStatus::NotApplied,
        ] {
            for notice in [
                WorkbenchClientNotice::CreateNotCreated,
                WorkbenchClientNotice::RenameNotRenamed,
                WorkbenchClientNotice::LocalBusy,
            ] {
                assert_eq!(
                    status_with_setup(
                        locale,
                        "phase".to_owned(),
                        false,
                        Some(notice),
                        Some(setup),
                        true
                    ),
                    locale.native_notice(notice)
                );
                assert_eq!(
                    status_with_setup(
                        locale,
                        "retired outcome unknown".to_owned(),
                        true,
                        Some(notice),
                        Some(setup),
                        true
                    ),
                    "retired outcome unknown"
                );
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn current_failure_keeps_the_known_setup_recovery_fact() {
    use crate::workbench_client::{SetupStatus, WorkbenchClientNotice};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for setup in [
            SetupStatus::CommittedNeedRefresh,
            SetupStatus::OutcomeUnknown,
        ] {
            let notice = WorkbenchClientNotice::LocalBusy;
            let status = status_with_setup(
                locale,
                "phase".to_owned(),
                false,
                Some(notice),
                Some(setup),
                false,
            );
            assert!(status.starts_with(&locale.native_notice(notice)));
            assert!(status.contains(locale.setup_status(setup)));
            let retired =
                status_with_setup(locale, "retired".to_owned(), true, None, Some(setup), false);
            assert!(retired.starts_with("retired"));
            assert!(
                retired.contains(if setup == SetupStatus::CommittedNeedRefresh {
                    locale.setup_committed_fact()
                } else {
                    locale.setup_status(setup)
                })
            );
            assert!(!retired.contains("Ctrl+B w"));
            assert_eq!(
                status_with_setup(locale, "phase".to_owned(), false, None, Some(setup), false),
                locale.setup_status(setup)
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn workspace_created_only_offers_directory_setup_for_an_eligible_current_context() {
    use crate::workbench_client::SetupStatus;
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let setup = SetupStatus::WorkspaceCreated;
        let eligible =
            status_with_setup(locale, "phase".to_owned(), false, None, Some(setup), true);
        assert_eq!(
            eligible,
            match locale {
                Locale::EnUs => "Workspace created · its first directory is not configured",
                Locale::ZhCn => "工作区已创建 · 尚未配置首个目录",
            }
        );
        assert!(!eligible.contains("Ctrl+B"));
        let configured_or_nonempty =
            status_with_setup(locale, "phase".to_owned(), false, None, Some(setup), false);
        assert_eq!(configured_or_nonempty, locale.setup_status(setup));
        assert!(!configured_or_nonempty.contains("Ctrl+B e"));
        let retired =
            status_with_setup(locale, "retired".to_owned(), true, None, Some(setup), true);
        assert_eq!(retired, "retired");
    }
}

#[test]
fn location_rename_recovery_preserves_its_target_without_offering_recovery_after_retirement() {
    for target in [Target::Project, Target::Workspace] {
        let setup = SetupStatus::LocationLabel {
            target,
            status: Status::CommittedNeedRefresh,
        };
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let live =
                status_with_setup(locale, "live".to_owned(), false, None, Some(setup), false);
            assert!(
                live.contains(locale.location_label_status(target, Status::CommittedNeedRefresh))
            );
            assert!(!live.contains("Ctrl+B"));
            let retired =
                status_with_setup(locale, "retired".to_owned(), true, None, Some(setup), false);
            assert!(retired.starts_with("retired"));
            assert!(retired.contains(locale.location_rename_committed_fact(target)));
            assert!(!retired.contains("Ctrl+B w"));
            let notice = WorkbenchClientNotice::LocalBusy;
            let busy = status_with_setup(
                locale,
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
            let setup = SetupStatus::LocationLabel { target, status };
            for locale in [Locale::EnUs, Locale::ZhCn] {
                let notice = WorkbenchClientNotice::CreateNotCreated;
                assert_eq!(
                    status_with_setup(
                        locale,
                        "live".to_owned(),
                        false,
                        Some(notice),
                        Some(setup),
                        false
                    ),
                    locale.native_notice(notice)
                );
                assert_eq!(
                    status_with_setup(locale, "retired".to_owned(), true, None, Some(setup), false),
                    "retired"
                );
            }
        }
    }
}
