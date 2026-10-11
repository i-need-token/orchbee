use super::super::*;
#[test]
fn standalone_location_rename_reports_exact_local_refusal_without_wire_or_cursor_consumption() {
    for (action, target) in [
        (
            ClientAction::RequestProjectRename,
            NativeLocationLabelTarget::Project,
        ),
        (
            ClientAction::RequestWorkspaceRename,
            NativeLocationLabelTarget::Workspace,
        ),
    ] {
        let (mut ui, mailbox, _, _) = super::super::tests::workspace_setup::ready(false);
        ui.state
            .as_mut()
            .unwrap()
            .set_notice(SessionNotice::CreateNotCreated);
        let before = mailbox.cursors();
        let old = ui.state.as_ref().unwrap().desired().cloned();
        assert_eq!(
            ui.apply_client_action(action),
            SessionActionDisposition::KeepNavigator
        );
        assert_eq!(
            ui.view().setup_status(),
            Some(NativeSetupStatus::LocationLabel {
                target,
                status: NativeLocationLabelStatus::Unavailable
            })
        );
        assert_eq!(ui.state.as_ref().unwrap().notice(), None);
        assert_eq!(mailbox.cursors(), before);
        assert!(mailbox.try_recv().unwrap().is_none());
        assert_eq!(ui.state.as_ref().unwrap().desired(), old.as_ref());
        assert_eq!(ui.view().phase(), NativeDisplayPhase::Ready);
    }
}
