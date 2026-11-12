use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_REQUEST_BYTES, MAXIMUM_GRAPHICS_RESULT_BYTES, parse_workbench_request,
    parse_workbench_response, read_workbench_request, read_workbench_response,
    write_workbench_request, write_workbench_response,
};

#[test]
fn graphics_receipts_correlate_publisher_heartbeat_and_allow_an_observed_default_client() {
    use orchbee_workbench_protocol::*;
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-panel-graphics-conformance-v1.json"
    ))
    .unwrap();
    let find = |name: &str| {
        corpus["valid"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == name)
            .unwrap()["document"]
            .clone()
    };
    let WorkbenchRequest::PaneGraphics(request) = parse_workbench_request(
        &serde_json::to_vec(&find("observe-actual-cell-raster-draw")).unwrap(),
    )
    .unwrap() else {
        panic!("observe");
    };
    let WorkbenchResponse::PaneGraphics(mut reply) = parse_workbench_response(
        &serde_json::to_vec(&find("observed-correlated-heartbeat")).unwrap(),
    )
    .unwrap() else {
        panic!("observed");
    };
    assert!(reply.answers(&request));
    let PaneGraphicsOutcome::Observed { heartbeat, .. } = &mut reply.outcome else {
        unreachable!();
    };
    *heartbeat = ClientViewSequence::new(2).unwrap();
    assert!(!reply.answers(&request));
    let target = PaneGraphicsTarget {
        panel_id: WorkbenchPanelId::parse(&format!("{:032x}", 3)).unwrap(),
        presentation_id: Some(TerminalPresentationId::parse(&format!("{:032x}", 6)).unwrap()),
    };
    let mut request = request;
    request.action = PaneGraphicsAction::Info {
        target: target.clone(),
        client_id: None,
    };
    reply.outcome = PaneGraphicsOutcome::Info {
        target,
        revision: WorkbenchRevision::new(1),
        layer_count: 1,
        client_id: Some(WorkbenchClientId::parse(&format!("{:032x}", 4)).unwrap()),
        capabilities: None,
        panel_visible: None,
        drawn_revision: None,
        file_frame_directory: None,
    };
    assert!(reply.answers(&request));
    let PaneGraphicsAction::Info { client_id, .. } = &mut request.action else {
        unreachable!();
    };
    *client_id = Some(WorkbenchClientId::parse(&format!("{:032x}", 5)).unwrap());
    assert!(!reply.answers(&request));
}

#[test]
fn pane_graphics_conformance_covers_closed_images_receipts_and_direction_limits() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-panel-graphics-conformance-v1.json"
    ))
    .unwrap();
    for (group, accepted) in [("valid", true), ("invalid", false)] {
        for case in corpus[group].as_array().unwrap() {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            let valid = if case["direction"] == "request" {
                parse_workbench_request(&bytes)
                    .map(|request| {
                        let mut wire = Vec::new();
                        write_workbench_request(&mut wire, &request).unwrap();
                        assert_eq!(&wire[6..8], &57u16.to_be_bytes());
                        assert_eq!(
                            read_workbench_request(&mut wire.as_slice()).unwrap(),
                            request
                        );
                        wire[6..8].copy_from_slice(&55u16.to_be_bytes());
                        assert!(read_workbench_request(&mut wire.as_slice()).is_err());
                    })
                    .is_ok()
            } else {
                parse_workbench_response(&bytes)
                    .map(|response| {
                        let mut wire = Vec::new();
                        write_workbench_response(&mut wire, &response).unwrap();
                        assert_eq!(&wire[6..8], &58u16.to_be_bytes());
                        assert_eq!(
                            read_workbench_response(&mut wire.as_slice()).unwrap(),
                            response
                        );
                        wire[6..8].copy_from_slice(&56u16.to_be_bytes());
                        assert!(read_workbench_response(&mut wire.as_slice()).is_err());
                    })
                    .is_ok()
            };
            assert_eq!(valid, accepted, "{}", case["name"]);
        }
    }
    for (kind, limit, request) in [
        (57u16, MAXIMUM_GRAPHICS_REQUEST_BYTES, true),
        (58, MAXIMUM_GRAPHICS_RESULT_BYTES, false),
    ] {
        let mut header = b"OBWP".to_vec();
        header.extend(1u16.to_be_bytes());
        header.extend(kind.to_be_bytes());
        header.extend(u32::try_from(limit + 1).unwrap().to_be_bytes());
        assert!(if request {
            read_workbench_request(&mut header.as_slice()).is_err()
        } else {
            read_workbench_response(&mut header.as_slice()).is_err()
        });
    }
}
