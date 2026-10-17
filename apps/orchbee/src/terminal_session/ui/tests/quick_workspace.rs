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

pub(in crate::terminal_session::ui) fn empty_ui(
    snapshot: Snapshot,
) -> (NativeUi, Arc<CommandMailbox>, LocalOperationClock, Snapshot) {
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
fn directory_review_prepares_without_launching_and_submits_only_the_visible_edited_path() {
    use orchbee_workbench_protocol::{WorkbenchAction, WorkbenchDirectoryPath};
    let (mut ui, mailbox, clock, snapshot) = no_workspace();
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestWorkspaceAtDirectory(None)),
        SessionActionDisposition::LayoutChanged
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
    assert_eq!(draft.stage(), NativeSetupStage::QuickDirectory);
    assert_eq!(draft.text(), "C:/旧目录 项目");
    let token = draft.token();
    ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
    ui.edit_setup_draft(token, NativeSetupEdit::Clear);
    ui.edit_setup_draft(token, NativeSetupEdit::Paste("C:/第二个 项目"));
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

#[test]
fn cancelling_directory_review_keeps_the_source_and_submits_nothing() {
    let (mut ui, mailbox, clock, snapshot) = no_workspace();
    let source = ui.state.as_ref().unwrap().cache().clone();
    ui.apply_client_action(ClientAction::RequestWorkspaceAtDirectory(None));
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

#[cfg(windows)]
fn after_last_workspace_close() -> (
    NativeUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Snapshot,
    u64,
) {
    use super::super::tests::{membership, workspace_close};
    use crate::terminal_session::{WorkspaceCloseStatus, mailbox::EventDeadline};
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

#[cfg(windows)]
#[test]
fn empty_create_tab_shell_tick_installs_and_enters_its_exact_workspace_unless_newer_focus_wins() {
    use super::super::tests::workspace_setup::{launched, place};
    use crate::{
        client_view::ClientView,
        local_client::workbench::workspace_setup::WorkbenchSetupCommit,
        localization::Locale,
        terminal_session::{
            SessionBackend, TerminalSession, TerminalSessionView,
            state::workspace_setup::SetupWorkerEvidence,
            workbench_projection::NativeContextLocation,
        },
        tui_pointer::HitMapBuilder,
        tui_theme::Theme,
    };
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
                let (shutdown, _peer) = orchbee_local_ipc::LocalStream::pair().unwrap();
                let mut session = TerminalSession {
                    position_writer: None,
                    position_startup: None,
                    configuration: None,
                    notifications: None,
                    window_title: None,
                    client_control: None,
                    backend: SessionBackend::Native(Box::new(ui)),
                    task: None,
                    shutdown: crate::local_client::tests::server::shutdown_handle(shutdown),
                    retirement_redraw_pending: false,
                    origin: None,
                    recovery_shutdown: None,
                };
                let mut view = ClientView::new(width);
                let transition = view.decide_action(ClientAction::CreateTab);
                let disposition = session
                    .backend
                    .native_mut()
                    .unwrap()
                    .apply_client_action(transition.action().unwrap());
                view.commit(transition, disposition);
                finish_preparation(
                    session.backend.native_mut().unwrap(),
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
                    // Even a refused action in the empty location is newer focus intent.
                    let transition = view.decide_action(ClientAction::SelectNextContext);
                    let disposition = session
                        .backend
                        .native_mut()
                        .unwrap()
                        .apply_client_action(transition.action().unwrap());
                    view.commit(transition, disposition);
                }
                session.client_control_tick(&mut view, Rect::new(0, 0, width, 24), Locale::EnUs);
                let wait = place(&mailbox, &clock, &lease, receipt, committed.clone());
                session.backend.native_mut().unwrap().tick();
                let Some(UiReply::Refresh(settlement)) =
                    mailbox.take_deposited_reply(&wait).unwrap()
                else {
                    panic!("accepted launch must settle through the real membership install");
                };
                let native = session.backend.native_mut().unwrap();
                let state = native.state.as_mut().unwrap();
                assert_eq!(state.cache().canonical_snapshot(), &committed);
                assert_eq!(committed.navigation.workspaces.len(), 1);
                assert!(
                    committed
                        .terminal_contexts
                        .iter()
                        .find(|context| context.terminal_context_id
                            == committed.navigation.standalone_terminal_context_id)
                        .unwrap()
                        .tabs
                        .is_empty()
                );
                if newer_focus {
                    assert_eq!(settlement.decision(), &RefreshDecision::Continue);
                    assert_eq!(state.cache().location(), &source);
                    assert!(state.desired().is_none());
                    assert!(!view.project_quick_entry_pending());
                } else {
                    assert_eq!(
                        state.cache().location(),
                        &NativeContextLocation::Workspace {
                            workspace_id: expected.workspace_id.clone(),
                            terminal_context_id: expected.terminal_context_id.clone(),
                        },
                        "retained Close={retained_close}: membership existence alone is not entry"
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
                    assert!(state.record_setup_configuration_t(
                        &key,
                        ControllerMode::ReadOnly,
                        None
                    ));
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
                    assert!(view.project_quick_entry_pending());
                }
                mailbox.finish_lease(lease).unwrap();
                session.client_control_tick(&mut view, Rect::new(0, 0, width, 24), Locale::EnUs);
                assert!(!view.project_quick_entry_pending());
                let native = session.backend.native_mut().unwrap();
                assert!(
                    !native.take_project_quick_terminal_entry(),
                    "one terminal entry only"
                );
                assert!(native.state.as_ref().unwrap().invariant_holds());
                if !newer_focus {
                    assert_eq!(native.view().phase(), NativeDisplayPhase::Ready);
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
                                Some(TerminalSessionView::Native(native.view())),
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
fn no_workspace_screen_keeps_content_blank_and_creation_controls_available_across_size_tiers() {
    use crate::{
        client_view::ClientView,
        localization::Locale,
        terminal_session::TerminalSessionView,
        tui_pointer::{HitMapBuilder, HitTarget},
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
                                Some(TerminalSessionView::Native(ui.view())),
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
                    assert!(!text.contains(locale.shell().standalone), "{text}");
                    let hits = hits.finish();
                    let presentable = areas.tier != crate::tui_layout::ViewportTier::TooSmall;
                    let has_target = |target: HitTarget| {
                        (0..height)
                            .flat_map(|y| (0..width).map(move |x| (x, y)))
                            .any(|(x, y)| hits.hit_target_at(x, y) == Some(&target))
                    };
                    assert_eq!(has_target(HitTarget::MenuButton), presentable);
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
                (locale.shell().prefix_desc)(PrefixDesc::CreateTab)
            } else {
                locale.status().create_workspace
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
            view.commit(prefix, SessionActionDisposition::KeepNavigator);
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
