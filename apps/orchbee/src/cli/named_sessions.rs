use super::source::{ArmedSignals, Finish, SignalWatch};
use super::{Failure, FailureKind, Output, config, failure_output, output};
use crate::{
    invocation::{NamedSessionCommand, OutputFormat},
    localization::Locale,
    presentation::ProcessResult,
};
use orchbee_local_client_protocol::{
    ConnectClientError, ServerMode, exchange_server_metadata_before,
};
use orchbee_local_site_runtime::{
    ActivationState, ModeCheckedLocalSiteRendezvous as Rendezvous, RendezvousError,
    SiteLifetimeObservation, WindowsNamedSiteCatalog as Catalog, WindowsNamedSiteDeleteFailure,
    WindowsNamedSiteDeleteOutcome, WindowsNamedSiteDeleteReason as Reason, WindowsNamedSiteEntry,
};
use serde_json::{Value, json};
use std::{
    io,
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SessionState {
    Running,
    Stopping,
    Stopped,
    Absent,
    Starting,
    Deleting,
    RecoveryRequired,
    Recoverable,
}
impl SessionState {
    pub(crate) fn token(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Absent => "absent",
            Self::Starting => "starting",
            Self::Deleting => "deleting",
            Self::RecoveryRequired => "recovery-required",
            Self::Recoverable => "recoverable",
        }
    }
    fn running(self) -> Option<bool> {
        match self {
            Self::Running | Self::Stopping => Some(true),
            Self::Stopped | Self::Absent | Self::Recoverable => Some(false),
            _ => None,
        }
    }
}

pub(crate) fn run(command: &NamedSessionCommand, format: OutputFormat, locale: Locale) -> Output {
    match command {
        NamedSessionCommand::Attach { mouse, .. } => {
            let result = match crate::local_startup::run(locale, *mouse) {
                Ok(()) => ProcessResult {
                    status: 0,
                    stdout: "",
                    stderr: "",
                },
                Err(failure) => ProcessResult {
                    status: 1,
                    stdout: "",
                    stderr: locale.startup_failure(failure),
                },
            };
            super::process_result(result, locale)
        }
        NamedSessionCommand::Stop { command, .. } => super::site_stop(command, format, locale),
        NamedSessionCommand::List => list(format, locale),
        NamedSessionCommand::Delete { name } => delete(name, format, locale),
    }
}

fn delete(
    name: &orchbee_local_site_runtime::WindowsSiteName,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if name.as_str() == "default" {
        return delete_output(
            name.as_str(),
            Err(WindowsNamedSiteDeleteFailure {
                reason: Reason::DefaultSession,
                pending: false,
            }),
            format,
            locale,
        );
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => return failed(Failure::new(FailureKind::InternalFailure), format, locale),
    };
    let result = (|| {
        signals.check_before(deadline)?;
        let (base, _) = crate::local_activation::runtime_location().map_err(local_error)?;
        let local_app_data = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| Failure::new(FailureKind::CheckFailed))?;
        let catalog = Catalog::try_lock(&base, &local_app_data).map_err(local_error)?;
        Ok::<_, Failure>(catalog.delete(name, &local_app_data, deadline, || {
            signals.take().map(|signal| signal.is_some())
        }))
    })();
    let pending = matches!(&result, Ok(Err(failure)) if failure.pending);
    let mut finalization = None;
    let result = match signals.finish() {
        Ok(Finish::Clear) => result,
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            match result {
                Ok(_) => Ok(Err(WindowsNamedSiteDeleteFailure {
                    reason: Reason::Interrupted,
                    pending,
                })),
                Err(_) => Err(Failure::new(FailureKind::Cancelled)),
            }
        }
        Err(_) => Err(Failure::new(FailureKind::InternalFailure)),
    };
    let mut output = match result {
        Ok(result) => delete_output(name.as_str(), result, format, locale),
        Err(failure) => failed(failure, format, locale),
    };
    output.finalization = finalization;
    output
}

fn failed(failure: Failure, format: OutputFormat, locale: Locale) -> Output {
    failure_output(
        failure.source(config::local_runtime_source()),
        Some(format),
        locale,
    )
}
fn local_error(error: io::Error) -> Failure {
    Failure::new(if error.kind() == io::ErrorKind::TimedOut {
        FailureKind::Timeout
    } else {
        FailureKind::CheckFailed
    })
}

fn list(format: OutputFormat, locale: Locale) -> Output {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => return failed(Failure::new(FailureKind::InternalFailure), format, locale),
    };
    let collected = (|| {
        let (base, _) = crate::local_activation::runtime_location().map_err(local_error)?;
        let local = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| Failure::new(FailureKind::CheckFailed))?;
        let entries = Catalog::list(&base, &local, deadline).map_err(local_error)?;
        let mut documents = Vec::new();
        let mut text = String::new();
        for entry in entries {
            if Instant::now() >= deadline {
                return Err(Failure::new(FailureKind::Timeout));
            }
            if signals
                .take()
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?
                .is_some()
            {
                return Err(Failure::new(FailureKind::Cancelled));
            }
            let (state, pid) = observe(&entry, deadline, &signals)?;
            let [endpoint, _] = Rendezvous::socket_paths(&entry.runtime);
            let path = Value::Object(serde_json::Map::from_iter([config::path_detail(
                &entry.runtime,
            )]));
            documents.push(
                json!({"name":entry.name.as_str(),"default":entry.name.as_str()=="default",
                "state":state.token(),"running":state.running(),"pid":pid,
                "runtimeDirectory":path,"endpoint":endpoint.to_str()}),
            );
            text.push_str(&format!(
                "{}\t{}\t{}\n",
                entry.name.as_str(),
                locale.cli_session_state(state),
                super::escape::escape(&entry.runtime.to_string_lossy())
            ));
        }
        Ok((json!({"sessions":documents}), text))
    })();
    let mut finalization = None;
    let collected = match signals.finish() {
        Ok(Finish::Clear) => collected,
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            Err(Failure::new(FailureKind::Cancelled))
        }
        Err(_) => Err(Failure::new(FailureKind::InternalFailure)),
    };
    let mut output = match collected {
        Ok((result, text)) => emit("orchbee.session.list", result, text, 0, format, locale),
        Err(failure) => failed(failure, format, locale),
    };
    output.finalization = finalization;
    output
}

fn observe(
    entry: &WindowsNamedSiteEntry,
    deadline: Instant,
    signals: &ArmedSignals,
) -> Result<(SessionState, Option<u32>), Failure> {
    if entry.deleting {
        return Ok((SessionState::Deleting, None));
    }
    if entry.cold_recoverable().map_err(local_error)? {
        return Ok((SessionState::Recoverable, None));
    }
    let site = match Rendezvous::from_runtime_directory(&entry.runtime) {
        Ok(site) => site,
        Err(RendezvousError::System { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
            let state = match entry.absent_runtime_state().map_err(local_error)? {
                None => SessionState::Absent,
                Some(ActivationState::Stopped { .. }) => SessionState::Stopped,
                _ => SessionState::RecoveryRequired,
            };
            return Ok((state, None));
        }
        Err(_) => return Err(Failure::new(FailureKind::CheckFailed)),
    };
    match exchange_server_metadata_before(site.client_socket_path(), deadline, signals.poll_fd()) {
        Ok(metadata) => Ok((
            if metadata.mode() == ServerMode::ManagedStopping {
                SessionState::Stopping
            } else {
                SessionState::Running
            },
            Some(metadata.process_id().get()),
        )),
        Err(ConnectClientError::ConnectFailed(error))
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            let marker = site.activation_state().map_err(local_error)?;
            let lifetime = site.observe_site_lifetime_lock().map_err(local_error)?;
            let state = if entry.accepts_runtime_state(marker) {
                inactive_state(marker, lifetime)
            } else {
                SessionState::RecoveryRequired
            };
            Ok((state, None))
        }
        Err(ConnectClientError::ConnectFailed(error) | ConnectClientError::Io(error)) => {
            Err(local_error(error))
        }
        Err(_) => Err(Failure::new(FailureKind::CheckFailed)),
    }
}

fn inactive_state(marker: ActivationState, lifetime: SiteLifetimeObservation) -> SessionState {
    match (marker, lifetime) {
        (
            ActivationState::Absent,
            SiteLifetimeObservation::Absent | SiteLifetimeObservation::Free,
        ) => SessionState::Absent,
        (ActivationState::Stopped { .. }, SiteLifetimeObservation::Free) => SessionState::Stopped,
        (ActivationState::Active, SiteLifetimeObservation::Held) => SessionState::Starting,
        _ => SessionState::RecoveryRequired,
    }
}

fn delete_output(
    name: &str,
    result: Result<WindowsNamedSiteDeleteOutcome, WindowsNamedSiteDeleteFailure>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let (outcome, reason, pending, status) = match result {
        Ok(WindowsNamedSiteDeleteOutcome::Deleted) => ("deleted", None, false, 0),
        Ok(WindowsNamedSiteDeleteOutcome::Absent) => ("absent", None, false, 0),
        Err(failure) => (
            if failure.reason == Reason::Interrupted {
                "interrupted"
            } else if failure.pending {
                "incomplete"
            } else {
                "refused"
            },
            Some(failure.reason),
            failure.pending,
            match failure.reason {
                Reason::Filesystem => 1,
                Reason::TimedOut => FailureKind::Timeout.exit_code(),
                Reason::Interrupted => FailureKind::Cancelled.exit_code(),
                _ => 3,
            },
        ),
    };
    let reason_token = reason.map(|reason| match reason {
        Reason::DefaultSession => "default-session",
        Reason::InUse => "in-use",
        Reason::RecoveryRequired => "recovery-required",
        Reason::Filesystem => "filesystem",
        Reason::TimedOut => "timed-out",
        Reason::Interrupted => "interrupted",
    });
    let result = json!({"name":name,"outcome":outcome,"reason":reason_token,"pending":pending});
    let diagnostic = locale.cli_session_delete(name, outcome, reason);
    let mut output = emit(
        "orchbee.session.delete",
        result,
        diagnostic.clone(),
        status,
        format,
        locale,
    );
    if output.status == status && status != 0 {
        output.stderr = diagnostic;
        if format == OutputFormat::Text {
            output.stdout.clear();
        }
    }
    output
}

fn emit(
    schema: &str,
    result: Value,
    text: String,
    status: u8,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let encoded = match format {
        OutputFormat::Json => output::encode_document(
            &json!({"schema":schema,"version":if schema == "orchbee.session.list" {2} else {1},"source":config::local_runtime_source(),"result":result}),
        ),
        OutputFormat::Text => output::encode_text(&text),
    };
    match encoded {
        Ok(stdout) => Output {
            status,
            stdout,
            stderr: String::new(),
            finalization: None,
        },
        Err(_) => failed(Failure::new(FailureKind::OutputTooLarge), format, locale),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recoverable_session_is_closed_to_v2_and_does_not_claim_a_running_server() {
        let schema: Value =
            serde_json::from_str(include_str!("schemas/orchbee.session.list.json")).unwrap();
        let validator = jsonschema::draft202012::new(&schema).unwrap();
        let result = json!({"sessions":[{"name":"default","default":true,"state":"recoverable","running":false,"pid":null,"runtimeDirectory":{"path":"C:/runtime"},"endpoint":"pipe"}]});
        let output = emit(
            "orchbee.session.list",
            result,
            String::new(),
            0,
            OutputFormat::Json,
            Locale::EnUs,
        );
        let mut document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(validator.is_valid(&document));
        document["version"] = json!(1);
        assert!(!validator.is_valid(&document));
        document["result"]["sessions"][0]["state"] = json!("stopped");
        assert!(validator.is_valid(&document));
        document["version"] = json!(2);
        document["result"]["sessions"][0]["state"] = json!("recoverable");
        document["result"]["sessions"][0]["running"] = json!(true);
        assert!(!validator.is_valid(&document));
    }

    #[test]
    fn deletion_receipts_validate_without_hiding_partial_effects_or_diagnostics() {
        let schema: Value =
            serde_json::from_str(include_str!("schemas/orchbee.session.delete.json")).unwrap();
        let validator = jsonschema::draft202012::new(&schema).unwrap();
        for result in [
            Err(WindowsNamedSiteDeleteFailure {
                reason: Reason::Interrupted,
                pending: false,
            }),
            Err(WindowsNamedSiteDeleteFailure {
                reason: Reason::Interrupted,
                pending: true,
            }),
            Ok(WindowsNamedSiteDeleteOutcome::Deleted),
            Ok(WindowsNamedSiteDeleteOutcome::Absent),
            Err(WindowsNamedSiteDeleteFailure {
                reason: Reason::DefaultSession,
                pending: false,
            }),
            Err(WindowsNamedSiteDeleteFailure {
                reason: Reason::Filesystem,
                pending: true,
            }),
            Err(WindowsNamedSiteDeleteFailure {
                reason: Reason::TimedOut,
                pending: true,
            }),
        ] {
            for locale in [Locale::EnUs, Locale::ZhCn] {
                let output = delete_output("work", result, OutputFormat::Json, locale);
                let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert!(validator.is_valid(&doc), "{doc}");
                assert_eq!(output.status == 0, result.is_ok());
                assert_eq!(output.stderr.is_empty(), result.is_ok());
                let human = delete_output("work", result, OutputFormat::Text, locale);
                if result.is_err() {
                    assert!(human.stdout.is_empty());
                    assert!(!human.stderr.is_empty());
                }
                if doc["result"]["pending"] == true {
                    let mut bad = doc.clone();
                    bad["result"]["outcome"] = json!("deleted");
                    assert!(!validator.is_valid(&bad));
                }
            }
        }
    }
    #[test]
    fn absent_endpoint_never_turns_unknown_lifecycle_into_stopped() {
        assert_eq!(
            inactive_state(ActivationState::Active, SiteLifetimeObservation::Free),
            SessionState::RecoveryRequired
        );
        assert_eq!(
            inactive_state(ActivationState::Active, SiteLifetimeObservation::Held),
            SessionState::Starting
        );
        assert_eq!(
            inactive_state(
                ActivationState::Stopped {
                    request_id: [1; 16]
                },
                SiteLifetimeObservation::Free
            ),
            SessionState::Stopped
        );
        for lifetime in [
            SiteLifetimeObservation::Held,
            SiteLifetimeObservation::Unsafe,
            SiteLifetimeObservation::Absent,
        ] {
            assert_eq!(
                inactive_state(
                    ActivationState::Stopped {
                        request_id: [1; 16]
                    },
                    lifetime
                ),
                SessionState::RecoveryRequired
            );
        }
    }
}
