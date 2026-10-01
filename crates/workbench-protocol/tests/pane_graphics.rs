use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_REQUEST_BYTES, MAXIMUM_GRAPHICS_RESULT_BYTES, parse_workbench_request,
    parse_workbench_response, read_workbench_request, read_workbench_response,
    write_workbench_request, write_workbench_response,
};

#[test]
fn pane_graphics_conformance_covers_closed_images_receipts_and_direction_limits() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-pane-graphics-conformance-v1.json"
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
