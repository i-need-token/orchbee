use super::super::tests::workspace_setup::{finish_preparation, finish_preparation_with_directory};
use super::super::*;
use crate::workbench_state::workspace_setup::{SetupEdit, SetupStage};
use orchbee_workbench_protocol::Snapshot;

fn no_workspace() -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Snapshot,
) {
    let mut snapshot = super::super::tests::membership::snapshot(0, 1);
    snapshot.navigation.workspaces.clear();
    snapshot.navigation.workspace_order.clear();
    snapshot.terminal_contexts.clear();
    let result = empty_ui(snapshot);
    assert!(result.0.view().no_current_workspace());
    assert_eq!(result.0.view().context_count(), 0);
    assert!(result.0.view().tab_create_target().is_none());
    result
}

pub(in crate::workbench_client::ui) fn empty_ui(
    snapshot: Snapshot,
) -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Snapshot,
) {
    let clock = crate::local_client::test_support::server::adjustable_operation_clock(Arc::new(
        std::sync::atomic::AtomicU64::new(0),
    ));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = WorkbenchClientUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: crate::workbench_projection::project_native_context_tabs(
                    &snapshot,
                    &crate::workbench_projection::preferred_workspace_location(&snapshot).unwrap(),
                )
                .unwrap(),
            },
            super::super::super::mailbox::EventDeadline::Startup(
                clock.now() + crate::workbench_state::OPERATION_TOTAL,
            ),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    (ui, mailbox, clock, snapshot)
}

#[test]
fn ordinary_workspace_entry_launches_the_prepared_directory_without_a_review() {
    use orchbee_workbench_protocol::WorkbenchAction;
    let (mut ui, mailbox, clock, snapshot) = no_workspace();
    ui.apply_client_action(ClientAction::RequestQuickWorkspace);
    finish_preparation(&mut ui, &mailbox, &clock, &snapshot, false);
    assert!(
        ui.view().setup_draft().is_none(),
        "quick creation never presents a review"
    );
    assert!(ui.view().workspace_creation_preview().unwrap().current);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
        panic!("single Workspace launch")
    };
    let expected = "C:/项目 space";
    assert!(
        matches!(&marker.0.command.action, WorkbenchAction::WorkspaceLaunch { options, .. }
        if options.cwd.as_ref().unwrap().as_str() == expected)
    );
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn directory_review_prepares_without_launching_and_submits_only_the_visible_edited_path() {
    use orchbee_workbench_protocol::{WorkbenchAction, WorkbenchDirectoryPath};
    let (mut ui, mailbox, clock, snapshot) = no_workspace();
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestWorkspaceAtDirectory),
        ClientActionDisposition::LayoutChanged
    );
    finish_preparation_with_directory(
        &mut ui,
        &mailbox,
        &clock,
        &snapshot,
        false,
        Some(WorkbenchDirectoryPath::parse("C:/旧目录 项目").unwrap()),
    );
    assert!(mailbox.try_recv().unwrap().is_none());
    let draft = ui.view().setup_draft().unwrap();
    assert_eq!(draft.stage(), SetupStage::QuickDirectory);
    assert_eq!(draft.text(), "C:/旧目录 项目");
    let token = draft.token();
    ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
    ui.edit_setup_draft(
        token,
        SetupEdit::Line(crate::terminal_text::line_editor::LineKey::KillToStart),
    );
    ui.edit_setup_draft(token, SetupEdit::Paste("C:/第二个 项目"));
    ui.submit_setup_draft(token);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
        panic!("single Workspace launch")
    };
    assert!(
        matches!(&marker.0.command.action, WorkbenchAction::WorkspaceLaunch { options, .. }
        if options.cwd.as_ref().unwrap().as_str() == "C:/第二个 项目")
    );
    assert!(ui.view().workspace_creation_preview().unwrap().current);
    assert!(ui.view().setup_draft().is_none());
}

/// The Worker names why it refused a directory, so the review reopens with the typed fields and
/// shows that cause beside the directory until the text changes, without blocking a retry.
#[test]
fn refused_directory_reopens_the_review_with_the_typed_text_and_the_server_cause() {
    use crate::localization::FieldError;
    use crate::workbench_state::workspace_setup::SetupResult;
    use orchbee_workbench_protocol::WorkbenchDirectoryPath;
    let (mut ui, mailbox, clock, snapshot) = no_workspace();
    ui.apply_client_action(ClientAction::RequestWorkspaceAtDirectory);
    finish_preparation_with_directory(
        &mut ui,
        &mailbox,
        &clock,
        &snapshot,
        false,
        Some(WorkbenchDirectoryPath::parse("C:/旧目录 项目").unwrap()),
    );
    let token = ui.view().setup_draft().unwrap().token();
    ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
    ui.edit_setup_draft(
        token,
        SetupEdit::Line(crate::terminal_text::line_editor::LineKey::KillToStart),
    );
    ui.edit_setup_draft(token, SetupEdit::Paste("C:/missing"));
    ui.submit_setup_draft(token);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
        panic!("single Workspace launch")
    };
    let operation = marker.0.clone();
    assert!(ui.view().setup_draft().is_none());
    assert_eq!(
        mailbox.mark_setup_in_flight(&operation).unwrap(),
        MutationResult::Applied
    );
    assert!(mailbox.retain_setup_not_applied(&operation));
    let wait = mailbox
        .publish_setup_result_before(
            operation,
            SetupResult::DirectoryRefused(FieldError::DirectoryMissing),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());

    let draft = ui.view().setup_draft().expect("the refused review reopens");
    assert_eq!(draft.stage(), SetupStage::QuickDirectory);
    assert_eq!(draft.quick_text(true), "C:/missing");
    assert_eq!(
        draft.shown_field_errors()[1],
        Some(FieldError::DirectoryMissing)
    );
    assert_eq!(
        draft.field_errors()[1],
        None,
        "the cause never blocks a retry"
    );
    let token = draft.token();
    ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
    ui.edit_setup_draft(token, SetupEdit::Append("2"));
    assert_eq!(
        ui.view().setup_draft().unwrap().shown_field_errors()[1],
        None
    );
}

#[test]
fn cancelling_directory_review_keeps_the_source_and_submits_nothing() {
    let (mut ui, mailbox, clock, snapshot) = no_workspace();
    let source = ui.state.as_ref().unwrap().cache().clone();
    ui.apply_client_action(ClientAction::RequestWorkspaceAtDirectory);
    finish_preparation(&mut ui, &mailbox, &clock, &snapshot, false);
    let token = ui.view().setup_draft().unwrap().token();
    ui.cancel_setup_draft(token);
    assert!(mailbox.try_recv().unwrap().is_none());
    assert!(ui.view().setup_draft().is_none());
    assert_eq!(ui.state.as_ref().unwrap().cache(), &source);
}

fn rendered_rows(buffer: &ratatui::buffer::Buffer) -> String {
    rendered_area(buffer, buffer.area)
}

fn rendered_area(buffer: &ratatui::buffer::Buffer, area: ratatui::layout::Rect) -> String {
    (area.y..area.bottom())
        .map(|y| {
            let mut row = String::new();
            let mut x = area.x;
            while x < area.right() {
                let symbol = buffer[(x, y)].symbol();
                row.push_str(symbol);
                x += u16::try_from(crate::terminal_text::display_width(symbol).max(1)).unwrap();
            }
            row
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn after_last_workspace_close() -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Snapshot,
    u64,
) {
    use super::super::tests::{membership, workspace_close};
    use crate::workbench_client::{WorkspaceCloseStatus, mailbox::EventDeadline};
    use orchbee_workbench_protocol::{WorkbenchRevision, WorkbenchSnapshotOutcome};

    let (mut ui, mailbox, clock, mut snapshot) = empty_ui(membership::canonical_snapshot(0, 1));
    let removed = snapshot.navigation.workspaces[0].clone();
    let lease = workspace_close::submit(&mut ui, &mailbox);
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("one last-Workspace Close");
    };
    let operation = marker.workspace_close().unwrap().clone();
    assert_eq!(
        mailbox
            .mark_refresh_in_flight(operation.generation)
            .unwrap(),
        MutationResult::Applied
    );
    assert!(
        mailbox
            .retain_workspace_close_result(&operation, workspace_close::receipt(&operation, true))
    );
    snapshot.revision = WorkbenchRevision::new(2);
    snapshot.navigation.workspaces.clear();
    snapshot.navigation.workspace_order.clear();
    snapshot
        .terminal_contexts
        .retain(|context| context.terminal_context_id != removed.terminal_context_id);
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::WorkspaceCloseResult {
                evidence: mailbox.workspace_close_worker_loss().unwrap(),
                outcome: Box::new(WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot.clone(),
                }),
            },
            EventDeadline::PlacementOnly(operation.deadlines.forward_cutoff()),
            &clock,
        )
        .unwrap();
    ui.tick();
    let Some(UiReply::Refresh(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("last-Workspace removal install");
    };
    assert_eq!(
        settlement.decision(),
        &RefreshDecision::SettledWithoutAttachment
    );
    mailbox.finish_lease(lease).unwrap();
    let status = ui.view();
    let status = status.workspace_close_status().unwrap();
    assert_eq!(status.status(), WorkspaceCloseStatus::Closed);
    assert_eq!(status.operation_id(), Some(&operation.request_id));
    assert_eq!(status.workspace_id(), Some(&removed.workspace_id));
    assert!(ui.view().no_current_workspace());
    assert!(ui.state.as_ref().unwrap().desired().is_none());
    assert!(mailbox.try_recv().unwrap().is_none());
    (ui, mailbox, clock, snapshot, operation.generation)
}

#[test]
fn empty_create_tab_shell_tick_installs_and_enters_its_exact_workspace_unless_newer_focus_wins() {
    use super::super::tests::workspace_setup::{launched, place};
    use crate::client_view::ClientView;
    use crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit;
    use crate::localization::Locale;
    use crate::tui_hit_map::HitMapBuilder;
    use crate::tui_theme::Theme;
    use crate::workbench_client::{ClientBackend, WorkbenchClient};
    use crate::workbench_projection::ClientContextLocation;
    use crate::workbench_state::workspace_setup::SetupWorkerEvidence;
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    for width in [80, 120] {
        for retained_close in [false, true] {
            for newer_focus in [false, true] {
                let (ui, mailbox, clock, snapshot, closed_generation) = if retained_close {
                    after_last_workspace_close()
                } else {
                    let (ui, mailbox, clock, snapshot) = no_workspace();
                    (ui, mailbox, clock, snapshot, 0)
                };
                let source = ui.state.as_ref().unwrap().cache().location().clone();
                assert_eq!(source, ClientContextLocation::NoWorkspace);
                let (shutdown, _peer) = orchbee_local_ipc::LocalStream::pair().unwrap();
                let mut workbench_client = WorkbenchClient {
                    position_writer: None,
                    position_startup: None,
                    configuration: None,
                    notifications: None,
                    window_title: None,
                    client_control: None,
                    backend: ClientBackend::Native(Box::new(ui)),
                    task: None,
                    shutdown: crate::local_client::test_support::server::shutdown_handle(shutdown),
                    retirement_redraw_pending: false,
                    origin: None,
                    recovery_shutdown: None,
                };
                let mut view = ClientView::new(width);
                let transition = view.decide_action(ClientAction::CreateTab);
                let disposition = workbench_client
                    .backend
                    .native_mut()
                    .unwrap()
                    .apply_client_action(transition.action().unwrap());
                view.commit(transition, disposition);
                finish_preparation(
                    workbench_client.backend.native_mut().unwrap(),
                    &mailbox,
                    &clock,
                    &snapshot,
                    false,
                );
                let lease = mailbox.try_recv().unwrap().unwrap();
                let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
                    panic!("one redirected WorkspaceLaunch, never TerminalOpen or Tab Create");
                };
                assert!(marker.0.generation > closed_generation);
                assert_eq!(marker.0.command.expected_revision, snapshot.revision);
                assert_eq!(
                    mailbox.mark_setup_in_flight(&marker.0).unwrap(),
                    MutationResult::Applied
                );
                let (committed, receipt) = launched(&snapshot, &lease);
                let WorkbenchSetupCommit::WorkspaceLaunched(expected) = &receipt else {
                    unreachable!();
                };
                let expected = expected.clone();
                let known = marker.0.known_commit(receipt.clone()).unwrap();
                if newer_focus {
                    let transition = view.decide_action(ClientAction::OpenWorkbenchNavigator);
                    let disposition = workbench_client
                        .backend
                        .native_mut()
                        .unwrap()
                        .apply_client_action(transition.action().unwrap());
                    view.commit(transition, disposition);
                    assert!(view.navigator_active());
                    // Even a refused action in the empty location is newer focus intent.
                    let transition = view.decide_action(ClientAction::SelectNextContext);
                    let disposition = workbench_client
                        .backend
                        .native_mut()
                        .unwrap()
                        .apply_client_action(transition.action().unwrap());
                    view.commit(transition, disposition);
                }
                workbench_client.client_control_tick(
                    &mut view,
                    Rect::new(0, 0, width, 24),
                    Locale::EnUs,
                );
                let wait = place(&mailbox, &clock, &lease, receipt, committed.clone());
                workbench_client.backend.native_mut().unwrap().tick();
                let Some(UiReply::Refresh(settlement)) =
                    mailbox.take_deposited_reply(&wait).unwrap()
                else {
                    panic!("accepted launch must settle through the real membership install");
                };
                let native = workbench_client.backend.native_mut().unwrap();
                let state = native.state.as_mut().unwrap();
                assert_eq!(state.cache().canonical_snapshot(), &committed);
                assert_eq!(committed.navigation.workspaces.len(), 1);
                assert_eq!(committed.terminal_contexts.len(), 1);
                assert_eq!(committed.terminal_contexts[0].tabs.len(), 1);
                assert_eq!(
                    state.cache().location(),
                    &ClientContextLocation::Workspace {
                        workspace_id: expected.workspace_id.clone(),
                        terminal_context_id: expected.terminal_context_id.clone(),
                    },
                    "the first launch replaces the now-invalid NoWorkspace location"
                );
                assert!(matches!(
                    settlement.decision(),
                    RefreshDecision::AttachRecovered(_)
                ));
                let key = state.desired().unwrap().clone();
                assert_eq!(key.anchor().tab_id(), &expected.tab_id);
                assert_eq!(key.panel(), &expected.panel_id);
                assert_eq!(
                    state
                        .selected_presentation(state.cache().tabs()[0].panel(0).unwrap())
                        .presentation_id(),
                    &expected.presentation_id
                );
                assert!(
                    state.attached_key().is_none(),
                    "no inherited Controller grant"
                );
                assert!(state.install_projection_t(&key));
                assert!(state.record_setup_configuration_t(&key, ControllerMode::ReadOnly, None));
                assert!(state.stage_drawn_frame_t(&key, FrameIdentity::new(1)));
                assert!(state.mark_feedback_accepted_t(&key, FrameIdentity::new(1)));
                assert!(state.publish_ready_t(
                    &key,
                    FrameIdentity::new(1),
                    ControllerMode::ReadOnly,
                    None
                ));
                assert!(state.workspace_close_status().is_none());
                assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known)));
                assert_eq!(view.project_quick_entry_pending(), !newer_focus);
                assert_eq!(view.navigator_active(), newer_focus);
                mailbox.finish_lease(lease).unwrap();
                workbench_client.client_control_tick(
                    &mut view,
                    Rect::new(0, 0, width, 24),
                    Locale::EnUs,
                );
                assert!(!view.project_quick_entry_pending());
                assert_eq!(view.navigator_active(), newer_focus);
                let native = workbench_client.backend.native_mut().unwrap();
                assert!(
                    !native.take_project_quick_terminal_entry(),
                    "one terminal entry only"
                );
                assert!(native.state.as_ref().unwrap().invariant_holds());
                assert_eq!(native.view().phase(), DisplayPhase::Ready);
                if !newer_focus {
                    let observed = native.control_observation(false).unwrap().1;
                    let selected = observed.selection.unwrap();
                    assert_eq!(selected.terminal_context_id, expected.terminal_context_id);
                    assert_eq!(selected.tab_id, Some(expected.tab_id.clone()));
                    assert_eq!(selected.panel_id, Some(expected.panel_id.clone()));
                    let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
                    terminal
                        .draw(|frame| {
                            crate::tui_render::render_terminal_shell(
                                frame,
                                &view,
                                Locale::EnUs,
                                Some(native.view()),
                                &Theme::terminal_native(),
                                &mut HitMapBuilder::new(),
                            );
                        })
                        .unwrap();
                    assert!(
                        !rendered_rows(terminal.backend().buffer())
                            .contains(Locale::EnUs.no_current_workspace())
                    );
                }
                assert!(
                    mailbox.try_recv().unwrap().is_none(),
                    "no replay or automatic extra launch"
                );
            }
        }
    }
}

#[test]
fn empty_compatibility_create_tab_uses_workspace_launch_and_cancel_never_resurrects_a_terminal() {
    for cancelled in [false, true] {
        let (mut ui, mailbox, clock, snapshot) = no_workspace();
        ui.apply_client_action(ClientAction::TabMenu {
            action: crate::action_menu::MenuAction::NewGroupWithTab,
            tab_id: None,
        });
        assert_eq!(ui.view().notice(), Some(WorkbenchClientNotice::NoWorkspace));
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
fn no_workspace_screen_keeps_content_blank_and_creation_controls_available_across_size_tiers() {
    use crate::{
        client_view::ClientView,
        localization::Locale,
        tui_hit_map::{HitMapBuilder, HitTarget},
        tui_theme::Theme,
    };
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    let (ui, mailbox, _, _) = no_workspace();
    for available in [false, true] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
                for (width, height) in [(80, 24), (120, 40), (80, 18), (50, 10), (19, 4)] {
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let mut view = ClientView::new(width);
                    view.machines.available = available;
                    let areas = crate::tui_layout::project_shell_areas(
                        Rect::new(0, 0, width, height),
                        view.sidebar_surface(),
                    );
                    let mut hits = HitMapBuilder::new();
                    terminal
                        .draw(|frame| {
                            crate::tui_render::render_terminal_shell(
                                frame,
                                &view,
                                locale,
                                Some(ui.view()),
                                &theme,
                                &mut hits,
                            );
                        })
                        .unwrap();
                    let text = rendered_rows(terminal.backend().buffer());
                    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                    let expected: String = locale
                        .no_current_workspace()
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect();
                    assert!(
                        !compact.contains(&expected),
                        "{width}x{height} machines={available}: {text}"
                    );
                    assert!(!text.contains("Standalone terminal"), "{text}");
                    assert!(!text.contains("独立终端"), "{text}");
                    let hits = hits.finish();
                    let presentable = areas.tier != crate::tui_layout::ViewportTier::TooSmall;
                    let has_target = |target: HitTarget| {
                        (0..height)
                            .flat_map(|y| (0..width).map(move |x| (x, y)))
                            .any(|(x, y)| hits.hit_target_at(x, y) == Some(&target))
                    };
                    assert!(!has_target(HitTarget::MenuButton));
                    if available {
                        for target in [
                            HitTarget::NewWorkspaceButton,
                            HitTarget::SearchButton,
                            HitTarget::SettingsButton,
                        ] {
                            assert_eq!(has_target(target), areas.sidebar.is_some());
                        }
                    }
                    if presentable {
                        for y in areas.panel.y..areas.status.y {
                            for x in areas.panel.x..areas.panel.right() {
                                if areas
                                    .sidebar
                                    .is_some_and(|sidebar| sidebar.area.contains((x, y).into()))
                                    || matches!(
                                        hits.hit_target_at(x, y),
                                        Some(HitTarget::MenuButton)
                                    )
                                {
                                    continue;
                                }
                                assert_eq!(
                                    terminal.backend().buffer()[(x, y)].symbol(),
                                    " ",
                                    "{width}x{height} at ({x},{y}): {text}"
                                );
                                assert!(
                                    hits.hit_target_at(x, y).is_none(),
                                    "empty content cannot expose a stale target"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    mailbox.close(CarrierFailure::Closed);
}

#[test]
fn effective_create_binding_invokes_workspace_or_canonical_tab_with_guidance_only_for_the_latter() {
    use crate::{
        client_view::{ClientView, KeyCode, KeyEvent, Modifiers, PrefixInput},
        localization::Locale,
        tui_hit_map::HitMapBuilder,
        tui_theme::Theme,
    };
    use orchbee_local_runtime::{KeyChord, KeyOverride, KeySettings, KeyTrigger};
    use orchbee_workbench_protocol::{
        Directory, WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkerId,
    };
    use ratatui::{Terminal, backend::TestBackend};
    let chord = |key: &str, ctrl| KeyChord {
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
            } else {
                snapshot.navigation.workspaces.clear();
                snapshot.navigation.workspace_order.clear();
                snapshot.terminal_contexts.clear();
            }
            let (mut ui, mailbox, clock, snapshot) = empty_ui(snapshot);
            assert_eq!(ui.view().no_current_workspace(), !canonical);
            let mut view = ClientView::new(120);
            view.configure_keys(&KeySettings {
                commands: Vec::new(),
                prefix: chord("a", true),
                overrides: vec![KeyOverride {
                    action: "new_tab",
                    triggers: vec![KeyTrigger {
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
                        Some(ui.view()),
                        &Theme::terminal_native(),
                        &mut HitMapBuilder::new(),
                    );
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            let text = rendered_rows(buffer);
            let areas = crate::tui_layout::project_shell_areas(buffer.area, view.sidebar_surface());
            // The status overlays the panel's final row; it is not authored empty-state content.
            let main = ratatui::layout::Rect::new(
                areas.panel.x,
                areas.panel.y,
                areas.panel.width,
                areas.status.y.saturating_sub(areas.panel.y),
            );
            let main_text = rendered_area(buffer, main);
            let status_text = rendered_area(buffer, areas.status);
            let label = if canonical {
                locale.shortcut_label(crate::localization::ShortcutLabel::NewTab)
            } else {
                locale.status().create_workspace.to_owned()
            };
            assert_eq!(
                main_text.contains(&format!("Ctrl+a n {label}")),
                canonical,
                "{text}"
            );
            if !canonical {
                assert!(!text.contains(locale.status().create_tab), "{text}");
                assert!(main_text.trim().is_empty(), "{main_text}");
                assert!(
                    status_text.contains(&format!("Ctrl+a n {label}")),
                    "the fixed status hint still advertises the effective creation binding"
                );
            }
            assert!(!text.contains("Ctrl+b c"), "{text}");
            let prefix = view.decide(PrefixInput::Key(KeyEvent::press(
                KeyCode::Char('a'),
                Modifiers::CTRL,
            )));
            view.commit(prefix, ClientActionDisposition::KeepNavigator);
            let transition = view.decide(PrefixInput::Key(KeyEvent::press(
                KeyCode::Char('n'),
                Modifiers::NONE,
            )));
            let action = transition.action().unwrap();
            assert_eq!(action, ClientAction::CreateTab);
            let disposition = ui.apply_client_action(action);
            view.commit(transition, disposition);
            assert_eq!(view.project_quick_entry_pending(), !canonical);
            if canonical {
                let lease = mailbox.try_recv().unwrap().unwrap();
                let QueuedCommand::Create(marker) = lease.record() else {
                    panic!("empty canonical Tab Create");
                };
                assert_eq!(
                    marker.operation().origin(),
                    crate::workbench_state::CreateOriginKind::Empty
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
    ui: &mut WorkbenchClientUi,
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
    assert_eq!(ui.view().setup_status(), Some(SetupStatus::NotApplied));
    assert_eq!(ui.state.as_ref().unwrap().desired(), source.as_ref());
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn workspace_at_directory_launches_only_from_a_drawn_review_in_either_locale() {
    use crate::{
        client_view::ClientView, localization::Locale, tui_hit_map::HitMapBuilder, tui_theme::Role,
    };
    use orchbee_workbench_protocol::{
        Directory, WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkerId,
    };
    use ratatui::{Terminal, backend::TestBackend};
    // A second known Workspace directory lists below the prepared one.
    let mut known = super::super::tests::membership::snapshot(1, 1);
    let directory_id = WorkbenchDirectoryId::parse(&format!("{:032x}", 7200)).unwrap();
    known.navigation.workspaces[0].primary_directory_id = Some(directory_id.clone());
    known.navigation.directories.push(Directory {
        directory_id,
        worker_id: WorkerId::parse(&format!("{:032x}", 7201)).unwrap(),
        path: WorkbenchDirectoryPath::parse("C:/Desktop").unwrap(),
    });
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        // A host too small for the review creates nothing; Enter cannot accept unseen defaults.
        for (width, height, drawn) in [(120, 40, true), (80, 24, true), (22, 8, false)] {
            let (mut ui, mailbox, clock, snapshot) =
                super::super::tests::workspace_setup::ready_snapshot(known.clone());
            ui.apply_client_action(ClientAction::RequestWorkspaceAtDirectory);
            finish_preparation(&mut ui, &mailbox, &clock, &snapshot, false);
            assert!(
                mailbox.try_recv().unwrap().is_none(),
                "the review creates nothing before it is accepted"
            );
            let token = ui.view().setup_draft().unwrap().token();
            let choices: Vec<_> = ui
                .view()
                .setup_draft()
                .unwrap()
                .directory_choices()
                .iter()
                .map(|path| path.as_str().to_owned())
                .collect();
            let mut hits = HitMapBuilder::new();
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut receipt = DrawReceipt::default();
            terminal
                .draw(|frame| {
                    receipt = crate::tui_render::render_terminal_shell(
                        frame,
                        &ClientView::new(width),
                        locale,
                        Some(ui.view()),
                        &theme,
                        &mut hits,
                    );
                })
                .unwrap();
            assert_eq!(
                receipt.editing_cursor_shown(),
                drawn,
                "{locale:?} {width}x{height}: the directory field is editable only when drawn"
            );
            let map = hits.finish();
            if drawn {
                use crate::tui_hit_map::HitTarget;
                use crate::tui_hit_map::ModalControl;
                let submit_y = (0..height)
                    .find(|y| {
                        (0..width).any(|x| {
                            matches!(
                                map.hit_target_at(x, *y),
                                Some(HitTarget::Setup {
                                    control: ModalControl::Submit,
                                    ..
                                })
                            )
                        })
                    })
                    .unwrap();
                assert_eq!(choices, ["C:/项目 space", "C:/Desktop"]);
                let buffer = terminal.backend().buffer();
                for (index, path) in choices.iter().enumerate() {
                    let (x, y) = (0..height)
                        .flat_map(|y| (0..width).map(move |x| (x, y)))
                        .find(|(x, y)| matches!(
                            map.hit_target_at(*x, *y), Some(HitTarget::Setup { control: ModalControl::Choice(choice), .. }) if *choice == index
                        ))
                        .expect("every displayed candidate retains its own hit row");
                    assert!(
                        y + 1 < submit_y,
                        "candidate and action rows must be disjoint"
                    );
                    let mut line = String::new();
                    let mut column = 0;
                    while column < width {
                        let symbol = buffer[(column, y)].symbol();
                        line.push_str(symbol);
                        column += u16::try_from(crate::terminal_text::display_width(symbol).max(1))
                            .unwrap();
                    }
                    // The row names its folder and leaves the whole path to the field.
                    let folder = path.rsplit('/').next().unwrap();
                    assert!(
                        line.contains(folder) && !line.contains(path.as_str()),
                        "{line}"
                    );
                    // Only the folder the field holds carries the cursor and selection fill.
                    let selected = index == 0;
                    assert_eq!(
                        buffer[(x, y)].symbol() == theme.glyphs.navigation_marker,
                        selected,
                        "{line}"
                    );
                    assert_eq!(
                        buffer[(x, y)].bg == theme.style(Role::SurfaceSelection).bg.unwrap(),
                        selected,
                        "{line}"
                    );
                }
            }
            ui.after_close_draw(receipt);
            assert!(mailbox.try_recv().unwrap().is_none());
            ui.submit_setup_draft(token);
            if !drawn {
                assert!(mailbox.try_recv().unwrap().is_none());
                mailbox.close(CarrierFailure::WorkerExited);
                continue;
            }
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

// The empty-location Create Tab fallback must refuse before it queues directory preparation.
#[test]
fn unsupported_directory_entry_never_queues_preparation_or_creates_a_draft() {
    use crate::tui_capabilities::{Capabilities, Capability};
    for action in [
        ClientAction::RequestQuickWorkspace,
        ClientAction::CreateTab,
        ClientAction::RequestWorkspaceDirectory,
    ] {
        let (mut ui, mailbox, _, _) = no_workspace();
        ui.capabilities = Capabilities::none();
        assert_eq!(
            ui.apply_client_action(action),
            ClientActionDisposition::KeepNavigator
        );
        assert_eq!(
            ui.view().notice(),
            Some(WorkbenchClientNotice::Unsupported(
                Capability::DirectoryInput
            ))
        );
        assert!(ui.view().setup_draft().is_none());
        assert!(ui.view().workspace_creation_preview().is_none());
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}
