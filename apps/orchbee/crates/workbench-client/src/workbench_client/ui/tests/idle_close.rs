//! A Tab or Panel whose terminal is a shell at its prompt closes without a confirmation; any
//! other sample, and no sample within the budget, presents the confirmation.
use super::super::test_support::{controlling_ui, install_projection};
use super::super::*;
use std::sync::atomic::Ordering;

/// The next queued command that is not an ordinary Poll.
fn next(mailbox: &CommandMailbox) -> crate::workbench_client::mailbox::CommandLease {
    loop {
        let lease = mailbox.try_recv().unwrap().expect("queued command");
        if !matches!(lease.record(), QueuedCommand::Poll(_)) {
            return lease;
        }
        mailbox.finish_lease(lease).unwrap();
    }
}

/// Settles the one queued process sample of `key` as `idle`.
fn settle(ui: &mut WorkbenchClientUi, mailbox: &CommandMailbox, key: &PaneRef, idle: Option<bool>) {
    let lease = next(mailbox);
    let QueuedCommand::ProcessProbe(tagged) = lease.record() else {
        panic!("process sample: {:?}", lease.record())
    };
    assert_eq!(tagged.key(), key);
    if let Some(idle) = idle {
        mailbox
            .publish_notice_before(
                WorkerEvent::ProcessProbeOutcome {
                    key: key.clone(),
                    probe: *tagged.value(),
                    idle,
                },
                ui.clock.now() + crate::workbench_state::OPERATION_TOTAL,
                &ui.clock,
            )
            .unwrap();
        ui.tick();
    }
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn an_idle_shell_closes_at_once_and_every_other_sample_presents_the_confirmation() {
    let (mut ui, mailbox, _, elapsed) = controlling_ui();
    ui.idle_close = super::super::idle_close::IdleClose::new(true);
    let key = install_projection(&mut ui, Some((0, false, 24)), 0);

    // A running program presents the hidden confirmation once its sample arrives.
    ui.apply_client_action(ClientAction::RequestCloseTab);
    assert!(ui.view().close_confirmation().is_none());
    settle(&mut ui, &mailbox, &key, Some(false));
    assert!(ui.view().close_confirmation().is_some());
    ui.cancel_close();

    // No sample within the budget presents it too.
    ui.apply_client_action(ClientAction::ClosePanel);
    settle(&mut ui, &mailbox, &key, None);
    assert!(ui.view().panel_close_confirmation().is_none());
    elapsed.fetch_add(501, Ordering::SeqCst);
    ui.tick();
    assert!(ui.view().panel_close_confirmation().is_some());
    ui.cancel_panel_close();

    // A shell at its prompt closes the Tab without presenting anything.
    ui.apply_client_action(ClientAction::RequestCloseTab);
    settle(&mut ui, &mailbox, &key, Some(true));
    assert!(ui.view().close_confirmation().is_none());
    assert!(matches!(next(&mailbox).record(), QueuedCommand::Close(_)));
}
