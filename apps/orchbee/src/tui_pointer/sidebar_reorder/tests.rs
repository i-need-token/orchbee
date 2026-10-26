use super::*;
use crate::{
    machine_discovery::Context,
    machine_picker::SidebarRow,
    terminal_input::{PointerProbe, TerminalInputUnavailable},
    tui_pointer::{HitLayer, HitMapBuilder, InteractionSpec},
    workbench_client::NativeContextTarget,
};
use orchbee_workbench_protocol::{
    Project, ProtocolVersion, Snapshot, TerminalContext, WorkbenchProjectId, WorkbenchRevision,
    WorkbenchTerminalContextId, WorkbenchWorkspaceId, Workspace,
};

fn identity(value: usize) -> String {
    format!("{value:032x}")
}
fn project(value: usize) -> WorkbenchProjectId {
    WorkbenchProjectId::parse(&identity(value)).unwrap()
}
fn workspace(value: usize) -> WorkbenchWorkspaceId {
    WorkbenchWorkspaceId::parse(&identity(20 + value)).unwrap()
}
struct Port {
    source: SidebarReorderSource,
    writes: Vec<NavigationConfiguration>,
}
impl TerminalInputPort for Port {
    fn forward(&mut self, _: &[u8]) -> Result<(), TerminalInputUnavailable> {
        panic!("sidebar gestures never enter a pane")
    }
    fn pointer_probe(&mut self) -> PointerProbe {
        PointerProbe::default()
    }
    fn sidebar_reorder_source(&mut self) -> Option<SidebarReorderSource> {
        Some(self.source.clone())
    }
    fn commit_sidebar_reorder(
        &mut self,
        source: &SidebarReorderSource,
        operation: NavigationConfiguration,
    ) {
        assert_eq!(source, &self.source);
        self.writes.push(operation);
    }
}
fn fixture() -> (PointerState, ClientView, Port, FrameHitMap) {
    let navigation = {
        let fixture_context_id = WorkbenchTerminalContextId::parse(&identity(500)).unwrap();
        let fixture_workspace_id =
            orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(fixture_context_id.as_str())
                .unwrap();
        Navigation {
            project_order: vec![],
            workspace_order: Vec::new(),
            directories: vec![],
            projects: [1, 2]
                .into_iter()
                .map(|id| Project {
                    project_id: project(id),
                    label: format!("Project {id}"),
                })
                .collect(),
            workspaces: std::iter::once(orchbee_workbench_protocol::Workspace {
                workspace_id: fixture_workspace_id.clone(),
                terminal_context_id: fixture_context_id.clone(),
                project_id: None,
                primary_directory_id: None,
                label: "Workspace".into(),
            })
            .chain(
                [Some(1), Some(1), Some(2), None]
                    .into_iter()
                    .enumerate()
                    .map(|(index, group)| Workspace {
                        workspace_id: workspace(index),
                        project_id: group.map(project),
                        label: format!("Workspace {index}"),
                        terminal_context_id: WorkbenchTerminalContextId::parse(&identity(
                            100 + index,
                        ))
                        .unwrap(),
                        primary_directory_id: None,
                    })
                    .collect(),
            )
            .collect(),
        }
    };
    let site_id = SiteId::parse(&identity(9)).unwrap();
    let mut view = ClientView::new(120);
    view.machines.available = true;
    view.machines.observe_agent_sidebar(true);
    let contexts: Vec<_> = navigation
        .workspaces
        .iter()
        .map(|workspace| Context {
            target: NativeContextTarget {
                site_id: site_id.clone(),
                location: NativeContextLocation::Workspace {
                    workspace_id: workspace.workspace_id.clone(),
                    terminal_context_id: workspace.terminal_context_id.clone(),
                },
            },
            project_id: workspace.project_id.clone(),
            project: workspace
                .project_id
                .as_ref()
                .map(|id| navigation.project(id).unwrap().label.clone()),
            workspace: Some(workspace.label.clone()),
        })
        .collect();
    view.machines.observe_local(contexts.clone());
    view.machines.current_context = Some(contexts[0].target.clone());
    let source = view.machines.sidebar_source();
    let rows = view.machines.sidebar_navigation_rows();
    let window = SidebarWindow {
        top: 0,
        capacity: rows.len(),
        total: rows.len(),
        maximum: 0,
        insertion: None,
    };
    let mut hits = HitMapBuilder::new();
    hits.record_action_menu_source(Some(crate::action_menu::DrawnMenuSource {
        site: site_id.clone(),
        revision: WorkbenchRevision::new(1),
    }));
    for (index, row) in rows.iter().enumerate() {
        let area = Rect::new(0, 3 + u16::try_from(index).unwrap() * 2, 30, 1);
        if matches!(row, SidebarRow::Project { .. }) {
            hits.record_interaction(
                HitLayer::Foreground,
                Rect::new(12, area.y, 1, 1),
                Rect::new(12, area.y, 1, 1),
                InteractionTarget::Sidebar(
                    source
                        .drawn(row.target(), SidebarPart::Disclosure, window)
                        .unwrap(),
                ),
                InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
            );
        }
        hits.record_interaction(
            HitLayer::Foreground,
            area,
            area,
            InteractionTarget::Sidebar(
                source
                    .drawn(row.target(), SidebarPart::Primary, window)
                    .unwrap(),
            ),
            InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
        );
    }
    hits.record_sidebar_reorder_paint(SidebarReorderPaint {
        viewport: Rect::new(0, 0, 120, 30),
        layer: HitLayer::Foreground,
        body: Rect::new(0, 2, 30, 20),
        footer: Rect::new(0, 22, 30, 1),
        window,
        source,
    });
    let map = hits.finish();
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.publish(map.clone());
    (
        pointer,
        view,
        Port {
            source: SidebarReorderSource::from_snapshot(std::sync::Arc::new(Snapshot {
                protocol_version: ProtocolVersion,
                site_id,
                revision: WorkbenchRevision::new(1),
                terminal_contexts: navigation
                    .workspaces
                    .iter()
                    .map(|workspace| workspace.terminal_context_id.clone())
                    .chain(std::iter::once(
                        navigation.workspaces[0].terminal_context_id.clone(),
                    ))
                    .map(|terminal_context_id| TerminalContext {
                        terminal_context_id,
                        groups: vec![],
                        tabs: vec![],
                    })
                    .collect(),
                navigation,
            })),
            writes: vec![],
        },
        map,
    )
}
fn row(map: &FrameHitMap, item: Item) -> Rect {
    map.regions
        .iter()
        .find_map(|region| {
            let InteractionTarget::Sidebar(target) = region.interaction.as_ref()?.target() else {
                return None;
            };
            (target.part() == SidebarPart::Primary
                && Item::from_target(target.target()).as_ref() == Some(&item))
            .then_some(region.rect)
        })
        .unwrap()
}
fn send(
    pointer: &mut PointerState,
    view: &mut ClientView,
    port: &mut Port,
    action: MouseAction,
    area: Rect,
) -> PointerOutcome {
    pointer.event(
        view,
        port,
        Instant::now(),
        MouseEvent {
            action,
            button: MouseButton::Left,
            mods: Modifiers::NONE,
            column: area.x + 3,
            row: area.y,
        },
    )
}

#[test]
fn clicks_activate_once_but_drag_returning_to_origin_cannot_click() {
    let (mut pointer, mut view, mut port, map) = fixture();
    let origin = row(&map, Item::Workspace(workspace(0)));
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Press,
        origin,
    );
    assert!(pointer.sidebar_reorder_captured());
    assert!(matches!(
        send(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Release,
            origin
        ),
        PointerOutcome::Interaction {
            contract: PrimaryContract::ActivateOnRelease,
            ..
        }
    ));
    assert!(port.writes.is_empty());
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Press,
        origin,
    );
    let elsewhere = row(&map, Item::Project(project(2)));
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Motion,
        elsewhere,
    );
    assert!(pointer.sidebar_reorder_preview().is_some());
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Motion,
        origin,
    );
    assert_eq!(
        send(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Release,
            origin
        ),
        PointerOutcome::SidebarReorderEnded
    );
    assert!(port.writes.is_empty());
}

#[test]
fn workspace_drag_moves_into_between_and_out_of_projects_or_reorders_siblings() {
    for (source, target, expected_group, expected_before) in [
        (0, Some(Item::Project(project(2))), Some(project(2)), None),
        (3, Some(Item::Project(project(1))), Some(project(1)), None),
        (0, None, None, None),
        (
            0,
            Some(Item::Workspace(workspace(1))),
            Some(project(1)),
            None,
        ),
        (
            1,
            Some(Item::Workspace(workspace(0))),
            Some(project(1)),
            Some(workspace(0)),
        ),
        (
            0,
            Some(Item::Workspace(workspace(2))),
            Some(project(2)),
            None,
        ),
    ] {
        let (mut pointer, mut view, mut port, map) = fixture();
        let origin = row(&map, Item::Workspace(workspace(source)));
        let target = target.map_or(map.sidebar_reorder.as_ref().unwrap().footer, |item| {
            row(&map, item)
        });
        send(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Press,
            origin,
        );
        send(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Motion,
            target,
        );
        assert!(
            pointer
                .sidebar_reorder_preview()
                .unwrap()
                .resolve(pointer.map.as_ref().unwrap())
                .unwrap()
                .destination
                .is_some()
        );
        assert_eq!(
            send(
                &mut pointer,
                &mut view,
                &mut port,
                MouseAction::Release,
                target
            ),
            PointerOutcome::SidebarReorderEnded
        );
        assert_eq!(
            port.writes,
            [NavigationConfiguration::MoveWorkspace {
                workspace_id: workspace(source),
                project_id: expected_group,
                before_workspace_id: expected_before
            }]
        );
        send(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Release,
            target,
        );
        assert_eq!(port.writes.len(), 1);
    }
}

#[test]
fn project_drag_and_keyboard_sort_share_the_same_canonical_move() {
    let (mut pointer, mut view, mut port, map) = fixture();
    let origin = row(&map, Item::Project(project(1)));
    let target = row(&map, Item::Project(project(2)));
    let target_semantic = view
        .machines
        .sidebar_navigation_rows()
        .into_iter()
        .find(|row| matches!(row,SidebarRow::Project {project_id,..} if **project_id==project(1)))
        .unwrap()
        .target();
    let keyboard = port.source.step(&target_semantic, false).unwrap();
    assert!(port.source.step(&target_semantic, true).is_none());
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Press,
        origin,
    );
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Motion,
        target,
    );
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Release,
        target,
    );
    assert_eq!(port.writes, [keyboard]);
    assert_eq!(
        port.writes[0],
        NavigationConfiguration::MoveProject {
            project_id: project(1),
            before_project_id: None
        }
    );
}

#[test]
fn cancelled_stale_resized_and_modified_drags_retain_only_release_cleanup() {
    for cause in 0..6 {
        let (mut pointer, mut view, mut port, map) = fixture();
        let origin = row(&map, Item::Workspace(workspace(0)));
        let target = row(&map, Item::Project(project(2)));
        send(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Press,
            origin,
        );
        send(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Motion,
            target,
        );
        match cause {
            0 => {
                assert!(pointer.cancel_sidebar_reorder());
            }
            1 => {
                pointer.set_host_focus(false);
            }
            2 => {
                pointer.set_capture(false);
            }
            3 => {
                std::sync::Arc::make_mut(&mut port.source.0).revision = WorkbenchRevision::new(2);
            }
            4 => {
                let mut changed = map.clone();
                changed.sidebar_reorder.as_mut().unwrap().viewport.width += 1;
                pointer.publish(changed);
            }
            _ => {}
        }
        let outcome = if cause == 5 {
            pointer.event(
                &mut view,
                &mut port,
                Instant::now(),
                MouseEvent {
                    action: MouseAction::Release,
                    button: MouseButton::Left,
                    mods: Modifiers::CTRL,
                    column: target.x + 3,
                    row: target.y,
                },
            )
        } else {
            send(
                &mut pointer,
                &mut view,
                &mut port,
                MouseAction::Release,
                target,
            )
        };
        assert!(!matches!(outcome, PointerOutcome::Interaction { .. }));
        assert!(port.writes.is_empty());
        assert!(!pointer.sidebar_reorder_captured());
    }
}

#[test]
fn disclosure_keeps_its_click_contract_and_scroll_repaint_keeps_a_drag() {
    let (mut pointer, mut view, mut port, map) = fixture();
    let mut disclosure = row(&map, Item::Project(project(1)));
    disclosure.x = 9;
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Press,
        disclosure,
    );
    assert!(!pointer.sidebar_reorder_captured());
    assert!(
        matches!(send(&mut pointer,&mut view,&mut port,MouseAction::Release,disclosure),PointerOutcome::Interaction {target:InteractionTarget::Sidebar(target),..} if target.part()==SidebarPart::Disclosure)
    );
    let origin = row(&map, Item::Workspace(workspace(0)));
    let target = row(&map, Item::Project(project(2)));
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Press,
        origin,
    );
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Motion,
        target,
    );
    let mut scrolled = map.clone();
    scrolled.sidebar_reorder.as_mut().unwrap().window.top = 1;
    pointer.publish(scrolled);
    assert!(pointer.sidebar_reorder_captured());
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Release,
        target,
    );
    assert_eq!(port.writes.len(), 1);
}

#[test]
fn preview_resolves_the_candidate_frame_instead_of_a_stale_scroll_destination() {
    let (mut pointer, mut view, mut port, map) = fixture();
    let origin = row(&map, Item::Workspace(workspace(0)));
    let target = row(&map, Item::Project(project(2)));
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Press,
        origin,
    );
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Motion,
        target,
    );
    let preview = pointer.sidebar_reorder_preview().unwrap();
    assert!(preview.resolve(&map).unwrap().destination.is_some());
    let mut candidate = map.clone();
    for region in &mut candidate.regions {
        if region.rect.y == target.y {
            region.rect.y += 1;
        }
    }
    assert!(preview.resolve(&candidate).unwrap().destination.is_none());
    assert!(port.writes.is_empty());
    pointer.publish(candidate);
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Release,
        target,
    );
    assert!(port.writes.is_empty());
}

#[test]
fn edge_autoscroll_is_bounded_and_does_not_submit_or_rearm_a_click() {
    let (mut pointer, mut view, mut port, mut map) = fixture();
    let paint = map.sidebar_reorder.as_mut().unwrap();
    paint.body.height = 4;
    paint.footer.y = 6;
    paint.window.capacity = 2;
    paint.window.maximum = 4;
    map.regions.retain(|region| region.rect.y < 6);
    pointer.publish(map.clone());
    let origin = row(&map, Item::Workspace(workspace(0)));
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Press,
        origin,
    );
    let mut edge = origin;
    edge.x += 3;
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Motion,
        edge,
    );
    let sample = Instant::now();
    assert!(!pointer.tick_sidebar_reorder(&mut view, &mut port, sample));
    assert!(pointer.tick_sidebar_reorder(
        &mut view,
        &mut port,
        sample + Duration::from_millis(121)
    ));
    assert!(!pointer.tick_sidebar_reorder(
        &mut view,
        &mut port,
        sample + Duration::from_millis(180)
    ));
    assert!(view.machines.settle_sidebar_scroll());
    assert!(port.writes.is_empty());
    pointer.cancel_sidebar_reorder();
    assert!(!matches!(
        send(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Release,
            edge
        ),
        PointerOutcome::Interaction { .. }
    ));
}

#[test]
fn an_old_painted_canonical_revision_cannot_arm_or_keep_a_reorder() {
    let (mut pointer, mut view, mut port, map) = fixture();
    let origin = row(&map, Item::Workspace(workspace(0)));
    let target = row(&map, Item::Project(project(2)));
    std::sync::Arc::make_mut(&mut port.source.0).revision = WorkbenchRevision::new(2);
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Press,
        origin,
    );
    assert!(!pointer.sidebar_reorder_captured());
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Release,
        origin,
    );
    assert!(port.writes.is_empty());
    std::sync::Arc::make_mut(&mut port.source.0).revision = WorkbenchRevision::new(1);
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Press,
        origin,
    );
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Motion,
        target,
    );
    let mut newer = map;
    newer.menu_source.as_mut().unwrap().revision = WorkbenchRevision::new(2);
    assert!(
        pointer
            .sidebar_reorder_preview()
            .unwrap()
            .resolve(&newer)
            .is_none()
    );
    pointer.publish(newer);
    assert!(!pointer.sidebar_reorder_captured());
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Release,
        target,
    );
    assert!(port.writes.is_empty());
}

#[test]
fn publishing_a_pressed_projection_does_not_create_an_endless_redraw_request() {
    let (mut pointer, mut view, mut port, mut map) = fixture();
    let origin = row(&map, Item::Workspace(workspace(0)));
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Press,
        origin,
    );
    map.projection_revision = pointer.projection().revision();
    pointer.publish(map.clone());
    assert!(!pointer.take_projection_redraw());
    pointer.publish(map);
    assert!(!pointer.take_projection_redraw());
}

#[test]
fn a_current_native_revision_cannot_authorize_stale_sidebar_membership_or_display_order() {
    for fault in 0..3 {
        let (mut pointer, mut view, mut port, mut map) = fixture();
        let origin = row(&map, Item::Workspace(workspace(0)));
        let canonical = std::sync::Arc::make_mut(&mut port.source.0);
        canonical.revision = WorkbenchRevision::new(2);
        if fault == 0 {
            canonical.navigation.workspaces[2].project_id = Some(project(2));
        } else if fault == 1 {
            canonical.navigation.project_order = vec![project(2), project(1)];
        } else {
            canonical.navigation.workspace_order =
                vec![workspace(1), workspace(0), workspace(2), workspace(3)];
        }
        map.menu_source.as_mut().unwrap().revision = WorkbenchRevision::new(2);
        pointer.publish(map);
        assert!(
            !port
                .source
                .matches_sidebar(&view.machines.sidebar_source(), &view.machines.current)
        );
        send(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Press,
            origin,
        );
        assert!(!pointer.sidebar_reorder_captured());
        send(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Release,
            origin,
        );
        assert!(port.writes.is_empty());
    }
}
