//! One invocation submission, or a finite newest-first log read under one deadline.
use super::super::{
    Failure, FailureKind, Output, failure_output, output, query,
    source::{ArmedSignals, Finish, SignalWatch},
};
use crate::{
    invocation::{OutputFormat, plugins::PluginCommand},
    localization::Locale,
};
use orchbee_workbench_protocol::{PluginAction, PluginOutcome, PluginRequest, PluginResult};
use std::time::{Duration, Instant};
pub(super) fn run(command: PluginCommand, format: OutputFormat, locale: Locale) -> Output {
    let deadline = Instant::now() + Duration::from_secs(10);
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
    let mut operation_id = None;
    let operation = if matches!(command, PluginCommand::Invoke { .. }) {
        "invoke"
    } else {
        "logs"
    };
    let submitted = (|| {
        signals.check_before(deadline)?;
        let (mut client, snapshot) = query::stream_snapshot(deadline, signals.poll_fd())?;
        let (mut action, mut remaining) = match command {
            PluginCommand::Invoke {
                plugin_id,
                action_id,
                context,
            } => {
                let id = query::request_id()?.as_str().to_owned();
                operation_id = Some(id.clone());
                (
                    PluginAction::Invoke {
                        operation_id: id,
                        plugin_id,
                        action_id,
                        context,
                    },
                    0,
                )
            }
            PluginCommand::Logs { plugin_id, limit } => (
                PluginAction::Logs {
                    plugin_id,
                    cursor: None,
                    limit: limit.min(4) as u8,
                },
                limit,
            ),
            _ => unreachable!("only runtime commands routed here"),
        };
        let mut replies = Vec::new();
        let mut encoded_bytes = 0;
        loop {
            signals.check_before(deadline)?;
            let request = PluginRequest {
                protocol_version: snapshot.protocol_version,
                request_id: query::request_id()?,
                site_id: snapshot.site_id.clone(),
                action: action.clone(),
            };
            let reply = match client.plugin(request, deadline, signals.poll_fd()) {
                Ok(reply) => reply,
                Err(error) if error.submitted && action.mutating() => return Ok((replies, false)),
                Err(_) => {
                    signals.check_before(deadline)?;
                    return Err(Failure::new(FailureKind::TransportLoss));
                }
            };
            let next = match (&action, &reply.outcome) {
                (PluginAction::Logs { plugin_id, .. }, PluginOutcome::Logs { logs, next, .. }) => {
                    remaining = remaining.saturating_sub(logs.len() as u16);
                    next.as_ref()
                        .filter(|_| remaining > 0)
                        .map(|cursor| PluginAction::Logs {
                            plugin_id: plugin_id.clone(),
                            cursor: Some(cursor.clone()),
                            limit: remaining.min(4) as u8,
                        })
                }
                _ => None,
            };
            let value = serde_json::to_value(&reply)
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
            encoded_bytes += output::encode_record(&value)
                .map_err(|_| Failure::new(FailureKind::OutputTooLarge))?
                .len();
            if encoded_bytes > output::LIMIT - 4096 {
                return Err(Failure::new(FailureKind::OutputTooLarge));
            }
            replies.push(reply);
            match next {
                Some(next) => action = next,
                None => return Ok((replies, true)),
            }
        }
    })();
    let mut finalization = None;
    let (interrupted, cleanup_failed) = match signals.finish() {
        Ok(Finish::Clear) => (false, false),
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            (true, false)
        }
        Err(_) => (false, true),
    };
    let mut output = match submitted {
        Err(failure) => failure_output(failure, Some(format), locale),
        Ok((replies, complete)) => {
            let status = if !complete
                || replies
                    .iter()
                    .any(|r| matches!(r.outcome, PluginOutcome::Unknown { .. }))
            {
                8
            } else if replies
                .iter()
                .any(|r| matches!(r.outcome, PluginOutcome::Failed { .. }))
            {
                3
            } else {
                0
            };
            let document = if operation == "logs" {
                history_document(&replies, complete, interrupted, cleanup_failed)
            } else {
                serde_json::json!({"schema":"orchbee.plugin.runtime","version":1,"source":{"kind":"local-runtime"},"operation":operation,"result":{"operationId":operation_id,"replies":replies,"complete":complete,"interrupted":interrupted,"cleanupFailed":cleanup_failed}})
            };
            let encoded = match format {
                OutputFormat::Json => output::encode_document(&document),
                OutputFormat::Text => render(&replies, complete, operation_id.as_deref(), locale),
            };
            match encoded {
                Ok(stdout) => Output {
                    status: if cleanup_failed {
                        1
                    } else if interrupted {
                        130
                    } else {
                        status
                    },
                    stdout,
                    stderr: String::new(),
                    finalization: None,
                },
                Err(_) => failure_output(
                    Failure::new(FailureKind::OutputTooLarge),
                    Some(format),
                    locale,
                ),
            }
        }
    };
    output.finalization = finalization;
    output
}
fn render(
    replies: &[PluginResult],
    complete: bool,
    operation_id: Option<&str>,
    locale: Locale,
) -> Result<Vec<u8>, output::TooLarge> {
    use super::super::escape::escape;
    output::render_text(|out| {
        if let Some(id) = operation_id {
            out.write(format_args!(
                "{}: {id}\n",
                locale.plugin_runtime_state("operation-id")
            ))?;
        }
        if !complete {
            out.push(locale.plugin_runtime_state("unknown"))?;
            out.push("\n")?;
        }
        for reply in replies.iter().rev() {
            match &reply.outcome {
                PluginOutcome::Failed { reason } => {
                    out.push(locale.plugin_runtime_failure(*reason))?;
                    out.push("\n")?;
                }
                PluginOutcome::Unknown { .. } => {
                    out.push(locale.plugin_runtime_state("unknown"))?;
                    out.push("\n")?;
                }
                outcome => {
                    let logs: &[orchbee_workbench_protocol::PluginCommandLog] = match outcome {
                        PluginOutcome::Started { log, .. } => std::slice::from_ref(log),
                        PluginOutcome::Logs { logs, .. } => logs,
                        _ => &[],
                    };
                    for log in logs.iter().rev() {
                        if let Some(code) = log.exit_code {
                            out.write(format_args!(
                                "{}: {code}\n",
                                locale.plugin_runtime_state("exit-code")
                            ))?;
                        }
                        out.write(format_args!(
                            "{}\t{}.{}\t{}\n",
                            escape(&log.job_id),
                            escape(&log.plugin_id),
                            escape(&log.command_id),
                            locale.plugin_runtime_state(if log.finished_ms.is_none() {
                                "running"
                            } else if log.failure.is_some() {
                                "failed"
                            } else {
                                "succeeded"
                            })
                        ))?;
                        if !log.stdout.is_empty() {
                            out.write(format_args!("stdout: {}\n", escape(&log.stdout)))?;
                        }
                        if !log.stderr.is_empty() {
                            out.write(format_args!("stderr: {}\n", escape(&log.stderr)))?;
                        }
                        if log.stdout_truncated || log.stderr_truncated {
                            out.push(locale.plugin_runtime_state("truncated"))?;
                            out.push("\n")?;
                        }
                        if let Some(reason) = log.failure {
                            out.push(locale.plugin_command_failure(reason))?;
                            out.push("\n")?;
                        }
                    }
                }
            }
        }
        Ok(())
    })
}

fn history_document(
    replies: &[PluginResult],
    complete: bool,
    interrupted: bool,
    cleanup_failed: bool,
) -> serde_json::Value {
    let worker = replies.iter().find_map(|r| match &r.outcome {
        PluginOutcome::Logs { worker, .. } => Some(worker),
        _ => None,
    });
    let failure = replies.iter().find_map(|r| match r.outcome {
        PluginOutcome::Failed { reason } => Some(reason),
        _ => None,
    });
    let mut logs: Vec<_> = replies
        .iter()
        .flat_map(|r| match &r.outcome {
            PluginOutcome::Logs { logs, .. } => logs.as_slice(),
            _ => &[],
        })
        .collect();
    // Internal cursor pages are descending and nonoverlapping; presentation is chronological.
    logs.reverse();
    serde_json::json!({"schema":"orchbee.plugin.logs","version":1,"source":{"kind":"local-runtime"},"result":{"worker":worker,"logs":logs,"failure":failure,"complete":complete,"interrupted":interrupted,"cleanupFailed":cleanup_failed}})
}
