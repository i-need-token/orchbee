//! A lost or miscorrelated save receipt preserves uncertainty without a retry.
use super::*;
use crate::local_client::tests::server::FakeServer;
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::{
    PreferenceKey, ProtocolVersion, SessionId, SettingsChanges, SettingsFailure, SettingsOperation,
    SettingsOutcome, SettingsSource, WorkbenchRequestId, read_workbench_request,
    write_workbench_response,
};
use std::{io, num::NonZeroU64, time::Duration};
fn request() -> SettingsRequest {
    SettingsRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
        session_id: SessionId::parse(&format!("{:032x}", 2)).unwrap(),
        operation: SettingsOperation::Save {
            source: SettingsSource::parse("missing").unwrap(),
            changes: SettingsChanges::reset([PreferenceKey::ApplicationTheme]),
        },
    }
}
#[test]
fn settings_lost_and_wrong_receipts_are_submitted_unknown_and_never_replayed() {
    for wrong_receipt in [false, true] {
        let server = FakeServer::start_connections(1, move |_, mut stream| {
            crate::local_client::test_support::handshake::answer_offer(&mut stream)?;
            assert!(matches!(
                read_message(&mut stream).map_err(io::Error::other)?,
                LocalClientMessage::ClientHello
            ));
            write_message(
                &mut stream,
                &LocalClientMessage::ClientAccepted {
                    generation: NonZeroU64::new(1).unwrap(),
                },
            )
            .map_err(io::Error::other)?;
            let WorkbenchRequest::Settings(received) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("one Settings request");
            };
            assert!(matches!(received.operation, SettingsOperation::Save { .. }));
            if wrong_receipt {
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::Settings(SettingsResult {
                        protocol_version: ProtocolVersion,
                        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 3)).unwrap(),
                        session_id: received.session_id,
                        outcome: SettingsOutcome::Refused {
                            reason: SettingsFailure::Denied,
                        },
                    }),
                )
                .map_err(io::Error::other)?;
            }
            Ok(())
        })
        .unwrap();
        let mut client = LocalClient::connect(server.endpoint()).unwrap();
        let result = client
            .settings(
                request(),
                client.operation_clock().now() + Duration::from_secs(2),
                None,
            )
            .unwrap_err();
        assert!(result.submitted);
        if wrong_receipt {
            assert!(matches!(result.reason, MetadataExchangeReason::Correlation));
        }
        let later = client
            .settings(
                request(),
                client.operation_clock().now() + Duration::from_secs(2),
                None,
            )
            .unwrap_err();
        assert!(!later.submitted);
        assert!(matches!(
            later.reason,
            MetadataExchangeReason::ConnectionUnavailable
        ));
        server.finish().unwrap();
    }
}
