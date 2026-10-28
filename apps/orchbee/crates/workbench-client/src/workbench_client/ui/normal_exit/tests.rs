use super::super::test_support::{controlling_ui, install_projection};
use super::super::{
    CommandMailbox, LocalOperationClock, MutationResult, QueuedCommand, Tagged, WorkbenchClientUi,
};
use super::*;
use crate::workbench_client::state::{AttachmentKey, PaneKey};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, CreationId, Envelope, LifecycleControl, MessageRole, PayloadEncoding,
    TerminalExited, TerminalId, TerminalIdentity, encode_lifecycle_control,
    encode_transport_envelope,
};
use orchbee_workbench_protocol::{WorkbenchPanelId, WorkbenchTabId};
use std::sync::atomic::Ordering;

fn pane(incarnation: u32) -> PaneRef {
    let tab = WorkbenchTabId::parse(&format!("{:032x}", 5)).unwrap();
    let panel = WorkbenchPanelId::parse(&format!("{:032x}", 6)).unwrap();
    PaneRef::new(PaneKey::new(AttachmentKey::new(1, tab), panel), incarnation)
}

#[test]
fn only_a_sole_presentation_exiting_with_code_zero_holds_until_its_bound_or_departure() {
    let now = Instant::now();
    let normal = TerminalEnd::Exited(ExitResult::ExitCode(0));
    let mut holds = NormalExitHolds::default();
    for (end, sole) in [
        (TerminalEnd::Exited(ExitResult::ExitCode(1)), true),
        (TerminalEnd::Exited(ExitResult::Signal(9)), true),
        (TerminalEnd::Exited(ExitResult::Unknown(0)), true),
        (TerminalEnd::Failed, true),
        (normal, false),
    ] {
        holds.observe(&pane(1), end, sole, now);
        assert!(!holds.active(), "{end:?} sole={sole}");
    }

    holds.observe(&pane(1), normal, true, now);
    assert!(holds.holds(&pane(1)) && !holds.holds(&pane(2)));
    assert!(!holds.release(now + BOUND - Duration::from_millis(1), |_| true));
    assert!(holds.release(now + BOUND, |_| true));
    assert!(!holds.active());

    holds.observe(&pane(1), normal, true, now);
    assert!(holds.release(now, |key| *key != pane(1)));
    assert!(!holds.active());
}

#[test]
fn a_normal_exit_presents_no_settlement_until_its_hold_ends() {
    for code in [0, 1] {
        let (mut ui, mailbox, clock, elapsed) = controlling_ui();
        let key = install_projection(&mut ui, Some((0, false, 24)), 0);
        let panel = key.pane().panel().clone();
        let record = exited(&ui, &key, code);
        poll(&mut ui, &mailbox, &clock, &key, record);
        let end = TerminalEnd::Exited(ExitResult::ExitCode(code));
        let view = ui.view();
        assert_eq!(view.settled_end(&panel), Some(end));
        assert_eq!(view.normal_exit_closing(), code == 0);
        assert_eq!(view.terminal_end(&panel), (code != 0).then_some(end));

        ui.after_draw();
        assert!(!ui.tick());
        elapsed.fetch_add(u64::try_from(BOUND.as_millis()).unwrap(), Ordering::SeqCst);
        // The released hold alone owes the next draw.
        assert_eq!(ui.tick(), code == 0);
        assert!(!ui.view().normal_exit_closing());
        assert_eq!(ui.view().terminal_end(&panel), Some(end));
    }
}

/// Delivers one viewer record through the ordinary Poll path.
fn poll(
    ui: &mut WorkbenchClientUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    key: &PaneRef,
    record: Vec<u8>,
) {
    let admitted = mailbox.pane_ref_for_anchor(key.anchor()).unwrap();
    assert_eq!(&admitted, key);
    mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(admitted, ())))
        .expect("Poll admission");
    let lease = mailbox.try_recv().unwrap().expect("queued Poll");
    let poll_key = lease.poll_key().expect("Poll key");
    let cutoff = clock.now() + crate::workbench_client::state::OPERATION_TOTAL;
    assert_eq!(
        mailbox
            .publish_poll_result_before(poll_key, Some(record), false, cutoff, clock)
            .unwrap(),
        MutationResult::Applied
    );
    assert!(ui.tick());
    mailbox.finish_lease(lease).unwrap();
}

/// The Host exit record for the terminal `install_projection` binds.
fn exited(ui: &WorkbenchClientUi, key: &PaneRef, code: u32) -> Vec<u8> {
    let projection = &ui
        .projections
        .iter()
        .find(|projection| &projection.key == key)
        .unwrap()
        .value;
    let epoch = *projection.epoch();
    let end = encode_lifecycle_control(&LifecycleControl::TerminalExited(TerminalExited {
        identity: TerminalIdentity {
            creation_id: CreationId::from_bytes([9; 16]).unwrap(),
            host_epoch: epoch,
            terminal_id: TerminalId::from_bytes(7_003u128.to_be_bytes()).unwrap(),
        },
        final_revision: 1,
        exit: ExitResult::ExitCode(code),
    }))
    .unwrap();
    encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
        kind: end.kind,
        payload_encoding: PayloadEncoding::ControlJson,
        host_epoch: Some(epoch),
        correlation_id: None,
        optional_message: false,
        payload: &end.payload,
    })
    .unwrap()
}
