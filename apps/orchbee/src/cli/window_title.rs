//! One outer-title submission. Lost delivery replies are never retried automatically.
use super::{Failure, FailureKind, Output, failure_output};
use crate::{invocation::OutputFormat, localization::Locale};
use orchbee_workbench_protocol::WindowTitle;

pub(crate) fn run(title: Option<WindowTitle>, format: OutputFormat, locale: Locale) -> Output {
    #[cfg(windows)]
    {
        windows(title, format, locale)
    }
    #[cfg(not(windows))]
    {
        let _ = title;
        failure_output(
            Failure::new(FailureKind::ServiceUnavailable).details(
                crate::cli::stage_reason_details("platform", "unsupported-platform"),
            ),
            Some(format),
            locale,
        )
    }
}

#[cfg(windows)]
fn windows(title: Option<WindowTitle>, format: OutputFormat, locale: Locale) -> Output {
    use super::{
        output, query,
        source::{ArmedSignals, Finish, SignalWatch},
    };
    use orchbee_workbench_protocol::{WindowTitleFailure, WindowTitleOutcome, WindowTitleRequest};
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
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
    let submitted = (|| {
        signals.check_before(deadline)?;
        let (mut client, snapshot) = query::stream_snapshot(deadline, signals.poll_fd())?;
        let request = WindowTitleRequest {
            protocol_version: snapshot.protocol_version,
            request_id: query::request_id()?,
            site_id: snapshot.site_id,
            action: match title {
                Some(title) => orchbee_workbench_protocol::WindowTitleAction::Set { title },
                None => orchbee_workbench_protocol::WindowTitleAction::Clear {},
            },
        };
        signals.check_before(deadline)?;
        match client.window_title(request, deadline, signals.poll_fd()) {
            Ok(reply) => Ok(Some(reply)),
            Err(error) if error.submitted => Ok(None),
            Err(_) => {
                signals.check_before(deadline)?;
                Err(Failure::new(FailureKind::TransportLoss))
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
    let mut result = match submitted {
        Err(failure) => failure_output(failure, Some(format), locale),
        Ok(reply) => {
            let (status, label) = match reply.as_ref().map(|reply| &reply.outcome) {
                None => (8, "unknown"),
                Some(WindowTitleOutcome::Published { reason, .. }) => (
                    0,
                    match reason {
                        orchbee_workbench_protocol::WindowTitleReason::Set => "set",
                        orchbee_workbench_protocol::WindowTitleReason::Cleared => "cleared",
                        orchbee_workbench_protocol::WindowTitleReason::NoForegroundClient => {
                            "no-foreground-client"
                        }
                    },
                ),
                Some(WindowTitleOutcome::Failed { reason }) => (
                    if *reason == WindowTitleFailure::Unavailable {
                        4
                    } else {
                        3
                    },
                    "failed",
                ),
                Some(WindowTitleOutcome::Listening { .. }) => (8, "unknown"),
            };
            let document = serde_json::json!({"schema":"orchbee.terminal.title","version":1,"source":{"kind":"local-runtime"},"result":{"reply":reply,"interrupted":interrupted,"cleanupFailed":cleanup_failed}});
            let encoded = match format {
                OutputFormat::Json => output::encode_document(&document),
                OutputFormat::Text => output::encode_text(locale.window_title_result(label)),
            };
            match encoded {
                Ok(stdout) => Output {
                    status: if cleanup_failed { 1 } else { status },
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
    result.finalization = finalization;
    result
}
