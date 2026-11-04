//! Ready and controlling Workbench Client UI fixtures shared by unit tests and the owning
//! application's whole-frame tests.

pub use super::pane_graphics::test_support::actual_native_shell_places_confirmed_pixels_inside_one_frame_and_clears_under_a_board;
pub use super::settings::test_support::{
    SettingsFrameSource, covering_client_owner_uses_confirmed_appearance_until_settings_returns,
    draft_appearance_reaches_background_chrome_and_program_defaults_without_runtime_effects,
    external_confirmed_values_update_unedited_presentation_and_cancel_reveals_the_latest_snapshot,
};
use super::{
    Arc, ClientProjectionStatus, CommandMailbox, ControllerMode, FrameIdentity, Geometry,
    KeyedProjection, LocalOperationClock, PaneRef, ProtocolVersion, TerminalProjection,
    WorkbenchClientUi, WorkbenchPanelId, WorkbenchTabId, WorkerEvent,
};
use crate::local_client::test_support::server::adjustable_operation_clock;
use crate::workbench_client::mailbox::EventDeadline;
use orchbee_client_server_protocol::ControllerId;
use orchbee_terminal_protocol::{
    AttachmentId, HostEpoch, TerminalFrameRoute, TerminalInteraction, TerminalPixelGeometry,
    TerminalTextCell, TerminalTextCursor, TerminalTextFrame, TerminalTextRow, TerminalTextStyle,
};
use orchbee_workbench_protocol::{
    Navigation, PanelNode, Presentation, PresentationBinding, Project, SessionId, Snapshot, Tab,
    TerminalContext, TerminalId, TerminalPresentationId, WorkbenchProjectId, WorkbenchRevision,
    WorkbenchTerminalContextId, WorkerId,
};
use std::{num::NonZeroU64, sync::atomic::AtomicU64};

pub(in crate::workbench_client) fn attachment() -> AttachmentId {
    AttachmentId::parse(&format!("{:032x}", 7_001)).unwrap()
}

pub(in crate::workbench_client) fn epoch() -> HostEpoch {
    HostEpoch::parse(&format!("{:032x}", 7_002)).unwrap()
}

/// A minimal live frame whose gate-relevant fields the caller controls,
/// routed to the projection it will be installed on.
pub(in crate::workbench_client) fn frame(
    projection: &TerminalProjection,
    scrollback_rows: u64,
    alternate_screen: bool,
    rows: u16,
) -> TerminalTextFrame {
    TerminalTextFrame {
        route: TerminalFrameRoute {
            attachment_id: *projection.attachment(),
            host_epoch: *projection.epoch(),
            sequence: NonZeroU64::new(1).unwrap(),
            generation: NonZeroU64::new(1).unwrap(),
            revision: 1,
            base_revision: None,
        },
        geometry: TerminalPixelGeometry {
            cell_pixel_height: 16,
            cell_pixel_width: 8,
            columns: 80,
            rows: 24,
        },
        cursor: TerminalTextCursor {
            column: 0,
            row: 0,
            visible: true,
        },
        alternate_screen,
        interaction: TerminalInteraction::default(),
        foreground: 0,
        background: 0,
        theme_revision: 1,
        scrollback_rows,
        progress: None,
        graphics_present: None,
        command_completion: None,
        command_observation: None,
        title: None,
        current_directory: None,
        rows: (0..rows)
            .map(|index| TerminalTextRow {
                prompt_class: 0,
                index,
                flags: 0,
                cells: vec![TerminalTextCell {
                    text: "x".into(),
                    style: TerminalTextStyle {
                        foreground: 0,
                        background: 0,
                        underline_color: 0,
                        flags: 0,
                        underline: 0,
                    },
                    width: 0,
                }],
            })
            .collect(),
    }
}

/// Installs a `KeyedProjection` for the exact focus incarnation — with the
/// frame spec `(scrollback_rows, alternate_screen, rows)` and read
/// watermark the test needs — bypassing the event path for fixtures that
/// exercise gates rather than the owner seam.
pub(in crate::workbench_client) fn install_projection(
    ui: &mut WorkbenchClientUi,
    frame_spec: Option<(u64, bool, u16)>,
    read_watermark: u64,
) -> PaneRef {
    let key = ui
        .state
        .as_ref()
        .unwrap()
        .focus_pane_ref()
        .expect("focus pane");
    let mut value = TerminalProjection::new(attachment(), epoch()).with_host_terminal(
        orchbee_client_server_protocol::HostTerminalId::parse(&format!("{:032x}", 7_003)).unwrap(),
    );
    if let Some((scrollback_rows, alternate_screen, rows)) = frame_spec {
        let frame = frame(&value, scrollback_rows, alternate_screen, rows);
        value.install_frame_for_test(frame);
    }
    ui.projections.push(KeyedProjection {
        effect_stream: super::super::terminal_effects::Stream::new(
            *value.attachment(),
            *value.epoch(),
        ),
        key: key.clone(),
        value,
        status: ClientProjectionStatus::FrameAvailable,
        record_ready: false,
        viewport: None,
        tail: None,
        tail_attempted: None,
        history_sequence: 0,
        read_watermark,
        selection: None,
    });
    key
}

/// A ready fixture under a `Controlling` mode — `forward` requires it.
pub(in crate::workbench_client) fn controlling_ui() -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Arc<AtomicU64>,
) {
    controlling_ui_with(2)
}

/// [`controlling_ui`] over `tab_count` Tabs, the first one selected.
pub(in crate::workbench_client) fn controlling_ui_with(
    tab_count: usize,
) -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Arc<AtomicU64>,
) {
    let elapsed = Arc::new(AtomicU64::new(0));
    let clock = adjustable_operation_clock(Arc::clone(&elapsed));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = WorkbenchClientUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: crate::workbench_projection::project_preferred_workspace(&snapshot(
                    tab_count, 1,
                ))
                .unwrap(),
            },
            EventDeadline::Startup(clock.now() + crate::workbench_state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    let state = ui.state.as_mut().unwrap();
    let key = state.desired().unwrap().clone();
    let geometry = Geometry::new(80, 24, 8, 16).unwrap();
    let mode = ControllerMode::Controlling(ControllerId::parse(&format!("{:032x}", 8)).unwrap());
    assert!(
        state
            .pane_ref_for(&key)
            .is_some_and(|pane| state.install_projection(&pane))
    );
    assert!(
        state
            .pane_ref_for(&key)
            .is_some_and(|pane| state.record_setup_configuration(
                &pane,
                mode.clone(),
                Some(geometry)
            ))
    );
    assert!(
        state
            .pane_ref_for(&key)
            .is_some_and(|pane| state.stage_drawn_frame(&pane, FrameIdentity::new(1)))
    );
    assert!(
        state
            .pane_ref_for(&key)
            .is_some_and(|pane| state.mark_feedback_accepted(&pane, FrameIdentity::new(1)))
    );
    assert!(
        state
            .pane_ref_for(&key)
            .is_some_and(|pane| state.publish_ready(
                &pane,
                FrameIdentity::new(1),
                mode,
                Some(geometry)
            ))
    );
    (ui, mailbox, clock, elapsed)
}
pub(in crate::workbench_client) fn project() -> Project {
    Project {
        project_id: WorkbenchProjectId::parse(&format!("{:032x}", 270)).unwrap(),
        label: "Project".into(),
    }
}

pub(in crate::workbench_client) fn ready(
    with_project: bool,
) -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Snapshot,
) {
    let mut snapshot = snapshot(1, 1);
    if with_project {
        snapshot.navigation.projects.push(project());
    }
    ready_snapshot(snapshot)
}

pub(in crate::workbench_client) fn ready_snapshot(
    snapshot: Snapshot,
) -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Snapshot,
) {
    let clock = adjustable_operation_clock(Arc::new(AtomicU64::new(0)));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = WorkbenchClientUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: crate::workbench_projection::project_preferred_workspace(&snapshot).unwrap(),
            },
            EventDeadline::Startup(clock.now() + crate::workbench_state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    let state = ui.state.as_mut().unwrap();
    let key = state.desired().unwrap().clone();
    if let Some(pane) = state.pane_ref_for(&key) {
        state.install_projection(&pane);
    }
    if let Some(pane) = state.pane_ref_for(&key) {
        state.record_setup_configuration(&pane, ControllerMode::ReadOnly, None);
    }
    if let Some(pane) = state.pane_ref_for(&key) {
        state.stage_drawn_frame(&pane, FrameIdentity::new(1));
    }
    if let Some(pane) = state.pane_ref_for(&key) {
        state.mark_feedback_accepted(&pane, FrameIdentity::new(1));
    }
    if let Some(pane) = state.pane_ref_for(&key) {
        state.publish_ready(&pane, FrameIdentity::new(1), ControllerMode::ReadOnly, None);
    }
    (ui, mailbox, clock, snapshot)
}
pub(in crate::workbench_client) fn snapshot_terminal(index: usize) -> TerminalId {
    TerminalId::parse(&format!("{:032x}", 1004 + index * 10)).unwrap()
}

pub(in crate::workbench_client) fn snapshot(count: usize, revision: u64) -> Snapshot {
    let context = WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap();
    Snapshot {
        navigation: {
            let fixture_context_id = context.clone();
            let fixture_workspace_id = orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                fixture_context_id.as_str(),
            )
            .unwrap();
            Navigation {
                project_order: Vec::new(),
                directories: Vec::new(),
                workspaces: std::iter::once(orchbee_workbench_protocol::Workspace {
                    terminal_directory: None,
                    automatic_label: false,
                    workspace_id: fixture_workspace_id.clone(),
                    terminal_context_id: fixture_context_id.clone(),
                    project_id: None,
                    primary_directory_id: None,
                    label: "Workspace".into(),
                })
                .chain(Vec::new())
                .collect(),
                workspace_order: Vec::new(),
                projects: Vec::new(),
            }
        },
        protocol_version: ProtocolVersion,
        revision: WorkbenchRevision::new(revision),
        session_id: SessionId::parse(&format!("{:032x}", 1)).unwrap(),
        terminal_contexts: vec![TerminalContext {
            groups: Vec::new(),
            terminal_context_id: context,
            tabs: (0..count)
                .map(|index| {
                    let base = 1000 + index * 10;
                    Tab {
                        automatic_label: false,
                        group_id: None,
                        pinned: false,
                        label: format!("Tab {index}"),
                        tab_id: WorkbenchTabId::parse(&format!("{base:032x}")).unwrap(),
                        root: PanelNode::Panel {
                            right_click: Default::default(),
                            child_presentations: Vec::new(),
                            panel_id: WorkbenchPanelId::parse(&format!("{:032x}", base + 1))
                                .unwrap(),
                            primary_presentation: Presentation {
                                custom_label: None,
                                label: format!("Terminal {index}"),
                                presentation_id: TerminalPresentationId::parse(&format!(
                                    "{:032x}",
                                    base + 2
                                ))
                                .unwrap(),
                                binding: Some(PresentationBinding {
                                    terminal_id: snapshot_terminal(index),
                                    worker_id: WorkerId::parse(&format!("{:032x}", base + 3))
                                        .unwrap(),
                                }),
                                launch_failure: None,
                                launch_program: None,
                            },
                        },
                    }
                })
                .collect(),
        }],
    }
}
