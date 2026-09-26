use super::{Answer, Observation, Wait, emit};
use crate::{
    cli::{
        Failure, FailureKind, Output, failure_output, query,
        source::{ArmedSignals, Finish, SignalWatch},
    },
    invocation::{OutputFormat, WorktreeCommand, WorktreeStartKind},
    localization::Locale,
};
use orchbee_workbench_protocol::{
    SiteId, WorkbenchRequestId, WorkbenchWorktreeAction, WorkbenchWorktreeRequest,
};
use std::{
    thread,
    time::{Duration, Instant},
};

pub(super) fn run(
    command: &WorktreeCommand,
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
        post_open: matches!(command, WorktreeCommand::Start(start) if start.focus)
            .then(super::post_open::Receipt::new),
    };
    let mut site_id = None;
    let result = observe(command, &mut answer, &mut site_id, &mut signals);
    let mut finalization = None;
    let result = match signals.finish() {
        Ok(Finish::Clear) => result,
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            if answer
                .post_open
                .as_ref()
                .is_some_and(|receipt| !receipt.complete())
                || result.is_err()
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
        Ok(()) => emit(site_id.as_ref(), answer, format, locale),
        Err(error) => failure_output(error, Some(format), locale),
    };
    output.finalization = finalization;
    output
}

fn observe(
    command: &WorktreeCommand,
    answer: &mut Answer,
    site_id: &mut Option<SiteId>,
    signals: &mut ArmedSignals,
) -> Result<(), Failure> {
    let (detached, timeout, read) = match command {
        WorktreeCommand::Start(start) => (start.detach, start.timeout_ms, false),
        WorktreeCommand::Resume {
            detach, timeout_ms, ..
        } => (*detach, *timeout_ms, false),
        WorktreeCommand::Status { .. } => (false, 5_000, true),
    };
    let deadline = Instant::now() + Duration::from_millis(u64::from(timeout));
    if interrupted(answer, signals, deadline)? {
        return Ok(());
    }
    let (mut client, _, snapshot) = query::snapshot_session_until(deadline)?;
    *site_id = Some(snapshot.site_id.clone());
    if interrupted(answer, signals, deadline)? {
        return Ok(());
    }
    let mut selected = None;
    let action = match command {
        WorktreeCommand::Start(start) => {
            let (project_id, intent) = if matches!(start.kind, WorktreeStartKind::Remove { .. }) {
                super::selection::intent(start, &snapshot, std::env::current_dir)?
            } else {
                let prepared = crate::cli::worktree_source::windows::prepare(
                    crate::cli::worktree_source::windows::Request {
                        project: &start.project,
                        workspace: &start.workspace,
                        cwd: &start.cwd,
                        client: start.client.as_ref(),
                        focus: start.focus,
                        require_project: true,
                    },
                    &mut client,
                    &snapshot,
                    deadline,
                    signals,
                )?;
                selected = prepared.client;
                if let Some(receipt) = &mut answer.post_open {
                    receipt.client_id = selected.as_ref().map(|client| client.client_id.clone());
                }
                super::selection::intent_from_source(
                    start,
                    prepared
                        .project
                        .ok_or_else(|| Failure::new(FailureKind::ProtocolInvalid))?,
                    prepared.source,
                )?
            };
            WorkbenchWorktreeAction::Start {
                operation_id: answer.operation_id.clone(),
                project_id,
                intent,
            }
        }
        WorktreeCommand::Status { .. } => WorkbenchWorktreeAction::Read {
            operation_id: answer.operation_id.clone(),
        },
        WorktreeCommand::Resume {
            expected_attempt, ..
        } => WorkbenchWorktreeAction::Resume {
            operation_id: answer.operation_id.clone(),
            expected_attempt: *expected_attempt,
        },
    };
    let request = WorkbenchWorktreeRequest {
        protocol_version: snapshot.protocol_version,
        request_id: query::request_id()?,
        site_id: snapshot.site_id.clone(),
        action,
    };
    if interrupted(answer, signals, deadline)? {
        return Ok(());
    }
    match client.submit_worktree_operation(
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
    if read {
        return Ok(());
    }
    if detached && answer.pending() {
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
        let request = WorkbenchWorktreeRequest {
            protocol_version: snapshot.protocol_version,
            request_id: query::request_id()?,
            site_id: snapshot.site_id.clone(),
            action: WorkbenchWorktreeAction::Read {
                operation_id: answer.operation_id.clone(),
            },
        };
        match client.submit_worktree_operation(
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
    if let (Some(receipt), Some(selected)) = (&mut answer.post_open, selected.as_ref())
        && let Some((project, workspace)) = super::post_open::opened(&answer.observation)
    {
        receipt.stage = "focus-observation";
        if let Err(failure) = super::focus_windows::execute(
            &mut client,
            &snapshot,
            (project, workspace),
            selected,
            (deadline, signals),
            receipt,
        ) {
            receipt.failure = Some(failure.kind);
        }
    }
    Ok(())
}

fn same_attempt(
    previous: &Observation,
    next: &orchbee_workbench_protocol::WorkbenchWorktreeResult,
) -> bool {
    use orchbee_workbench_protocol::WorkbenchWorktreeOutcome as Outcome;
    let Observation::Server { reply } = previous else {
        return false;
    };
    matches!((&reply.outcome, &next.outcome), (Outcome::Available {project_id: before_project, attempt: before_attempt, ..},
        Outcome::Available {project_id, attempt, ..}) if before_project == project_id && before_attempt == attempt)
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

#[cfg(test)]
mod tests {
    use super::*;
    use orchbee_workbench_protocol::{
        WorkbenchProjectId, WorkbenchWorktreeOutcome, WorkbenchWorktreeResult,
    };

    #[test]
    fn later_observation_cannot_replace_known_project_or_attempt_with_unrelated_state() {
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../schemas/workbench/v1/workbench-worktree-conformance-v1.json"
        ))
        .unwrap();
        let mut value = corpus["valid"][5]["document"].clone();
        value.as_object_mut().unwrap().remove("kind");
        let reply: WorkbenchWorktreeResult = serde_json::from_value(value).unwrap();
        let previous = Observation::Server {
            reply: reply.clone(),
        };
        assert!(same_attempt(&previous, &reply));
        for case in 0..3 {
            let mut changed = reply.clone();
            match case {
                0 => {
                    let WorkbenchWorktreeOutcome::Available { attempt, .. } = &mut changed.outcome
                    else {
                        unreachable!()
                    };
                    *attempt += 1;
                }
                1 => {
                    let WorkbenchWorktreeOutcome::Available { project_id, .. } =
                        &mut changed.outcome
                    else {
                        unreachable!()
                    };
                    *project_id =
                        WorkbenchProjectId::parse("ffffffffffffffffffffffffffffffff").unwrap();
                }
                _ => changed.outcome = WorkbenchWorktreeOutcome::NotFound {},
            }
            assert!(!same_attempt(&previous, &changed));
        }
    }
}
