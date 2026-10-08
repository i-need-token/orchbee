use super::*;
use crate::{
    client_view::{ClientKey, ClientView},
    localization::Locale,
    terminal_input::{InputDispatcher, TerminalInputPort, TerminalInputUnavailable},
    terminal_session::{
        TerminalSessionView,
        state::workspace_close::{WorkspaceCloseOperation, WorkspaceCloseResult},
        workbench_projection::{NativeContextLocation, project_native_context_tabs},
    },
    tui_pointer::HitMapBuilder,
    tui_theme::Theme,
};
use orchbee_workbench_protocol::*;
use ratatui::{Terminal, backend::TestBackend};
use std::{io, time::Instant};

pub(in crate::terminal_session) fn ready()
-> (NativeUi, Arc<CommandMailbox>, LocalOperationClock, Snapshot) {
    let clock = crate::local_client::test_server::adjustable_operation_clock(Arc::new(
        std::sync::atomic::AtomicU64::new(0),
    ));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
    let mut snapshot = super::membership_tests::snapshot(0, 1);
    let project_id = WorkbenchProjectId::parse(&format!("{:032x}", 270)).unwrap();
    let workspace_id = WorkbenchWorkspaceId::parse(&format!("{:032x}", 271)).unwrap();
    let terminal_context_id = WorkbenchTerminalContextId::parse(&format!("{:032x}", 272)).unwrap();
    snapshot.navigation.projects.push(Project {
        project_id: project_id.clone(),
        label: "Project".into(),
        directories: Vec::new(),
        workspaces: vec![Workspace {
            workspace_id: workspace_id.clone(),
            terminal_context_id: terminal_context_id.clone(),
            label: "项目 space 界".into(),
            primary_directory_id: None,
        }],
    });
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: terminal_context_id.clone(),
        tabs: Vec::new(),
        groups: Vec::new(),
    });
    let location = NativeContextLocation::Workspace {
        project_id,
        workspace_id,
        terminal_context_id,
    };
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: project_native_context_tabs(&snapshot, &location).unwrap(),
            },
            super::super::mailbox::EventDeadline::Startup(
                clock.now() + super::super::state::OPERATION_TOTAL,
            ),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
    (ui, mailbox, clock, snapshot)
}
pub(in crate::terminal_session) fn submit(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
) -> super::super::mailbox::CommandLease {
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestWorkspaceClose),
        SessionActionDisposition::DismissNavigator
    );
    let token = ui.view().workspace_close_confirmation().unwrap().token();
    ui.after_workspace_close_draw(Some(ModalPresentation::Presented(token)));
    assert!(ui.capture_confirmation_key(ClientKey::Plain('y')));
    mailbox.try_recv().unwrap().unwrap()
}
pub(in crate::terminal_session) fn assert_repair(ui: &NativeUi, removed: bool) {
    let cache = ui.state.as_ref().unwrap().cache();
    assert_eq!(
        matches!(cache.location(), NativeContextLocation::Standalone(_)),
        removed
    );
    assert_eq!(cache.revision(), WorkbenchRevision::new(2));
}
pub(in crate::terminal_session) fn receipt(
    operation: &WorkspaceCloseOperation,
    closed: bool,
) -> WorkbenchWorkspaceGroupCloseResult {
    let WorkspaceGroupCloseAction::Start {
        operation_id,
        project_id,
        workspace_id,
    } = &operation.request.action
    else {
        panic!("Start");
    };
    WorkbenchWorkspaceGroupCloseResult {
        protocol_version: ProtocolVersion,
        request_id: operation.request.request_id.clone(),
        operation_id: operation_id.clone(),
        outcome: WorkspaceGroupCloseOutcome::Available {
            project_id: project_id.clone(),
            workspace_id: workspace_id.clone(),
            attempt: 1,
            status: if closed {
                WorkspaceGroupCloseStatus::Closed {}
            } else {
                WorkspaceGroupCloseStatus::Pending {}
            },
            members: vec![WorkspaceGroupCloseMember {
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
                label: WorkbenchLabel::parse("项目 space 界").unwrap(),
                close: Some(WorkspaceGroupCloseChild {
                    request_id: WorkbenchRequestId::parse(&format!("{:032x}", 900)).unwrap(),
                    observation: closed.then(|| WorkspaceGroupCloseObservation {
                        attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                        status: WorkbenchTabCloseStatus::Closed {
                            revision: WorkbenchRevision::new(2),
                        },
                    }),
                }),
            }],
        },
    }
}
fn draw(ui: &mut NativeUi, locale: Locale, theme: &Theme, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let view = ClientView::new(width);
    let mut receipt = None;
    terminal
        .draw(|frame| {
            receipt = Some(crate::tui_render::render_terminal_shell(
                frame,
                &view,
                locale,
                Some(TerminalSessionView::Native(ui.view())),
                theme,
                &mut HitMapBuilder::new(),
            ));
        })
        .unwrap();
    ui.after_close_draw(receipt.unwrap());
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}
fn alnum(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).collect()
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
                ui.apply_client_action(ClientAction::RequestWorkspaceClose);
                assert!(ui.modal_owns_input());
                #[cfg(windows)]
                {
                    assert!(ui.control_dialog());
                    assert!(ui.view().position_draw(std::iter::empty()).is_none());
                }
                let text = draw(&mut ui, locale, &theme, width, height);
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
                    ui.capture_confirmation_key(ClientKey::Plain('y'));
                    assert!(
                        matches!(mailbox.try_recv().unwrap().unwrap().record(), QueuedCommand::Refresh(marker) if marker.workspace_close().is_some())
                    );
                } else {
                    assert!(ui.view().workspace_close_confirmation().is_none());
                    assert_eq!(mailbox.cursors(), before);
                    assert!(mailbox.try_recv().unwrap().is_none());
                }
            }
        }
    }
}
struct Port<'a> {
    ui: &'a mut NativeUi,
    forwarded: &'a mut Vec<u8>,
}
impl TerminalInputPort for Port<'_> {
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(input);
        Ok(())
    }
    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        Some(self.ui.apply_client_action(action))
    }
    fn capture_confirmation_key(&mut self, key: ClientKey) -> bool {
        self.ui.capture_confirmation_key(key)
    }
}
fn dispatch(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    ui: &mut NativeUi,
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
        let (mut ui, mailbox, _, _) = ready();
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
            b"y".as_slice(),
            b"Y",
            b"\r",
            b"\x1bTRAILING",
            b"\x1b[200~y\x1b[201~",
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
            if cancel { b"\x1b" } else { b"yTRAILING\x02N" },
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
        assert_eq!(mailbox.try_recv().unwrap().is_some(), !cancel);
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}
#[test]
fn workspace_close_mailbox_preserves_observed_children_and_original_cleanup_deadlines() {
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
    let known = WorkspaceCloseResult::Observed(Box::new(receipt(operation, true)));
    assert!(mailbox.retain_workspace_close_result(operation, known.clone()));
    assert!(!mailbox.retain_workspace_close_result(operation, WorkspaceCloseResult::Unknown));
    assert!(!mailbox.retain_workspace_close_result(
        operation,
        WorkspaceCloseResult::Observed(Box::new(receipt(operation, false)))
    ));
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
