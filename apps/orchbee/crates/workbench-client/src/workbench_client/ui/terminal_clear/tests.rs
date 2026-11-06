use super::*;
use crate::workbench_client::ui::tests::move_membership::ready_ui;

#[test]
fn clear_admission_never_writes_input_or_canonical_membership_and_stale_feedback_is_ignored() {
    let (mut ui, mailbox, _, _) = ready_ui();
    let original = ui.state.as_ref().unwrap().cache().clone();
    let key = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
    assert_eq!(
        ui.request_terminal_clear(),
        ClientActionDisposition::DismissNavigator
    );
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(lease.record(), QueuedCommand::ClearTerminal(tagged) if tagged.key() == &key));
    assert_eq!(ui.state.as_ref().unwrap().cache(), &original);
    assert_eq!(
        ui.view().notice(),
        Some(WorkbenchClientNotice::TerminalClearing)
    );
    ui.apply_terminal_clear_outcome(key.clone(), TerminalControlOutcome::OutcomeUnknown {});
    assert_eq!(
        ui.view().notice(),
        Some(WorkbenchClientNotice::TerminalClearUnknown)
    );
    ui.state = None;
    assert!(!ui.apply_terminal_clear_outcome(key, TerminalControlOutcome::OutcomeUnknown {}));
}
