use super::*;
use crate::action_menu::{ActionMenu, MenuAction, MenuDraw, MenuTarget};
use crate::client_view::{KeyEvent, Overlay};
use crate::terminal_input::{PointerPane, PointerProbe};
use crate::tui_pointer::{HitLayer, HitMapBuilder, HitTarget};
use ratatui::layout::Rect;

struct Port {
    menu: ActionMenu,
    actions: Vec<MenuAction>,
    calls: Rc<RefCell<Vec<Call>>>,
    target: Option<MenuTarget>,
    probe: PointerProbe,
    selections: Vec<crate::terminal_input::PointerSelectionIntent>,
    focus_on_request: bool,
}

fn sidebar_context() -> crate::terminal_session::NativeContextTarget {
    use orchbee_workbench_protocol::*;
    crate::terminal_session::NativeContextTarget {
        site_id: SiteId::parse(&format!("{:032x}", 1)).unwrap(),
        location: crate::terminal_session::NativeContextLocation::Workspace {
            project_id: WorkbenchProjectId::parse(&format!("{:032x}", 2)).unwrap(),
            workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 3)).unwrap(),
            terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{:032x}", 4)).unwrap(),
        },
    }
}

fn sidebar_view() -> ClientView {
    let mut view = ClientView::new(120);
    view.machines.available = true;
    view.machines.observe_agent_sidebar(true);
    view.machines
        .observe_local(vec![crate::machine_discovery::Context {
            target: sidebar_context(),
            project: Some("Project".into()),
            workspace: Some("Workspace".into()),
        }]);
    view.machines.observe_focus(
        crate::machine_picker::Choice::Local,
        Some(sidebar_context()),
        None,
    );
    view
}

fn sidebar_project_target() -> crate::machine_picker::SidebarTarget {
    let target = sidebar_context();
    let crate::terminal_session::NativeContextLocation::Workspace { project_id, .. } =
        target.location
    else {
        unreachable!()
    };
    crate::machine_picker::SidebarTarget::Project {
        machine: crate::machine_picker::Choice::Local,
        site_id: target.site_id,
        project_id,
    }
}

fn sidebar_menu_map() -> crate::tui_pointer::FrameHitMap {
    sidebar_target_map(crate::machine_picker::SidebarTarget::Context {
        machine: crate::machine_picker::Choice::Local,
        target: sidebar_context(),
    })
}

fn sidebar_target_map(
    target: crate::machine_picker::SidebarTarget,
) -> crate::tui_pointer::FrameHitMap {
    let mut hits = HitMapBuilder::new();
    hits.record_action_menu_source(Some(crate::action_menu::DrawnMenuSource {
        site: sidebar_context().site_id,
        revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
    }));
    hits.record(
        HitLayer::Foreground,
        Rect::new(20, 2, 3, 1),
        HitTarget::SidebarMenu(target),
    );
    hits.finish()
}

#[test]
fn sidebar_project_pointer_acquires_the_input_lease_before_focus_or_disclosure() {
    for fail in [true, false] {
        let mut view = sidebar_view();
        let target = sidebar_project_target();
        let expanded = view.machines.sidebar_project_expanded(&target);
        let mut port = Port::new(MenuAction::Help);
        let mut dispatcher = InputDispatcher::default();
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        let mut hits = HitMapBuilder::new();
        hits.record(
            HitLayer::Foreground,
            Rect::new(2, 3, 1, 1),
            HitTarget::SidebarDisclosure(target.clone()),
        );
        dispatcher.publish_hit_map(hits.finish());
        let result = dispatcher.dispatch(
            &mut view,
            b"\x1b[<0;3;4Mtail",
            &mut port,
            Instant::now(),
            |_, _| Ok::<(), io::Error>(()),
            &mut |mode| {
                if fail && mode == ModeRequest::BracketedPaste(true) {
                    Err(io::Error::other("input lease unavailable"))
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result.is_err(), fail);
        assert_eq!(view.machines.sidebar_navigation_active(), !fail);
        assert_eq!(
            view.machines.sidebar_project_expanded(&target),
            if fail { expanded } else { !expanded }
        );
        assert!(view.machines.request.is_none());
        assert!(forwarded_bytes(&port.calls).is_empty());
    }
}

#[test]
fn sidebar_mode_owns_only_explicit_navigation_and_fences_exit_packets() {
    let mut view = sidebar_view();
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[A\t\r");
    assert_eq!(forwarded_bytes(&port.calls), b"\x1b[A\t\r");
    port.calls.borrow_mut().clear();
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02Btail");
    assert!(view.machines.sidebar_navigation_active());
    assert!(view.machines.request.is_none());
    dispatcher.publish_hit_map(sidebar_menu_map());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[Aunknown");
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[200~\r\x1bOQ\x1b[201~",
    );
    assert!(view.machines.request.is_none());
    assert!(forwarded_bytes(&port.calls).is_empty());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27uignored");
    assert!(!view.machines.sidebar_navigation_active());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"old frame");
    assert!(forwarded_bytes(&port.calls).is_empty());
    dispatcher.publish_hit_map(sidebar_menu_map());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"terminal");
    assert_eq!(forwarded_bytes(&port.calls), b"terminal");

    port.calls.borrow_mut().clear();
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02B");
    dispatcher.publish_hit_map(sidebar_menu_map());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[");
    view.machines.observe_agent_sidebar(false);
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    assert!(!view.machines.sidebar_navigation_active());
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"Anew input");
    assert_eq!(forwarded_bytes(&port.calls), b"new input");
}

#[test]
fn sidebar_keyboard_and_row_menu_share_drawn_identity_and_cancel_returns_to_navigation() {
    for project in [false, true] {
        for entry in [
            b"\x1bOQtail".as_slice(),
            b"\x1b[<0;21;3Mtail",
            b"\x1b[<2;21;3Mtail",
        ] {
            let mut view = sidebar_view();
            let mut port = Port::new(MenuAction::Help);
            let mut dispatcher = InputDispatcher::default();
            dispatcher.set_pointer_capture(&mut view, &mut port, true);
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02B");
            let target = if project {
                sidebar_project_target()
            } else {
                crate::machine_picker::SidebarTarget::Context {
                    machine: crate::machine_picker::Choice::Local,
                    target: sidebar_context(),
                }
            };
            assert!(view.machines.focus_sidebar_target(&target));
            dispatcher.publish_hit_map(sidebar_target_map(target.clone()));
            let before = view.machines.sidebar_focused_target().cloned();
            dispatch_all(&mut dispatcher, &mut view, &mut port, entry);
            assert!(port.menu.active());
            assert_eq!(
                port.target,
                Some(MenuTarget::Drawn {
                    source: crate::action_menu::DrawnMenuSource {
                        site: sidebar_context().site_id,
                        revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
                    },
                    target: Box::new(match target {
                        crate::machine_picker::SidebarTarget::Project { project_id, .. } =>
                            MenuTarget::Project(project_id),
                        crate::machine_picker::SidebarTarget::Context { target, .. } =>
                            MenuTarget::Workspace(target.location),
                    }),
                })
            );
            assert_eq!(view.machines.sidebar_focused_target(), before.as_ref());
            assert!(view.machines.request.is_none());
            port.arm();
            let mut modes = Vec::new();
            dispatcher
                .dispatch(
                    &mut view,
                    b"\x1b[27utail",
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<(), io::Error>(()),
                    &mut |mode| {
                        modes.push(mode);
                        Ok(())
                    },
                )
                .unwrap();
            assert!(!port.menu.active());
            assert!(view.machines.sidebar_navigation_active());
            assert_eq!(view.machines.sidebar_focused_target(), before.as_ref());
            assert!(forwarded_bytes(&port.calls).is_empty());
            assert!(modes.contains(&ModeRequest::BracketedPaste(true)));
        }
    }
}

#[test]
fn sidebar_help_covers_navigation_without_activating_or_losing_its_owner() {
    for width in [70, 120] {
        let mut view = sidebar_view();
        view.resize(width);
        let mut port = Port::new(MenuAction::Help);
        let mut dispatcher = InputDispatcher::default();
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02B");
        let before = view.machines.sidebar_focused_target().cloned();
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1bOP");
        assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
        let sidebar = crate::tui_layout::project_shell_areas(
            Rect::new(0, 0, width, 30),
            view.sidebar_surface(),
        )
        .sidebar;
        view.machines.observe_agent_sidebar(sidebar.is_some());
        assert!(view.machines.sidebar_navigation_active());
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[B\r");
        assert_eq!(view.machines.sidebar_focused_target(), before.as_ref());
        assert!(view.machines.request.is_none());
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[6~");
        assert_eq!(view.help_page(), 1);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27u");
        assert_eq!(
            view.overlay(),
            if width < 96 {
                Some(Overlay::BusinessSidebar)
            } else {
                None
            }
        );
        assert!(view.machines.sidebar_navigation_active());
        assert!(forwarded_bytes(&port.calls).is_empty());
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1bOP");
        let mut hits = HitMapBuilder::new();
        hits.mark_overlay_active();
        hits.record(
            HitLayer::OverlaySurface,
            Rect::new(25, 5, 20, 10),
            HitTarget::OverlaySurface,
        );
        dispatcher.publish_hit_map(hits.finish());
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;2;2Mtail");
        assert_eq!(
            view.overlay(),
            if width < 96 {
                Some(Overlay::BusinessSidebar)
            } else {
                None
            }
        );
        assert!(view.machines.sidebar_navigation_active());
        assert!(forwarded_bytes(&port.calls).is_empty());
    }
}

#[test]
fn sidebar_keyboard_menu_requires_a_fresh_draw_and_live_row() {
    for removed in [false, true] {
        let mut view = sidebar_view();
        let mut port = Port::new(MenuAction::Help);
        let mut dispatcher = InputDispatcher::default();
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02B");
        if removed {
            dispatcher.publish_hit_map(sidebar_menu_map());
            view.machines.observe_local(Vec::new());
        }
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1bOQ");
        assert!(!port.menu.active());
        assert!(view.machines.request.is_none());
        assert!(forwarded_bytes(&port.calls).is_empty());
    }
}
impl Port {
    fn new(action: MenuAction) -> Self {
        Self {
            menu: ActionMenu::default(),
            actions: vec![action],
            calls: Rc::new(RefCell::new(Vec::new())),
            target: None,
            probe: PointerProbe::default(),
            selections: Vec::new(),
            focus_on_request: false,
        }
    }
    fn arm(&mut self) {
        let token = self.menu.view().unwrap().token;
        self.menu.draw(Some(MenuDraw { token, rows: 8 }));
    }
}
impl TerminalInputPort for Port {
    fn forward(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), super::super::super::TerminalInputUnavailable> {
        self.calls.borrow_mut().push(Call::Forward(bytes.to_vec()));
        Ok(())
    }
    fn action_menu_active(&self) -> bool {
        self.menu.active()
    }
    fn open_action_menu(
        &mut self,
        target: MenuTarget,
        anchor: Option<(u16, u16)>,
        _: bool,
    ) -> bool {
        self.target = Some(target);
        self.menu.open(String::new(), self.actions.clone(), anchor)
    }
    fn capture_action_menu_key(&mut self, event: KeyEvent) {
        self.menu.key(event);
    }
    fn cancel_action_menu(&mut self) {
        self.menu.cancel();
    }
    fn click_action_menu(&mut self, token: crate::action_menu::MenuToken, row: usize) {
        self.menu.click(token, row);
    }
    fn take_action_menu_action(&mut self, _: bool) -> Option<ClientAction> {
        self.menu.take().map(MenuAction::client_action)
    }
    fn pointer_probe(&mut self) -> PointerProbe {
        self.probe.clone()
    }
    fn pointer_selection(
        &mut self,
        _: &crate::terminal_session::state::PaneRef,
        intent: crate::terminal_input::PointerSelectionIntent,
    ) -> crate::terminal_input::PointerSelectionAdmission {
        self.selections.push(intent);
        crate::terminal_input::PointerSelectionAdmission::Admitted
    }
    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        if self.focus_on_request
            && let ClientAction::FocusPanel(panel) = &action
        {
            self.probe.focus = self
                .probe
                .panes
                .iter()
                .find(|p| p.record.pane().panel() == panel)
                .map(|p| p.record.clone());
        }
        self.calls.borrow_mut().push(Call::Action(action));
        Some(SessionActionDisposition::KeepNavigator)
    }
}

#[test]
fn menu_prefix_split_reads_preserve_underlay_and_capture_opening_and_decision_tails() {
    let opening = b"\x02\x1bOQtail";
    for split in 0..=opening.len() - 4 {
        let mut port = Port::new(MenuAction::Help);
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        view.commit(
            view.decide_action(ClientAction::OpenWorkbenchNavigator),
            SessionActionDisposition::KeepNavigator,
        );
        dispatch_all(&mut dispatcher, &mut view, &mut port, &opening[..split]);
        dispatch_all(&mut dispatcher, &mut view, &mut port, &opening[split..]);
        assert!(port.menu.active(), "split {split}");
        assert_eq!(view.overlay(), Some(Overlay::WorkbenchNavigator));
        assert!(forwarded_bytes(&port.calls).is_empty());
        port.arm();
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"\rignored\x1b[200~paste\x1b[201~",
        );
        assert!(!port.menu.active());
        assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
        assert!(forwarded_bytes(&port.calls).is_empty());
        assert_eq!(
            port.calls
                .borrow()
                .iter()
                .filter(|call| matches!(call, Call::Action(ClientAction::OpenShortcutHelp)))
                .count(),
            1
        );
        dispatcher.publish_hit_map(HitMapBuilder::new().finish());
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"normal");
        assert_eq!(forwarded_bytes(&port.calls), b"normal");
    }
}

#[test]
fn menu_shared_dispatch_applies_resize_layout_pointer_modes_and_shell_exit() {
    for action in [
        MenuAction::Resize,
        MenuAction::Sidebar,
        MenuAction::ReleasePointer,
        MenuAction::Detach,
    ] {
        let mut port = Port::new(action);
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQ");
        port.arm();
        let mut layouts = 0;
        let mut modes = Vec::new();
        let outcome = dispatcher
            .dispatch(
                &mut view,
                b"\rtail",
                &mut port,
                Instant::now(),
                |_, _| {
                    layouts += 1;
                    Ok::<(), io::Error>(())
                },
                &mut |mode| {
                    modes.push(mode);
                    Ok(())
                },
            )
            .unwrap();
        assert!(forwarded_bytes(&port.calls).is_empty());
        match action {
            MenuAction::Resize => assert!(view.resize_mode()),
            MenuAction::Sidebar => assert_eq!(layouts, 1),
            MenuAction::ReleasePointer => {
                assert!(view.pointer_released());
                assert!(modes.contains(&ModeRequest::PointerCapture(false)));
            }
            MenuAction::Detach => assert_eq!(outcome, DispatchOutcome::Exit),
            _ => unreachable!(),
        }
    }
}

#[test]
fn ux_menu_disabled_decision_packets_stay_captured_until_cancel_and_fresh_draw() {
    let mut port = Port::new(MenuAction::WorkspaceDirectory);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQ");
    port.menu
        .refresh_availability(|_| Some(crate::action_menu::MenuUnavailable::DirectoryConfigured));
    port.arm();
    for packet in [b"\rtail".as_slice(), b"\x1b[200~paste\x1b[201~", b"\r"] {
        dispatch_all(&mut dispatcher, &mut view, &mut port, packet);
        assert!(port.menu.active());
        assert_eq!(port.menu.confirmed(), None);
        assert!(forwarded_bytes(&port.calls).is_empty());
        assert!(
            !port
                .calls
                .borrow()
                .iter()
                .any(|call| matches!(call, Call::Action(_)))
        );
    }
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27uignored");
    assert!(!port.menu.active());
    assert!(forwarded_bytes(&port.calls).is_empty());
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"normal");
    assert_eq!(forwarded_bytes(&port.calls), b"normal");
}

#[test]
fn right_menu_does_not_focus_and_policy_chrome_modifiers_and_release_keep_ownership() {
    for (content, policy, mods, opens) in [
        (
            true,
            orchbee_workbench_protocol::PanelRightClickTarget::Orchbee,
            0,
            true,
        ),
        (
            true,
            orchbee_workbench_protocol::PanelRightClickTarget::Panel,
            0,
            false,
        ),
        (
            false,
            orchbee_workbench_protocol::PanelRightClickTarget::Panel,
            0,
            true,
        ),
        (
            true,
            orchbee_workbench_protocol::PanelRightClickTarget::Orchbee,
            4,
            false,
        ),
        (
            true,
            orchbee_workbench_protocol::PanelRightClickTarget::Orchbee,
            16,
            false,
        ),
    ] {
        let mut port = Port::new(MenuAction::Help);
        let pane = crate::terminal_input::legacy_pane_ref();
        port.probe.panes.push(PointerPane {
            record: pane.clone(),
            attached: true,
            controlling: Some(true),
            frame_available: true,
            right_click: policy,
            right_click_reporting: false,
        });
        let mut hits = HitMapBuilder::new();
        let source = crate::action_menu::DrawnMenuSource {
            site: orchbee_workbench_protocol::SiteId::parse("00000000000000000000000000000001")
                .unwrap(),
            revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
        };
        hits.record_action_menu_source(Some(source.clone()));
        hits.record(
            HitLayer::Foreground,
            Rect::new(0, 0, 80, 20),
            HitTarget::Pane {
                pane: pane.clone(),
                content: Rect::new(0, 1, 80, 19),
            },
        );
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatcher.publish_hit_map(hits.finish());
        let row = if content { 2 } else { 1 };
        let press = format!("\x1b[<{};5;{}M", 2 + mods, row);
        dispatch_all(&mut dispatcher, &mut view, &mut port, press.as_bytes());
        assert_eq!(port.menu.active(), opens);
        assert!(
            !port
                .calls
                .borrow()
                .iter()
                .any(|call| matches!(call, Call::Action(ClientAction::FocusPanel(_))))
        );
        if opens {
            assert_eq!(
                port.target,
                Some(MenuTarget::Drawn {
                    source,
                    target: Box::new(MenuTarget::Pane(pane))
                })
            );
            port.cancel_action_menu();
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<2;5;2m");
            assert!(forwarded_bytes(&port.calls).is_empty());
            assert!(dispatcher.menu_release.is_none());
        }
    }
}

#[test]
fn cancel_and_fragmented_captured_paste_never_reach_the_pane() {
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQ");
    port.arm();
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[200~literal");
    port.cancel_action_menu();
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"text\x1b[201~");
    assert!(forwarded_bytes(&port.calls).is_empty());
    assert_eq!(view.overlay(), None);
    assert!(
        !port
            .calls
            .borrow()
            .iter()
            .any(|call| matches!(call, Call::Action(_)))
    );
}

#[test]
fn invalidated_painted_menu_captures_input_until_a_fresh_frame_and_host_lease_precedes_mouse_admission()
 {
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQ");
    port.arm();
    port.cancel_action_menu();
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"stale-enter\r");
    assert!(forwarded_bytes(&port.calls).is_empty());
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"normal");
    assert_eq!(forwarded_bytes(&port.calls), b"normal");
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let mut hits = HitMapBuilder::new();
    hits.record(
        HitLayer::Foreground,
        Rect::new(0, 0, 8, 1),
        HitTarget::ActionMenuButton,
    );
    dispatcher.publish_hit_map(hits.finish());
    assert!(
        dispatcher
            .dispatch(
                &mut view,
                b"\x1b[<0;2;1M",
                &mut port,
                Instant::now(),
                |_, _| Ok(()),
                &mut |request| {
                    if request == ModeRequest::BracketedPaste(true) {
                        Err(io::Error::other("lease refused"))
                    } else {
                        Ok(())
                    }
                }
            )
            .is_err()
    );
    assert!(!port.menu.active());
    assert_eq!(port.target, None);
}

#[test]
fn escape_timeout_and_idle_action_completion_keep_the_already_polled_packet_captured() {
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQ");
    port.arm();
    let now = Instant::now();
    dispatcher
        .dispatch(
            &mut view,
            b"\x1b",
            &mut port,
            now,
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(port.menu.active());
    dispatcher
        .synchronize(
            &mut view,
            &mut port,
            now + Duration::from_secs(1),
            &mut |_| Ok::<(), io::Error>(()),
        )
        .unwrap();
    assert!(!port.menu.active());
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"polled-before-draw\r",
    );
    assert!(forwarded_bytes(&port.calls).is_empty());
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"normal");
    assert_eq!(forwarded_bytes(&port.calls), b"normal");

    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQ");
    port.arm();
    assert!(port.menu.confirm());
    assert_eq!(
        dispatcher
            .complete_action_menu(
                &mut view,
                &mut port,
                Instant::now(),
                &mut |_, _| Ok::<(), io::Error>(()),
                &mut |_| Ok(())
            )
            .unwrap(),
        Some(DispatchOutcome::Continue)
    );
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"polled-before-idle-handoff\r",
    );
    assert!(forwarded_bytes(&port.calls).is_empty());
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"normal");
    assert_eq!(forwarded_bytes(&port.calls), b"normal");
}

#[test]
fn global_menu_respects_existing_pane_selection_pending_link_and_wheel_owners() {
    use crate::terminal_input::{PointerFrameFacts, PointerTab};
    use orchbee_terminal_protocol::{MouseFormat, MouseTracking};
    for owner in ["pane", "selection", "pending", "link", "wheel"] {
        let pane = crate::terminal_input::legacy_pane_ref();
        let mut port = Port::new(MenuAction::Help);
        port.probe = PointerProbe {
            focus: (owner != "pending").then_some(pane.clone()),
            desired: Some(pane.pane().clone()),
            input_ready: owner != "pending",
            selection_ready: true,
            panes: vec![PointerPane {
                record: pane.clone(),
                attached: true,
                controlling: Some(true),
                frame_available: true,
                right_click: Default::default(),
                right_click_reporting: false,
            }],
            tabs: vec![PointerTab {
                id: pane.anchor().tab_id().clone(),
                position: 1,
            }],
            frame: Some(PointerFrameFacts {
                tracking: if owner == "pane" {
                    MouseTracking::Button
                } else {
                    MouseTracking::None
                },
                format: MouseFormat::Sgr,
                cell_pixel: (0, 0),
                alternate_screen: owner == "wheel",
                alternate_scroll: owner == "wheel",
                application_cursor: false,
            }),
            ..PointerProbe::default()
        };
        if owner == "pending" {
            port.focus_on_request = true;
            port.probe.frame = None;
        }
        let mut hits = HitMapBuilder::new();
        let content = Rect::new(0, 0, 10, 3);
        hits.record(
            HitLayer::Foreground,
            content,
            HitTarget::Pane {
                pane: pane.clone(),
                content,
            },
        );
        if owner == "link" {
            hits.record_link_source(content, super::dispatcher_link_source(pane.clone()));
        }
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatcher.publish_hit_map(hits.finish());
        let press = if owner == "link" {
            b"\x1b[<16;2;2M".to_vec()
        } else if owner == "wheel" {
            b"\x1b[<65;2;2M".repeat(20)
        } else {
            b"\x1b[<0;2;2M".to_vec()
        };
        dispatch_all(&mut dispatcher, &mut view, &mut port, &press);
        assert!(!dispatcher.pointer.can_open_action_menu(), "{owner}");
        let selections = port.selections.len();
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQ");
        assert!(!port.menu.active(), "{owner}");
        assert_eq!(port.target, None, "{owner}");
        assert_eq!(
            port.selections.len(),
            selections,
            "{owner}: existing selection must not be cancelled"
        );
        assert!(
            !dispatcher.pointer.can_open_action_menu(),
            "{owner}: owner retained"
        );
        if owner == "pane" {
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;2;2m");
            assert!(forwarded_bytes(&port.calls).ends_with(b"\x1b[<0;2;2m"));
            assert!(dispatcher.pointer.can_open_action_menu());
        } else if owner == "pending" || owner == "selection" {
            port.probe.input_ready = true;
            port.probe.frame = Some(PointerFrameFacts {
                tracking: MouseTracking::None,
                format: MouseFormat::Sgr,
                cell_pixel: (0, 0),
                alternate_screen: false,
                alternate_scroll: false,
                application_cursor: false,
            });
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;2;2m");
            assert!(
                matches!(
                    port.selections.last(),
                    Some(crate::terminal_input::PointerSelectionIntent::End { .. })
                ),
                "{owner}"
            );
            assert!(dispatcher.pointer.can_open_action_menu());
        } else if owner == "link" {
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<16;2;2m");
            assert!(dispatcher.pointer.can_open_action_menu());
        } else {
            for _ in 0..30 {
                dispatcher
                    .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
                        Ok::<(), io::Error>(())
                    })
                    .unwrap();
            }
            assert!(dispatcher.pointer.can_open_action_menu());
            assert_eq!(forwarded_bytes(&port.calls), b"\x1b[B".repeat(60));
        }
    }
}

#[test]
fn lost_menu_release_cannot_swallow_a_later_program_owned_right_release() {
    use crate::terminal_input::{PointerFrameFacts, PointerTab};
    use orchbee_terminal_protocol::{MouseFormat, MouseTracking};
    for lost_focus in [false, true] {
        let pane = crate::terminal_input::legacy_pane_ref();
        let mut port = Port::new(MenuAction::Help);
        port.probe = PointerProbe {
            focus: Some(pane.clone()),
            desired: Some(pane.pane().clone()),
            input_ready: true,
            selection_ready: true,
            panes: vec![PointerPane {
                record: pane.clone(),
                attached: true,
                controlling: Some(true),
                frame_available: true,
                right_click: Default::default(),
                right_click_reporting: false,
            }],
            tabs: vec![PointerTab {
                id: pane.anchor().tab_id().clone(),
                position: 1,
            }],
            frame: Some(PointerFrameFacts {
                tracking: MouseTracking::Button,
                format: MouseFormat::Sgr,
                cell_pixel: (0, 0),
                alternate_screen: false,
                alternate_scroll: false,
                application_cursor: false,
            }),
            ..PointerProbe::default()
        };
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        let source = crate::action_menu::DrawnMenuSource {
            site: orchbee_workbench_protocol::SiteId::parse("00000000000000000000000000000001")
                .unwrap(),
            revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
        };
        let publish = |dispatcher: &mut InputDispatcher| {
            let mut hits = HitMapBuilder::new();
            hits.record_action_menu_source(Some(source.clone()));
            hits.record(
                HitLayer::Foreground,
                Rect::new(0, 0, 10, 3),
                HitTarget::Pane {
                    pane: pane.clone(),
                    content: Rect::new(0, 0, 10, 3),
                },
            );
            dispatcher.publish_hit_map(hits.finish());
        };
        publish(&mut dispatcher);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<2;2;2M");
        assert!(port.menu.active());
        port.capture_action_menu_key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
        dispatcher
            .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
                Ok::<(), io::Error>(())
            })
            .unwrap();
        publish(&mut dispatcher);
        assert_eq!(
            dispatcher.menu_release,
            Some(super::super::super::MouseButton::Right)
        );
        if lost_focus {
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[O\x1b[I");
            assert_eq!(dispatcher.menu_release, None);
        }
        port.probe.panes[0].right_click = orchbee_workbench_protocol::PanelRightClickTarget::Panel;
        port.probe.panes[0].right_click_reporting = true;
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<2;2;2M");
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<2;2;2m");
        assert_eq!(forwarded_bytes(&port.calls), b"\x1b[<2;2;2M\x1b[<2;2;2m");
        assert!(dispatcher.pointer.can_open_action_menu());
    }
}
