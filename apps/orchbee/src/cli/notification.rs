//! One notification submission. Lost delivery replies are never retried automatically.
use super::{Failure, FailureKind, Output, failure_output};
use crate::{invocation::OutputFormat, localization::Locale};
use orchbee_workbench_protocol::Notification;

pub(crate) fn run(message: Notification, format: OutputFormat, locale: Locale) -> Output {
    #[cfg(windows)]
    {
        windows(message, format, locale)
    }
    #[cfg(not(windows))]
    {
        let _ = message;
        failure_output(
            Failure::new(FailureKind::ServiceUnavailable),
            Some(format),
            locale,
        )
    }
}

#[cfg(windows)]
fn windows(message: Notification, format: OutputFormat, locale: Locale) -> Output {
    use super::{
        output, query,
        source::{ArmedSignals, Finish, SignalWatch},
    };
    use orchbee_workbench_protocol::{
        NotificationFailure, NotificationOutcome, NotificationRequest,
    };
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
        let request = NotificationRequest {
            protocol_version: snapshot.protocol_version,
            request_id: query::request_id()?,
            site_id: snapshot.site_id,
            action: orchbee_workbench_protocol::NotificationAction::Show {
                notification: message,
            },
        };
        signals.check_before(deadline)?;
        match client.notification(request, deadline, signals.poll_fd()) {
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
                Some(NotificationOutcome::Show { reason, .. }) => (
                    0,
                    match reason {
                        orchbee_workbench_protocol::NotificationReason::Disabled => "disabled",
                        orchbee_workbench_protocol::NotificationReason::Shown => "shown",
                        orchbee_workbench_protocol::NotificationReason::Busy => "busy",
                        orchbee_workbench_protocol::NotificationReason::RateLimited => {
                            "rate-limited"
                        }
                        orchbee_workbench_protocol::NotificationReason::NoForegroundClient => {
                            "no-foreground-client"
                        }
                    },
                ),
                Some(NotificationOutcome::Failed { reason }) => (
                    if *reason == NotificationFailure::Unavailable {
                        4
                    } else {
                        3
                    },
                    "failed",
                ),
                Some(NotificationOutcome::Listening { .. }) => (8, "unknown"),
            };
            let document = serde_json::json!({"schema":"orchbee.notification.show","version":2,"source":{"kind":"local-runtime"},"result":{"reply":reply,"interrupted":interrupted,"cleanupFailed":cleanup_failed}});
            let encoded = match format {
                OutputFormat::Json => output::encode_document(&document),
                OutputFormat::Text => output::encode_text(locale.notification_result(label)),
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
