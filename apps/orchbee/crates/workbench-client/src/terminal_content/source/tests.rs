use std::num::NonZeroU64;

use super::{ContentSource, PaintedRows, SnapshotMember, TerminalScreen};
use crate::workbench_state::{AttachmentKey, PaneKey, PaneRef};
use orchbee_terminal_protocol::{
    TerminalFrameRoute, TerminalInteraction, TerminalPixelGeometry, TerminalTextCursor,
    TerminalTextFrame,
};

#[test]
fn point_maps_live_rows_and_excludes_history_chrome() {
    let mut rows = PaintedRows {
        attachment: orchbee_terminal_protocol::AttachmentId::from_bytes([1; 16]).unwrap(),
        host_epoch: orchbee_terminal_protocol::HostEpoch::from_bytes([2; 16]).unwrap(),
        sequence: NonZeroU64::MIN,
        generation: NonZeroU64::MIN,
        revision: 3,
        columns: 80,
        start_column: 0,
        total_rows: 120,
        start_row: 96,
        row_count: 24,
        history: false,
    };
    assert_eq!(rows.point(7, 2), Some((7, 98)));
    assert_eq!(rows.point(80, 2), None);
    rows.start_column = 12;
    assert_eq!(rows.point(7, 2), Some((19, 98)));
    assert_eq!(rows.point(68, 2), None);
    rows.start_column = 0;
    rows.history = true;
    assert_eq!(rows.point(7, 0), None);
    assert_eq!(rows.point(7, 23), None);
    assert_eq!(rows.point(7, 2), Some((7, 98)));
}

#[test]
fn complete_cross_attach_fence_includes_host_terminal_and_screen() {
    let frame = TerminalTextFrame {
        route: TerminalFrameRoute {
            attachment_id: orchbee_terminal_protocol::AttachmentId::from_bytes([1; 16]).unwrap(),
            host_epoch: orchbee_terminal_protocol::HostEpoch::from_bytes([2; 16]).unwrap(),
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 3,
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
        alternate_screen: true,
        interaction: TerminalInteraction::default(),
        foreground: 0,
        background: 0,
        theme_revision: 1,
        scrollback_rows: 96,
        title: None,
        current_directory: None,
        progress: None,
        program_status: None,
        graphics_present: None,
        graphics: None,
        command_completion: None,
        command_observation: None,
        rows: Vec::new(),
    };
    let id = |value: u128| format!("{value:032x}");
    let tab = orchbee_workbench_protocol::WorkbenchTabId::parse(&id(4)).unwrap();
    let panel = orchbee_workbench_protocol::WorkbenchPanelId::parse(&id(5)).unwrap();
    let terminal = orchbee_client_server_protocol::HostTerminalId::parse(&id(11)).unwrap();
    let source = ContentSource {
        pane: PaneRef::new(
            PaneKey::new(AttachmentKey::new(1, tab.clone()), panel.clone()),
            0,
        ),
        member: SnapshotMember {
            session: orchbee_workbench_protocol::SessionId::parse(&id(6)).unwrap(),
            context: orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(&id(7)).unwrap(),
            tab,
            panel,
            presentation: orchbee_workbench_protocol::TerminalPresentationId::parse(&id(8))
                .unwrap(),
            worker: orchbee_workbench_protocol::WorkerId::parse(&id(9)).unwrap(),
            terminal: orchbee_workbench_protocol::TerminalId::parse(&id(10)).unwrap(),
        },
        host_terminal: terminal.clone(),
        screen: TerminalScreen::Alternate,
        rows: PaintedRows {
            attachment: frame.route.attachment_id,
            host_epoch: frame.route.host_epoch,
            sequence: frame.route.sequence,
            generation: frame.route.generation,
            revision: frame.route.revision,
            columns: frame.geometry.columns,
            start_column: 0,
            total_rows: 120,
            start_row: 96,
            row_count: 24,
            history: false,
        },
    };
    assert!(source.matches_frame(&terminal, &frame, 120));
    let mut independent = frame.clone();
    independent.route.attachment_id =
        orchbee_terminal_protocol::AttachmentId::from_bytes([12; 16]).unwrap();
    independent.route.sequence = NonZeroU64::new(2).unwrap();
    assert!(source.matches_frame(&terminal, &independent, 120));
    let other = orchbee_client_server_protocol::HostTerminalId::parse(&id(12)).unwrap();
    assert!(!source.matches_frame(&other, &frame, 120));
    let mut primary = frame.clone();
    primary.alternate_screen = false;
    assert!(!source.matches_frame(&terminal, &primary, 120));
    let mut changed = frame.clone();
    changed.route.host_epoch = orchbee_terminal_protocol::HostEpoch::from_bytes([13; 16]).unwrap();
    assert!(!source.matches_frame(&terminal, &changed, 120));
    let mut changed = frame.clone();
    changed.route.generation = NonZeroU64::new(2).unwrap();
    assert!(!source.matches_frame(&terminal, &changed, 120));
    let mut changed = frame.clone();
    changed.route.revision += 1;
    assert!(!source.matches_frame(&terminal, &changed, 120));
    assert!(!source.matches_frame(&terminal, &frame, 119));
    let mut changed = frame;
    changed.geometry.columns += 1;
    assert!(!source.matches_frame(&terminal, &changed, 120));
}
