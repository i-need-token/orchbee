use super::controllers::{acquire, control, input, observed};
use super::*;
use orchbee_client_server_protocol::{
    ControlRejectedReason, ControlUnavailableReason, ControllerId,
    TerminalControlAction as ControlAction, TerminalControlOutcome as ControlOutcome, ViewerId,
    ViewerUnavailableReason,
};
use std::num::NonZeroU64;

fn acknowledge(
    client: &mut UnixStream,
    attachment: &ViewerId,
    frame: &TerminalFrameRoute,
) -> TestResult {
    assert_eq!(frame.attachment_id.to_text(), attachment.as_str());
    assert_eq!(
        viewer(
            client,
            Action::FrameApplied {
                attachment_id: attachment.clone(),
                sequence: frame.sequence,
                generation: frame.generation,
                revision: frame.revision,
            }
        )?,
        Outcome::FeedbackAccepted {}
    );
    Ok(())
}

fn attach_after_cleanup(client: &mut UnixStream, reference: &RequestId) -> TestResult<ViewerId> {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        match viewer(
            client,
            Action::Attach {
                terminal_create_request_id: reference.clone(),
            },
        )? {
            Outcome::Attached { attachment_id, .. } => return Ok(attachment_id),
            // A previous Session may still own the bounded Worker queue during cleanup.
            Outcome::Unavailable {
                reason: ViewerUnavailableReason::ServerBusy,
            } if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            other => {
                return Err(format!("attachment after session cleanup failed: {other:?}").into());
            }
        }
    }
}

fn acquire_after_cleanup(
    client: &mut UnixStream,
    attachment: &ViewerId,
) -> TestResult<ControllerId> {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        match control(
            client,
            ControlAction::Acquire {
                attachment_id: attachment.clone(),
            },
        )? {
            ControlOutcome::Acquired { controller_id } => return Ok(controller_id),
            ControlOutcome::Rejected {
                reason: ControlRejectedReason::ControllerBusy,
            }
            | ControlOutcome::Unavailable {
                reason: ControlUnavailableReason::ServerBusy,
            } if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            other => {
                return Err(
                    format!("session cleanup did not release controller: {other:?}").into(),
                );
            }
        }
    }
}

fn shell_witness(
    client: &mut UnixStream,
    attachment: &ViewerId,
    grant: &ControllerId,
    sequence: u64,
    path: &Path,
    assignment: &str,
) -> TestResult<String> {
    let quoted = format!("'{}'", path.display().to_string().replace('\'', "'\\''"));
    assert_eq!(input(client, attachment, grant, sequence,
        format!("{assignment}printf '%s\\n' \"$route_state\" > {quoted}; stty size >> {quoted}; printf 'done\\n' >> {quoted}\n").into_bytes())?,
        ControlOutcome::InputAccepted {});
    observed(path, "done\n")
}

#[test]
fn one_session_routes_two_real_terminals_and_releases_every_grant_without_losing_shells()
-> TestResult {
    let server = RunningServer::start()?;
    let mut real = RealTerminalRuntime::start(&server)?;
    let runtime = real.directory.0.as_path();
    let (mut client, _) = connect_bounded_client(server.client_endpoint())?;
    for reference in [1, 2] {
        assert!(matches!(exchange(&mut client, &create(reference)?)?,
            ServerMessage::TerminalCreateResult(result) if result.outcome() == CreateOutcome::Accepted { state: CreateAcceptedState::AwaitingStart }));
        assert!(
            matches!(exchange(&mut client, &start(10 + reference, reference)?)?,
            ServerMessage::TerminalStartResult(result) if result.outcome() == StartOutcome::Accepted { geometry: geometry()?, transition: StartTransition::Started })
        );
    }
    let a = attach(&mut client, &id(1)?)?;
    let b = attach(&mut client, &id(2)?)?;
    assert_ne!(a, b);
    let grant_a = acquire(&mut client, &a)?;
    let grant_b = acquire(&mut client, &b)?;
    assert_ne!(grant_a, grant_b);

    let frame_a = next_frame(&mut client, &a)?;
    let frame_b = next_frame(&mut client, &b)?;
    assert_eq!(frame_a.attachment_id.to_text(), a.as_str());
    assert_eq!(frame_b.attachment_id.to_text(), b.as_str());
    assert_eq!(
        viewer(
            &mut client,
            Action::NeedSnapshot {
                attachment_id: b.clone(),
                sequence: frame_b.sequence,
                reason: ViewerSnapshotReason::StoreReset,
            }
        )?,
        Outcome::FeedbackAccepted {}
    );
    let replacement_b = next_frame(&mut client, &b)?;
    assert!(replacement_b.sequence > frame_b.sequence);
    assert_eq!(replacement_b.base_revision, None);
    assert_eq!(
        viewer(
            &mut client,
            Action::Poll {
                attachment_id: a.clone()
            }
        )?,
        Outcome::Empty {}
    );
    assert_eq!(
        viewer(
            &mut client,
            Action::FrameApplied {
                attachment_id: a.clone(),
                sequence: frame_a
                    .sequence
                    .checked_add(1)
                    .ok_or("frame sequence overflow")?,
                generation: frame_a.generation,
                revision: frame_a.revision,
            }
        )?,
        Outcome::Rejected {
            reason: ViewerRejectedReason::FeedbackMismatch
        }
    );
    acknowledge(&mut client, &a, &frame_a)?;
    acknowledge(&mut client, &b, &replacement_b)?;

    let (mut foreign, _) = connect_bounded_client(server.client_endpoint())?;
    assert_eq!(
        viewer(
            &mut foreign,
            Action::Poll {
                attachment_id: a.clone()
            }
        )?,
        Outcome::Rejected {
            reason: ViewerRejectedReason::TerminalUnavailable
        }
    );
    assert_eq!(
        control(
            &mut foreign,
            ControlAction::Acquire {
                attachment_id: a.clone()
            }
        )?,
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::ViewerUnavailable
        }
    );
    assert_eq!(
        input(&mut foreign, &a, &grant_a, 1, b"invalid\n".to_vec())?,
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::ViewerUnavailable
        }
    );
    assert_eq!(
        viewer(
            &mut foreign,
            Action::Detach {
                attachment_id: a.clone()
            }
        )?,
        Outcome::Detached {}
    );
    assert_eq!(
        input(&mut client, &b, &grant_a, 1, b"invalid\n".to_vec())?,
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::NotController
        }
    );
    // The same sequence is independently valid on both grants. Each shell keeps its own state.
    assert_eq!(
        shell_witness(
            &mut client,
            &a,
            &grant_a,
            1,
            &runtime.join("a-initial"),
            "route_state=alpha; "
        )?,
        "alpha\n24 80\ndone\n"
    );
    assert_eq!(
        shell_witness(
            &mut client,
            &b,
            &grant_b,
            1,
            &runtime.join("b-initial"),
            "route_state=beta; "
        )?,
        "beta\n24 80\ndone\n"
    );
    for (attachment, grant, size) in [
        (&a, &grant_a, Geometry::new(101, 31, 8, 16)?),
        (&b, &grant_b, Geometry::new(92, 27, 8, 16)?),
    ] {
        assert_eq!(
            control(
                &mut client,
                ControlAction::Resize {
                    attachment_id: attachment.clone(),
                    controller_id: grant.clone(),
                    sequence: NonZeroU64::new(2).ok_or("zero sequence")?,
                    geometry: size,
                }
            )?,
            ControlOutcome::Resized { geometry: size }
        );
    }
    assert_eq!(
        shell_witness(&mut client, &a, &grant_a, 3, &runtime.join("a-resized"), "")?,
        "alpha\n31 101\ndone\n"
    );
    assert_eq!(
        shell_witness(&mut client, &b, &grant_b, 3, &runtime.join("b-resized"), "")?,
        "beta\n27 92\ndone\n"
    );
    assert_eq!(
        viewer(
            &mut client,
            Action::Detach {
                attachment_id: a.clone()
            }
        )?,
        Outcome::Detached {}
    );
    assert_eq!(
        shell_witness(
            &mut client,
            &b,
            &grant_b,
            4,
            &runtime.join("b-survives"),
            ""
        )?,
        "beta\n27 92\ndone\n"
    );
    let a_replacement = attach(&mut client, &id(1)?)?;
    assert_ne!(a_replacement, a);
    let a_replacement_grant = acquire(&mut client, &a_replacement)?;
    assert_ne!(a_replacement_grant, grant_a);
    assert_eq!(
        viewer(
            &mut client,
            Action::Poll {
                attachment_id: a.clone()
            }
        )?,
        Outcome::Rejected {
            reason: ViewerRejectedReason::TerminalUnavailable
        }
    );
    assert_eq!(
        input(&mut client, &a, &grant_a, 4, b"invalid\n".to_vec())?,
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::ViewerUnavailable
        }
    );
    assert_eq!(
        viewer(&mut client, Action::Detach { attachment_id: a })?,
        Outcome::Detached {}
    );
    assert_eq!(
        shell_witness(
            &mut client,
            &a_replacement,
            &a_replacement_grant,
            1,
            &runtime.join("a-reattached"),
            ""
        )?,
        "alpha\n31 101\ndone\n"
    );
    assert_eq!(
        shell_witness(
            &mut client,
            &b,
            &grant_b,
            5,
            &runtime.join("b-after-stale-detach"),
            ""
        )?,
        "beta\n27 92\ndone\n"
    );
    drop(client);

    // Worker remains connected: successful reacquisition witnesses cleanup of both session grants.
    let (mut reopened, _) = connect_bounded_client(server.client_endpoint())?;
    for (reference, previous, expected, suffix) in [
        (1, &a_replacement, "alpha\n31 101\ndone\n", "a-reopened"),
        (2, &b, "beta\n27 92\ndone\n", "b-reopened"),
    ] {
        let attachment = attach_after_cleanup(&mut reopened, &id(reference)?)?;
        assert_ne!(&attachment, previous);
        let grant = acquire_after_cleanup(&mut reopened, &attachment)?;
        let frame = next_frame(&mut reopened, &attachment)?;
        acknowledge(&mut reopened, &attachment, &frame)?;
        assert_eq!(
            shell_witness(
                &mut reopened,
                &attachment,
                &grant,
                1,
                &runtime.join(suffix),
                ""
            )?,
            expected
        );
    }
    drop(reopened);
    drop(foreign);
    drop(real.worker.take());
    assert_eq!(running_inventory(&real.endpoint)?, 2);
    Ok(())
}
