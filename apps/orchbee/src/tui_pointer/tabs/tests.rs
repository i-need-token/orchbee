use super::*;
use crate::{
    action_menu::DrawnMenuSource,
    client_view::SessionActionDisposition,
    terminal_input::{PointerProbe, PointerTab, TerminalInputUnavailable},
    terminal_session::NativeContextLocation,
    tui_pointer::{HitLayer, HitMapBuilder},
};
use orchbee_workbench_protocol::{SiteId, WorkbenchRevision, WorkbenchTerminalContextId};
use std::time::Instant;

struct Port {
    source: TabDragSource,
    writes: Vec<TabConfiguration>,
    actions: Vec<ClientAction>,
}
impl TerminalInputPort for Port {
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
    fn commit_tab_drag(&mut self, source: &DrawnTabCreate, operation: TabConfiguration) {
        assert_eq!(source, &self.source.drawn);
        self.writes.push(operation);
    }
    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        self.actions.push(action);
        Some(SessionActionDisposition::KeepNavigator)
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
                site: SiteId::parse(&format!("{:032x}", 1)).unwrap(),
                revision: WorkbenchRevision::new(1),
            },
            location: NativeContextLocation::Standalone(
                WorkbenchTerminalContextId::parse(&format!("{:032x}", 50)).unwrap(),
            ),
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
            hits.record(HitLayer::Foreground, Rect::new(x, 0, 9, 1), target);
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
                pointer.tick(&mut view, &mut port, Instant::now());
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
