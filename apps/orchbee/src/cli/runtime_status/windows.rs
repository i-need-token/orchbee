//! One read-only Windows probe and an independent optional local cohort check.
use super::{
    failed,
    projection::{Absence, Cohort, CohortIssue, Report, Server, ServerReport, emit},
    *,
};
use crate::cli::{
    query,
    source::{ArmedSignals, Finish, SignalWatch},
};
use orchbee_local_client_protocol::{
    ConnectClientError, RejectionReason, exchange_server_metadata_before,
};
use orchbee_local_site_runtime::{
    ModeCheckedLocalSiteRendezvous, NativeCohort, RendezvousError, SiteScope,
};
use std::{
    io,
    time::{Duration, Instant},
};

pub(super) fn run(scope: RuntimeStatusScope, format: OutputFormat, locale: Locale) -> Output {
    let deadline = Instant::now() + Duration::from_secs(5);
    let signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => return failed(Failure::new(FailureKind::InternalFailure), format, locale),
    };
    let result = collect(scope, deadline, &signals);
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

fn collect(
    scope: RuntimeStatusScope,
    deadline: Instant,
    signals: &ArmedSignals,
) -> Result<Report, Failure> {
    if scope == RuntimeStatusScope::Client {
        let executable =
            std::env::current_exe().map_err(|_| Failure::new(FailureKind::InternalFailure))?;
        let admitted = admitted_cohort(deadline);
        let installation = crate::cli::update::inspect_installation(
            &executable,
            admitted.as_ref().ok(),
            env!("CARGO_PKG_VERSION"),
        );
        let cohort = project_cohort(admitted);
        return Ok(Report::Client {
            executable,
            cohort,
            installation,
        });
    }
    let (base, scope) = crate::local_activation::runtime_location().map_err(|_| {
        Failure::new(FailureKind::CheckFailed).details(crate::cli::stage_details("runtime-base"))
    })?;
    let runtime = base.join(&scope);
    let selected =
        SiteScope::parse(&scope).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let site = match ModeCheckedLocalSiteRendezvous::validate_mode_checked(&base, selected) {
        Ok(site) => Some(site),
        Err(RendezvousError::System { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
            None
        }
        Err(error) => return Err(query::classify_rendezvous(error, &runtime)),
    };
    if Instant::now() >= deadline {
        return Err(Failure::new(FailureKind::Timeout));
    }
    let (runtime, endpoint, server) = match site {
        None => (runtime, None, Server::Absent(Absence::Runtime)),
        Some(site) => {
            let endpoint = site.client_socket_path();
            let server =
                match exchange_server_metadata_before(endpoint, deadline, signals.poll_fd()) {
                    Ok(metadata) => Server::Observed(metadata),
                    Err(ConnectClientError::ConnectFailed(error))
                        if matches!(
                            error.kind(),
                            io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                        ) =>
                    {
                        Server::Absent(Absence::Endpoint)
                    }
                    Err(error) => return Err(classify(error)),
                };
            (
                site.runtime_directory().to_owned(),
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
        cohort: local_cohort(deadline),
    }))
}

fn local_cohort(deadline: Instant) -> Cohort {
    project_cohort(admitted_cohort(deadline))
}
fn project_cohort(admitted: Result<NativeCohort, CohortIssue>) -> Cohort {
    match admitted {
        Ok(cohort) => Cohort::Available(cohort.identity().to_owned()),
        Err(reason) => Cohort::Unavailable(reason),
    }
}
fn admitted_cohort(deadline: Instant) -> Result<NativeCohort, CohortIssue> {
    NativeCohort::current(deadline).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => CohortIssue::Missing,
        io::ErrorKind::InvalidData | io::ErrorKind::Unsupported => CohortIssue::Invalid,
        io::ErrorKind::PermissionDenied => CohortIssue::Denied,
        io::ErrorKind::TimedOut => CohortIssue::TimedOut,
        _ => CohortIssue::Io,
    })
}

fn classify(error: ConnectClientError) -> Failure {
    match error {
        ConnectClientError::ConnectFailed(ref io) | ConnectClientError::Io(ref io)
            if io.kind() == io::ErrorKind::TimedOut =>
        {
            Failure::new(FailureKind::Timeout)
        }
        ConnectClientError::ConnectFailed(ref io) | ConnectClientError::Io(ref io)
            if io.kind() == io::ErrorKind::PermissionDenied =>
        {
            Failure::new(FailureKind::Denied)
        }
        // Old OBLC readers reject the new, fixed request before dispatch.
        ConnectClientError::Rejected(
            RejectionReason::WrongMessage | RejectionReason::MalformedFrame,
        ) => {
            Failure::new(FailureKind::Incompatible).details(crate::cli::stage_details("admission"))
        }
        other => query::classify_connect(crate::local_client::LocalClientConnectError::Admission(
            other,
        )),
    }
}
