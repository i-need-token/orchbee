use super::*;
use crate::localization::Locale;
use crate::terminal_session::ui::tests::move_membership::ready_ui;

fn custom() -> UserLaunchProfile {
    UserLaunchProfile {
        id: "literal-launch".into(),
        name: Some("PowerShell 测试".into()),
        command: UserLaunchCommand::CustomExecutable {
            executable: r"C:\Program Files\PowerShell\7\pwsh.exe".into(),
            arguments: vec![
                "".into(),
                "a b".into(),
                "\"quoted\"".into(),
                "中文".into(),
                r"C:\tail\".into(),
            ],
        },
    }
}

#[test]
fn one_shot_selection_keeps_literal_arguments_and_shared_default() {
    let profile = custom();
    let launch = resolve_profile(&profile).unwrap();
    let UserLaunchCommand::CustomExecutable {
        executable,
        arguments,
    } = &profile.command
    else {
        panic!("custom")
    };
    assert_eq!(launch.executable(), executable);
    assert_eq!(launch.arguments(), arguments);
    let (mut ui, mailbox, _, _) = ready_ui();
    ui.preferences
        .launch_profiles
        .profiles
        .push(profile.clone());
    let saved = ui.preferences.launch_profiles.clone();
    assert_eq!(
        ui.create_tab_with_profile(&profile),
        SessionActionDisposition::DismissNavigator
    );
    assert_eq!(ui.preferences.launch_profiles, saved);
    assert!(mailbox.try_recv().unwrap().is_some());
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let view = ui.view();
        let feedback = locale.native_notice_with_context(view.notice().unwrap(), &view);
        assert!(feedback.contains("PowerShell 测试"));
    }
}

#[test]
fn removed_changed_or_unsupported_profile_never_falls_back_to_default() {
    for changed in [false, true] {
        let (mut ui, mailbox, _, _) = ready_ui();
        let mut current = custom();
        if changed {
            current.name = Some("Changed".into());
            ui.preferences.launch_profiles.profiles.push(current);
        }
        let saved = ui.preferences.clone();
        assert_eq!(
            ui.create_tab_with_profile(&custom()),
            SessionActionDisposition::KeepNavigator
        );
        assert!(mailbox.try_recv().unwrap().is_none());
        assert_eq!(ui.preferences, saved);
        assert_eq!(
            ui.view().notice(),
            Some(NativeSessionNotice::LaunchProfileUnavailable)
        );
    }
    let mut profile = custom();
    profile.command = UserLaunchCommand::SystemShell { login: true };
    assert!(resolve_profile(&profile).is_err());
    profile.command = UserLaunchCommand::CustomExecutable {
        executable: "relative.exe".into(),
        arguments: vec![],
    };
    assert!(resolve_profile(&profile).is_err());
}

#[test]
fn command_refusals_keep_the_command_identity_and_distinct_localized_recovery() {
    use crate::terminal_session::client_control::CommandCompletion;
    use orchbee_workbench_protocol::ConfiguredCommandFailure as F;
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let (mut ui, _, _, _) = ready_ui();
        let mut messages = std::collections::BTreeSet::new();
        for reason in [
            F::Denied,
            F::InvalidIntent,
            F::SiteMismatch,
            F::Unavailable,
            F::Busy,
            F::Conflict,
            F::NotFound,
            F::StaleSource,
            F::PhysicalUnknown,
        ] {
            ui.configured_command_completed(CommandCompletion::refused(
                "cmd_test_0".into(),
                reason,
            ));
            let view = ui.view();
            assert_eq!(
                view.notice(),
                Some(NativeSessionNotice::CommandRefused(reason))
            );
            let message = locale.native_notice_with_context(view.notice().unwrap(), &view);
            assert!(message.starts_with("cmd_test_0 · "));
            assert!(messages.insert(message));
        }
    }
}
