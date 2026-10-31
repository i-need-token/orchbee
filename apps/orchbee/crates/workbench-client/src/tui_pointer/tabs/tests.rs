use super::*;
use crate::workbench_client::input::TabDragTab;
use crate::{
    action_menu::DrawnMenuSource,
    client_view::ClientActionDisposition,
    terminal_input::{PointerProbe, PointerTab, TerminalInputUnavailable},
    tui_pointer::{
        HitLayer, HitMapBuilder, InteractionSpec, InteractionTarget, MotionObservability,
        PrimaryContract, ProjectedPointer,
    },
    workbench_client::ClientContextLocation,
};
use orchbee_workbench_protocol::{SessionId, WorkbenchRevision, WorkbenchTerminalContextId};
use std::time::Instant;

struct Port {
    source: TabDragSource,
    writes: Vec<TabConfiguration>,
    actions: Vec<ClientAction>,
    follows: Vec<WorkbenchTabId>,
}
impl Port {
    fn request_drawn_tab_rename(
        &mut self,
        source: &DrawnTabCreate,
        tab: &WorkbenchTabId,
    ) -> Option<ClientActionDisposition> {
        assert_eq!(source, &self.source.drawn);
        assert_eq!(tab, &id(2));
        self.actions.push(ClientAction::RequestRenameTab);
        Some(ClientActionDisposition::KeepNavigator)
    }

    fn commit_tab_drag(&mut self, source: &DrawnTabCreate, operation: TabConfiguration) {
        assert_eq!(source, &self.source.drawn);
        self.writes.push(operation);
    }

    fn follow_drawn_tab_strip(&mut self, source: &DrawnTabCreate, tab: &WorkbenchTabId) {
        assert_eq!(source, &self.source.drawn);
        self.follows.push(tab.clone());
    }
}

impl TerminalInputPort for Port {
    fn surface_input(&mut self, input: crate::terminal_input::SurfaceInput<'_>) -> bool {
        use crate::terminal_input::SurfaceInput;
        match input {
            SurfaceInput::TabStripFollow { source, tab } => {
                self.follow_drawn_tab_strip(source, tab);
                true
            }
            _ => false,
        }
    }

    fn surface_action(
        &mut self,
        action: crate::terminal_input::SurfaceAction<'_>,
    ) -> Option<crate::client_view::ClientActionDisposition> {
        use crate::terminal_input::{SurfaceAction, TabAction};
        match action {
            SurfaceAction::Tabs(TabAction::RequestRename { source, tab }) => {
                self.request_drawn_tab_rename(source, tab)
            }
            SurfaceAction::Tabs(TabAction::Drag { source, operation }) => {
                self.commit_tab_drag(source, operation);
                None
            }
            _ => None,
        }
    }

    fn forward(&mut self, _: &[u8]) -> Result<(), TerminalInputUnavailable> {
        panic!("drag cannot forward terminal input")
    }
    fn pointer_probe(&mut self) -> PointerProbe {
        PointerProbe {
            tabs: self
                .source
                .tabs
                .iter()
                .enumerate()
                .map(|(index, tab)| PointerTab {
                    id: tab.id.clone(),
                    position: index + 1,
                })
                .collect(),
            ..PointerProbe::default()
        }
    }
    fn tab_drag_source(&mut self) -> Option<TabDragSource> {
        Some(self.source.clone())
    }
    fn apply_client_action(
        &mut self,
        command: crate::workbench_client::InputAction,
    ) -> Option<ClientActionDisposition> {
        let action = command.intent();
        self.actions.push(action);
        Some(ClientActionDisposition::KeepNavigator)
    }
}
fn id(n: usize) -> WorkbenchTabId {
    WorkbenchTabId::parse(&format!("{n:032x}")).unwrap()
}
fn group() -> WorkbenchTabGroupId {
    WorkbenchTabGroupId::parse(&format!("{:032x}", 99)).unwrap()
}
fn fixture(collapsed: bool) -> (PointerState, Port, ClientView, FrameHitMap) {
    let source = TabDragSource {
        drawn: DrawnTabCreate {
            source: DrawnMenuSource {
                session: SessionId::parse(&format!("{:032x}", 1)).unwrap(),
                revision: WorkbenchRevision::new(1),
            },
            location: {
                let fixture_context_id =
                    WorkbenchTerminalContextId::parse(&format!("{:032x}", 50)).unwrap();
                ClientContextLocation::Workspace {
                    workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                        fixture_context_id.as_str(),
                    )
                    .unwrap(),
                    terminal_context_id: fixture_context_id,
                }
            },
        },
        tabs: (1..=5)
            .map(|n| TabDragTab {
                id: id(n),
                pinned: n == 1,
                group: (n == 3 || n == 4).then(group),
                selected: n == 2,
                collapsed: (n == 3 || n == 4) && collapsed,
            })
            .collect(),
    };
    let mut hits = HitMapBuilder::new();
    hits.record_action_menu_source(Some(source.drawn.source.clone()));
    for (x, target) in [
        (0, HitTarget::Tab(id(1))),
        (10, HitTarget::Tab(id(2))),
        (
            20,
            HitTarget::TabGroup {
                tab: id(4),
                group: group(),
            },
        ),
        (30, HitTarget::Tab(id(3))),
        (40, HitTarget::Tab(id(4))),
        (50, HitTarget::Tab(id(5))),
        (60, HitTarget::TabCreate(source.drawn.clone())),
    ] {
        if !collapsed || !matches!(&target, HitTarget::Tab(tab) if tab == &id(3) || tab == &id(4)) {
            let rect = Rect::new(x, 0, 9, 1);
            match target {
                HitTarget::TabCreate(drawn) => hits.record_interaction(
                    HitLayer::Foreground,
                    rect,
                    rect,
                    InteractionTarget::TabCreate(drawn),
                    InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
                ),
                target => hits.record_tab_target(&source.drawn, rect, target),
            }
        }
    }
    let map = hits.finish();
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.publish(map.clone());
    (
        pointer,
        Port {
            source,
            writes: Vec::new(),
            actions: Vec::new(),
            follows: Vec::new(),
        },
        ClientView::new(120),
        map,
    )
}
fn send(
    pointer: &mut PointerState,
    port: &mut Port,
    view: &mut ClientView,
    action: MouseAction,
    x: u16,
    y: u16,
) {
    pointer.event(
        view,
        port,
        Instant::now(),
        MouseEvent {
            action,
            button: MouseButton::Left,
            mods: Modifiers::NONE,
            column: x,
            row: y,
        },
    );
}

fn projected_tab(pointer: &PointerState, source: &DrawnTabCreate) -> ProjectedPointer {
    HitMapBuilder::with_pointer(pointer.projection()).projected_tab_pointer(
        source,
        &HitTarget::Tab(id(2)),
        Rect::new(10, 0, 9, 1),
    )
}

#[test]
fn leaving_a_pressed_tab_clears_pressed_without_rearming_or_blocking_a_later_drag() {
    let (mut pointer, mut port, mut view, _) = fixture(false);
    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Press,
        18,
        0,
    );
    assert_eq!(
        projected_tab(&pointer, &port.source.drawn),
        ProjectedPointer::Pressed
    );

    // One cell enters the gap without crossing the two-cell drag threshold.
    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Motion,
        19,
        0,
    );
    assert_eq!(
        projected_tab(&pointer, &port.source.drawn),
        ProjectedPointer::Rest
    );
    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Motion,
        18,
        0,
    );
    assert_eq!(
        projected_tab(&pointer, &port.source.drawn),
        ProjectedPointer::Rest
    );
    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Release,
        18,
        0,
    );
    assert!(port.actions.is_empty() && port.writes.is_empty());

    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Press,
        18,
        0,
    );
    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Motion,
        19,
        0,
    );
    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Motion,
        34,
        0,
    );
    assert!(pointer.tab_drag_preview().is_some());
    assert_eq!(
        projected_tab(&pointer, &port.source.drawn),
        ProjectedPointer::Rest
    );
    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Release,
        34,
        0,
    );
    assert_eq!(port.writes.len(), 1);
    assert!(port.actions.is_empty());
}

#[test]
fn tab_projection_requires_any_motion_but_button_only_press_still_projects_pressed() {
    let (mut pointer, mut port, mut view, map) = fixture(false);
    let source = port.source.drawn.clone();
    let motion = MouseEvent {
        action: MouseAction::Motion,
        button: MouseButton::None,
        mods: Modifiers::NONE,
        column: 12,
        row: 0,
    };
    assert_eq!(
        pointer.event(&mut view, &mut port, Instant::now(), motion),
        PointerOutcome::IdleMotion { changed: false },
    );
    assert_eq!(projected_tab(&pointer, &source), ProjectedPointer::Rest);

    pointer.set_motion_observability(MotionObservability::AnyMotion);
    assert_eq!(
        pointer.event(&mut view, &mut port, Instant::now(), motion),
        PointerOutcome::IdleMotion { changed: true },
    );
    assert_eq!(projected_tab(&pointer, &source), ProjectedPointer::Hovered);

    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Press,
        12,
        0,
    );
    assert_eq!(projected_tab(&pointer, &source), ProjectedPointer::Pressed);
    pointer.publish(map);
    assert_eq!(projected_tab(&pointer, &source), ProjectedPointer::Pressed);
    pointer.set_host_focus(false);
    assert_eq!(projected_tab(&pointer, &source), ProjectedPointer::Rest);
    assert_eq!(
        pointer.cancelled_ui_event(MouseEvent {
            action: MouseAction::Release,
            button: MouseButton::None,
            mods: Modifiers::NONE,
            column: 12,
            row: 0,
        }),
        PointerOutcome::TabGestureEnded,
    );
}

#[test]
fn motion_is_private_and_release_commits_once_including_selected_set() {
    let (mut pointer, mut port, mut view, map) = fixture(false);
    port.source.tabs[4].selected = true;
    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Press,
        12,
        0,
    );
    assert!(pointer.tab_drag_preview().is_none());
    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Motion,
        34,
        0,
    );
    let preview = pointer.tab_drag_preview().unwrap();
    assert_eq!(preview.count, 2);
    assert_eq!(preview.marker, Some((38, 0)));
    pointer.publish(map);
    assert!(port.writes.is_empty());
    assert!(port.actions.is_empty());
    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Release,
        34,
        0,
    );
    assert_eq!(
        port.writes,
        vec![TabConfiguration::MoveTabsByDrag {
            tab_ids: vec![id(2), id(5)].try_into().unwrap(),
            insert_index: WorkbenchTabInsertIndex::new(3).unwrap(),
            group_id: Some(group()),
        }]
    );
    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Release,
        34,
        0,
    );
    assert_eq!(port.writes.len(), 1);
    assert!(port.follows.is_empty(), "a drag never selects a Tab");
}

#[test]
fn compositor_cover_retires_tab_drag_preview_and_owns_one_late_release() {
    for button in [MouseButton::Left, MouseButton::None] {
        let (mut pointer, mut port, mut view, _) = fixture(false);
        send(
            &mut pointer,
            &mut port,
            &mut view,
            MouseAction::Press,
            12,
            0,
        );
        send(
            &mut pointer,
            &mut port,
            &mut view,
            MouseAction::Motion,
            34,
            0,
        );
        assert!(pointer.tab_drag_preview().is_some());

        assert!(pointer.retire_covered_chrome_gesture());
        assert!(pointer.tab_drag_preview().is_none());
        let release = MouseEvent {
            action: MouseAction::Release,
            button,
            mods: Modifiers::NONE,
            column: 34,
            row: 0,
        };
        assert_eq!(
            pointer.cancelled_ui_event(release),
            PointerOutcome::TabGestureEnded,
        );
        assert_eq!(pointer.cancelled_ui_event(release), PointerOutcome::Ignored,);
        assert!(port.writes.is_empty() && port.actions.is_empty());
    }
}

#[test]
fn cancellation_traces_never_commit_or_reinterpret_release() {
    for cancel in 0..7 {
        let (mut pointer, mut port, mut view, mut map) = fixture(false);
        send(
            &mut pointer,
            &mut port,
            &mut view,
            MouseAction::Press,
            12,
            0,
        );
        send(
            &mut pointer,
            &mut port,
            &mut view,
            MouseAction::Motion,
            54,
            0,
        );
        match cancel {
            0 => {
                assert!(pointer.cancel_tab_drag());
            }
            1 => {
                pointer.invalidate_map();
                pointer.publish(map);
            }
            2 => {
                port.source.drawn.source.revision = WorkbenchRevision::new(2);
                pointer.tick(&mut view, &mut port, Instant::now(), None);
            }
            3 => {
                map.overlay_active = true;
                pointer.publish(map);
            }
            4 => {
                map.regions[0].rect.width -= 1;
                pointer.publish(map);
            }
            5 => {
                pointer.set_capture(false);
                pointer.set_capture(true);
            }
            _ => {}
        }
        send(
            &mut pointer,
            &mut port,
            &mut view,
            MouseAction::Release,
            54,
            if cancel == 6 { 4 } else { 0 },
        );
        assert!(port.writes.is_empty(), "cancel trace {cancel}");
        assert!(port.actions.is_empty(), "cancel trace {cancel}");
    }
}

#[test]
fn group_headers_move_complete_members_without_toggling_and_click_toggles_on_release() {
    for collapsed in [false, true] {
        let (mut pointer, mut port, mut view, _) = fixture(collapsed);
        send(
            &mut pointer,
            &mut port,
            &mut view,
            MouseAction::Press,
            22,
            0,
        );
        assert!(port.actions.is_empty());
        send(
            &mut pointer,
            &mut port,
            &mut view,
            MouseAction::Motion,
            58,
            0,
        );
        assert_eq!(pointer.tab_drag_preview().unwrap().count, 2);
        send(
            &mut pointer,
            &mut port,
            &mut view,
            MouseAction::Release,
            58,
            0,
        );
        assert_eq!(
            port.writes,
            vec![TabConfiguration::MoveTabsByDrag {
                tab_ids: vec![id(3), id(4)].try_into().unwrap(),
                insert_index: WorkbenchTabInsertIndex::new(5).unwrap(),
                group_id: None,
            }]
        );
        assert!(port.actions.is_empty());
        send(
            &mut pointer,
            &mut port,
            &mut view,
            MouseAction::Press,
            22,
            0,
        );
        send(
            &mut pointer,
            &mut port,
            &mut view,
            MouseAction::Release,
            22,
            0,
        );
        assert_eq!(port.actions, vec![ClientAction::ToggleTabGroup(group())]);
    }
}

#[test]
fn pinned_partition_collapsed_group_boundary_and_shift_bypass() {
    let (mut pointer, mut port, mut view, _) = fixture(true);
    send(&mut pointer, &mut port, &mut view, MouseAction::Press, 2, 0);
    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Motion,
        58,
        0,
    );
    assert!(pointer.tab_drag_preview().unwrap().marker.is_none());
    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Release,
        58,
        0,
    );
    assert!(port.writes.is_empty());
    pointer.event(
        &mut view,
        &mut port,
        Instant::now(),
        MouseEvent {
            action: MouseAction::Press,
            button: MouseButton::Left,
            mods: Modifiers::NONE.with_shift(),
            column: 12,
            row: 0,
        },
    );
    assert!(!pointer.tab_drag_captured());
    assert!(port.actions.is_empty());
    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Press,
        12,
        0,
    );
    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Motion,
        28,
        0,
    );
    send(
        &mut pointer,
        &mut port,
        &mut view,
        MouseAction::Release,
        28,
        0,
    );
    assert!(
        matches!(&port.writes[0], TabConfiguration::MoveTabsByDrag { insert_index, group_id: None, .. } if insert_index.value() == 4)
    );
}

#[test]
fn second_exact_tab_click_renames_but_focus_loss_cancels_the_pair() {
    for lose_focus in [false, true] {
        let (mut pointer, mut port, mut view, _) = fixture(false);
        for action in [MouseAction::Press, MouseAction::Release] {
            send(&mut pointer, &mut port, &mut view, action, 12, 0);
        }
        if lose_focus {
            pointer.set_host_focus(false);
            pointer.set_host_focus(true);
        }
        for action in [MouseAction::Press, MouseAction::Release] {
            send(&mut pointer, &mut port, &mut view, action, 12, 0);
        }
        assert_eq!(
            port.actions,
            vec![
                ClientAction::SelectTab(2),
                if lose_focus {
                    ClientAction::SelectTab(2)
                } else {
                    ClientAction::RequestRenameTab
                }
            ]
        );
        // Each selecting click keeps a paged strip window; the renaming click selects nothing.
        assert_eq!(port.follows, vec![id(2); 1 + usize::from(lose_focus)]);
        assert!(port.writes.is_empty());
    }
}

#[test]
fn double_click_survives_selection_revision_but_not_a_context_replacement() {
    for replace_context in [false, true] {
        let (mut pointer, mut port, mut view, _) = fixture(false);
        for action in [MouseAction::Press, MouseAction::Release] {
            send(&mut pointer, &mut port, &mut view, action, 12, 0);
        }
        port.source.drawn.source.revision = WorkbenchRevision::new(2);
        if replace_context {
            port.source.drawn.location = {
                let fixture_context_id =
                    WorkbenchTerminalContextId::parse(&format!("{:032x}", 51)).unwrap();
                ClientContextLocation::Workspace {
                    workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                        fixture_context_id.as_str(),
                    )
                    .unwrap(),
                    terminal_context_id: fixture_context_id,
                }
            };
        }
        let mut hits = HitMapBuilder::new();
        hits.record_action_menu_source(Some(port.source.drawn.source.clone()));
        hits.record_tab_target(
            &port.source.drawn,
            Rect::new(10, 0, 9, 1),
            HitTarget::Tab(id(2)),
        );
        pointer.publish(hits.finish());
        for action in [MouseAction::Press, MouseAction::Release] {
            send(&mut pointer, &mut port, &mut view, action, 12, 0);
        }
        assert_eq!(
            port.actions,
            [
                ClientAction::SelectTab(2),
                if replace_context {
                    ClientAction::SelectTab(2)
                } else {
                    ClientAction::RequestRenameTab
                }
            ]
        );
        assert!(port.writes.is_empty());
    }
}

#[test]
fn a_drag_keeps_whole_tokens_while_the_close_track_reveals_or_hides() {
    // Tab 2 painted as the strip paints it: a body plus a three-cell track that is a close control
    // while revealed (hover, press, current) and part of the Tab while hidden (a drag at rest).
    for hovered_before_press in [true, false] {
        let (mut pointer, mut port, mut view, _) = fixture(false);
        let drawn = port.source.drawn.clone();
        let map = |revealed: bool| {
            let mut hits = HitMapBuilder::new();
            hits.record_action_menu_source(Some(drawn.source.clone()));
            for n in [1, 3, 4, 5] {
                let x = u16::try_from(n - 1).unwrap() * 10;
                hits.record_tab_target(&drawn, Rect::new(x, 0, 9, 1), HitTarget::Tab(id(n)));
            }
            hits.record_tab_target(&drawn, Rect::new(10, 0, 6, 1), HitTarget::Tab(id(2)));
            let track = Rect::new(16, 0, 3, 1);
            if revealed {
                hits.record_interaction(
                    HitLayer::Foreground,
                    track,
                    track,
                    InteractionTarget::TabCloseRequest {
                        source: drawn.clone(),
                        tab: id(2),
                    },
                    InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
                );
            } else {
                hits.record_tab_target(&drawn, track, HitTarget::Tab(id(2)));
            }
            hits.finish()
        };
        pointer.publish(map(hovered_before_press));
        send(
            &mut pointer,
            &mut port,
            &mut view,
            MouseAction::Press,
            12,
            0,
        );
        pointer.publish(map(true));
        assert_eq!(
            HitMapBuilder::with_pointer(pointer.projection()).projected_tab_pointer(
                &drawn,
                &HitTarget::Tab(id(2)),
                Rect::new(10, 0, 6, 1)
            ),
            ProjectedPointer::Pressed
        );
        send(
            &mut pointer,
            &mut port,
            &mut view,
            MouseAction::Motion,
            15,
            0,
        );
        send(
            &mut pointer,
            &mut port,
            &mut view,
            MouseAction::Motion,
            31,
            0,
        );
        pointer.publish(map(false));
        let preview = pointer
            .tab_drag_preview()
            .expect("the drag outlives its hidden track");
        assert_eq!(preview.grabbed, vec![Rect::new(10, 0, 9, 1)]);
        assert_eq!(preview.marker, Some((30, 0)));
        send(
            &mut pointer,
            &mut port,
            &mut view,
            MouseAction::Release,
            31,
            0,
        );
        assert_eq!(port.writes.len(), 1, "{hovered_before_press}");
    }
}
