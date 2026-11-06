use super::*;
use crate::machine_discovery::Context;
use crate::machine_picker::{SidebarRow, SidebarWindow};
use crate::terminal_input::{PointerProbe, TerminalInputUnavailable};
use crate::tui_hit_map::{HitLayer, HitMapBuilder, InteractionSpec};
use crate::workbench_projection::ClientContextTarget;
use orchbee_workbench_protocol::{
    Navigation, Project, ProtocolVersion, SessionId, Snapshot, TerminalContext, WorkbenchProjectId,
    WorkbenchRevision, WorkbenchTerminalContextId, WorkbenchWorkspaceId, Workspace,
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
    snapshot: std::sync::Arc<Snapshot>,
    writes: Vec<NavigationConfiguration>,
    /// A background refresh read holds the setup lane, so no reorder source is admitted.
    refreshing: bool,
}
impl Port {
    fn source(&self) -> SidebarReorderSource {
        SidebarReorderSource::from_snapshot(self.snapshot.clone())
    }
    fn commit_sidebar_reorder(
        &mut self,
        source: &SidebarReorderSource,
        operation: NavigationConfiguration,
    ) {
        assert_eq!(source, &self.source());
        self.writes.push(operation);
    }
}

impl TerminalInputPort for Port {
    fn surface_action(
        &mut self,
        action: crate::terminal_input::SurfaceAction<'_>,
    ) -> Option<crate::client_view::ClientActionDisposition> {
        use crate::terminal_input::{LayoutAction, SurfaceAction};
        match action {
            SurfaceAction::Layout(LayoutAction::SidebarReorder { source, operation }) => {
                self.commit_sidebar_reorder(source, operation);
                None
            }
            _ => None,
        }
    }

    fn forward(&mut self, _: &[u8]) -> Result<(), TerminalInputUnavailable> {
        panic!("sidebar gestures never enter a pane")
    }
    fn pointer_probe(&mut self) -> PointerProbe {
        PointerProbe::default()
    }
    fn sidebar_reorder_source(&mut self) -> Option<SidebarReorderSource> {
        (!self.refreshing).then(|| self.source())
    }
}
fn fixture() -> (PointerState, ClientView, Port, FrameHitMap) {
    fixture_with(1)
}
/// Workspace rows take `lines` painted lines; headings take one. A blank line follows each row.
fn fixture_with(lines: u16) -> (PointerState, ClientView, Port, FrameHitMap) {
    let navigation = {
        let fixture_context_id = WorkbenchTerminalContextId::parse(&identity(10)).unwrap();
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
                terminal_directory: None,
                automatic_label: false,
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
                        terminal_directory: None,
                        automatic_label: false,
                        workspace_id: workspace(index),
                        project_id: group.map(project),
                        label: format!("Workspace {index}"),
                        terminal_context_id: WorkbenchTerminalContextId::parse(&identity(
                            100 + index,
                        ))
                        .unwrap(),
                        primary_directory_id: None,
                    }),
            )
            .collect(),
        }
    };
    let session_id = SessionId::parse(&identity(9)).unwrap();
    let snapshot = std::sync::Arc::new(Snapshot {
        protocol_version: ProtocolVersion,
        session_id: session_id.clone(),
        revision: WorkbenchRevision::new(1),
        terminal_contexts: navigation
            .workspaces
            .iter()
            .map(|workspace| workspace.terminal_context_id.clone())
            .map(|terminal_context_id| TerminalContext {
                terminal_context_id,
                groups: vec![],
                tabs: vec![],
            })
            .collect(),
        navigation: navigation.clone(),
    });
    let mut view = ClientView::new(120);
    view.machines.available = true;
    view.machines.observe_agent_sidebar(true);
    let contexts: Vec<_> = navigation
        .workspaces
        .iter()
        .map(|workspace| Context {
            target: ClientContextTarget {
                session_id: session_id.clone(),
                location: ClientContextLocation::Workspace {
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
    let current = view.machines.current.clone();
    view.machines.observe_native_navigation(
        current,
        Some(SidebarNavigationProjection {
            snapshot: snapshot.clone(),
            pending: None,
        }),
    );
    view.machines.current_context = Some(contexts[0].target.clone());
    let map = paint(&view, lines);
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.publish(map.clone());
    (
        pointer,
        view,
        Port {
            snapshot,
            writes: vec![],
            refreshing: false,
        },
        map,
    )
}
/// Paints the sidebar the picker shows now, as the shell does after the drag's order reaches it.
fn paint(view: &ClientView, lines: u16) -> FrameHitMap {
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
        session: SessionId::parse(&identity(9)).unwrap(),
        revision: WorkbenchRevision::new(1),
    }));
    let mut y = 3;
    for row in &rows {
        let height = if matches!(row, SidebarRow::Context { .. }) {
            lines
        } else {
            1
        };
        let area = Rect::new(0, y, 30, height);
        y += height + 1;
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
    let body = Rect::new(0, 2, 30, 24);
    hits.record_sidebar_reorder_paint(SidebarReorderPaint {
        viewport: Rect::new(0, 0, 120, 30),
        layer: HitLayer::Foreground,
        body,
        footer: Rect::new(
            0,
            y.min(body.bottom()),
            30,
            body.bottom().saturating_sub(y).max(1),
        ),
        window,
        source,
    });
    hits.finish()
}
/// One shell frame: the drag's order reaches the sidebar, which paints and publishes.
fn repaint(pointer: &mut PointerState, view: &mut ClientView, lines: u16) -> FrameHitMap {
    view.machines.preview_sidebar_reorder(
        pointer.sidebar_reorder_order(),
        pointer.sidebar_reorder_held(),
    );
    let map = paint(view, lines);
    pointer.publish(map.clone());
    map
}
/// The entries the sidebar shows, in display order.
fn shown(view: &ClientView) -> Vec<Item> {
    view.machines
        .sidebar_navigation_rows()
        .iter()
        .filter_map(|row| Item::from_target(&row.target()))
        .collect()
}
fn fixture_workspace() -> WorkbenchWorkspaceId {
    WorkbenchWorkspaceId::parse(&identity(10)).unwrap()
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
fn sidebar_target(map: &FrameHitMap, item: Item) -> SidebarTarget {
    map.regions
        .iter()
        .find_map(|region| {
            let InteractionTarget::Sidebar(target) = region.interaction.as_ref()?.target() else {
                return None;
            };
            (target.part() == SidebarPart::Primary
                && Item::from_target(target.target()).as_ref() == Some(&item))
            .then(|| target.target().clone())
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

/// Another Client's rename publishes while a row is held: the refresh read it causes, the commit
/// and the repaint touch no fact of the pressed row, so its release still activates it.
#[test]
fn a_click_survives_the_refresh_and_repaint_of_an_unrelated_commit() {
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

    port.refreshing = true;
    pointer.tick(&mut view, &mut port, Instant::now(), None);
    port.refreshing = false;
    let snapshot = std::sync::Arc::make_mut(&mut port.snapshot);
    snapshot.revision = WorkbenchRevision::new(2);
    snapshot.navigation.workspaces[2].label = "Renamed elsewhere".into();
    let mut repainted = map;
    repainted.menu_source.as_mut().unwrap().revision = WorkbenchRevision::new(2);
    pointer.publish(repainted);
    pointer.tick(&mut view, &mut port, Instant::now(), None);

    assert!(matches!(
        send(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Release,
            origin
        ),
        PointerOutcome::Interaction {
            target: InteractionTarget::Sidebar(_),
            contract: PrimaryContract::ActivateOnRelease,
        }
    ));
    assert!(port.writes.is_empty());
    assert!(!pointer.sidebar_reorder_captured());
}

#[test]
fn workspace_drag_moves_into_between_and_out_of_projects_or_reorders_siblings() {
    for (source, target, expected_group, expected_before) in [
        // A Project heading is the top edge of its Workspaces: from above the dragged Workspace
        // becomes the first, and from below it ends the Project before, or leads the first one.
        (
            0,
            Item::Project(project(2)),
            Some(project(2)),
            Some(workspace(2)),
        ),
        (
            3,
            Item::Project(project(1)),
            Some(project(1)),
            Some(workspace(0)),
        ),
        (3, Item::Project(project(2)), Some(project(1)), None),
        // A Workspace outside every Project is the way out of one: the held Workspace lands after
        // it, among the ungrouped Workspaces.
        (0, Item::Workspace(workspace(3)), None, None),
        (0, Item::Workspace(workspace(1)), Some(project(1)), None),
        (
            1,
            Item::Workspace(workspace(0)),
            Some(project(1)),
            Some(workspace(0)),
        ),
        (0, Item::Workspace(workspace(2)), Some(project(2)), None),
    ] {
        let (mut pointer, mut view, mut port, map) = fixture();
        let origin = row(&map, Item::Workspace(workspace(source)));
        let target = row(&map, target);
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

/// The free rows below the list end a Project drag only. Over them a Workspace keeps the move the
/// sidebar shows, as over a gap, so the release commits that move and no move out of its Project.
#[test]
fn a_workspace_over_the_free_rows_below_the_list_keeps_the_shown_move() {
    let (mut pointer, mut view, mut port, map) = fixture();
    let origin = row(&map, Item::Workspace(workspace(0)));
    let anchor = row(&map, Item::Workspace(workspace(2)));
    let free = map.sidebar_reorder.as_ref().unwrap().footer;
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
        anchor,
    );
    let shown = pointer
        .sidebar_reorder_preview()
        .unwrap()
        .resolve(pointer.map.as_ref().unwrap())
        .unwrap()
        .destination;
    assert!(shown.is_some());
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Motion,
        free,
    );
    assert_eq!(
        pointer
            .sidebar_reorder_preview()
            .unwrap()
            .resolve(pointer.map.as_ref().unwrap())
            .unwrap()
            .destination,
        shown
    );
    assert_eq!(
        send(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Release,
            free
        ),
        PointerOutcome::SidebarReorderEnded
    );
    assert_eq!(
        port.writes,
        [NavigationConfiguration::MoveWorkspace {
            workspace_id: workspace(0),
            project_id: Some(project(2)),
            before_workspace_id: None,
        }]
    );
}

/// ORC-317: with no gap between two-line rows, the line under the pointer decides the side. A
/// Workspace dragged up from Recents onto the lower line of a Project's first Workspace lands
/// after it, where the sidebar shows it, instead of before it because the drag moved up.
#[test]
fn two_line_workspace_rows_place_a_drop_by_the_line_under_the_pointer() {
    for (source, anchor, line, group, before) in [
        (3, 0, 1, project(1), Some(workspace(1))),
        (3, 0, 0, project(1), Some(workspace(0))),
        (0, 2, 0, project(2), Some(workspace(2))),
        (0, 2, 1, project(2), None),
    ] {
        let (mut pointer, mut view, mut port, map) = fixture_with(2);
        let origin = row(&map, Item::Workspace(workspace(source)));
        let anchor = row(&map, Item::Workspace(workspace(anchor)));
        let target = Rect {
            y: anchor.y + line,
            height: 1,
            ..anchor
        };
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
        repaint(&mut pointer, &mut view, 2);
        let expected = NavigationConfiguration::MoveWorkspace {
            workspace_id: workspace(source),
            project_id: Some(group),
            before_workspace_id: before,
        };
        let moved = configure_navigation(&port.snapshot.navigation, &expected)
            .unwrap()
            .unwrap();
        assert!(
            view.machines.sidebar_source().matches_navigation(
                &view.machines.current,
                &port.snapshot.session_id,
                &moved
            ),
            "the sidebar shows the drop the release submits"
        );
        send(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Release,
            target,
        );
        assert_eq!(port.writes, [expected]);
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
    let keyboard = port.source().step(&target_semantic, false).unwrap();
    assert!(port.source().step(&target_semantic, true).is_none());
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
                std::sync::Arc::make_mut(&mut port.snapshot).revision = WorkbenchRevision::new(2);
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

/// A scroll moves other rows under a pointer that stays put, so the shown destination follows it
/// there and the frame that shows the new order is requested.
#[test]
fn a_scroll_repaint_moves_the_destination_to_the_row_now_under_the_pointer() {
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
    let mut scrolled = map.clone();
    scrolled.sidebar_reorder.as_mut().unwrap().window.top = 1;
    for region in &mut scrolled.regions {
        region.rect.y -= 2;
    }
    assert!(!pointer.take_projection_redraw());
    pointer.publish(scrolled);
    assert!(pointer.take_projection_redraw());
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Release,
        target,
    );
    assert_eq!(
        port.writes,
        [NavigationConfiguration::MoveWorkspace {
            workspace_id: workspace(0),
            project_id: Some(project(2)),
            before_workspace_id: None,
        }],
        "the pointer now rests on the line of Workspace 2, so the Workspace lands after it"
    );
}

/// Every motion moves the held row to its destination in the sidebar itself, across Projects too,
/// and a pointer that stays where the move put the row keeps it there; the release submits exactly
/// the order shown, which the admitted move's projection then shows without a step back.
#[test]
fn a_drag_moves_the_held_row_live_and_its_release_submits_the_order_shown() {
    let (mut pointer, mut view, mut port, map) = fixture_with(2);
    let canonical = shown(&view);
    let (p1, p2) = (Item::Project(project(1)), Item::Project(project(2)));
    let w = |index| Item::Workspace(workspace(index));
    let origin = row(&map, w(0));
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Press,
        origin,
    );
    // The lower line of the two-line Workspace below places the held row after it.
    let lower = Rect {
        y: row(&map, w(1)).y + 1,
        height: 1,
        ..origin
    };
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Motion,
        lower,
    );
    let map = repaint(&mut pointer, &mut view, 2);
    let reordered = vec![
        p1.clone(),
        w(1),
        w(0),
        p2.clone(),
        w(2),
        Item::Workspace(fixture_workspace()),
        w(3),
    ];
    assert_eq!(shown(&view), reordered);
    // The moved row now lies under the pointer, so the same cell keeps it and repaints nothing.
    assert!(contains(row(&map, w(0)), lower.x + 3, lower.y));
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Motion,
        lower,
    );
    assert!(!view.machines.preview_sidebar_reorder(
        pointer.sidebar_reorder_order(),
        pointer.sidebar_reorder_held(),
    ));

    // Onto the next Project's heading from above: the held row becomes its first Workspace.
    let heading = row(&map, p2.clone());
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Motion,
        heading,
    );
    let map = repaint(&mut pointer, &mut view, 2);
    let across = vec![
        p1,
        w(1),
        p2,
        w(0),
        w(2),
        Item::Workspace(fixture_workspace()),
        w(3),
    ];
    assert_eq!(shown(&view), across);
    assert!(contains(row(&map, w(0)), heading.x + 3, heading.y));
    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Motion,
        heading,
    );
    assert!(!view.machines.preview_sidebar_reorder(
        pointer.sidebar_reorder_order(),
        pointer.sidebar_reorder_held(),
    ));

    send(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Release,
        heading,
    );
    let submitted = NavigationConfiguration::MoveWorkspace {
        workspace_id: workspace(0),
        project_id: Some(project(2)),
        before_workspace_id: Some(workspace(2)),
    };
    assert_eq!(port.writes, std::slice::from_ref(&submitted));
    // The shell observes the admitted move before the next frame drops the drag's order.
    let current = view.machines.current.clone();
    view.machines.observe_native_navigation(
        current,
        Some(SidebarNavigationProjection {
            snapshot: port.snapshot.clone(),
            pending: Some(submitted),
        }),
    );
    assert_eq!(shown(&view), across);
    repaint(&mut pointer, &mut view, 2);
    assert_eq!(shown(&view), across);
    assert_ne!(shown(&view), canonical);
}

/// Escape and a release outside the list end the drag and the sidebar returns to its canonical
/// order, with nothing submitted.
#[test]
fn escape_or_a_release_outside_the_list_restores_the_canonical_order() {
    for outside in [false, true] {
        let (mut pointer, mut view, mut port, map) = fixture();
        let canonical = shown(&view);
        let held = sidebar_target(&map, Item::Workspace(workspace(0)));
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
        repaint(&mut pointer, &mut view, 1);
        assert_ne!(shown(&view), canonical);
        // The held Workspace shows as current from the moment the drag latches.
        assert_eq!(view.machines.sidebar_current_override(), Some(&held));
        if outside {
            let beyond = Rect { x: 40, ..target };
            send(
                &mut pointer,
                &mut view,
                &mut port,
                MouseAction::Motion,
                beyond,
            );
            repaint(&mut pointer, &mut view, 1);
            assert_eq!(
                shown(&view),
                canonical,
                "outside the list shows the canonical order"
            );
            send(
                &mut pointer,
                &mut view,
                &mut port,
                MouseAction::Release,
                beyond,
            );
        } else {
            // The route Escape takes through the dispatcher.
            assert!(pointer.escape_sidebar_reorder());
        }
        repaint(&mut pointer, &mut view, 1);
        assert_eq!(shown(&view), canonical);
        assert!(port.writes.is_empty());
        assert!(!pointer.sidebar_reorder_captured());
        // However the person ends it, the held Workspace stays current: the dispatcher switches
        // to it, and the sidebar keeps it filled until that switch settles.
        assert_eq!(pointer.take_sidebar_reorder_switch(), Some(held.clone()));
        assert!(view.machines.switch_to_held_workspace(&held));
        assert_eq!(view.machines.request, held.selection());
        assert_eq!(view.machines.sidebar_current_override(), Some(&held));
    }
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
    std::sync::Arc::make_mut(&mut port.snapshot).revision = WorkbenchRevision::new(2);
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
    std::sync::Arc::make_mut(&mut port.snapshot).revision = WorkbenchRevision::new(1);
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
        let canonical = std::sync::Arc::make_mut(&mut port.snapshot);
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
                .source()
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
