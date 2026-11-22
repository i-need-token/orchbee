//! One read-only `query` of the selected Session, or a local executable check.
use super::{
    Failure, FailureKind, Locale, Output, OutputFormat, RuntimeStatusScope, failed,
    projection::{
        Absence, Cohort, CohortDiagnostic, CohortIssue, Report, Server, ServerReport, emit,
    },
};
use crate::cli::{
    console_interrupt::{ArmedSignals, Finish},
    query,
};
use orchbee_local_client_protocol::ConnectClientError;
use orchbee_local_runtime::{
    self as runtime, ModeCheckedLocalSessionRendezvous, RendezvousError, SessionScope,
};
use std::{
    io,
    time::{Duration, Instant},
};

/// macOS captures the interrupt pair as the release commands do, so an interruption ends the
/// `query` and answers as cancelled; Windows arms its console interrupts.
pub(super) fn run(scope: RuntimeStatusScope, format: OutputFormat, locale: Locale) -> Output {
    let deadline = Instant::now() + Duration::from_secs(5);
    let signals = match ArmedSignals::arm_interrupts_except_linux() {
        Ok(signals) => signals,
        Err(_) => return failed(Failure::new(FailureKind::InternalFailure), format, locale),
    };
    let result = match scope {
        RuntimeStatusScope::Client => client(deadline),
        RuntimeStatusScope::Server => server(deadline, &signals),
    };
    let mut finalization = None;
    let result = match signals.finish() {
        Ok(Finish::Clear) => result,
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            Err(Failure::new(FailureKind::Cancelled))
        }
        Err(_) => Err(Failure::new(FailureKind::InternalFailure)),
    };
    let mut output = match result {
        Ok(report) => emit(report, format, locale),
        Err(error) => failed(error, format, locale),
    };
    output.finalization = finalization;
    output
}

/// This executable and its installation files, without contacting a Server. The program file is
/// the resolved one, so a macOS start through the command link still finds the release beside it.
pub(super) fn client(deadline: Instant) -> Result<Report, Failure> {
    let executable =
        runtime::current_executable().map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let admitted = admitted_cohort(deadline);
    let installation = crate::cli::installation::inspect(
        &executable,
        admitted.as_ref().ok(),
        env!("CARGO_PKG_VERSION"),
    )
    .map_err(|_| {
        Failure::new(FailureKind::ServiceUnavailable).details(crate::cli::stage_reason_details(
            "platform",
            "unsupported-platform",
        ))
    })?;
    let cohort = project_cohort(admitted);
    Ok(Report::Client {
        executable,
        cohort,
        installation,
    })
}

/// The selected Session's processes from one handshake `query`; no Session is a report, not a
/// failure.
pub(super) fn server(deadline: Instant, signals: &ArmedSignals) -> Result<Report, Failure> {
    let (base, scope) = crate::local_activation::runtime_location().map_err(|_| {
        Failure::new(FailureKind::CheckFailed).details(crate::cli::stage_details("runtime-base"))
    })?;
    let runtime = base.join(&scope);
    let selected =
        SessionScope::parse(&scope).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let session = match ModeCheckedLocalSessionRendezvous::validate_mode_checked(&base, selected) {
        Ok(session) => Some(session),
        Err(RendezvousError::System { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
            None
        }
        Err(error) => return Err(query::classify_rendezvous(error, &runtime)),
    };
    if Instant::now() >= deadline {
        return Err(Failure::new(FailureKind::Timeout));
    }
    let (runtime, endpoint, server) = match session {
        None => (runtime, None, Server::Absent(Absence::Runtime)),
        Some(session) => {
            let endpoint = session.client_socket_path();
            let server = match crate::cli::server_query::read(endpoint, deadline, signals.poll_fd())
            {
                Ok(result) => Server::Observed(result),
                Err(ConnectClientError::ConnectFailed(error))
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                    ) =>
                {
                    Server::Absent(Absence::Endpoint {
                        moved_to: moved_to(session.runtime_directory()),
                    })
                }
                Err(error) => return Err(super::classify_query(error)),
            };
            (
                session.runtime_directory().to_owned(),
                Some(endpoint.to_owned()),
                server,
            )
        }
    };
    if Instant::now() >= deadline {
        return Err(Failure::new(FailureKind::Timeout));
    }
    Ok(Report::Server(ServerReport {
        scope,
        runtime,
        endpoint,
        server,
    }))
}

fn project_cohort(
    admitted: Result<runtime::Cohort, (CohortIssue, Option<CohortDiagnostic>)>,
) -> Cohort {
    match admitted {
        Ok(cohort) => Cohort::Available(cohort.identity().to_owned()),
        Err((reason, diagnostic)) => Cohort::Unavailable(reason, diagnostic),
    }
}
fn admitted_cohort(
    deadline: Instant,
) -> Result<runtime::Cohort, (CohortIssue, Option<CohortDiagnostic>)> {
    runtime::Cohort::current_with_diagnostic(deadline).map_err(|error| {
        let issue = cohort_issue(error.kind());
        let diagnostic = CohortDiagnostic::from_error(&error);
        (issue, Some(diagnostic))
    })
}

fn cohort_issue(kind: io::ErrorKind) -> CohortIssue {
    match kind {
        io::ErrorKind::NotFound => CohortIssue::Missing,
        io::ErrorKind::InvalidData | io::ErrorKind::Unsupported => CohortIssue::Invalid,
        io::ErrorKind::PermissionDenied => CohortIssue::Denied,
        io::ErrorKind::TimedOut => CohortIssue::TimedOut,
        _ => CohortIssue::Io,
    }
}

/// The store's active version the idle move ended the Session in `runtime` to run, when its record
/// says so; anything unreadable reads as none.
fn moved_to(runtime: &std::path::Path) -> Option<String> {
    let root = orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name)).ok()?;
    orchbee_local_runtime::session_moved_to(&root, runtime)
        .ok()
        .flatten()
}
