//! Running-Site configuration control. A submitted reload is never automatically retried.
use super::{Failure, FailureKind, Output, failure_output};
use crate::{invocation::OutputFormat, localization::Locale};
use orchbee_workbench_protocol::ConfigurationAction;

pub(crate) fn run(action: ConfigurationAction, format: OutputFormat, locale: Locale) -> Output {
    #[cfg(windows)]
    {
        windows(action, format, locale)
    }
    #[cfg(not(windows))]
    {
        let _ = action;
        failure_output(
            Failure::new(FailureKind::ServiceUnavailable),
            Some(format),
            locale,
        )
    }
}

#[cfg(windows)]
fn windows(action: ConfigurationAction, format: OutputFormat, locale: Locale) -> Output {
    use super::{
        output, query,
        source::{ArmedSignals, Finish, SignalWatch},
    };
    use orchbee_workbench_protocol::{
        ConfigurationFailure, ConfigurationOutcome, ConfigurationRequest,
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
        let request = ConfigurationRequest {
            protocol_version: snapshot.protocol_version,
            request_id: query::request_id()?,
            site_id: snapshot.site_id,
            action,
        };
        signals.check_before(deadline)?;
        match client.configuration(request, deadline, signals.poll_fd()) {
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
            let (status, message, revision, restart) =
                match reply.as_ref().map(|reply| &reply.outcome) {
                    None => (
                        if action == ConfigurationAction::Reload {
                            8
                        } else {
                            4
                        },
                        "unknown",
                        None,
                        0,
                    ),
                    Some(
                        ConfigurationOutcome::Current { snapshot }
                        | ConfigurationOutcome::Reloaded { snapshot, .. },
                    ) => (
                        0,
                        if !snapshot.diagnostics.items.is_empty()
                            || snapshot.diagnostics.omitted_count > 0
                        {
                            "warning"
                        } else if snapshot.revision.value() == 0 {
                            "startup"
                        } else if snapshot.restart_required.is_empty() {
                            "published"
                        } else {
                            "restart"
                        },
                        Some(snapshot.revision.value()),
                        snapshot.restart_required.len(),
                    ),
                    Some(ConfigurationOutcome::Failed { reason, .. }) => (
                        match reason {
                            ConfigurationFailure::Denied
                            | ConfigurationFailure::SiteMismatch
                            | ConfigurationFailure::Busy => 3,
                            ConfigurationFailure::Unavailable => 4,
                            _ => 1,
                        },
                        "failed",
                        None,
                        0,
                    ),
                };
            let document = serde_json::json!({"schema":"orchbee.site.configuration","version":9,"source":{"kind":"local-runtime"},"result":{"action":action,"reply":reply,"interrupted":interrupted,"cleanupFailed":cleanup_failed}});
            let encoded = match format {
                OutputFormat::Json => output::encode_document(&document),
                OutputFormat::Text => {
                    let mut text = locale.cli_runtime_configuration(message, revision, restart);
                    if let Some(
                        ConfigurationOutcome::Current { snapshot }
                        | ConfigurationOutcome::Reloaded { snapshot, .. },
                    ) = reply.as_ref().map(|reply| &reply.outcome)
                    {
                        text.push_str(&locale.cli_configuration_notifications(
                            &super::escape::escape(&snapshot.notification_profile),
                        ));
                        text.push_str(&locale.cli_configuration_shell(
                            &super::escape::escape(&snapshot.default_shell),
                            snapshot.shell_mode.as_str(),
                        ));
                        text.push_str(&locale.cli_configuration_title(&super::escape::escape(
                            &snapshot.window_title,
                        )));
                        text.push_str(&format!(
                            "spacesSidebarProfile={}\n",
                            super::escape::escape(&snapshot.spaces_sidebar_profile)
                        ));
                        text.push_str(&format!("themeName={}\n", snapshot.theme_name.as_str()));
                        text.push_str(&locale.cli_configuration_sidebar(&super::escape::escape(
                            &snapshot.agent_sidebar_profile,
                        )));
                        text.push_str(&locale.cli_configuration_diagnostics(
                            snapshot.diagnostics.items.len(),
                            snapshot.diagnostics.omitted_count,
                        ));
                        for diagnostic in &snapshot.diagnostics.items {
                            text.push_str(&locale.cli_configuration_diagnostic(
                                &super::escape::escape(&diagnostic.field),
                                &diagnostic.reason,
                                match diagnostic.disposition {
                                    orchbee_workbench_protocol::ConfigurationDiagnosticDisposition::Defaulted => "defaulted",
                                    orchbee_workbench_protocol::ConfigurationDiagnosticDisposition::Disabled => "disabled",
                                    orchbee_workbench_protocol::ConfigurationDiagnosticDisposition::Ignored => "ignored",
                                    orchbee_workbench_protocol::ConfigurationDiagnosticDisposition::Unsupported => "unsupported",
                                },
                            ));
                        }
                    }
                    output::encode_text(&text)
                }
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
