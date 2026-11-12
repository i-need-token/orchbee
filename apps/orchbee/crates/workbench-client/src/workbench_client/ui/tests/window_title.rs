use super::super::*;
use crate::localization::Locale;
use orchbee_workbench_protocol::{PanelCustomLabel, PanelNode};
#[test]
fn window_title_context_uses_current_view_manual_name_and_exact_live_frame() {
    let mut source = super::super::tests::membership::snapshot(1, 1);
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut source.terminal_contexts[0].tabs[0].root
    else {
        panic!()
    };
    primary_presentation.custom_label = Some(PanelCustomLabel::parse("Manual").unwrap());
    let (mut ui, _, _, _) = super::super::tests::move_membership::ready_ui_with_snapshot(source);
    let context = ui.view().window_title_context(Locale::EnUs);
    assert_eq!(context.tab, "Tab 0");
    assert_eq!(context.panel, "Manual");
    assert_eq!(context.workspace, "Workspace");
    assert_eq!(context.terminal_title, "");
    let key = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
    let mut projection = TerminalProjection::new(
        orchbee_terminal_protocol::AttachmentId::parse(&format!("{:032x}", 7001)).unwrap(),
        orchbee_terminal_protocol::HostEpoch::parse(&format!("{:032x}", 7002)).unwrap(),
    );
    let mut frame = super::super::tests::viewport::frame(&projection, 0, false, 24);
    frame.title = Some("Administrator: ⠋ building".into());
    projection.install_frame_for_test(frame);
    ui.projections.push(KeyedProjection::for_test(
        key,
        projection,
        ClientProjectionStatus::FrameAvailable,
    ));
    assert_eq!(
        ui.view().window_title_context(Locale::EnUs).terminal_title,
        "building"
    );
    // A console default title names its program as Tab chrome does.
    let projection = &mut ui.projections[0].value;
    let mut frame = super::super::tests::viewport::frame(projection, 0, false, 24);
    frame.title = Some(r"C:\WINDOWS\system32\cmd.exe".into());
    projection.install_frame_for_test(frame);
    assert_eq!(
        ui.view().window_title_context(Locale::ZhCn).terminal_title,
        "命令提示符"
    );
    ui.projections[0].status = ClientProjectionStatus::Blocked;
    assert_eq!(
        ui.view().window_title_context(Locale::EnUs).terminal_title,
        ""
    );
}
