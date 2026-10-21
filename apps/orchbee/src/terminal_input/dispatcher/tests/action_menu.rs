use super::*;
use crate::action_menu::{ActionMenu, MenuAction, MenuDraw, MenuTarget};
use crate::client_view::{KeyEvent, Overlay};
use crate::terminal_input::{PointerPane, PointerProbe};
use crate::tui_pointer::{
    HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, PrimaryContract,
};
use ratatui::layout::Rect;

mod mouse_controls;
mod sidebar_overlap;
mod sidebar_prefix;
mod tab_drag;

struct Port {
    menu: ActionMenu,
    actions: Vec<MenuAction>,
    calls: Rc<RefCell<Vec<Call>>>,
    target: Option<MenuTarget>,
    probe: PointerProbe,
    selections: Vec<crate::terminal_input::PointerSelectionIntent>,
    focus_on_request: bool,
    tab_create_available: bool,
    drag: Option<crate::tui_pointer::TabDragSource>,
    arrangements: Vec<orchbee_workbench_protocol::TabConfiguration>,
    settings_capture: bool,
}

fn sidebar_context() -> crate::terminal_session::NativeContextTarget {
    use orchbee_workbench_protocol::*;
    crate::terminal_session::NativeContextTarget {
        site_id: SiteId::parse(&format!("{:032x}", 1)).unwrap(),
        location: crate::terminal_session::NativeContextLocation::Workspace {
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
            project_id: Some(
                orchbee_workbench_protocol::WorkbenchProjectId::parse(&format!("{:032x}", 2))
                    .unwrap(),
            ),
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

fn sidebar_view_with_agent() -> (ClientView, crate::machine_picker::SidebarTarget) {
    let mut view = sidebar_view();
    let mut agent = crate::machine_agents::tests::row(23);
    agent.target.context = sidebar_context();
    let mut checkpoint = crate::machine_agents::tests::checkpoint(vec![agent])
        .as_ref()
        .clone();
    checkpoint.contexts = vec![sidebar_context()].into();
    view.machines.observe_agents(vec![(
        crate::machine_picker::Choice::Local,
        crate::machine_agents::Observation::Ready(std::sync::Arc::new(checkpoint)),
    )]);
    let target = crate::machine_picker::SidebarTarget::Context {
        machine: crate::machine_picker::Choice::Local,
        target: sidebar_context(),
    };
    (view, target)
}

fn rendered_sidebar_map(view: &ClientView) -> crate::tui_pointer::FrameHitMap {
    rendered_sidebar_map_at(view, 120)
}

fn rendered_sidebar_map_at(view: &ClientView, width: u16) -> crate::tui_pointer::FrameHitMap {
    rendered_sidebar_map_with(view, width, HitMapBuilder::new())
}

fn rendered_sidebar_map_with(
    view: &ClientView,
    width: u16,
    mut hits: HitMapBuilder,
) -> crate::tui_pointer::FrameHitMap {
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, 40)).unwrap();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(
                frame,
                view,
                crate::localization::Locale::EnUs,
                None,
                &crate::tui_theme::Theme::terminal_native(),
                &mut hits,
            );
        })
        .unwrap();
    hits.record_action_menu_source(Some(crate::action_menu::DrawnMenuSource {
        site: sidebar_context().site_id,
        revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
    }));
    hits.finish()
}

fn sidebar_primary_cell(
    map: &crate::tui_pointer::FrameHitMap,
    target: &crate::machine_picker::SidebarTarget,
) -> (u16, u16) {
    for y in 0..40 {
        for x in 0..30 {
            let matches = match (map.hit_target_at(x, y), target) {
                (Some(HitTarget::MachineProjectRow { target: row, .. }), _) => row == target,
                (
                    Some(HitTarget::MachineWorkspaceRow { selected, .. }),
                    crate::machine_picker::SidebarTarget::Context { machine, target },
                ) => {
                    &selected.machine == machine
                        && matches!(&selected.context, Some(crate::terminal_session::NativeStartupTarget::Context(row)) if row == target)
                }
                _ => false,
            };
            if matches {
                return (x, y);
            }
        }
    }
    panic!("rendered primary sidebar row");
}

fn disclosure_packet(rect: Rect, twice: bool) -> Vec<u8> {
    let press = format!("\x1b[<0;{};{}M", rect.x + 1, rect.y + 1);
    let release = format!("\x1b[<0;{};{}m", rect.x + 1, rect.y + 1);
    format!(
        "{press}{}{release}{tail}",
        if twice { press.as_str() } else { "" },
        tail = "tail"
    )
    .into_bytes()
}

fn disclosure_cell(
    map: &crate::tui_pointer::FrameHitMap,
    target: &crate::machine_picker::SidebarTarget,
) -> Rect {
    let expected = HitTarget::SidebarDisclosure(target.clone());
    for y in 0..40 {
        for x in 0..30 {
            if map.hit_target_at(x, y) == Some(&expected) {
                return Rect::new(x, y, 1, 1);
            }
        }
    }
    panic!("rendered disclosure cell");
}

fn sidebar_project_target() -> crate::machine_picker::SidebarTarget {
    let target = sidebar_context();
    let crate::terminal_session::NativeContextLocation::Workspace { .. } = target.location else {
        unreachable!()
    };
    crate::machine_picker::SidebarTarget::Project {
        machine: crate::machine_picker::Choice::Local,
        site_id: target.site_id,
        project_id: orchbee_workbench_protocol::WorkbenchProjectId::parse(&format!("{:032x}", 2))
            .unwrap(),
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
fn sidebar_project_pointer_toggles_without_taking_keyboard_input() {
    let mut view = sidebar_view();
    let target = sidebar_project_target();
    let expanded = view.machines.sidebar_project_expanded(&target);
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let map = rendered_sidebar_map(&view);
    let disclosure = disclosure_cell(&map, &target);
    dispatcher.publish_hit_map(map);
    let mut modes = Vec::new();
    dispatcher
        .dispatch(
            &mut view,
            &disclosure_packet(disclosure, false),
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |mode| {
                modes.push(mode);
                Ok(())
            },
        )
        .unwrap();
    assert!(!view.machines.sidebar_navigation_active());
    assert_eq!(view.machines.sidebar_project_expanded(&target), !expanded);
    assert!(view.machines.request.is_none());
    assert!(!modes.contains(&ModeRequest::BracketedPaste(true)));
    assert!(forwarded_bytes(&port.calls).is_empty());
}

#[test]
fn context_disclosure_is_display_only_preserves_other_focus_and_fences_the_packet() {
    for other_focus in [false, true] {
        let (mut view, target) = sidebar_view_with_agent();
        if other_focus {
            assert!(view.machines.enter_sidebar_navigation());
            assert!(
                view.machines
                    .focus_sidebar_target(&sidebar_project_target())
            );
        }
        let before_focus = view.machines.sidebar_focused_target().cloned();
        let before_context = view.machines.current_context.clone();
        let before_expanded = view.machines.sidebar_context_agents_expanded(&target);
        assert!(view.machines.sidebar_target_exists(&target));
        assert!(view.machines.sidebar_context_has_agents(&target));
        let mut port = Port::new(MenuAction::Help);
        let mut dispatcher = InputDispatcher::default();
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        // Establish the dispatcher's current keyboard owner before publishing the
        // frame. Entering sidebar navigation invalidates the preceding hit map in
        // production; the click must resolve against the fresh frame after that edge.
        dispatcher
            .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
                Ok::<(), io::Error>(())
            })
            .unwrap();
        let map = rendered_sidebar_map(&view);
        let disclosure = disclosure_cell(&map, &target);
        dispatcher.publish_hit_map(map);
        let mut modes = Vec::new();
        dispatcher
            .dispatch(
                &mut view,
                &disclosure_packet(disclosure, true),
                &mut port,
                Instant::now(),
                |_, _| Ok::<bool, io::Error>(false),
                &mut |mode| {
                    modes.push(mode);
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(
            view.machines.sidebar_context_agents_expanded(&target),
            !before_expanded,
            "other_focus={other_focus}"
        );
        assert_eq!(
            view.machines.sidebar_focused_target(),
            before_focus.as_ref()
        );
        assert_eq!(view.machines.current_context, before_context);
        assert!(view.machines.request.is_none());
        assert!(
            modes
                .iter()
                .all(|mode| *mode != ModeRequest::BracketedPaste(true)),
            "Context disclosure must not acquire terminal keyboard ownership"
        );
        assert!(forwarded_bytes(&port.calls).is_empty());
    }
}

#[test]
fn stale_rendered_context_disclosure_rejects_mutation_but_still_fences_packet_tail() {
    let (mut view, target) = sidebar_view_with_agent();
    let map = rendered_sidebar_map(&view);
    let disclosure = disclosure_cell(&map, &target);
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher.publish_hit_map(map);
    view.machines.observe_agents(vec![(
        crate::machine_picker::Choice::Local,
        crate::machine_agents::Observation::Unavailable,
    )]);

    let mut modes = Vec::new();
    dispatcher
        .dispatch(
            &mut view,
            &disclosure_packet(disclosure, false),
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |mode| {
                modes.push(mode);
                Ok(())
            },
        )
        .unwrap();
    assert!(!view.machines.sidebar_context_agents_expanded(&target));
    assert!(view.machines.request.is_none());
    assert!(
        modes
            .iter()
            .all(|mode| *mode != ModeRequest::BracketedPaste(true))
    );
    assert!(forwarded_bytes(&port.calls).is_empty());
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
}

#[test]
fn sidebar_keyboard_and_row_menu_share_drawn_identity_and_cancel_returns_to_navigation() {
    for project in [false, true] {
        for keyboard in [false, true] {
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
            let map = rendered_sidebar_map(&view);
            let (x, y) = sidebar_primary_cell(&map, &target);
            dispatcher.publish_hit_map(map);
            let before = view.machines.sidebar_focused_target().cloned();
            let entry = if keyboard {
                b"\x1bOQtail".to_vec()
            } else {
                format!("\x1b[<2;{};{}Mtail", x + 1, y + 1).into_bytes()
            };
            dispatch_all(&mut dispatcher, &mut view, &mut port, &entry);
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
                        crate::machine_picker::SidebarTarget::Agent { .. } => unreachable!(),
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
                    |_, _| Ok::<bool, io::Error>(false),
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
            assert!(
                modes
                    .iter()
                    .all(|mode| !matches!(mode, ModeRequest::BracketedPaste(_))),
                "the child menu returns to the same Sidebar paste lease without reacquiring or releasing it"
            );
            assert_eq!(
                modes,
                [ModeRequest::PointerCapture(
                    PointerCaptureRequest::ButtonOnly
                )],
                "returning to Sidebar releases only the menu's scoped motion policy"
            );
        }
    }
}

#[test]
fn sidebar_primary_click_keeps_row_activation_separate_from_context_menus() {
    for project in [false, true] {
        let mut view = sidebar_view();
        let mut port = Port::new(MenuAction::Help);
        let mut dispatcher = InputDispatcher::default();
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        let target = if project {
            sidebar_project_target()
        } else {
            crate::machine_picker::SidebarTarget::Context {
                machine: crate::machine_picker::Choice::Local,
                target: sidebar_context(),
            }
        };
        let map = rendered_sidebar_map(&view);
        let (x, y) = sidebar_primary_cell(&map, &target);
        dispatcher.publish_hit_map(map);
        let input = format!("\x1b[<0;{x};{y}M\x1b[<0;{x};{y}m", x = x + 1, y = y + 1,);
        dispatch_all(&mut dispatcher, &mut view, &mut port, input.as_bytes());
        assert!(!port.menu.active());
        assert!(port.target.is_none());
        assert!(forwarded_bytes(&port.calls).is_empty());
    }
}

#[test]
fn workspace_close_is_pointer_revealed_and_requests_the_existing_confirmation() {
    let mut view = sidebar_view();
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let target = crate::machine_picker::SidebarTarget::Context {
        machine: crate::machine_picker::Choice::Local,
        target: sidebar_context(),
    };
    let close = HitTarget::SidebarClose(target.clone());
    let close_cell = |map: &crate::tui_pointer::FrameHitMap| {
        (0..40).find_map(|y| {
            (0..30)
                .find(|x| map.hit_target_at(*x, y) == Some(&close))
                .map(|x| (x, y))
        })
    };
    let paint = |dispatcher: &InputDispatcher, view: &ClientView| {
        rendered_sidebar_map_with(
            view,
            120,
            HitMapBuilder::with_pointer(dispatcher.pointer_projection()),
        )
    };
    let motion = |x: u16, y: u16| format!("\x1b[<35;{};{}M", x + 1, y + 1);

    let map = paint(&dispatcher, &view);
    assert_eq!(
        close_cell(&map),
        None,
        "close publishes no hidden target at rest"
    );
    let (x, y) = sidebar_primary_cell(&map, &target);
    dispatcher.publish_hit_map(map);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        motion(x, y).as_bytes(),
    );
    let map = paint(&dispatcher, &view);
    let (close_x, close_y) = close_cell(&map).expect("row hover reveals close");
    assert_eq!(close_y, y, "close shares the identity line");
    dispatcher.publish_hit_map(map);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        motion(close_x, close_y).as_bytes(),
    );
    let map = paint(&dispatcher, &view);
    assert_eq!(
        close_cell(&map),
        Some((close_x, close_y)),
        "close stays revealed under itself"
    );
    dispatcher.publish_hit_map(map);
    let click = format!(
        "\x1b[<0;{x};{y}M\x1b[<0;{x};{y}m",
        x = close_x + 1,
        y = close_y + 1
    );
    dispatch_all(&mut dispatcher, &mut view, &mut port, click.as_bytes());
    let crate::terminal_session::NativeContextLocation::Workspace { workspace_id, .. } =
        sidebar_context().location
    else {
        unreachable!()
    };
    let actions = port
        .calls
        .borrow()
        .iter()
        .filter_map(|call| match call {
            Call::Action(action) => Some(action.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actions,
        [ClientAction::RequestWorkspaceCloseAt(workspace_id)]
    );
    assert!(
        view.machines.request.is_none(),
        "close never activates the row"
    );
    assert!(forwarded_bytes(&port.calls).is_empty());
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
        assert_eq!(view.help_page(), 9);
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
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, 30)).unwrap();
        let mut hits = HitMapBuilder::with_pointer(dispatcher.pointer_projection());
        terminal
            .draw(|frame| {
                crate::tui_render::render_terminal_shell(
                    frame,
                    &view,
                    crate::localization::Locale::EnUs,
                    None,
                    &crate::tui_theme::Theme::terminal_native(),
                    &mut hits,
                );
            })
            .unwrap();
        let source = dispatcher.publish_hit_map_with_help(hits.finish());
        assert!(matches!(
            source,
            Some(crate::client_view::help::HelpDrawSource::Presented { .. })
        ));
        view.settle_help_draw(source);
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
            tab_create_available: false,
            drag: None,
            arrangements: Vec::new(),
            settings_capture: false,
        }
    }
    fn arm(&mut self) {
        let (token, visible_rows, selected, result_actions) = {
            let view = self.menu.view().unwrap();
            (
                view.token,
                view.actions.len(),
                view.selected,
                view.result_actions.to_vec(),
            )
        };
        assert!(self.menu.presented(MenuDraw {
            token,
            first: 0,
            visible_rows,
            selected,
            result_actions,
        }));
    }
}
impl TerminalInputPort for Port {
    fn native_capture_claims(&self) -> crate::tui_composition::NativeCaptureClaims {
        crate::tui_composition::NativeCaptureClaims::default()
            .with_settings(self.settings_capture)
            .with_action_menu(self.menu.input_context().is_some())
    }
    fn native_capture_context(
        &self,
        owner: crate::tui_composition::NativeCaptureOwner,
    ) -> Option<crate::terminal_session::NativeCaptureContext> {
        (owner == crate::tui_composition::NativeCaptureOwner::ActionMenu)
            .then(|| self.menu.input_context())
            .flatten()
            .map(crate::terminal_session::NativeCaptureContext::ActionMenu)
    }
    fn tab_drag_source(&mut self) -> Option<crate::tui_pointer::TabDragSource> {
        self.drag.clone()
    }
    fn commit_tab_drag(
        &mut self,
        _: &crate::action_menu::DrawnTabCreate,
        operation: orchbee_workbench_protocol::TabConfiguration,
    ) {
        self.arrangements.push(operation);
    }
    fn tab_create_action(
        &mut self,
        _: &crate::action_menu::DrawnTabCreate,
    ) -> Option<ClientAction> {
        self.tab_create_available.then_some(ClientAction::CreateTab)
    }
    fn forward(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), super::super::super::TerminalInputUnavailable> {
        self.calls.borrow_mut().push(Call::Forward(bytes.to_vec()));
        Ok(())
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
    fn capture_action_menu_key_for(
        &mut self,
        expected: crate::action_menu::MenuInputContext,
        event: KeyEvent,
    ) -> bool {
        if self.menu.input_context() != Some(expected) {
            return false;
        }
        if expected.readiness() == crate::action_menu::MenuInputReadiness::Recovery {
            if event.kind != crate::client_view::KeyKind::Release
                && crate::client_view::Binding::new(
                    crate::client_view::KeyCode::Esc,
                    crate::client_view::Modifiers::NONE,
                )
                .matches(event)
            {
                self.menu.cancel();
            }
            return true;
        }
        self.menu.key(event);
        true
    }
    fn cancel_action_menu(&mut self) {
        self.menu.cancel();
    }
    fn scroll_action_menu(
        &mut self,
        token: crate::action_menu::MenuToken,
        up: bool,
        lines: usize,
    ) -> Option<crate::action_menu::MenuToken> {
        self.menu.scroll(token, up, lines)
    }
    fn hover_action_menu_row(
        &mut self,
        source: &MenuTarget,
        token: crate::action_menu::MenuToken,
        row: usize,
    ) -> bool {
        self.menu
            .view()
            .is_some_and(|view| view.source == source && view.token == token)
            && self.menu.hover(token, row)
    }
    fn activate_action_menu_row(
        &mut self,
        source: &MenuTarget,
        token: crate::action_menu::MenuToken,
        row: usize,
    ) {
        if self
            .menu
            .view()
            .is_some_and(|view| view.source == source && view.token == token)
        {
            self.menu.click(token, row);
        }
    }
    fn activate_action_menu_control(
        &mut self,
        source: &MenuTarget,
        token: crate::action_menu::MenuToken,
        control: crate::action_menu::MenuControl,
    ) {
        if self
            .menu
            .view()
            .is_some_and(|view| view.source == source && view.token == token)
            && control == crate::action_menu::MenuControl::Close
        {
            self.menu.cancel();
        }
    }
    fn take_action_menu_action(&mut self, _: bool) -> Option<ClientAction> {
        self.menu.take().and_then(|action| action.client_action())
    }
    fn pointer_probe(&mut self) -> PointerProbe {
        let mut probe = self.probe.clone();
        probe.input_captured |= self.menu.active() || self.settings_capture;
        probe
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

fn paint_menu(dispatcher: &mut InputDispatcher, port: &mut Port) {
    let menu = port.menu.view().unwrap();
    let token = menu.token;
    let source = (*menu.source).clone();
    let rows = menu.actions.len();
    let selected = menu.selected;
    let mut hits = HitMapBuilder::new();
    hits.start_action_menu();
    hits.record(
        HitLayer::OverlaySurface,
        Rect::new(10, 4, 30, 8),
        HitTarget::OverlaySurface,
    );
    for row in 0..rows {
        let rect = Rect::new(11, 5 + u16::try_from(row).unwrap(), 28, 1);
        hits.record_interaction(
            HitLayer::Overlay,
            rect,
            rect,
            InteractionTarget::ActionMenuRow {
                source: source.clone(),
                token,
                row,
            },
            if menu.unavailable[row].is_none() {
                InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
            } else {
                InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
            },
        );
    }
    let result_actions = port.menu.view().unwrap().result_actions.to_vec();
    assert!(port.menu.presented(MenuDraw {
        token,
        first: 0,
        visible_rows: rows,
        selected,
        result_actions,
    }));
    dispatcher.publish_hit_map(hits.finish());
}

#[test]
fn globally_admitted_menu_hover_tracks_motion_without_activating_or_forwarding() {
    let mut port = Port::new(MenuAction::Help);
    port.actions = vec![
        MenuAction::SplitPanel(crate::client_view::PanelDirection::Right),
        MenuAction::Help,
        MenuAction::Detach,
    ];
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatcher.settle_pointer_capture(&mut view, &mut port, PointerCaptureRequest::AnyMotion);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQ");
    port.menu.refresh_availability(|action| {
        (action == MenuAction::Detach).then_some(crate::action_menu::MenuUnavailable::OperationBusy)
    });
    paint_menu(&mut dispatcher, &mut port);
    for bytes in [b"\x1b[<35;12;".as_slice(), b"7M\x1b[<35;12;8M"] {
        dispatch_all(&mut dispatcher, &mut view, &mut port, bytes);
    }
    assert_eq!(port.menu.view().unwrap().selected, 1);
    assert_eq!(port.menu.confirmed(), None);
    for bytes in [
        b"\x1b[<39;12;6M".as_slice(), // Shift bypass
        b"\x1b[<51;12;6M",            // Ctrl
        b"\x1b[<32;12;6M",            // held-button drag
        b"\x1b[<35;11;5M",            // frame
        b"\x1b[<35;1;1M",             // outside
    ] {
        dispatch_all(&mut dispatcher, &mut view, &mut port, bytes);
        assert_eq!(port.menu.view().unwrap().selected, 1);
        assert_eq!(port.menu.confirmed(), None);
    }
    paint_menu(&mut dispatcher, &mut port);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;12;8M\x1b[<0;12;8mtail",
    );
    assert!(
        port.menu.active(),
        "a painted disabled row still cannot execute"
    );
    assert!(port.calls.borrow().is_empty());
    paint_menu(&mut dispatcher, &mut port);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;12;7M\x1b[<0;12;7mtail",
    );
    assert!(!port.menu.active());
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
    assert_eq!(
        *port.calls.borrow(),
        [Call::Action(ClientAction::OpenShortcutHelp)]
    );
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;12;7m");
    assert!(forwarded_bytes(&port.calls).is_empty());
}

#[test]
fn menu_motion_is_scoped_and_never_rearms_released_or_replaces_admitted_global_capture() {
    for tracking in [
        PointerCaptureRequest::Release,
        PointerCaptureRequest::ButtonOnly,
        PointerCaptureRequest::AnyMotion,
    ] {
        let capture = tracking != PointerCaptureRequest::Release;
        for closing in ["escape", "outside", "invalidate", "handoff", "release"] {
            let mut port = Port::new(MenuAction::Help);
            let mut dispatcher = InputDispatcher::default();
            let mut view = ClientView::new(120);
            dispatcher.settle_pointer_capture(&mut view, &mut port, tracking);
            let mut modes = Vec::new();
            dispatcher
                .dispatch(
                    &mut view,
                    b"\x02\x1bOQ",
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<bool, io::Error>(false),
                    &mut |mode| {
                        modes.push(mode);
                        Ok(())
                    },
                )
                .unwrap();
            let mut expected = if tracking == PointerCaptureRequest::ButtonOnly {
                vec![PointerCaptureRequest::MenuMotion]
            } else {
                Vec::new()
            };
            assert!(
                !modes
                    .iter()
                    .any(|mode| matches!(mode, ModeRequest::PointerAdmissionQuery(_)))
            );
            paint_menu(&mut dispatcher, &mut port);
            let bytes: &[u8] = match closing {
                "outside" if capture => b"\x1b[<0;1;1Mtail",
                "handoff" => b"\rtail",
                "invalidate" => {
                    port.menu.cancel();
                    b"tail"
                }
                "release" => {
                    // Production settles this only after the lifecycle's release succeeds.
                    dispatcher.set_pointer_capture(&mut view, &mut port, false);
                    b"tail"
                }
                _ => b"\x1b[27utail",
            };
            dispatcher
                .dispatch(
                    &mut view,
                    bytes,
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<bool, io::Error>(false),
                    &mut |mode| {
                        modes.push(mode);
                        Ok(())
                    },
                )
                .unwrap();
            assert!(!port.menu.active(), "{capture} {closing}");
            if tracking == PointerCaptureRequest::ButtonOnly && closing != "release" {
                expected.push(PointerCaptureRequest::ButtonOnly);
            }
            let actual = modes
                .iter()
                .filter_map(|mode| match mode {
                    ModeRequest::PointerCapture(request) => Some(*request),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "{tracking:?} {closing}");
            assert!(
                !modes
                    .iter()
                    .any(|mode| matches!(mode, ModeRequest::PointerAdmissionQuery(_)))
            );
            assert!(forwarded_bytes(&port.calls).is_empty());
        }
    }
}

#[test]
fn covered_menu_returns_its_motion_lease_until_it_owns_input_again() {
    for cover in ["settings", "machine"] {
        let mut port = Port::new(MenuAction::Help);
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        assert!(port.menu.open(String::new(), port.actions.clone(), None));
        let mut requests = Vec::new();
        let mut mode = |mode| {
            if let ModeRequest::PointerCapture(request) = mode {
                requests.push(request);
            }
            Ok::<_, ()>(())
        };
        dispatcher
            .synchronize(&mut view, &mut port, Instant::now(), &mut mode)
            .unwrap();
        port.settings_capture = cover == "settings";
        view.machines.open = cover == "machine";
        dispatcher
            .synchronize(&mut view, &mut port, Instant::now(), &mut mode)
            .unwrap();
        assert!(port.menu.active(), "a covered menu remains retained");
        assert_eq!(
            dispatcher.pointer_capture,
            PointerCaptureRequest::ButtonOnly
        );
        port.settings_capture = false;
        view.machines.open = false;
        dispatcher
            .synchronize(&mut view, &mut port, Instant::now(), &mut mode)
            .unwrap();
        port.menu.cancel();
        dispatcher
            .synchronize(&mut view, &mut port, Instant::now(), &mut mode)
            .unwrap();
        assert_eq!(
            requests,
            [
                PointerCaptureRequest::MenuMotion,
                PointerCaptureRequest::ButtonOnly,
                PointerCaptureRequest::MenuMotion,
                PointerCaptureRequest::ButtonOnly,
            ],
            "{cover}"
        );
        assert!(port.calls.borrow().is_empty());
    }
}

#[test]
fn failed_menu_motion_write_never_publishes_or_retries_the_lease() {
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let result = dispatcher.dispatch(
        &mut view,
        b"\x02\x1bOQ",
        &mut port,
        Instant::now(),
        |_, _| Ok::<_, ()>(false),
        &mut |mode| {
            if mode == ModeRequest::PointerCapture(PointerCaptureRequest::MenuMotion) {
                Err(())
            } else {
                Ok(())
            }
        },
    );
    assert_eq!(result, Err(()));
    assert_eq!(
        dispatcher.pointer_capture,
        PointerCaptureRequest::ButtonOnly
    );
    assert!(dispatcher.pointer_transition_failed);
    let mut retries = 0;
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |mode| {
            if matches!(mode, ModeRequest::PointerCapture(_)) {
                retries += 1;
            }
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(retries, 0, "failed mode effects require lifecycle teardown");
}

#[test]
fn a_late_successful_global_probe_supersedes_the_menu_lease_and_survives_menu_exit() {
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let now = Instant::now();
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher
        .begin_pointer_admission(now, &mut |_| Ok::<_, ()>(()))
        .unwrap();
    dispatcher
        .dispatch(
            &mut view,
            b"\x02\x1bOQ",
            &mut port,
            now,
            |_, _| Ok::<_, ()>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(
        dispatcher.pointer_capture,
        PointerCaptureRequest::MenuMotion
    );
    dispatcher
        .dispatch(
            &mut view,
            b"\x1b[?1003;2$y",
            &mut port,
            now + Duration::from_millis(1),
            |_, _| Ok::<_, ()>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(dispatcher.pointer_capture, PointerCaptureRequest::AnyMotion);
    paint_menu(&mut dispatcher, &mut port);
    dispatcher
        .dispatch(
            &mut view,
            b"\x1b[27u",
            &mut port,
            now + Duration::from_millis(2),
            |_, _| Ok::<_, ()>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(!port.menu.active());
    assert_eq!(dispatcher.pointer_capture, PointerCaptureRequest::AnyMotion);
}

#[test]
fn tab_create_click_uses_the_keyboard_action_and_fences_release_packet_tails_even_on_refusal() {
    for available in [false, true] {
        let mut port = Port::new(MenuAction::NewTab);
        port.tab_create_available = available;
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        let context = sidebar_context();
        let mut hits = HitMapBuilder::new();
        hits.record_interaction(
            HitLayer::Foreground,
            Rect::new(10, 0, 3, 1),
            Rect::new(10, 0, 3, 1),
            InteractionTarget::TabCreate(crate::action_menu::DrawnTabCreate {
                source: crate::action_menu::DrawnMenuSource {
                    site: context.site_id,
                    revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
                },
                location: context.location,
            }),
            InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
        );
        dispatcher.publish_hit_map(hits.finish());
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;12;1M");
        assert!(
            !port
                .calls
                .borrow()
                .iter()
                .any(|call| matches!(call, Call::Action(_)))
        );
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"\x1b[<0;12;1mtail\x1b[200~paste\x1b[201~",
        );
        assert!(forwarded_bytes(&port.calls).is_empty());
        assert_eq!(
            port.calls
                .borrow()
                .iter()
                .filter(|call| matches!(call, Call::Action(ClientAction::CreateTab)))
                .count(),
            usize::from(available)
        );
        dispatcher.publish_hit_map(HitMapBuilder::new().finish());
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"next packet");
        assert_eq!(forwarded_bytes(&port.calls), b"next packet");
    }
}

#[test]
fn tab_create_focus_loss_cancels_action_and_fences_its_late_release_packet() {
    let mut port = Port::new(MenuAction::NewTab);
    port.tab_create_available = true;
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let context = sidebar_context();
    let mut hits = HitMapBuilder::new();
    hits.record_interaction(
        HitLayer::Foreground,
        Rect::new(10, 0, 3, 1),
        Rect::new(10, 0, 3, 1),
        InteractionTarget::TabCreate(crate::action_menu::DrawnTabCreate {
            source: crate::action_menu::DrawnMenuSource {
                site: context.site_id,
                revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
            },
            location: context.location,
        }),
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
    );
    dispatcher.publish_hit_map(hits.finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;12;1M");
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[O\x1b[I");
    port.calls.borrow_mut().clear();
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;12;1mtail\x1b[200~paste\x1b[201~\x1b[",
    );
    assert!(
        !port
            .calls
            .borrow()
            .iter()
            .any(|call| matches!(call, Call::Action(ClientAction::CreateTab)))
    );
    assert!(forwarded_bytes(&port.calls).is_empty());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"A");
    assert!(forwarded_bytes(&port.calls).is_empty());
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"fresh");
    assert_eq!(forwarded_bytes(&port.calls), b"fresh");
}

#[test]
fn capturing_navigator_prefix_switches_to_menu_without_forwarding() {
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02w");
    assert!(view.navigator_active());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQtail");
    assert!(
        !view.navigator_active(),
        "the global menu prefix replaces Navigator"
    );
    assert!(
        port.menu.active(),
        "the menu owner captures the complete packet"
    );
    assert!(forwarded_bytes(&port.calls).is_empty());
}

#[test]
fn menu_prefix_split_reads_replace_help_and_capture_opening_and_decision_tails() {
    let opening = b"\x02\x1bOQtail";
    for split in 0..=opening.len() - 4 {
        let mut port = Port::new(MenuAction::Help);
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        view.commit(
            view.decide_action(ClientAction::OpenShortcutHelp),
            SessionActionDisposition::KeepNavigator,
        );
        dispatch_all(&mut dispatcher, &mut view, &mut port, &opening[..split]);
        dispatch_all(&mut dispatcher, &mut view, &mut port, &opening[split..]);
        assert!(port.menu.active(), "split {split}");
        assert_eq!(
            view.overlay(),
            None,
            "the global menu prefix replaces the Help overlay at split {split}"
        );
        assert!(forwarded_bytes(&port.calls).is_empty());
        port.arm();
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"\rignored\x1b[200~paste\x1b[201~",
        );
        assert!(!port.menu.active());
        assert_eq!(
            view.overlay(),
            Some(Overlay::ShortcutHelp),
            "choosing Help opens it after the menu has replaced the prior overlay"
        );
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
        assert_eq!(view.help_filter(), "normal");
        assert!(forwarded_bytes(&port.calls).is_empty());
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
        let mut port = Port::new(action.clone());
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
                    Ok::<bool, io::Error>(false)
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
                assert!(
                    modes.contains(&ModeRequest::PointerCapture(PointerCaptureRequest::Release))
                );
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
fn searchable_menu_escape_after_query_edit_cancels_and_restores_terminal_forwarding() {
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(80);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQ");
    assert!(port.menu.active());
    port.arm();

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"x");
    assert_eq!(port.menu.query(), Some("x"));
    assert_eq!(
        port.menu.input_context().unwrap().readiness(),
        crate::action_menu::MenuInputReadiness::Continuation
    );
    assert!(forwarded_bytes(&port.calls).is_empty());

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27u");
    assert!(!port.menu.active(), "Esc cancels before the next menu draw");
    assert!(
        forwarded_bytes(&port.calls).is_empty(),
        "Esc stays menu-owned"
    );

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
    let source = view.machines.sidebar_source();
    let target = view
        .machines
        .drawn_sidebar_fixed(&source, crate::machine_picker::SidebarFixedAction::Settings);
    let mut hits = HitMapBuilder::new();
    hits.record_interaction(
        HitLayer::Foreground,
        Rect::new(0, 0, 8, 1),
        Rect::new(0, 0, 8, 1),
        InteractionTarget::SidebarFixed(target),
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
    );
    dispatcher.publish_hit_map(hits.finish());
    assert!(
        dispatcher
            .dispatch(
                &mut view,
                b"\x1b[<0;2;1M\x1b[<0;2;1m",
                &mut port,
                Instant::now(),
                |_, _| Ok::<bool, io::Error>(false),
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
            |_, _| Ok::<bool, io::Error>(false),
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
                &mut |_, _| Ok::<bool, io::Error>(false),
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
    assert_eq!(view.help_filter(), "normal");
    assert!(forwarded_bytes(&port.calls).is_empty());
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
        port.menu
            .key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
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
