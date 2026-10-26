use super::*;
use crate::local_client::workbench::{WorkbenchTabCloseFailure, WorkbenchTabCloseNotSubmitted};

#[derive(Clone, Copy, Debug)]
enum Family {
    Command,
    Read,
    Resume,
}
const FAMILIES: [Family; 3] = [Family::Command, Family::Read, Family::Resume];

enum Submission {
    Command(WorkbenchTabCloseSubmission),
    Read(Result<WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadError>),
    Resume(WorkbenchTabCloseResumeSubmission),
}
impl Family {
    fn request(self) -> WorkbenchRequest {
        match self {
            Self::Command => WorkbenchRequest::Command(close_command(7)),
            Self::Read => WorkbenchRequest::TabCloseRead(close_read()),
            Self::Resume => WorkbenchRequest::TabCloseResume(resume(1)),
        }
    }
    fn response(self) -> WorkbenchResponse {
        match self {
            Self::Command => command_response(FIRST_REQUEST_ID, accepted(FIRST_REQUEST_ID, 7)),
            Self::Read => read_response(available(
                FIRST_REQUEST_ID,
                2,
                WorkbenchTabCloseStatus::Closing {
                    phase: WorkbenchTabClosePhase::Queued,
                },
            )),
            Self::Resume => resume_response(WorkbenchTabCloseResumeOutcome::Resumed {
                attempt_generation: generation(2),
            }),
        }
    }
    fn submit(self, client: &mut LocalClient, deadline: Instant) -> Submission {
        match self {
            Self::Command => Submission::Command(client.submit_workbench_tab_close(
                WorkbenchTabCloseRequest::try_from(close_command(7)).unwrap(),
                deadline,
            )),
            Self::Read => Submission::Read(client.submit_workbench_tab_close_read(
                close_read(),
                None,
                deadline,
            )),
            Self::Resume => {
                Submission::Resume(client.submit_workbench_tab_close_resume(resume(1), deadline))
            }
        }
    }
}

fn require_not_submitted(submission: Submission) -> WorkbenchTabCloseNotSubmitted {
    match submission {
        Submission::Command(WorkbenchTabCloseSubmission::DefinitelyNotSubmitted(reason))
        | Submission::Resume(WorkbenchTabCloseResumeSubmission::DefinitelyNotSubmitted(reason))
        | Submission::Read(Err(WorkbenchTabCloseReadError::NotSubmitted(reason))) => reason,
        _ => panic!("request must remain definitely not submitted"),
    }
}

fn require_failure(submission: Submission, check_reason: impl FnOnce(WorkbenchTabCloseFailure)) {
    let (request_id, reason) = match submission {
        Submission::Command(WorkbenchTabCloseSubmission::OutcomeUnknown { request_id, reason })
        | Submission::Read(Err(WorkbenchTabCloseReadError::Failed { request_id, reason })) => {
            (request_id, reason)
        }
        // Resume exposes uncertainty only; Command and Read additionally expose diagnostics.
        Submission::Resume(WorkbenchTabCloseResumeSubmission::OutcomeUnknown) => return,
        _ => panic!("effectful exchanges must remain unknown and status Reads must remain failed"),
    };
    assert_eq!(request_id.as_str(), FIRST_REQUEST_ID);
    check_reason(reason);
}

#[test]
fn close_family_prewrite_deadlines_preserve_the_mixed_carrier_and_write_nothing() -> TestResult {
    for family in FAMILIES {
        let server = FakeServer::start(|mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 62)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::SnapshotRead(snapshot_request(SECOND_REQUEST_ID))
            );
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Snapshot(snapshot_result(SECOND_REQUEST_ID)),
            )
            .map_err(io::Error::other)
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let deadline = client.operation_clock().now();
        assert!(
            matches!(require_not_submitted(family.submit(&mut client,deadline)),WorkbenchTabCloseNotSubmitted::RequestNotSubmitted(error) if error.kind()==io::ErrorKind::TimedOut)
        );
        assert_eq!(client.generation().map(NonZeroU64::get), Some(62));
        assert_eq!(
            client.submit_workbench_snapshot_read(snapshot_request(SECOND_REQUEST_ID))?,
            snapshot_result(SECOND_REQUEST_ID)
        );
        drop(client);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn close_family_partial_handoff_expires_and_retires_without_replay() -> TestResult {
    for family in FAMILIES {
        let mut frame = Vec::new();
        write_workbench_request(&mut frame, &family.request())?;
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 63)?;
            let mut received = Vec::new();
            stream.read_to_end(&mut received)?;
            assert!(!received.is_empty() && received.len() < frame.len());
            assert_eq!(received, frame[..received.len()]);
            Ok(())
        })?;
        let clock =
            SteppingMonotonicClock::new(Duration::from_millis(10), Arc::new(AtomicU64::new(0)));
        let deadline = clock.origin + Duration::from_millis(90);
        let mut client = LocalClient::connect_with_submission_runtime(
            server.endpoint(),
            COHORT,
            SERVER_GUARD_TIMEOUT,
            Box::new(clock),
            IoChunkLimits::new(usize::MAX, 1),
        )?;
        require_failure(family.submit(&mut client, deadline), |reason| {
            assert!(
                matches!(reason, WorkbenchTabCloseFailure::Write(error) if error.kind() == io::ErrorKind::TimedOut)
            );
        });
        assert_eq!(client.generation(), None);
        assert!(matches!(
            require_not_submitted(family.submit(&mut client, deadline)),
            WorkbenchTabCloseNotSubmitted::ConnectionUnavailable
        ));
        drop(client);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn close_family_complete_response_cannot_extend_caller_or_exchange_deadline() -> TestResult {
    for family in FAMILIES {
        for caller_is_earlier in [true, false] {
            let server = FakeServer::start(move |mut stream| {
                expect_hello(&mut stream, COHORT)?;
                admit(&mut stream, 64)?;
                assert_eq!(
                    read_workbench_request(&mut stream).map_err(io::Error::other)?,
                    family.request()
                );
                write_workbench_response(&mut stream, &family.response())
                    .map_err(io::Error::other)?;
                require_retirement(&mut stream)
            })?;
            let clock =
                SteppingMonotonicClock::new(Duration::from_millis(10), Arc::new(AtomicU64::new(0)));
            let deadline = clock.origin
                + if caller_is_earlier {
                    Duration::from_millis(80)
                } else {
                    SERVER_GUARD_TIMEOUT
                };
            let timeout = if caller_is_earlier {
                SERVER_GUARD_TIMEOUT
            } else {
                Duration::from_millis(80)
            };
            let mut client = LocalClient::connect_with_submission_runtime(
                server.endpoint(),
                COHORT,
                timeout,
                Box::new(clock),
                IoChunkLimits::UNBOUNDED,
            )?;
            require_failure(family.submit(&mut client, deadline), |reason| {
                assert!(
                    matches!(reason, WorkbenchTabCloseFailure::ResponseDeadline(error) if error.kind() == io::ErrorKind::TimedOut)
                );
            });
            assert_eq!(client.generation(), None);
            drop(client);
            server.finish()?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum Corruption {
    WrongDirection,
    Oversized,
    DuplicateKey,
    ZeroGeneration,
    OverflowGeneration,
}

#[test]
fn close_family_direction_size_duplicate_and_generation_corruption_retire() -> TestResult {
    for family in FAMILIES {
        for corruption in [
            Corruption::WrongDirection,
            Corruption::Oversized,
            Corruption::DuplicateKey,
            Corruption::ZeroGeneration,
            Corruption::OverflowGeneration,
        ] {
            if matches!(family, Family::Command)
                && matches!(
                    corruption,
                    Corruption::ZeroGeneration | Corruption::OverflowGeneration
                )
            {
                continue;
            }
            let server = FakeServer::start(move |mut stream| {
                expect_hello(&mut stream, COHORT)?;
                admit(&mut stream, 65)?;
                assert_eq!(
                    read_workbench_request(&mut stream).map_err(io::Error::other)?,
                    family.request()
                );
                if matches!(corruption, Corruption::WrongDirection) {
                    let mut frame = Vec::new();
                    write_workbench_request(&mut frame, &family.request())
                        .map_err(io::Error::other)?;
                    // Direction must fail at the header without waiting for its declared body.
                    stream.write_all(&frame[..12])?;
                } else {
                    let kind = match family {
                        Family::Command => 4,
                        Family::Read => 6,
                        Family::Resume => 8,
                    };
                    if matches!(corruption, Corruption::Oversized) {
                        write_workbench_header(
                            &mut stream,
                            kind,
                            MAXIMUM_ENCODED_INTENT_BYTES + 1,
                        )?;
                    } else {
                        let document = String::from_utf8(
                            orchbee_workbench_protocol::serialize_workbench_response(
                                &family.response(),
                            )
                            .map_err(io::Error::other)?,
                        )
                        .map_err(io::Error::other)?;
                        let document = match corruption {
                            Corruption::DuplicateKey => document.replacen(
                                "\"requestId\":",
                                &format!("\"requestId\":\"{FIRST_REQUEST_ID}\",\"requestId\":"),
                                1,
                            ),
                            Corruption::ZeroGeneration => document.replace(
                                "\"attemptGeneration\":\"2\"",
                                "\"attemptGeneration\":\"0\"",
                            ),
                            Corruption::OverflowGeneration => document.replace(
                                "\"attemptGeneration\":\"2\"",
                                "\"attemptGeneration\":\"18446744073709551616\"",
                            ),
                            _ => unreachable!(),
                        };
                        write_workbench_header(&mut stream, kind, document.len())?;
                        stream.write_all(document.as_bytes())?;
                    }
                }
                require_retirement(&mut stream)
            })?;
            let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
            let deadline = client.operation_clock().now() + SERVER_GUARD_TIMEOUT;
            require_failure(family.submit(&mut client, deadline), |reason| {
                assert!(
                    matches!(reason, WorkbenchTabCloseFailure::Response(_)),
                    "{family:?} {corruption:?}"
                );
            });
            assert_eq!(client.generation(), None);
            drop(client);
            server.finish()?;
        }
    }
    Ok(())
}
