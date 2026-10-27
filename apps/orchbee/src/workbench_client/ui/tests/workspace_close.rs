use super::super::*;
use crate::{
    client_view::{ClientKey, ClientView},
    localization::Locale,
    terminal_input::{InputDispatcher, TerminalInputPort, TerminalInputUnavailable},
    tui_pointer::{FrameHitMap, HitMapBuilder, HitTarget},
    tui_theme::Theme,
    workbench_client::{
        state::{
            ControllerMode, FrameIdentity,
            workspace_close::{
                WorkspaceCloseOperation, WorkspaceCloseResult, WorkspaceCloseTarget,
            },
        },
        workbench_projection::{ClientContextLocation, project_native_context_tabs},
    },
};
use orchbee_workbench_protocol::*;
use ratatui::{Terminal, backend::TestBackend};
use std::{io, time::Instant};

pub(in crate::workbench_client) fn ready() -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Snapshot,
) {
    ready_with_background_target(false)
}

/// Settles the guarded IfIdle route as a resumable host refusal so tests can
/// exercise the explicit destructive confirmation that follows that outcome.
pub(in crate::workbench_client) fn ready_after_guard_refusal() -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Snapshot,
) {
    let (mut ui, mailbox, clock, snapshot) = ready();
    let workspace = snapshot.navigation.workspaces[1].workspace_id.clone();
    assert_eq!(
        ui.apply_client_action(crate::client_view::ClientAction::RequestWorkspaceClose),
        crate::client_view::ClientActionDisposition::DismissNavigator
    );
    assert!(ui.view().workspace_close_confirmation().is_none());
    let lease = mailbox
        .try_recv()
        .unwrap()
        .expect("the idle check is submitted before explicit confirmation");
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("conditional Workspace close refresh");
    };
    let operation = marker
        .workspace_close()
        .expect("conditional Workspace close");
    assert_eq!(
        operation.workspace_command().unwrap().action,
        WorkbenchAction::WorkspaceCloseIfIdle {
            workspace_id: workspace.clone()
        }
    );
    assert_eq!(
        mailbox
            .mark_refresh_in_flight(operation.generation)
            .unwrap(),
        MutationResult::Applied
    );
    let mut result = receipt(operation, false);
    let WorkspaceCloseResult::WorkspaceObserved { outcome, .. } = &mut result else {
        panic!("guarded Workspace close observation");
    };
    let WorkbenchWorkspaceCloseReadOutcome::Available { status, .. } = outcome.as_mut() else {
        panic!("available guarded Workspace close observation");
    };
    *status = WorkbenchTabCloseStatus::CleanupBlocked {
        reason: WorkbenchTabCloseBlockedReason::HostRefused,
        resumable: true,
    };
    assert!(mailbox.retain_workspace_close_result(operation, result));
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::WorkspaceCloseResult {
                evidence: mailbox.workspace_close_worker_loss().unwrap(),
                outcome: Box::new(WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot.clone(),
                }),
            },
            super::super::super::mailbox::EventDeadline::PlacementOnly(
                operation.deadlines.forward_cutoff(),
            ),
            &clock,
        )
        .unwrap();
    assert!(ui.tick());
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).unwrap();
    assert!(mailbox.try_recv().unwrap().is_none());
    assert!(
        ui.state
            .as_ref()
            .unwrap()
            .can_confirm_guarded_workspace_close(&workspace)
    );
    (ui, mailbox, clock, snapshot)
}

fn ready_with_background_target(
    background: bool,
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
    let mut snapshot = super::super::tests::membership::snapshot(usize::from(background), 1);
    let project_id = WorkbenchProjectId::parse(&format!("{:032x}", 270)).unwrap();
    let workspace_id = WorkbenchWorkspaceId::parse(&format!("{:032x}", 271)).unwrap();
    let terminal_context_id = WorkbenchTerminalContextId::parse(&format!("{:032x}", 272)).unwrap();
    snapshot.navigation.projects.push(Project {
        project_id: project_id.clone(),
        label: "Project 项目".into(),
    });
    snapshot.navigation.workspaces.push(Workspace {
        workspace_id: workspace_id.clone(),
        project_id: Some(project_id),
        terminal_context_id: terminal_context_id.clone(),
        label: "项目 space 界".into(),
        primary_directory_id: None,
    });
    snapshot.navigation.workspace_order = snapshot
        .navigation
        .workspaces
        .iter()
        .map(|workspace| workspace.workspace_id.clone())
        .collect();
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: terminal_context_id.clone(),
        tabs: Vec::new(),
        groups: Vec::new(),
    });
    let location = if background {
        {
            let fixture_context_id = snapshot.navigation.workspaces[0]
                .terminal_context_id
                .clone();
            ClientContextLocation::Workspace {
                workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                    fixture_context_id.as_str(),
                )
                .unwrap(),
                terminal_context_id: fixture_context_id,
            }
        }
    } else {
        ClientContextLocation::Workspace {
            workspace_id,
            terminal_context_id,
        }
    };
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: project_native_context_tabs(&snapshot, &location).unwrap(),
            },
            super::super::super::mailbox::EventDeadline::Startup(
                clock.now() + super::super::super::state::OPERATION_TOTAL,
            ),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    if background {
        let state = ui.state.as_mut().unwrap();
        let desired = state.desired().unwrap().clone();
        assert!(state.install_projection_t(&desired));
        assert!(state.record_setup_configuration_t(&desired, ControllerMode::ReadOnly, None));
        assert!(state.stage_drawn_frame_t(&desired, FrameIdentity::new(1)));
        assert!(state.mark_feedback_accepted_t(&desired, FrameIdentity::new(1)));
        assert!(state.publish_ready_t(
            &desired,
            FrameIdentity::new(1),
            ControllerMode::ReadOnly,
            None
        ));
        assert_eq!(ui.view().phase(), DisplayPhase::Ready);
    } else {
        assert_eq!(ui.view().phase(), DisplayPhase::Empty);
    }
    (ui, mailbox, clock, snapshot)
}
pub(in crate::workbench_client) fn open_confirmation(
    ui: &mut WorkbenchClientUi,
    target: Option<&WorkbenchWorkspaceId>,
) -> ClientActionDisposition {
    let (Some(mailbox), Some(state)) = (ui.mailbox.as_ref(), ui.state.as_mut()) else {
        return ClientActionDisposition::KeepNavigator;
    };
    if mailbox
        .apply_local_for(state, |state| {
            match target {
                Some(target) => state.open_workspace_close_at(target),
                None => state.open_workspace_close(),
            }
            .ok_or(())
        })
        .is_ok()
    {
        ClientActionDisposition::DismissNavigator
    } else {
        ClientActionDisposition::KeepNavigator
    }
}
pub(in crate::workbench_client) fn submit(
    ui: &mut WorkbenchClientUi,
    mailbox: &CommandMailbox,
) -> super::super::super::mailbox::CommandLease {
    assert_eq!(
        open_confirmation(ui, None),
        ClientActionDisposition::DismissNavigator
    );
    let token = ui.view().workspace_close_confirmation().unwrap().token();
    ui.after_workspace_close_draw(Some(ModalPresentation::Presented(token)));
    assert!(ui.capture_confirmation_key(ClientKey::Enter));
    mailbox.try_recv().unwrap().unwrap()
}
pub(in crate::workbench_client::ui) fn receipt(
    operation: &WorkspaceCloseOperation,
    closed: bool,
) -> WorkspaceCloseResult {
    let WorkspaceCloseTarget::Workspace {
        workspace_id,
        terminal_context_id,
    } = &operation.target
    else {
        unreachable!()
    };
    let accepted = crate::local_client::workbench::WorkbenchWorkspaceCloseAccepted {
        close_request_id: operation.request_id.clone(),
        workspace_id: workspace_id.clone(),
        terminal_context_id: terminal_context_id.clone(),
        accepted_revision: operation.baseline.revision(),
    };
    WorkspaceCloseResult::WorkspaceObserved {
        outcome: Box::new(WorkbenchWorkspaceCloseReadOutcome::Available {
            close_request_id: accepted.close_request_id.clone(),
            workspace_id: accepted.workspace_id.clone(),
            terminal_context_id: accepted.terminal_context_id.clone(),
            attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
            status: if closed {
                WorkbenchTabCloseStatus::Closed {
                    revision: WorkbenchRevision::new(2),
                }
            } else {
                WorkbenchTabCloseStatus::Closing {
                    phase: WorkbenchTabClosePhase::Queued,
                }
            },
            counts: WorkbenchWorkspaceCloseCounts::default(),
            members: Vec::new(),
        }),
        accepted,
    }
}

#[test]
fn workspace_close_refuses_a_blocked_first_draw_and_resumes_an_admitted_prompt() {
    let (mut refused, refused_mailbox, _, _) = ready();
    assert_eq!(
        open_confirmation(&mut refused, None),
        ClientActionDisposition::DismissNavigator
    );
    refused.after_workspace_close_draw(Some(ModalPresentation::NotPresented));
    assert!(refused.view().workspace_close_confirmation().is_none());
    assert_eq!(refused.view().notice(), None);
    assert!(!refused.capture_confirmation_key(ClientKey::Enter));
    assert!(refused_mailbox.try_recv().unwrap().is_none());

    let (mut suspended, mailbox, _, _) = ready();
    assert_eq!(
        open_confirmation(&mut suspended, None),
        ClientActionDisposition::DismissNavigator
    );
    let token = suspended
        .view()
        .workspace_close_confirmation()
        .unwrap()
        .token();
    suspended.after_workspace_close_draw(Some(ModalPresentation::Presented(token)));
    suspended.after_workspace_close_draw(Some(ModalPresentation::NotPresented));
    let prompt = suspended.view().workspace_close_confirmation().unwrap();
    assert_eq!(prompt.token(), token);
    assert!(!prompt.interactive());
    assert!(suspended.capture_confirmation_key(ClientKey::Enter));
    suspended.click_workspace_close(token, true);
    assert!(mailbox.try_recv().unwrap().is_none());

    suspended.after_workspace_close_draw(Some(ModalPresentation::Presented(token)));
    assert!(
        suspended
            .view()
            .workspace_close_confirmation()
            .unwrap()
            .interactive()
    );
    assert!(suspended.capture_confirmation_key(ClientKey::Enter));
    assert!(mailbox.try_recv().unwrap().is_some());
}

#[test]
fn suspended_workspace_close_keeps_only_local_escape_cancellation_active() {
    let (mut ui, mailbox, _, _) = ready();
    assert_eq!(
        open_confirmation(&mut ui, None),
        ClientActionDisposition::DismissNavigator
    );
    let token = ui.view().workspace_close_confirmation().unwrap().token();
    ui.after_workspace_close_draw(Some(ModalPresentation::Presented(token)));
    ui.after_workspace_close_draw(Some(ModalPresentation::NotPresented));
    assert!(ui.capture_confirmation_key(ClientKey::Escape));
    assert!(ui.view().workspace_close_confirmation().is_none());
    assert!(mailbox.try_recv().unwrap().is_none());
}
fn draw(
    ui: &mut WorkbenchClientUi,
    locale: Locale,
    theme: &Theme,
    width: u16,
    height: u16,
) -> String {
    draw_with_hits(ui, locale, theme, width, height).0
}
fn draw_with_hits(
    ui: &mut WorkbenchClientUi,
    locale: Locale,
    theme: &Theme,
    width: u16,
    height: u16,
) -> (String, FrameHitMap) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let view = ClientView::new(width);
    let mut receipt = None;
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            receipt = Some(crate::tui_render::render_terminal_shell(
                frame,
                &view,
                locale,
                Some(ui.view()),
                theme,
                &mut hits,
            ));
        })
        .unwrap();
    ui.after_close_draw(receipt.unwrap());
    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    (text, hits.finish())
}
fn alnum(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).collect()
}

#[test]
fn keyboard_and_addressed_workspace_close_share_conditional_admission_without_a_local_prompt() {
    for addressed in [false, true] {
        let (mut ui, mailbox, _, snapshot) = ready();
        let workspace = snapshot.navigation.workspaces[1].workspace_id.clone();
        let action = if addressed {
            ClientAction::RequestWorkspaceCloseAt(workspace.clone())
        } else {
            ClientAction::RequestWorkspaceClose
        };
        assert_eq!(
            ui.apply_client_action(action),
            ClientActionDisposition::DismissNavigator
        );
        assert!(ui.view().workspace_close_confirmation().is_none());
        let evidence = mailbox.workspace_close_worker_loss().unwrap();
        assert_eq!(
            evidence.operation.workspace_command().unwrap().action,
            WorkbenchAction::WorkspaceCloseIfIdle {
                workspace_id: workspace
            }
        );
        assert!(mailbox.try_recv().unwrap().is_some());
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}
#[test]
fn background_workspace_close_ui_keeps_the_live_origin_through_cancel_and_canonical_removal() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for cancel in [false, true] {
            let (mut ui, mailbox, clock, mut snapshot) = ready_with_background_target(true);
            let target = snapshot.navigation.workspaces[1].clone();
            let state = ui.state.as_ref().unwrap();
            let origin = state.cache().location().clone();
            let desired = state.desired().unwrap().clone();
            let pane = state.pane_ref_for(&desired).unwrap();
            let binding = state.pane_record(&desired).unwrap().binding().clone();
            let cursors = mailbox.cursors();
            assert_eq!(
                ui.request_workspace_close_at(
                    &WorkbenchWorkspaceId::parse(&format!("{:032x}", 999)).unwrap()
                ),
                ClientActionDisposition::KeepNavigator
            );
            assert!(ui.view().workspace_close_confirmation().is_none());
            assert_eq!(mailbox.cursors(), cursors);
            assert_eq!(
                open_confirmation(&mut ui, Some(&target.workspace_id)),
                ClientActionDisposition::DismissNavigator
            );
            assert_eq!(ui.state.as_ref().unwrap().cache().location(), &origin);
            assert_eq!(
                ui.view().workspace_close_confirmation().unwrap().label(),
                target.label
            );
            ui.capture_confirmation_key(ClientKey::Enter);
            assert_eq!(
                mailbox.cursors(),
                cursors,
                "an addressed target still requires a full draw"
            );
            let text = draw(&mut ui, locale, &Theme::terminal_native(), 120, 40);
            assert!(alnum(&text).contains(&alnum(&target.label)));
            ui.capture_confirmation_key(if cancel {
                ClientKey::Escape
            } else {
                ClientKey::Enter
            });
            if cancel {
                assert_eq!(mailbox.cursors(), cursors);
                assert!(mailbox.try_recv().unwrap().is_none());
            } else {
                let lease = mailbox.try_recv().unwrap().unwrap();
                let QueuedCommand::Refresh(marker) = lease.record() else {
                    panic!("one Workspace close flight");
                };
                let operation = marker.workspace_close().unwrap();
                assert_eq!(operation.baseline.location(), &origin);
                assert_eq!(
                    operation.target,
                    WorkspaceCloseTarget::Workspace {
                        workspace_id: target.workspace_id.clone(),
                        terminal_context_id: target.terminal_context_id.clone(),
                    }
                );
                assert_eq!(
                    mailbox
                        .mark_refresh_in_flight(operation.generation)
                        .unwrap(),
                    MutationResult::Applied
                );
                assert!(mailbox.retain_workspace_close_result(operation, receipt(operation, true)));
                snapshot.revision = WorkbenchRevision::new(2);
                snapshot
                    .navigation
                    .workspaces
                    .retain(|workspace| workspace.workspace_id != target.workspace_id);
                snapshot
                    .navigation
                    .workspace_order
                    .retain(|workspace| workspace != &target.workspace_id);
                snapshot
                    .terminal_contexts
                    .retain(|context| context.terminal_context_id != target.terminal_context_id);
                let wait = mailbox
                    .publish_event_before(
                        WorkerEvent::WorkspaceCloseResult {
                            evidence: mailbox.workspace_close_worker_loss().unwrap(),
                            outcome: Box::new(WorkbenchSnapshotOutcome::Available { snapshot }),
                        },
                        super::super::super::mailbox::EventDeadline::PlacementOnly(
                            operation.deadlines.forward_cutoff(),
                        ),
                        &clock,
                    )
                    .unwrap();
                assert!(ui.tick());
                let Some(UiReply::Refresh(settlement)) =
                    mailbox.take_deposited_reply(&wait).unwrap()
                else {
                    panic!("background publication settlement");
                };
                assert_eq!(settlement.decision(), &RefreshDecision::Continue);
                assert_eq!(settlement.desired(), Some(&desired));
                mailbox.finish_lease(lease).unwrap();
                assert!(
                    mailbox.try_recv().unwrap().is_none(),
                    "no origin detach or attach"
                );
                assert!(
                    ui.state
                        .as_ref()
                        .unwrap()
                        .cache()
                        .canonical_snapshot()
                        .navigation
                        .workspace(&target.workspace_id)
                        .is_none()
                );
            }
            let state = ui.state.as_ref().unwrap();
            assert!(ui.view().workspace_close_confirmation().is_none());
            assert_eq!(state.cache().location(), &origin);
            assert_eq!(state.desired(), Some(&desired));
            assert_eq!(state.pane_ref_for(&desired), Some(pane.clone()));
            assert_eq!(state.pane_record(&desired).unwrap().binding(), &binding);
            assert!(state.pane_attached(&pane));
            assert!(state.pane_projected(&pane));
            assert_eq!(ui.view().phase(), DisplayPhase::Ready);
            assert!(state.invariant_holds());
        }
    }
}
#[test]
fn workspace_close_full_shell_draw_matrix_discloses_scope_and_gates_submission() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::rgb_test_dark(),
            Theme::one_light(),
            Theme::one_dark(),
        ] {
            for (width, height, complete) in [
                (80, 24, true),
                (120, 40, true),
                (60, 24, true),
                (50, 30, true),
                (22, 8, false),
                (19, 4, false),
            ] {
                let (mut ui, mailbox, _, _) = ready();
                let before = mailbox.cursors();
                open_confirmation(&mut ui, None);
                let token = ui.view().workspace_close_confirmation().unwrap().token();
                assert!(ui.modal_owns_input());
                {
                    assert!(ui.control_dialog());
                    assert!(ui.view().position_draw(std::iter::empty()).is_none());
                }
                let (text, hits) = draw_with_hits(&mut ui, locale, &theme, width, height);
                let cancel_controls = (0..height)
                    .flat_map(|y| (0..width).map(move |x| (x, y)))
                    .filter(|(x, y)| {
                        matches!(
                            hits.hit_target_at(*x, *y),
                            Some(HitTarget::WorkspaceClose {
                                token: painted,
                                confirm: false,
                            }) if *painted == token
                        )
                    })
                    .count();
                let confirm_controls = (0..height)
                    .flat_map(|y| (0..width).map(move |x| (x, y)))
                    .filter(|(x, y)| {
                        matches!(
                            hits.hit_target_at(*x, *y),
                            Some(HitTarget::WorkspaceClose {
                                token: painted,
                                confirm: true,
                            }) if *painted == token
                        )
                    })
                    .count();
                let controls = cancel_controls + confirm_controls;
                if complete {
                    assert!(
                        ui.view().workspace_close_confirmation().is_some(),
                        "{locale:?} {width}x{height}"
                    );
                    assert!(
                        alnum(&text).contains(&alnum(locale.workspace_close_warning())),
                        "{text}"
                    );
                    assert!(alnum(&text).contains("项目space界"));
                    let compact = text.replace(' ', "");
                    assert!(
                        compact
                            .contains(&format!("[{}Esc]", locale.workspace_close_cancel_label()))
                    );
                    assert!(compact.contains(&format!(
                        "[{}Enter]",
                        locale.workspace_close_confirm_label()
                    )));
                    let focus_width =
                        crate::terminal_text::display_width(theme.glyphs.active_marker).max(1);
                    assert_eq!(
                        cancel_controls,
                        focus_width
                            + 4
                            + crate::terminal_text::display_width(
                                locale.workspace_close_cancel_label()
                            )
                            + 2 + 3 // Gap and Esc inside the button.
                            + 3 // The independent header [x] is also a cancel control.
                    );
                    assert_eq!(
                        confirm_controls,
                        focus_width
                            + 4
                            + crate::terminal_text::display_width(
                                locale.workspace_close_confirm_label()
                            )
                            + 2
                            + 5 // Gap and Enter inside the button.
                    );
                    ui.capture_confirmation_key(ClientKey::Enter);
                    assert!(
                        matches!(mailbox.try_recv().unwrap().unwrap().record(), QueuedCommand::Refresh(marker) if marker.workspace_close().is_some())
                    );
                } else {
                    assert!(
                        controls <= 3,
                        "only the independent header close may remain"
                    );
                    assert!(ui.view().workspace_close_confirmation().is_none());
                    assert_eq!(mailbox.cursors(), before);
                    assert!(mailbox.try_recv().unwrap().is_none());
                }
            }
        }
    }
}

#[test]
fn workspace_close_mouse_decisions_require_current_drawn_token_for_each_scope() {
    for project in [false, true] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let (mut ui, mailbox, _, snapshot) = ready();
            let open = |ui: &mut WorkbenchClientUi| {
                if project {
                    ui.request_project_close_all(&snapshot.navigation.projects[0].project_id)
                } else {
                    open_confirmation(ui, None)
                }
            };
            assert_eq!(open(&mut ui), ClientActionDisposition::DismissNavigator);
            let old = ui.view().workspace_close_confirmation().unwrap().token();
            ui.click_workspace_close(old, true);
            assert!(mailbox.try_recv().unwrap().is_none());
            draw(&mut ui, locale, &Theme::terminal_native(), 80, 24);
            ui.click_workspace_close(old, false);
            assert!(ui.view().workspace_close_confirmation().is_none());
            assert!(mailbox.try_recv().unwrap().is_none());

            assert_eq!(open(&mut ui), ClientActionDisposition::DismissNavigator);
            let current = ui.view().workspace_close_confirmation().unwrap().token();
            assert_ne!(current, old);
            let (_, hits) = draw_with_hits(&mut ui, locale, &Theme::terminal_native(), 80, 24);
            ui.click_workspace_close(old, false);
            ui.click_workspace_close(old, true);
            assert_eq!(
                ui.view().workspace_close_confirmation().unwrap().token(),
                current
            );
            assert!(mailbox.try_recv().unwrap().is_none());
            let mut decisions = [false, false];
            for y in 0..24 {
                for x in 0..80 {
                    if let Some(HitTarget::WorkspaceClose { token, confirm }) =
                        hits.hit_target_at(x, y)
                    {
                        assert_eq!(*token, current);
                        decisions[usize::from(*confirm)] = true;
                    }
                }
            }
            assert_eq!(decisions, [true, true]);
            ui.click_workspace_close(current, true);
            ui.click_workspace_close(current, true);
            let lease = mailbox.try_recv().unwrap().unwrap();
            let QueuedCommand::Refresh(marker) = lease.record() else {
                panic!("Close refresh");
            };
            assert_eq!(
                marker
                    .workspace_close()
                    .unwrap()
                    .project_request()
                    .is_some(),
                project
            );
            assert!(mailbox.try_recv().unwrap().is_none());
        }
    }
}
struct Port<'a> {
    ui: &'a mut WorkbenchClientUi,
    forwarded: &'a mut Vec<u8>,
}
impl TerminalInputPort for Port<'_> {
    fn native_capture_claims(&self) -> crate::tui_composition::CaptureClaims {
        self.ui.capture_claims()
    }
    fn native_capture_context(
        &self,
        owner: crate::tui_composition::WorkbenchCaptureOwner,
    ) -> Option<crate::workbench_client::CaptureContext> {
        self.ui.capture_context(owner)
    }
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(input);
        Ok(())
    }
    fn apply_client_action(
        &mut self,
        command: crate::workbench_client::InputAction,
    ) -> Option<ClientActionDisposition> {
        Some(self.ui.apply_client_action(command))
    }
    fn capture_tab_confirmation_key_for(
        &mut self,
        token: CloseConfirmationToken,
        key: ClientKey,
    ) -> bool {
        self.ui.capture_tab_confirmation_key(token, key)
    }
    fn capture_panel_confirmation_key_for(
        &mut self,
        token: PanelCloseConfirmationToken,
        key: ClientKey,
    ) -> bool {
        self.ui.capture_panel_confirmation_key(token, key)
    }
    fn capture_workspace_confirmation_key_for(
        &mut self,
        token: WorkspaceCloseToken,
        key: ClientKey,
    ) -> bool {
        self.ui.capture_workspace_close_key(token, key)
    }
}
fn dispatch(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    ui: &mut WorkbenchClientUi,
    forwarded: &mut Vec<u8>,
    bytes: &[u8],
) {
    dispatcher
        .dispatch(
            view,
            bytes,
            &mut Port { ui, forwarded },
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
}
#[test]
fn workspace_close_dispatcher_requires_draw_and_consumes_the_whole_confirmation_packet() {
    for cancel in [false, true] {
        let (mut ui, mailbox, _, snapshot) = ready_after_guard_refusal();
        let workspace = snapshot.navigation.workspaces[1].workspace_id.clone();
        let mut view = ClientView::new(120);
        let mut dispatcher = InputDispatcher::default();
        let mut forwarded = Vec::new();
        let before = mailbox.cursors();
        dispatch(
            &mut dispatcher,
            &mut view,
            &mut ui,
            &mut forwarded,
            b"\x02D",
        );
        assert!(ui.view().workspace_close_confirmation().is_some());
        for bytes in [
            b"\r".as_slice(),
            b"Y",
            b"y",
            b"\x1bTRAILING",
            b"\x1b[200~\r\x1b[201~",
        ] {
            dispatch(&mut dispatcher, &mut view, &mut ui, &mut forwarded, bytes);
            assert_eq!(mailbox.cursors(), before);
            assert!(ui.view().workspace_close_confirmation().is_some());
        }
        draw(&mut ui, Locale::EnUs, &Theme::terminal_native(), 120, 40);
        dispatch(
            &mut dispatcher,
            &mut view,
            &mut ui,
            &mut forwarded,
            if cancel { b"\x1b" } else { b"\rTRAILING\x02N" },
        );
        if cancel {
            dispatcher
                .synchronize(
                    &mut view,
                    &mut Port {
                        ui: &mut ui,
                        forwarded: &mut forwarded,
                    },
                    Instant::now() + std::time::Duration::from_secs(1),
                    &mut |_| Ok::<_, io::Error>(()),
                )
                .unwrap();
        }
        assert!(forwarded.is_empty());
        assert!(ui.view().workspace_close_confirmation().is_none());
        assert!(ui.view().setup_draft().is_none());
        let submitted = mailbox.try_recv().unwrap();
        assert_eq!(submitted.is_some(), !cancel);
        if let Some(lease) = submitted {
            let QueuedCommand::Refresh(marker) = lease.record() else {
                panic!("explicit Workspace close refresh");
            };
            assert_eq!(
                marker
                    .workspace_close()
                    .unwrap()
                    .workspace_command()
                    .unwrap()
                    .action,
                WorkbenchAction::WorkspaceClose {
                    workspace_id: workspace
                }
            );
        }
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}
#[test]
fn workspace_close_mailbox_preserves_observed_scope_and_original_cleanup_deadlines() {
    let (mut ui, mailbox, _, _) = ready();
    let lease = submit(&mut ui, &mailbox);
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("Close refresh");
    };
    let operation = marker.workspace_close().unwrap();
    assert_eq!(
        mailbox
            .mark_refresh_in_flight(operation.generation)
            .unwrap(),
        MutationResult::Applied
    );
    let known = receipt(operation, true);
    assert!(mailbox.retain_workspace_close_result(operation, known.clone()));
    assert!(!mailbox.retain_workspace_close_result(operation, WorkspaceCloseResult::Unknown));
    assert!(!mailbox.retain_workspace_close_result(operation, receipt(operation, false)));
    let evidence = mailbox.workspace_close_worker_loss().unwrap();
    assert_eq!(evidence.result, known);
    assert!(ui.finalize_workspace_close_worker(Some(evidence.clone())));
    assert_eq!(
        ui.state.as_ref().unwrap().workspace_close_deadlines(),
        Some(operation.deadlines)
    );
    mailbox.close(CarrierFailure::Transport);
    assert_eq!(mailbox.workspace_close_worker_loss(), Some(evidence));
    assert_eq!(
        ui.view().workspace_close_status().unwrap().status(),
        WorkspaceCloseStatus::Closed
    );
}

#[test]
fn workspace_close_mailbox_keeps_admission_before_the_first_successful_read() {
    let (mut ui, mailbox, _, _) = ready();
    let lease = submit(&mut ui, &mailbox);
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("Close refresh");
    };
    let operation = marker.workspace_close().unwrap();
    assert_eq!(
        mailbox
            .mark_refresh_in_flight(operation.generation)
            .unwrap(),
        MutationResult::Applied
    );
    let WorkspaceCloseResult::WorkspaceObserved { accepted, .. } = receipt(operation, false) else {
        unreachable!()
    };
    let known = WorkspaceCloseResult::WorkspaceAccepted(accepted);
    assert!(mailbox.retain_workspace_close_result(operation, WorkspaceCloseResult::Unknown));
    assert!(mailbox.retain_workspace_close_result(operation, known.clone()));
    assert!(!mailbox.retain_workspace_close_result(operation, WorkspaceCloseResult::Unknown));
    assert!(!mailbox.retain_workspace_close_result(operation, WorkspaceCloseResult::Refused));
    let evidence = mailbox.workspace_close_worker_loss().unwrap();
    assert_eq!(evidence.result, known);
    assert!(ui.finalize_workspace_close_worker(Some(evidence.clone())));
    mailbox.close(CarrierFailure::Transport);
    assert_eq!(mailbox.workspace_close_worker_loss(), Some(evidence));
    assert_eq!(
        ui.view().workspace_close_status().unwrap().status(),
        WorkspaceCloseStatus::Closing
    );
}

#[test]
fn project_close_all_dispatcher_confirms_only_drawn_exact_scope_and_consumes_packet_tail() {
    for cancel in [false, true] {
        let (mut ui, mailbox, _, snapshot) = ready();
        let mut view = ClientView::new(120);
        let mut dispatcher = InputDispatcher::default();
        let mut forwarded = Vec::new();
        let before = mailbox.cursors();
        let project_id = snapshot.navigation.projects[0].project_id.clone();
        assert_eq!(
            ui.request_project_close_all(&project_id),
            ClientActionDisposition::DismissNavigator
        );
        assert_eq!(
            ui.view()
                .workspace_close_confirmation()
                .unwrap()
                .project_member_count(),
            Some(1)
        );
        dispatch(&mut dispatcher, &mut view, &mut ui, &mut forwarded, b"\r");
        assert_eq!(mailbox.cursors(), before);
        draw(&mut ui, Locale::EnUs, &Theme::terminal_native(), 120, 40);
        for bytes in [
            b"Y".as_slice(),
            b"y",
            b"\x1b[13;2u",
            b"\x1b[13;5u",
            b"\x1b[200~\r\x1b[201~",
        ] {
            dispatch(&mut dispatcher, &mut view, &mut ui, &mut forwarded, bytes);
            assert_eq!(mailbox.cursors(), before);
        }
        dispatch(
            &mut dispatcher,
            &mut view,
            &mut ui,
            &mut forwarded,
            if cancel { b"\x1b" } else { b"\rTRAILING\x02N" },
        );
        if cancel {
            dispatcher
                .synchronize(
                    &mut view,
                    &mut Port {
                        ui: &mut ui,
                        forwarded: &mut forwarded,
                    },
                    Instant::now() + std::time::Duration::from_secs(1),
                    &mut |_| Ok::<_, io::Error>(()),
                )
                .unwrap();
        }
        assert!(forwarded.is_empty());
        assert!(ui.view().workspace_close_confirmation().is_none());
        let lease = mailbox.try_recv().unwrap();
        assert_eq!(lease.is_some(), !cancel);
        if let Some(lease) = lease {
            let QueuedCommand::Refresh(marker) = lease.record() else {
                panic!("Close refresh");
            };
            let operation = marker.workspace_close().unwrap();
            assert!(operation.workspace_command().is_none());
            assert!(matches!(operation.project_request().unwrap().action,
                WorkspaceGroupCloseAction::Start { project_id: actual, .. } if actual == project_id));
        }
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}
