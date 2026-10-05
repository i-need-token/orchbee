use super::super::{
    AttachmentKey, CommandMailbox, Geometry, PaneRef, WorkbenchPanelId, WorkbenchTabId,
};
use super::*;
use crate::{
    local_client::test_server::adjustable_operation_clock,
    terminal_content::TerminalScreen,
    terminal_input::{PointerLinkAdmission, PointerSelectionCell},
    terminal_link_activation::{Notice, PaintedLinkSource, PaintedRows, SnapshotMember},
};
use std::{
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

#[test]
fn synchronous_link_rejection_replaces_an_older_lane_notice() {
    let clock = adjustable_operation_clock(Arc::new(AtomicU64::new(0)));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = NativeUi::new(mailbox, clock);
    ui.links.set_notice_for_test(Notice::Handled);
    assert_eq!(
        ui.view().command_notice,
        Some(NativeSessionNotice::LinkHandled)
    );

    assert_eq!(
        ui.pointer_link(source(), PointerSelectionCell { column: 0, row: 0 }),
        PointerLinkAdmission::Rejected
    );
    assert_eq!(
        ui.view().command_notice,
        Some(NativeSessionNotice::LinkFailed)
    );
}

#[test]
fn synchronous_link_notice_expiry_requests_one_idle_redraw() {
    let elapsed = Arc::new(AtomicU64::new(0));
    let clock = adjustable_operation_clock(Arc::clone(&elapsed));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = NativeUi::new(mailbox, clock);
    assert_eq!(
        ui.pointer_link(source(), PointerSelectionCell { column: 0, row: 0 }),
        PointerLinkAdmission::Rejected
    );
    assert_eq!(
        ui.view().command_notice,
        Some(NativeSessionNotice::LinkFailed)
    );

    elapsed.store(3_001, Ordering::Release);
    assert!(ui.tick(), "the expiry edge must wake an idle redraw");
    assert_eq!(ui.view().command_notice, None);
}

#[test]
fn accepted_link_retires_an_older_synchronous_link_notice() {
    let clock = adjustable_operation_clock(Arc::new(AtomicU64::new(0)));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = NativeUi::new(mailbox, clock);
    ui.report_link_notice(NativeSessionNotice::LinkBusy);
    ui.links.set_notice_for_test(Notice::Handled);
    assert_eq!(
        ui.view().command_notice,
        Some(NativeSessionNotice::LinkHandled)
    );

    // A successful admission clears the older lane outcome before returning.
    // The UI adapter must also retire the synchronous rejection it supersedes.
    ui.links.clear_notice();
    assert_eq!(
        ui.view().command_notice,
        Some(NativeSessionNotice::LinkBusy)
    );
    ui.clear_synchronous_link_notice();
    assert_eq!(ui.view().command_notice, None);

    ui.command_notice = ui
        .clock
        .now()
        .checked_add(std::time::Duration::from_secs(3))
        .map(|until| (NativeSessionNotice::CommandFailed, until));
    ui.clear_synchronous_link_notice();
    assert_eq!(
        ui.view().command_notice,
        Some(NativeSessionNotice::CommandFailed),
        "admitting a link must preserve a notice owned by another command"
    );
}

fn source() -> PaintedLinkSource {
    fn id(value: u128) -> String {
        format!("{value:032x}")
    }
    let tab = WorkbenchTabId::parse(&id(1)).unwrap();
    let panel = WorkbenchPanelId::parse(&id(2)).unwrap();
    PaintedLinkSource {
        pane: PaneRef::new(
            crate::terminal_session::state::PaneKey::new(
                AttachmentKey::new(1, tab.clone()),
                panel.clone(),
            ),
            0,
        ),
        member: SnapshotMember {
            site: orchbee_workbench_protocol::SiteId::parse(&id(3)).unwrap(),
            context: orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(&id(4)).unwrap(),
            tab,
            panel,
            presentation: orchbee_workbench_protocol::TerminalPresentationId::parse(&id(5))
                .unwrap(),
            worker: orchbee_workbench_protocol::WorkerId::parse(&id(6)).unwrap(),
            terminal: orchbee_workbench_protocol::TerminalId::parse(&id(7)).unwrap(),
        },
        host_terminal: orchbee_client_server_protocol::HostTerminalId::parse(&id(10)).unwrap(),
        screen: TerminalScreen::Primary,
        rows: PaintedRows {
            attachment: orchbee_terminal_protocol::AttachmentId::from_bytes([8; 16]).unwrap(),
            host_epoch: orchbee_terminal_protocol::HostEpoch::from_bytes([9; 16]).unwrap(),
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 1,
            columns: 80,
            total_rows: 24,
            start_row: 0,
            row_count: 24,
            history: false,
        },
    }
}
