//! Submits and observes one retained Workspace-group close with exact attempt identity
//! under a cancellable total deadline. It does not close member terminals locally or retract
//! accepted closure when observation stops.

use super::{Answer, Observation, Wait, emit};
use crate::{
    cli::{
        Failure, FailureKind, Output,
        console_interrupt::{ArmedSignals, Finish},
        failure_output, query,
    },
    invocation::{OutputFormat, WorkspaceGroupCloseCommand},
    localization::Locale,
};
use orchbee_workbench_protocol::{
    SessionId, WorkbenchRequestId, WorkbenchWorkspaceGroupCloseRequest, WorkspaceGroupCloseAction,
};
use std::{
    thread,
    time::{Duration, Instant},
};

pub(super) fn run(
    command: &WorkspaceGroupCloseCommand,
    operation_id: WorkbenchRequestId,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let mut answer = Answer {
        operation_id,
        observation: Observation::Unavailable {
            reason: "not-submitted",
        },
        wait: None,
    };
    let mut session_id = None;
    let result = observe(command, &mut answer, &mut session_id, &mut signals);
    let mut finalization = None;
    let result = match signals.finish() {
        Ok(Finish::Clear) => result,
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            if result.is_err()
                || answer.pending()
                || matches!(
                    answer.observation,
                    Observation::Unknown { .. } | Observation::Unavailable { .. }
                )
            {
                answer.wait = Some(Wait::Cancelled);
            }
            Ok(())
        }
        Err(_) => Err(Failure::new(FailureKind::InternalFailure)),
    };
    let result = if result.is_err()
        && matches!(
            answer.observation,
            Observation::Server { .. } | Observation::Unknown { .. }
        ) {
        answer.wait = Some(Wait::ObservationFailed);
        Ok(())
    } else {
        result
    };
    let mut output = match result {
        Ok(()) => emit(session_id.as_ref(), answer, format, locale),
        Err(error) => failure_output(error, Some(format), locale),
    };
    output.finalization = finalization;
    output
}

fn observe(
    command: &WorkspaceGroupCloseCommand,
    answer: &mut Answer,
    session_id: &mut Option<SessionId>,
    signals: &mut ArmedSignals,
) -> Result<(), Failure> {
    let (detached, timeout, read) = match command {
        WorkspaceGroupCloseCommand::Start {
            detach, timeout_ms, ..
        } => (*detach, *timeout_ms, false),
        WorkspaceGroupCloseCommand::Resume {
            detach, timeout_ms, ..
        } => (*detach, *timeout_ms, false),
        WorkspaceGroupCloseCommand::Status { .. } => (false, 5_000, true),
    };
    let deadline = Instant::now() + Duration::from_millis(u64::from(timeout));
    if interrupted(answer, signals, deadline)? {
        return Ok(());
    }
    let (mut client, _, snapshot) = query::snapshot_session_until(deadline)?;
    *session_id = Some(snapshot.session_id.clone());
    if interrupted(answer, signals, deadline)? {
        return Ok(());
    }
    let action = match command {
        WorkspaceGroupCloseCommand::Start { target, .. } => {
            let project = crate::cli::results::resolve_project(&snapshot, target)?;
            WorkspaceGroupCloseAction::Start {
                operation_id: answer.operation_id.clone(),
                project_id: project.project_id.clone(),
            }
        }
        WorkspaceGroupCloseCommand::Status { .. } => WorkspaceGroupCloseAction::Read {
            operation_id: answer.operation_id.clone(),
        },
        WorkspaceGroupCloseCommand::Resume {
            expected_attempt, ..
        } => WorkspaceGroupCloseAction::Resume {
            operation_id: answer.operation_id.clone(),
            expected_attempt: *expected_attempt,
        },
    };
    let request = WorkbenchWorkspaceGroupCloseRequest {
        protocol_version: snapshot.protocol_version,
        request_id: query::request_id()?,
        session_id: snapshot.session_id.clone(),
        action,
    };
    if interrupted(answer, signals, deadline)? {
        return Ok(());
    }
    match client.submit_workspace_group_close(
        request,
        deadline.min(Instant::now() + Duration::from_secs(2)),
        signals.poll_fd(),
    ) {
        Ok(reply) => answer.observation = Observation::Server { reply },
        Err(error) => {
            answer.observation = if error.submitted && !read {
                Observation::Unknown {
                    reason: error.reason,
                }
            } else {
                Observation::Unavailable {
                    reason: error.reason,
                }
            };
            interrupted(answer, signals, deadline)?;
            return Ok(());
        }
    }
    if read || !answer.pending() {
        return Ok(());
    }
    if detached {
        answer.wait = Some(Wait::Detached);
        return Ok(());
    }
    while answer.pending() {
        if interrupted(answer, signals, deadline)? {
            return Ok(());
        }
        thread::sleep(
            Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now())),
        );
        if interrupted(answer, signals, deadline)? {
            return Ok(());
        }
        let request = WorkbenchWorkspaceGroupCloseRequest {
            protocol_version: snapshot.protocol_version,
            request_id: query::request_id()?,
            session_id: snapshot.session_id.clone(),
            action: WorkspaceGroupCloseAction::Read {
                operation_id: answer.operation_id.clone(),
            },
        };
        match client.submit_workspace_group_close(
            request,
            deadline.min(Instant::now() + Duration::from_secs(2)),
            signals.poll_fd(),
        ) {
            Ok(reply) if same_attempt(&answer.observation, &reply) => {
                answer.observation = Observation::Server { reply }
            }
            Ok(_) => {
                answer.wait = Some(Wait::ObservationFailed);
                return Ok(());
            }
            Err(_) => {
                if !interrupted(answer, signals, deadline)? {
                    answer.wait = Some(Wait::TransportLost);
                }
                return Ok(());
            }
        }
    }
    Ok(())
}

pub(super) fn same_attempt(
    previous: &Observation,
    next: &orchbee_workbench_protocol::WorkbenchWorkspaceGroupCloseResult,
) -> bool {
    use orchbee_workbench_protocol::WorkspaceGroupCloseOutcome as Outcome;
    let Observation::Server { reply } = previous else {
        return false;
    };
    let (
        Outcome::Available {
            project_id: before_project,
            attempt: before_attempt,
            members: before_members,
            ..
        },
        Outcome::Available {
            project_id,
            attempt,
            members,
            ..
        },
    ) = (&reply.outcome, &next.outcome)
    else {
        return false;
    };
    before_project == project_id
        && before_attempt == attempt
        && before_members.len() == members.len()
        && before_members.iter().zip(members).all(|(before, after)| {
            before.workspace_id == after.workspace_id
                && before.label == after.label
                && before.close.as_ref().is_none_or(|child| {
                    after.close.as_ref().is_some_and(|next| {
                        child.request_id == next.request_id
                            && (!child.is_closed() || child == next)
                            && child.observation.as_ref().is_none_or(|old| {
                                next.observation.as_ref().is_some_and(|new| {
                                    new.attempt_generation >= old.attempt_generation
                                })
                            })
                    })
                })
        })
}

fn interrupted(
    answer: &mut Answer,
    signals: &mut ArmedSignals,
    deadline: Instant,
) -> Result<bool, Failure> {
    if signals
        .take()
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?
        .is_some()
    {
        answer.wait = Some(Wait::Cancelled);
        Ok(true)
    } else if Instant::now() >= deadline {
        answer.wait = Some(Wait::TimedOut);
        Ok(true)
    } else {
        Ok(false)
    }
}
