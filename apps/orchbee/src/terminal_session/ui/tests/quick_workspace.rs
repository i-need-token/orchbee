use super::super::tests::workspace_setup::{finish_preparation, finish_preparation_with_directory};
use super::super::*;
use orchbee_workbench_protocol::Snapshot;

fn no_workspace() -> (NativeUi, Arc<CommandMailbox>, LocalOperationClock, Snapshot) {
    let snapshot = super::super::tests::membership::snapshot(0, 1);
    let result = empty_ui(snapshot);
    assert!(result.0.view().no_current_workspace());
    assert_eq!(result.0.view().context_count(), 0);
    assert!(result.0.view().tab_create_target().is_none());
    result
}

fn empty_ui(snapshot: Snapshot) -> (NativeUi, Arc<CommandMailbox>, LocalOperationClock, Snapshot) {
    let clock = crate::local_client::tests::server::adjustable_operation_clock(Arc::new(
        std::sync::atomic::AtomicU64::new(0),
    ));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: crate::terminal_session::workbench_projection::project_native_context_tabs(
                    &snapshot,
                    &crate::terminal_session::workbench_projection::preferred_workspace_location(
                        &snapshot,
                    )
                    .unwrap(),
                )
                .unwrap(),
            },
            super::super::super::mailbox::EventDeadline::Startup(
                clock.now() + super::super::super::state::OPERATION_TOTAL,
            ),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    (ui, mailbox, clock, snapshot)
}

#[test]
fn empty_compatibility_create_tab_uses_workspace_launch_and_cancel_never_resurrects_a_terminal() {
    for cancelled in [false, true] {
        let (mut ui, mailbox, clock, snapshot) = no_workspace();
        ui.apply_client_action(ClientAction::TabMenu {
            action: crate::action_menu::MenuAction::NewGroupWithTab,
            tab_id: None,
        });
        assert_eq!(ui.view().notice(), Some(NativeSessionNotice::NoWorkspace));
        assert!(mailbox.try_recv().unwrap().is_none());
        ui.apply_client_action(ClientAction::CreateTab);
        finish_preparation(&mut ui, &mailbox, &clock, &snapshot, cancelled);
        if cancelled {
            assert!(ui.view().no_current_workspace());
            assert!(mailbox.try_recv().unwrap().is_none());
        } else {
            let lease = mailbox.try_recv().unwrap().unwrap();
            assert!(
                matches!(lease.record(), QueuedCommand::WorkspaceSetup(marker)
                if matches!(marker.0.command.action, WorkbenchAction::WorkspaceLaunch { .. }))
            );
        }
        assert_eq!(
            ui.state.as_ref().unwrap().cache().canonical_snapshot(),
            &snapshot
        );
        mailbox.close(CarrierFailure::Closed);
    }
}

#[test]
fn no_workspace_screen_exposes_creation_without_legacy_navigation_in_both_locales_and_size_tiers() {
    use crate::{
        client_view::ClientView, localization::Locale, terminal_session::TerminalSessionView,
        tui_pointer::HitMapBuilder, tui_theme::Theme,
    };
    use ratatui::{Terminal, backend::TestBackend};
    let (ui, mailbox, _, _) = no_workspace();
    for available in [false, true] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
                for (width, height) in [(80, 24), (120, 40), (80, 18), (50, 10), (19, 4)] {
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let mut view = ClientView::new(width);
                    view.machines.available = available;
                    terminal
                        .draw(|frame| {
                            crate::tui_render::render_terminal_shell(
                                frame,
                                &view,
                                locale,
                                Some(TerminalSessionView::Native(ui.view())),
                                &theme,
                                &mut HitMapBuilder::new(),
                            );
                        })
                        .unwrap();
                    let text: String = terminal
                        .backend()
                        .buffer()
                        .content
                        .iter()
                        .map(|cell| cell.symbol())
                        .collect();
                    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                    let expected: String = locale
                        .no_current_workspace()
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect();
                    assert_eq!(
                        compact.contains(&expected),
                        width >= 20,
                        "{width}x{height}: {text}"
                    );
                    assert!(!text.contains(locale.shell().standalone), "{text}");
                }
            }
        }
    }
    mailbox.close(CarrierFailure::Closed);
}

#[test]
fn effective_create_binding_renders_and_invokes_workspace_or_empty_canonical_tab_by_exact_scope() {
    use crate::{
        client_view::{ClientView, KeyCode, KeyEvent, Modifiers, PrefixDesc, PrefixInput},
        localization::Locale,
        terminal_session::TerminalSessionView,
        tui_pointer::HitMapBuilder,
        tui_theme::Theme,
    };
    use orchbee_local_site_runtime::{
        WindowsKeyChord, WindowsKeyOverride, WindowsKeySettings, WindowsKeyTrigger,
    };
    use orchbee_workbench_protocol::{
        Directory, WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkerId,
    };
    use ratatui::{Terminal, backend::TestBackend};
    let chord = |key: &str, ctrl| WindowsKeyChord {
        key: key.into(),
        ctrl,
        alt: false,
        shift: false,
        super_key: false,
    };
    for canonical in [false, true] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let mut snapshot = if canonical {
                super::super::tests::membership::canonical_snapshot(0, 1)
            } else {
                super::super::tests::membership::snapshot(0, 1)
            };
            if canonical {
                let directory_id = WorkbenchDirectoryId::parse(&format!("{:032x}", 7100)).unwrap();
                snapshot.navigation.workspaces[0].primary_directory_id = Some(directory_id.clone());
                snapshot.navigation.directories.push(Directory {
                    directory_id,
                    worker_id: WorkerId::parse(&format!("{:032x}", 7101)).unwrap(),
                    path: WorkbenchDirectoryPath::parse("C:/workspace").unwrap(),
                });
            }
            let (mut ui, mailbox, clock, snapshot) = empty_ui(snapshot);
            assert_eq!(ui.view().no_current_workspace(), !canonical);
            let mut view = ClientView::new(120);
            view.configure_keys(&WindowsKeySettings {
                commands: Vec::new(),
                prefix: chord("a", true),
                overrides: vec![WindowsKeyOverride {
                    action: "new_tab",
                    triggers: vec![WindowsKeyTrigger {
                        prefix: true,
                        chord: chord("n", false),
                    }],
                }],
            });
            let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
            terminal
                .draw(|frame| {
                    crate::tui_render::render_terminal_shell(
                        frame,
                        &view,
                        locale,
                        Some(TerminalSessionView::Native(ui.view())),
                        &Theme::terminal_native(),
                        &mut HitMapBuilder::new(),
                    );
                })
                .unwrap();
            let text: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            let label = if canonical {
                (locale.shell().prefix_desc)(PrefixDesc::CreateTab)
            } else {
                locale.status().create_workspace
            };
            assert!(text.contains(&format!("Ctrl+a n {label}")), "{text}");
            if !canonical {
                assert!(!text.contains(locale.status().create_tab), "{text}");
            }
            assert!(!text.contains("Ctrl+b c"), "{text}");
            let prefix = view.decide(PrefixInput::Key(KeyEvent::press(
                KeyCode::Char('a'),
                Modifiers::CTRL,
            )));
            view.commit(prefix, SessionActionDisposition::KeepNavigator);
            let action = view
                .decide(PrefixInput::Key(KeyEvent::press(
                    KeyCode::Char('n'),
                    Modifiers::NONE,
                )))
                .action()
                .unwrap();
            assert_eq!(action, ClientAction::CreateTab);
            ui.apply_client_action(action);
            if canonical {
                let lease = mailbox.try_recv().unwrap().unwrap();
                let QueuedCommand::Create(marker) = lease.record() else {
                    panic!("empty canonical Tab Create");
                };
                assert_eq!(
                    marker.operation().origin(),
                    super::super::super::state::CreateOriginKind::Empty
                );
            } else {
                finish_preparation(&mut ui, &mailbox, &clock, &snapshot, false);
                let lease = mailbox.try_recv().unwrap().unwrap();
                assert!(
                    matches!(lease.record(), QueuedCommand::WorkspaceSetup(marker)
                    if matches!(marker.0.command.action, WorkbenchAction::WorkspaceLaunch { .. }))
                );
            }
            mailbox.close(CarrierFailure::Closed);
        }
    }
}

fn prepare(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    snapshot: &Snapshot,
    cancelled: bool,
) {
    assert!(ui.request_quick_workspace());
    finish_preparation(ui, mailbox, clock, snapshot, cancelled);
}

#[test]
fn quick_workspace_late_preparation_after_escape_preserves_the_original_terminal_without_mutation()
{
    let (mut ui, mailbox, clock, snapshot) = super::super::tests::workspace_setup::ready(false);
    let old = ui.state.as_ref().unwrap().desired().cloned();
    prepare(&mut ui, &mailbox, &clock, &snapshot, true);
    assert!(ui.view().setup_draft().is_none());
    assert_eq!(ui.state.as_ref().unwrap().desired(), old.as_ref());
    assert!(ui.state.as_ref().unwrap().cache().projects().is_empty());
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn workspace_create_and_quick_launch_submit_one_ungrouped_shell_without_a_project_form() {
    for with_project in [false, true] {
        for action in [
            ClientAction::RequestWorkspaceCreate,
            ClientAction::RequestQuickWorkspace,
        ] {
            let (mut ui, mailbox, clock, snapshot) =
                super::super::tests::workspace_setup::ready(with_project);
            ui.apply_client_action(action);
            finish_preparation(&mut ui, &mailbox, &clock, &snapshot, false);
            let lease = mailbox.try_recv().unwrap().unwrap();
            let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
                panic!("WorkspaceLaunch")
            };
            assert_eq!(marker.0.command.expected_revision, snapshot.revision);
            assert!(matches!(&marker.0.command.action,
                WorkbenchAction::WorkspaceLaunch { placement: orchbee_workbench_protocol::WorkspacePlacement::Ungrouped {}, label, options, launch: None, profile: None, .. }
                if label.as_str() == "项目 space" && options.cwd.as_ref().unwrap().as_str() == "C:/项目 space"));
            assert_eq!(
                ui.state.as_ref().unwrap().cache().canonical_snapshot(),
                &snapshot
            );
            assert!(ui.view().setup_draft().is_none());
            assert!(!ui.request_quick_workspace());
            assert!(mailbox.try_recv().unwrap().is_none());
            assert!(ui.state.as_ref().unwrap().invariant_holds());
            mailbox.close(CarrierFailure::WorkerExited);
        }
    }
}

#[test]
fn quick_workspace_missing_prepared_directory_refuses_without_creating_or_leaving_the_source() {
    let (mut ui, mailbox, clock, snapshot) = super::super::tests::workspace_setup::ready(false);
    let source = ui.state.as_ref().unwrap().desired().cloned();
    assert!(ui.request_quick_workspace());
    finish_preparation_with_directory(&mut ui, &mailbox, &clock, &snapshot, false, None);
    assert!(ui.view().setup_draft().is_none());
    assert_eq!(
        ui.view().setup_status(),
        Some(NativeSetupStatus::NotApplied)
    );
    assert_eq!(ui.state.as_ref().unwrap().desired(), source.as_ref());
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn quick_workspace_launch_does_not_require_a_modal_draw_at_small_geometry_in_either_locale() {
    use crate::{
        client_view::ClientView, localization::Locale, terminal_session::TerminalSessionView,
        tui_pointer::HitMapBuilder,
    };
    use ratatui::{Terminal, backend::TestBackend};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(120, 40), (22, 8)] {
            let (mut ui, mailbox, clock, snapshot) =
                super::super::tests::workspace_setup::ready(false);
            prepare(&mut ui, &mailbox, &clock, &snapshot, false);
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut receipt = NativeDrawReceipt::default();
            terminal
                .draw(|frame| {
                    receipt = crate::tui_render::render_terminal_shell(
                        frame,
                        &ClientView::new(width),
                        locale,
                        Some(TerminalSessionView::Native(ui.view())),
                        &crate::tui_theme::Theme::rgb_test_dark(),
                        &mut HitMapBuilder::new(),
                    );
                })
                .unwrap();
            assert!(!receipt.editing_cursor_shown());
            ui.after_close_draw(receipt);
            assert!(ui.view().setup_draft().is_none());
            let lease = mailbox.try_recv().unwrap().unwrap();
            assert!(
                matches!(lease.record(), QueuedCommand::WorkspaceSetup(marker)
                if matches!(marker.0.command.action, WorkbenchAction::WorkspaceLaunch { .. }))
            );
            assert!(mailbox.try_recv().unwrap().is_none());
            mailbox.close(CarrierFailure::WorkerExited);
        }
    }
}
