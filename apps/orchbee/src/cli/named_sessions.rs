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
use orchbee_local_runtime::{
    ActivationState, ModeCheckedLocalSessionRendezvous as Rendezvous, NamedSessionDeleteFailure,
    NamedSessionDeleteOutcome, NamedSessionDeleteReason as Reason, NamedSessionEntry,
    RendezvousError, SessionCatalog as Catalog, SessionLifetimeObservation,
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
    Unreadable,
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
            Self::Unreadable => "unreadable",
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
        NamedSessionCommand::Stop { command, .. } => super::server_stop(command, format, locale),
        NamedSessionCommand::List => list(format, locale),
        NamedSessionCommand::Delete { name } => delete(name, format, locale),
    }
}

fn delete(
    name: &orchbee_local_runtime::SessionName,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if name.as_str() == "default" {
        return delete_output(
            name.as_str(),
            Err(NamedSessionDeleteFailure {
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
        let (base, _) = crate::local_activation::runtime_location()
            .map_err(|error| local_error_at(error, "runtime-base"))?;
        let local_app_data = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| local_error(io::ErrorKind::NotFound.into()))?;
        let catalog = Catalog::try_lock(&base, &local_app_data)
            .map_err(|error| local_error_at(error, "runtime-base"))?;
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
                Ok(_) => Ok(Err(NamedSessionDeleteFailure {
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
    local_error_at(error, "runtime-directory")
}
fn local_error_at(error: io::Error, stage: &str) -> Failure {
    if error.kind() == io::ErrorKind::TimedOut {
        Failure::new(FailureKind::Timeout)
    } else {
        Failure::new(FailureKind::CheckFailed).details(super::stage_details(stage))
    }
}

fn list(format: OutputFormat, locale: Locale) -> Output {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => return failed(Failure::new(FailureKind::InternalFailure), format, locale),
    };
    let collected = (|| {
        let (base, _) = crate::local_activation::runtime_location()
            .map_err(|error| local_error_at(error, "runtime-base"))?;
        let local = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| local_error(io::ErrorKind::NotFound.into()))?;
        let entries = Catalog::list(&base, &local, deadline)
            .map_err(|error| local_error_at(error, "runtime-base"))?;
        let mut documents = Vec::new();
        let mut rows = Vec::new();
        let mut absent = false;
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
            absent |= state == SessionState::Absent;
            rows.push(vec![
                entry.name.as_str().to_owned(),
                locale.cli_session_state(state).to_owned(),
                super::escape::native_path(&entry.runtime.to_string_lossy()),
            ]);
        }
        let text = if rows.is_empty() {
            locale.cli_session_empty().to_owned()
        } else {
            let mut text = super::present::columns(&locale.cli_session_headers(), &rows);
            if absent {
                text.push_str(locale.cli_session_absent_note());
            }
            text
        };
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
    entry: &NamedSessionEntry,
    deadline: Instant,
    signals: &ArmedSignals,
) -> Result<(SessionState, Option<u32>), Failure> {
    if entry.deleting {
        return Ok((SessionState::Deleting, None));
    }
    if entry.unreadable {
        return Ok((SessionState::Unreadable, None));
    }
    if entry.cold_recoverable().map_err(local_error)? {
        return Ok((SessionState::Recoverable, None));
    }
    let session = match Rendezvous::from_runtime_directory(&entry.runtime) {
        Ok(session) => session,
        Err(RendezvousError::System { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
            let state = match entry.absent_runtime_state().map_err(local_error)? {
                None => SessionState::Absent,
                Some(ActivationState::Stopped { .. }) => SessionState::Stopped,
                _ => SessionState::RecoveryRequired,
            };
            return Ok((state, None));
        }
        Err(_) => return Err(local_error(io::ErrorKind::InvalidData.into())),
    };
    match exchange_server_metadata_before(session.client_socket_path(), deadline, signals.poll_fd())
    {
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
            let marker = session.activation_state().map_err(local_error)?;
            let lifetime = session
                .observe_session_lifetime_lock()
                .map_err(local_error)?;
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
        Err(_) => Err(local_error_at(io::ErrorKind::InvalidData.into(), "socket")),
    }
}

fn inactive_state(marker: ActivationState, lifetime: SessionLifetimeObservation) -> SessionState {
    match (marker, lifetime) {
        (
            ActivationState::Absent,
            SessionLifetimeObservation::Absent | SessionLifetimeObservation::Free,
        ) => SessionState::Absent,
        (ActivationState::Stopped { .. }, SessionLifetimeObservation::Free) => {
            SessionState::Stopped
        }
        (ActivationState::Active, SessionLifetimeObservation::Held) => SessionState::Starting,
        _ => SessionState::RecoveryRequired,
    }
}

fn delete_output(
    name: &str,
    result: Result<NamedSessionDeleteOutcome, NamedSessionDeleteFailure>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if matches!(&result, Err(failure) if failure.reason == Reason::Unsupported) {
        return failure_output(
            Failure::new(FailureKind::ServiceUnavailable).details(super::stage_reason_details(
                "platform",
                "unsupported-platform",
            )),
            Some(format),
            locale,
        );
    }
    let (outcome, reason, pending, status) = match result {
        Ok(NamedSessionDeleteOutcome::Deleted) => ("deleted", None, false, 0),
        Ok(NamedSessionDeleteOutcome::Absent) => ("absent", None, false, 0),
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
        Reason::Unsupported => "unsupported-platform",
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
            &json!({"schema":schema,"version":1,"source":config::local_runtime_source(),"result":result}),
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
mod tests;
