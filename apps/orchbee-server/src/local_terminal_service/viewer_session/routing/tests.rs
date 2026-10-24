use super::*;
use orchbee_terminal_protocol::{
    Envelope, decode_terminal_render_row_window, encode_transport_envelope,
};
use std::num::NonZeroU64;

#[test]
fn viewport_response_detail_must_match_the_request() {
    let bytes = include_bytes!(
        "../../../../../../packages/terminal-protocol/fixtures/ghostty-render-row-window.bin"
    );
    let route = decode_terminal_render_row_window(bytes).unwrap().route();
    let service = super::super::tests::service();
    let carrier = crate::worker_obcn_carrier::create_worker_carrier(
        orchbee_local_site_protocol::WorkerId::from_bytes([7; 16]),
        NonZeroU64::MIN,
    );
    let mut viewer = super::super::tests::viewer(&service, carrier.request_handle());
    viewer.projection = crate::local_terminal_service::projection::ViewerProjection::new(
        route.attachment_id,
        route.host_epoch,
    );
    let operation = OperationId::from_bytes([3; 16]).unwrap();
    let record = encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        kind: 46,
        role: MessageRole::Response,
        payload_encoding: PayloadEncoding::RowWindow,
        host_epoch: Some(route.host_epoch),
        correlation_id: Some(CorrelationId::from_bytes(operation.as_bytes()).unwrap()),
        optional_message: false,
        payload: bytes,
    })
    .unwrap();
    for (detail, valid) in [
        (ViewerRowDetail::Render, true),
        (ViewerRowDetail::Text, false),
    ] {
        let action = Action::ReadViewportRows {
            attachment_id: viewer.attachment_id.clone(),
            detail,
            origin: ViewportRowOrigin::Tail { rows_before: 3 },
            row_count: route.row_count,
            revision: 0,
            generation: route.generation,
            request_sequence: route.sequence,
            maximum_bytes: bytes.len() as u32,
        };
        assert_eq!(
            valid_viewport_rows_record(&action, &viewer, operation, &record),
            valid
        );
    }
}
