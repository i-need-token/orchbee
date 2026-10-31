use super::*;
use crate::localization::Locale;
use crate::workbench_client::ui::tests::move_membership::ready_ui;

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
    let launch = Coordinator::default().resolve(&profile).unwrap();
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
        ClientActionDisposition::DismissNavigator
    );
    assert_eq!(ui.preferences.launch_profiles, saved);
    assert!(mailbox.try_recv().unwrap().is_some());
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let view = ui.view();
        let feedback =
            locale.native_notice_with_context(view.notice().unwrap(), view.notice_context());
        assert!(feedback.contains("PowerShell 测试"));
    }
}

#[test]
fn system_one_shot_requires_local_origin_while_ordinary_defaults_stay_unpinned() {
    use crate::local_client::test_support::server::{
        disconnected_origin, disconnected_remote_origin,
    };
    use crate::workbench_client::mailbox::QueuedCommand;

    let local = disconnected_origin();
    let remote = disconnected_remote_origin();
    assert!(local.is_local());
    assert!(!remote.is_local());
    for (initial, replacement, local_execution) in [
        (None, None, false),
        (Some(local.clone()), None, true),
        (Some(remote.clone()), None, false),
        (Some(local.clone()), Some(remote.clone()), false),
        (Some(remote), Some(local), true),
    ] {
        for one_shot in [false, true] {
            let (mut ui, mailbox, _, _) = ready_ui();
            for origin in [initial.as_ref(), replacement.as_ref()]
                .into_iter()
                .flatten()
            {
                ui.set_agent_board_origin(origin.clone());
            }
            let profile = ui.preferences.launch_profiles.profiles[0].clone();
            assert_eq!(
                profile.command,
                UserLaunchCommand::SystemShell { login: false }
            );
            let saved = ui.preferences.clone();
            let (launch, description) = ui.default_create_launch();
            assert!(launch.is_none());
            assert!(description.is_none());
            let disposition = if one_shot {
                ui.create_tab_with_profile(&profile)
            } else {
                ui.create_tab()
            };
            assert_eq!(ui.preferences, saved);
            if one_shot && !local_execution {
                assert_eq!(disposition, ClientActionDisposition::KeepNavigator);
                assert!(mailbox.try_recv().unwrap().is_none());
                assert_eq!(
                    ui.view().notice(),
                    Some(WorkbenchClientNotice::LaunchProfileUnavailable)
                );
            } else {
                assert_eq!(disposition, ClientActionDisposition::DismissNavigator);
                let lease = mailbox.try_recv().unwrap().unwrap();
                assert!(matches!(lease.record(), QueuedCommand::Create(_)));
                if one_shot {
                    assert_eq!(
                        ui.view().notice(),
                        Some(WorkbenchClientNotice::LaunchProfileSelected)
                    );
                }
            }
        }
    }
}

#[test]
fn remote_custom_selection_is_literal_and_local_system_selection_still_checks_profile_equality() {
    use crate::local_client::test_support::server::{
        disconnected_origin, disconnected_remote_origin,
    };

    let (mut ui, mailbox, _, _) = ready_ui();
    ui.set_agent_board_origin(disconnected_remote_origin());
    let profile = custom();
    ui.preferences
        .launch_profiles
        .profiles
        .push(profile.clone());
    let saved = ui.preferences.clone();
    let launch = ui.launch_profiles.resolve(&profile).unwrap();
    let UserLaunchCommand::CustomExecutable {
        executable,
        arguments,
    } = &profile.command
    else {
        panic!("custom launch");
    };
    assert_eq!(launch.executable(), executable);
    assert_eq!(launch.arguments(), arguments);
    assert_eq!(
        ui.create_tab_with_profile(&profile),
        ClientActionDisposition::DismissNavigator
    );
    assert_eq!(ui.preferences, saved);
    assert!(mailbox.try_recv().unwrap().is_some());

    let (mut ui, mailbox, _, _) = ready_ui();
    ui.set_agent_board_origin(disconnected_origin());
    let profile = ui.preferences.launch_profiles.profiles[0].clone();
    ui.preferences.launch_profiles.profiles[0].name = Some("Changed system profile".into());
    let saved = ui.preferences.clone();
    assert_eq!(
        ui.create_tab_with_profile(&profile),
        ClientActionDisposition::KeepNavigator
    );
    assert!(mailbox.try_recv().unwrap().is_none());
    assert_eq!(ui.preferences, saved);
    assert_eq!(
        ui.view().notice(),
        Some(WorkbenchClientNotice::LaunchProfileUnavailable)
    );
}

#[test]
fn failed_default_and_one_shot_launches_keep_their_exact_created_presentation_description() {
    use crate::local_client::workbench::WorkbenchTabCreateCommit;
    use crate::workbench_client::mailbox::{EventDeadline, QueuedCommand, UiReply, WorkerEvent};
    use crate::workbench_client::ui::tests::membership::snapshot;
    use crate::workbench_client::{LaunchProfile, ui::DisplayPhase};
    use crate::workbench_state::CreateProjectionDecision;
    use orchbee_workbench_protocol::{
        PanelNode, WorkbenchSnapshotOutcome, WorkbenchTerminalFailureReason,
        WorkbenchTerminalOutcome,
    };

    for one_shot in [false, true] {
        let (mut ui, mailbox, clock, _) = ready_ui();
        let profile = custom();
        let mut default = custom();
        default.id = "saved-default".into();
        default.name = Some("Saved default".into());
        default.command = UserLaunchCommand::CustomExecutable {
            executable: r"C:\missing-default.exe".into(),
            arguments: vec!["private argument".into()],
        };
        let selected = if one_shot { &profile } else { &default };
        let expected_name = selected.name.clone().unwrap();
        let expected_executable = Coordinator::default()
            .resolve(selected)
            .unwrap()
            .executable()
            .to_owned();
        ui.preferences.launch_profiles.default_profile_id = default.id.clone();
        ui.preferences.launch_profiles.profiles = vec![default, profile.clone()];
        let saved = ui.preferences.clone();
        let old = ui.state.as_ref().unwrap().desired().unwrap().clone();
        assert_eq!(
            if one_shot {
                ui.create_tab_with_profile(&profile)
            } else {
                ui.create_tab()
            },
            ClientActionDisposition::DismissNavigator
        );
        assert_eq!(ui.preferences, saved);
        let lease = mailbox.try_recv().unwrap().unwrap();
        let QueuedCommand::Create(marker) = lease.record() else {
            panic!("Create marker");
        };
        let operation = marker.operation().key().clone();
        let deadline = clock.now() + Duration::from_secs(10);
        // Later repairs must not relabel the already submitted launch.
        ui.preferences.launch_profiles = Default::default();
        let mut candidate = snapshot(3, 2);
        let terminal_context_id = candidate.terminal_contexts[0].terminal_context_id.clone();
        let created = &mut candidate.terminal_contexts[0].tabs[2];
        created.label = "3".into();
        created.automatic_label = true;
        let PanelNode::Panel {
            panel_id,
            primary_presentation,
            ..
        } = &mut created.root
        else {
            panic!("created Panel");
        };
        primary_presentation.label = "3".into();
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::CreateCommittedKnown {
                    operation: operation.clone(),
                    commit: WorkbenchTabCreateCommit::Created {
                        revision: candidate.revision,
                        terminal_context_id,
                        tab_id: created.tab_id.clone(),
                        panel_id: panel_id.clone(),
                        presentation_id: primary_presentation.presentation_id.clone(),
                        terminal_outcome: WorkbenchTerminalOutcome::Failed {
                            reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
                        },
                    },
                },
                EventDeadline::TotalCleanup(deadline),
                &clock,
            )
            .unwrap();
        ui.tick();
        assert!(matches!(
            mailbox.take_deposited_reply(&wait).unwrap(),
            Some(UiReply::CommitRetained)
        ));
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::CreateSnapshotResult {
                    operation,
                    outcome: WorkbenchSnapshotOutcome::Available {
                        snapshot: candidate,
                    },
                },
                EventDeadline::PlacementOnly(deadline),
                &clock,
            )
            .unwrap();
        ui.tick();
        let Some(UiReply::CreateProjection(CreateProjectionDecision::Cleanup(cleanup))) =
            mailbox.take_deposited_reply(&wait).unwrap()
        else {
            panic!("Create cleanup");
        };
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::MembershipDetached {
                    detached: old.anchor().clone(),
                    cleanup,
                },
                EventDeadline::TotalCleanup(deadline),
                &clock,
            )
            .unwrap();
        ui.tick();
        assert!(matches!(
            mailbox.take_deposited_reply(&wait).unwrap(),
            Some(UiReply::Membership(_))
        ));
        mailbox.finish_lease(lease).unwrap();
        assert_eq!(ui.view().phase(), DisplayPhase::SelectedWithoutAttachment);
        let state = ui.state.as_ref().unwrap();
        let old_panel = state.cache().tabs()[0].panel(0).unwrap();
        assert!(state.view().launch_description(old_panel).is_none());
        let created_panel = state.cache().tabs()[2].panel(0).unwrap();
        let view = ui.view();
        let failure = view.launch_failure(created_panel).unwrap();
        assert_eq!(failure.profile, LaunchProfile::Named(&expected_name));
        assert_eq!(failure.executable, Some(expected_executable.as_str()));
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let text = locale.terminal_launch_failure(failure);
            assert!(text.contains(&expected_name));
            assert!(text.contains(&expected_executable));
            assert!(!text.contains("private argument"));
        }
    }
}

#[test]
fn unknown_launch_profile_is_distinct_from_known_system_default_in_both_locales() {
    use crate::localization::LaunchFailure;
    use crate::workbench_client::LaunchProfile;
    for (locale, unknown_name, system_name, obsolete_guidance) in [
        (
            Locale::EnUs,
            "Launch profile: Unknown",
            "System default",
            "repair the default program",
        ),
        (Locale::ZhCn, "启动配置：未知", "系统默认", "修复默认程序"),
    ] {
        let unknown = locale.terminal_launch_failure(LaunchFailure {
            profile: LaunchProfile::Unknown,
            executable: None,
        });
        let known = locale.terminal_launch_failure(LaunchFailure {
            profile: LaunchProfile::SystemDefault,
            executable: None,
        });
        assert!(unknown.contains(unknown_name));
        assert!(!unknown.contains(system_name));
        assert!(known.contains(system_name));
        assert!(!unknown.contains(obsolete_guidance));
        assert!(!known.contains(obsolete_guidance));
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
            ClientActionDisposition::KeepNavigator
        );
        assert!(mailbox.try_recv().unwrap().is_none());
        assert_eq!(ui.preferences, saved);
        assert_eq!(
            ui.view().notice(),
            Some(WorkbenchClientNotice::LaunchProfileUnavailable)
        );
    }
    let mut profile = custom();
    profile.command = UserLaunchCommand::SystemShell { login: true };
    assert!(Coordinator::default().resolve(&profile).is_err());
    profile.command = UserLaunchCommand::CustomExecutable {
        executable: "relative.exe".into(),
        arguments: vec![],
    };
    assert!(Coordinator::default().resolve(&profile).is_err());
}

#[test]
fn command_refusals_keep_the_command_identity_and_distinct_localized_recovery() {
    use crate::workbench_client::client_control::CommandCompletion;
    use orchbee_workbench_protocol::ConfiguredCommandFailure as F;
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let (mut ui, _, _, _) = ready_ui();
        let mut messages = std::collections::BTreeSet::new();
        for reason in [
            F::Denied,
            F::InvalidIntent,
            F::SessionMismatch,
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
                Some(WorkbenchClientNotice::CommandRefused(reason))
            );
            let message =
                locale.native_notice_with_context(view.notice().unwrap(), view.notice_context());
            assert!(message.starts_with("cmd_test_0 · "));
            assert!(messages.insert(message));
        }
    }
}
