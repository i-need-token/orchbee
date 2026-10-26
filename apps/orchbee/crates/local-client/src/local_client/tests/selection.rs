//! Selection action/result pairing at the actual local transport boundary.
use super::*;
use orchbee_client_server_protocol::{
    TerminalViewerAction as Action, TerminalViewerOutcome as Outcome, TerminalViewerRequest,
    TerminalViewerResult, ViewerId, ViewerSelectionExportCancelReason, ViewerSelectionExportFormat,
    ViewerSelectionGestureAction as Gesture, ViewerSelectionGestureGeometry,
    ViewerSelectionGesturePosition, ViewerSelectionViewportPoint,
};

fn actions() -> Vec<Action> {
    let attachment_id = ViewerId::parse(FIRST_REQUEST_ID).expect("viewer identity");
    let point = ViewerSelectionViewportPoint { column: 2, row: 3 };
    let position = ViewerSelectionGesturePosition { x: 2, y: 3 };
    let mut actions = [
        Gesture::Start {
            click_count: 1,
            point,
            position,
            select_output: false,
        },
        Gesture::Update {
            geometry: ViewerSelectionGestureGeometry {
                cell_width: 1,
                columns: 80,
                padding_left: 0,
                screen_height: 24,
            },
            point,
            position,
            rectangle: false,
        },
        Gesture::End { point },
        Gesture::Cancel,
    ]
    .into_iter()
    .map(|gesture| Action::SelectionGesture {
        attachment_id: attachment_id.clone(),
        generation: NonZeroU64::MIN,
        gesture,
    })
    .collect::<Vec<_>>();
    actions.extend([
        Action::SelectionExport {
            attachment_id: attachment_id.clone(),
            export_sequence: NonZeroU64::MIN,
            format: ViewerSelectionExportFormat::Plain,
        },
        Action::SelectionExportChunkApplied {
            attachment_id: attachment_id.clone(),
            export_sequence: NonZeroU64::MIN,
            chunk_sequence: 0,
        },
        Action::SelectionExportCancel {
            attachment_id,
            export_sequence: NonZeroU64::MIN,
            reason: ViewerSelectionExportCancelReason::Replaced,
        },
    ]);
    actions
}

#[test]
fn selection_replies_cross_the_local_carrier_without_retiring_or_replaying() -> TestResult {
    let mut cases = actions()
        .into_iter()
        .map(|action| {
            let outcome = if matches!(action, Action::SelectionExportChunkApplied { .. }) {
                Outcome::FeedbackAccepted {}
            } else {
                // This layer relays opaque Host records. The selection decoder
                // separately owns their envelope, identity, and payload validity.
                Outcome::Record(vec![0xa5; 64])
            };
            (action, outcome)
        })
        .collect::<Vec<_>>();
    // A chunk acknowledgement may instead carry the Host's typed-failure record.
    cases.push((actions().remove(5), Outcome::Record(vec![0x5a; 64])));
    let expected = cases.clone();
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 9)?;
        for (action, outcome) in expected {
            let ClientMessage::TerminalViewer(request) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected selection request"));
            };
            assert_eq!(request.action(), &action);
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    request.request_id().clone(),
                    outcome,
                )),
            )
            .map_err(io::Error::other)?;
        }
        assert_eq!(stream.read(&mut [0])?, 0, "no extra selection request");
        Ok(())
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    for (index, (action, expected)) in cases.into_iter().enumerate() {
        let result = client.submit_terminal_viewer(TerminalViewerRequest::new(
            RequestId::parse(&format!("{:032x}", index + 1))?,
            action,
        ))?;
        assert_eq!(result.outcome(), &expected);
    }
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn selection_wrong_reply_pairings_retire_without_replay() -> TestResult {
    for action in actions() {
        let outcome = if matches!(action, Action::SelectionExportChunkApplied { .. }) {
            Outcome::Empty {}
        } else {
            Outcome::FeedbackAccepted {}
        };
        let expected = action.clone();
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 9)?;
            let ClientMessage::TerminalViewer(request) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected selection request"));
            };
            assert_eq!(request.action(), &expected);
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    request.request_id().clone(),
                    outcome,
                )),
            )
            .map_err(io::Error::other)?;
            assert_eq!(
                stream.read(&mut [0])?,
                0,
                "wrong pairing closes without retry"
            );
            Ok(())
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let request = TerminalViewerRequest::new(RequestId::parse(FIRST_REQUEST_ID)?, action);
        assert!(matches!(
            client.submit_terminal_viewer(request.clone()),
            Err(SubmissionError::OutcomeUnknown {
                reason: OutcomeUnknownReason::InvalidResult(_),
                ..
            })
        ));
        assert!(matches!(
            client.submit_terminal_viewer(request),
            Err(SubmissionError::ConnectionUnavailable)
        ));
        server.finish()?;
    }
    Ok(())
}
