use super::super::test_support::{controlling_ui, install_projection};
use super::super::*;
use crate::localization::TerminalEnd;
use crate::workbench_client::normal_exit_hold::BOUND;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, CreationId, Envelope, ExitResult, LifecycleControl, MessageRole,
    PayloadEncoding, TerminalExited, TerminalId, TerminalIdentity, encode_lifecycle_control,
    encode_transport_envelope,
};
use std::sync::atomic::Ordering;

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
pub(super) fn poll(
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
pub(super) fn exited(ui: &WorkbenchClientUi, key: &PaneRef, code: u32) -> Vec<u8> {
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
