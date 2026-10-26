use orchbee_workbench_protocol::{
    ClientCellRect, ClientPanelArea, ClientPanelDirection as Direction, ClientPanelLayout,
    ClientPanelNeighbors, ClientSplitRatio, WorkbenchPanelId,
};
use serde_json::json;

fn panel(number: u32, rect: Option<ClientCellRect>) -> ClientPanelArea {
    ClientPanelArea {
        panel_id: WorkbenchPanelId::parse(&format!("{number:032x}")).unwrap(),
        unzoomed: rect,
        terminal: rect,
        displayed: rect,
    }
}
fn rect(x: u16, y: u16, width: u16, height: u16) -> ClientCellRect {
    ClientCellRect {
        x,
        y,
        width,
        height,
    }
}
fn target(
    source: &ClientPanelArea,
    panels: &[ClientPanelArea],
    direction: Direction,
) -> Option<WorkbenchPanelId> {
    ClientPanelNeighbors::from_panels(&source.panel_id, panels)
        .get(direction)
        .cloned()
}

#[test]
fn navigation_orders_distance_overlap_center_then_traversal() {
    // Independent hand-computed rectangles exercise each lexicographic decision.
    let source = panel(1, Some(rect(30, 10, 20, 20)));
    let near = panel(2, Some(rect(51, 27, 4, 3)));
    let far = panel(3, Some(rect(52, 10, 4, 20)));
    assert_eq!(
        target(
            &source,
            &[source.clone(), far, near.clone()],
            Direction::Right
        ),
        Some(near.panel_id)
    );

    let greater_overlap = panel(4, Some(rect(51, 5, 4, 14)));
    let closer_center = panel(5, Some(rect(51, 16, 4, 8)));
    assert_eq!(
        target(
            &source,
            &[source.clone(), closer_center, greater_overlap.clone()],
            Direction::Right
        ),
        Some(greater_overlap.panel_id)
    );

    let centered = panel(6, Some(rect(51, 16, 4, 8)));
    let offset = panel(7, Some(rect(51, 10, 4, 8)));
    assert_eq!(
        target(
            &source,
            &[source.clone(), offset, centered.clone()],
            Direction::Right
        ),
        Some(centered.panel_id)
    );

    // Equal overlap and equal doubled-center distance: canonical order, not lexical identity.
    let first = panel(99, Some(rect(51, 10, 4, 8)));
    let second = panel(8, Some(rect(51, 22, 4, 8)));
    assert_eq!(
        target(
            &source,
            &[source.clone(), first.clone(), second],
            Direction::Right
        ),
        Some(first.panel_id)
    );

    let touches_corner = panel(9, Some(rect(51, 30, 4, 4)));
    let empty = panel(10, Some(rect(51, 12, 0, 8)));
    let hidden = panel(11, None);
    assert_eq!(
        target(
            &source,
            &[source.clone(), touches_corner, empty, hidden.clone()],
            Direction::Right
        ),
        None
    );
    assert_eq!(
        target(&hidden, &[source, hidden.clone()], Direction::Left),
        None
    );
}

#[test]
fn cell_navigation_is_translation_invariant_and_obeys_all_four_grid_edges() {
    for columns in 1_u16..=8 {
        for rows in 1_u16..=8 {
            for origin in [0_u16, 300, 60_000] {
                let mut panels = Vec::new();
                for row in 0..rows {
                    for column in 0..columns {
                        panels.push(panel(
                            u32::from(row * columns + column) + 1,
                            Some(rect(origin + column * 11, origin + row * 7, 10, 6)),
                        ));
                    }
                }
                for row in 0..rows {
                    for column in 0..columns {
                        let index = usize::from(row * columns + column);
                        let source = &panels[index];
                        let expected = [
                            (
                                Direction::Left,
                                column.checked_sub(1).map(|col| row * columns + col),
                            ),
                            (
                                Direction::Right,
                                (column + 1 < columns).then_some(row * columns + column + 1),
                            ),
                            (
                                Direction::Up,
                                row.checked_sub(1).map(|r| r * columns + column),
                            ),
                            (
                                Direction::Down,
                                (row + 1 < rows).then_some((row + 1) * columns + column),
                            ),
                        ];
                        for (direction, neighbor) in expected {
                            assert_eq!(
                                target(source, &panels, direction),
                                neighbor.map(|index| panels[usize::from(index)].panel_id.clone()),
                                "grid {columns}x{rows}, origin {origin}, source {index}, {direction:?}",
                            );
                        }
                    }
                }
            }
        }
    }
}

fn layout_document() -> serde_json::Value {
    json!({
        "terminalContextId": "00000000000000000000000000000001",
        "tabId": "00000000000000000000000000000002",
        "sourcePanelId": "00000000000000000000000000000003",
        "workbenchRevision": "8",
        "viewport": {"x":0,"y":0,"width":100,"height":30},
        "area": {"x":1,"y":2,"width":98,"height":26},
        "selected": true,
        "zoomed": false,
        "focusedPanelId": "00000000000000000000000000000003",
        "sourceAvailable": true,
        "neighbors": {"left":null,"right":null,"up":null,"down":null},
        "panels": [{
            "panelId":"00000000000000000000000000000003",
            "unzoomed":{"x":1,"y":2,"width":98,"height":26},
            "terminal":{"x":1,"y":2,"width":98,"height":26},
            "displayed":{"x":1,"y":2,"width":98,"height":26}
        }],
        "splits": []
    })
}
fn valid(value: serde_json::Value) -> bool {
    serde_json::from_value::<ClientPanelLayout>(value).is_ok_and(|layout| layout.validate().is_ok())
}

#[test]
fn layout_boundary_rejects_false_geometry_and_requires_explicit_absence() {
    assert!(valid(layout_document()));
    for field in ["focusedPanelId", "neighbors", "sourceAvailable"] {
        let mut value = layout_document();
        value.as_object_mut().unwrap().remove(field);
        assert!(!valid(value), "{field}");
    }
    let mut value = layout_document();
    value["panels"][0]
        .as_object_mut()
        .unwrap()
        .remove("displayed");
    assert!(!valid(value));

    for (field, invalid) in [
        ("sourcePanelId", json!("00000000000000000000000000000009")),
        ("focusedPanelId", json!(null)),
        ("sourceAvailable", json!(false)),
        ("viewport", json!({"x":65530,"y":0,"width":100,"height":30})),
        ("area", json!({"x":0,"y":0,"width":101,"height":30})),
    ] {
        let mut value = layout_document();
        value[field] = invalid;
        assert!(!valid(value), "{field}");
    }
    let mut value = layout_document();
    value["neighbors"]["right"] = value["sourcePanelId"].clone();
    assert!(!valid(value));
    let mut value = layout_document();
    value["panels"][0]["terminal"]["width"] = json!(99);
    assert!(!valid(value));

    let mut hidden = layout_document();
    hidden["selected"] = json!(false);
    hidden["focusedPanelId"] = json!(null);
    hidden["sourceAvailable"] = json!(false);
    for layer in ["unzoomed", "terminal", "displayed"] {
        hidden["panels"][0][layer] = json!(null);
    }
    assert!(valid(hidden));
}

#[test]
fn split_ratio_and_cell_extents_have_closed_numeric_bounds() {
    for ratio in [0.1, 0.5, 0.9] {
        let admitted = ClientSplitRatio::new(ratio).unwrap();
        assert_eq!(admitted.value(), ratio);
        assert_eq!(
            serde_json::from_str::<ClientSplitRatio>(&serde_json::to_string(&admitted).unwrap())
                .unwrap(),
            admitted
        );
    }
    for ratio in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -0.0,
        0.099,
        0.901,
    ] {
        assert!(ClientSplitRatio::new(ratio).is_err());
    }
    assert!(rect(65_530, 65_530, 5, 5).valid());
    assert!(!rect(65_530, 65_530, 6, 5).valid());
    assert!(rect(0, 0, 65_535, 65_535).contains(rect(65_535, 65_535, 0, 0)));
}

fn view_document() -> serde_json::Value {
    json!({"workbenchRevision":"8","phase":"ready","selection":{
        "terminalContextId":"00000000000000000000000000000001",
        "tabId":"00000000000000000000000000000002",
        "panelId":"00000000000000000000000000000003",
        "zoomed":false,"panelCount":1
    }})
}
fn layout_result_document() -> serde_json::Value {
    json!({
        "kind":"workbench.client-view.result","protocolVersion":1,
        "requestId":"00000000000000000000000000000004",
        "sessionId":"00000000000000000000000000000005",
        "outcome":{"kind":"operation","operation":{
            "command":{"operationId":"00000000000000000000000000000006",
                "clientId":"00000000000000000000000000000007",
                "expectedViewRevision":"1","minimumWorkbenchRevision":"8",
                "intent":{"kind":"layout","terminalContextId":"00000000000000000000000000000001",
                    "tabId":"00000000000000000000000000000002","panelId":"00000000000000000000000000000003"}},
            "status":{"kind":"layout-read","layout":layout_document(),"view":view_document()}
        }}
    })
}

#[test]
fn layout_receipts_match_only_the_exact_read_intent_and_observation() {
    use orchbee_workbench_protocol::parse_workbench_response;
    let parse =
        |value: serde_json::Value| parse_workbench_response(&serde_json::to_vec(&value).unwrap());
    assert!(parse(layout_result_document()).is_ok());
    for path in ["terminalContextId", "tabId", "panelId"] {
        let mut value = layout_result_document();
        value["outcome"]["operation"]["command"]["intent"][path] =
            json!("00000000000000000000000000000099");
        assert!(parse(value).is_err(), "{path}");
    }
    let mut value = layout_result_document();
    value["outcome"]["operation"]["command"]["intent"]["kind"] = json!("panel");
    assert!(parse(value).is_err());
    let mut value = layout_result_document();
    value["outcome"]["operation"]["command"]["minimumWorkbenchRevision"] = json!("9");
    assert!(parse(value).is_err());
    let mut value = layout_result_document();
    value["outcome"]["operation"]["status"] = json!({"kind":"applied","view":view_document()});
    assert!(parse(value).is_err());
    let mut value = layout_result_document();
    value["outcome"]["operation"]["status"]["layout"]["selected"] = json!(false);
    assert!(parse(value).is_err());
}

#[test]
fn layout_frames_have_dedicated_kinds_and_preserve_small_action_limits() {
    use orchbee_workbench_protocol::{
        MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES, MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES,
        MAXIMUM_ENCODED_INTENT_BYTES, WorkbenchWireError, parse_workbench_request,
        parse_workbench_response, read_workbench_request, read_workbench_response,
        write_workbench_request, write_workbench_response,
    };
    let response =
        parse_workbench_response(&serde_json::to_vec(&layout_result_document()).unwrap()).unwrap();
    let request_document = json!({
        "kind":"workbench.client-view","protocolVersion":1,
        "requestId":"00000000000000000000000000000004",
        "sessionId":"00000000000000000000000000000005",
        "action":{"kind":"synchronize","heartbeat":"2","viewRevision":"1","view":view_document(),
            "completion":{"operationId":"00000000000000000000000000000006",
                "outcome":{"kind":"layout-read","layout":layout_document()}}}
    });
    let request = parse_workbench_request(&serde_json::to_vec(&request_document).unwrap()).unwrap();
    let mut frame = Vec::new();
    write_workbench_request(&mut frame, &request).unwrap();
    assert_eq!(&frame[..8], b"OBWP\x00\x01\x00\x1b");
    assert_eq!(
        read_workbench_request(&mut frame.as_slice()).unwrap(),
        request
    );
    frame[7] = 25;
    assert!(matches!(
        read_workbench_request(&mut frame.as_slice()),
        Err(WorkbenchWireError::InvalidHeader)
    ));
    frame.clear();
    write_workbench_response(&mut frame, &response).unwrap();
    assert_eq!(&frame[..8], b"OBWP\x00\x01\x00\x1c");
    assert_eq!(
        read_workbench_response(&mut frame.as_slice()).unwrap(),
        response
    );

    let mut padded = vec![b' '; MAXIMUM_ENCODED_INTENT_BYTES];
    padded.extend(serde_json::to_vec(&request_document).unwrap());
    assert!(parse_workbench_request(&padded).is_ok());
    let mut ordinary = request_document.clone();
    ordinary["action"]["completion"] = json!(null);
    padded.truncate(MAXIMUM_ENCODED_INTENT_BYTES);
    padded.extend(serde_json::to_vec(&ordinary).unwrap());
    assert!(parse_workbench_request(&padded).is_err());

    let mut padded = vec![b' '; MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES];
    padded.extend(serde_json::to_vec(&layout_result_document()).unwrap());
    assert!(parse_workbench_response(&padded).is_ok());
    let mut ordinary = layout_result_document();
    ordinary["outcome"] = json!({"kind":"not-found"});
    padded.truncate(MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES);
    padded.extend(serde_json::to_vec(&ordinary).unwrap());
    assert!(parse_workbench_response(&padded).is_err());

    for kind in [27_u16, 28] {
        let mut header = b"OBWP\x00\x01".to_vec();
        header.extend(kind.to_be_bytes());
        header.extend(
            u32::try_from(MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES + 1)
                .unwrap()
                .to_be_bytes(),
        );
        let mut reader = std::io::Cursor::new(header);
        if kind == 27 {
            assert!(matches!(
                read_workbench_request(&mut reader),
                Err(WorkbenchWireError::Contract(_))
            ));
        } else {
            assert!(matches!(
                read_workbench_response(&mut reader),
                Err(WorkbenchWireError::Contract(_))
            ));
        }
        assert_eq!(reader.position(), 12);
    }
}

#[test]
fn physical_edges_do_not_infer_boundaries_from_missing_neighbors() {
    let mut layout: ClientPanelLayout = serde_json::from_value(layout_document()).unwrap();
    for direction in [
        Direction::Left,
        Direction::Right,
        Direction::Up,
        Direction::Down,
    ] {
        assert_eq!(layout.at_edge(direction), Some(true));
    }
    // No neighbor is present, but an inset source is not on a physical area edge.
    let inner = rect(2, 3, 96, 24);
    layout.panels[0].unzoomed = Some(inner);
    layout.panels[0].terminal = Some(inner);
    layout.panels[0].displayed = Some(inner);
    layout.validate().unwrap();
    for direction in [
        Direction::Left,
        Direction::Right,
        Direction::Up,
        Direction::Down,
    ] {
        assert!(layout.neighbors.get(direction).is_none());
        assert_eq!(layout.at_edge(direction), Some(false));
    }
    layout.source_available = false;
    layout.panels[0].unzoomed = None;
    layout.panels[0].terminal = None;
    layout.panels[0].displayed = None;
    layout.validate().unwrap();
    assert_eq!(layout.at_edge(Direction::Left), None);
}

#[test]
fn resize_amount_and_binary_paths_reject_unbounded_or_ambiguous_values() {
    use orchbee_workbench_protocol::{ClientResizeAmount, ClientSplitPath};
    for amount in [f64::NAN, f64::INFINITY, -0.01, 0.500_001] {
        assert!(ClientResizeAmount::new(amount).is_err());
    }
    let maximum = ClientResizeAmount::new(0.5).unwrap();
    assert_eq!(
        maximum
            .apply(ClientSplitRatio::HALF, Direction::Right)
            .value(),
        0.9
    );
    assert_eq!(
        maximum.apply(ClientSplitRatio::HALF, Direction::Up).value(),
        0.1
    );
    let zero = ClientResizeAmount::new(-0.0).unwrap();
    assert_eq!(serde_json::to_string(&zero).unwrap(), "0.0");
    assert_eq!(
        zero.apply(ClientSplitRatio::HALF, Direction::Left),
        ClientSplitRatio::HALF
    );
    for path in [vec![], vec![0], vec![1; 16]] {
        let typed = ClientSplitPath::new(path.clone()).unwrap();
        assert_eq!(typed.indices(), path);
        assert_eq!(
            serde_json::from_str::<ClientSplitPath>(&serde_json::to_string(&typed).unwrap())
                .unwrap(),
            typed
        );
    }
    for path in [vec![2], vec![0; 17]] {
        assert!(ClientSplitPath::new(path).is_err());
    }
    for encoded in ["[true]", "[0.5]", "[-1]", "[256]", "\"root\"", "null"] {
        assert!(
            serde_json::from_str::<ClientSplitPath>(encoded).is_err(),
            "{encoded}"
        );
    }
}

#[test]
fn resize_border_prioritizes_requested_edge_then_opposite_and_canonical_ties() {
    use orchbee_workbench_protocol::{
        ClientSplitArea, PanelOrientation, WorkbenchPanelSlotId, WorkbenchSplitId,
    };
    let mut layout: ClientPanelLayout = serde_json::from_value(layout_document()).unwrap();
    layout.panels[0].unzoomed = Some(rect(31, 12, 20, 10));
    let split = |id: u32, x: u16, y: u16, height: u16| ClientSplitArea {
        split_id: WorkbenchSplitId::parse(&format!("{id:032x}")).unwrap(),
        first_slot_id: WorkbenchPanelSlotId::parse(&format!("{:032x}", id + 100)).unwrap(),
        second_slot_id: WorkbenchPanelSlotId::parse(&format!("{:032x}", id + 200)).unwrap(),
        orientation: PanelOrientation::Horizontal,
        ratio: ClientSplitRatio::HALF,
        area: Some(rect(0, y, 80, height)),
        divider: Some(rect(x, y, 1, height)),
    };
    layout.splits = vec![
        split(8, 30, 2, 20),
        split(9, 51, 12, 10),
        split(1, 51, 12, 10),
    ];
    assert_eq!(
        layout
            .resize_split(Direction::Right)
            .unwrap()
            .unwrap()
            .split_id,
        layout.splits[1].split_id
    );
    assert_eq!(
        layout
            .resize_split(Direction::Left)
            .unwrap()
            .unwrap()
            .split_id,
        layout.splits[0].split_id
    );
    assert!(layout.resize_split(Direction::Up).unwrap().is_none());
    // Requested edge at distance 2 is ineligible; exterior resize uses the opposite edge.
    layout.splits[1].divider.as_mut().unwrap().x = 53;
    layout.splits[2].area.as_mut().unwrap().y = 22; // touches only, no positive overlap
    assert_eq!(
        layout
            .resize_split(Direction::Right)
            .unwrap()
            .unwrap()
            .split_id,
        layout.splits[0].split_id
    );
    layout.splits[0].divider = None;
    assert!(layout.resize_split(Direction::Right).unwrap().is_none());
    layout.panels[0].unzoomed = None;
    assert!(layout.resize_split(Direction::Right).is_err());
}
