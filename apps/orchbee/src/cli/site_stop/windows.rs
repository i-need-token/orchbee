//! One mutation exchange followed only by bounded, cancellable receipt/status reads.
use super::{Answer, Locale, Observation, Output, OutputFormat, SiteStopCommand, StopId, render};
use crate::cli::source::{ArmedSignals, Finish, SignalWatch};
use orchbee_local_client_protocol::{
    ClientHello, ConnectClientError, SiteStopRequest, SiteStopStatus, StopAction,
    exchange_site_stop_before,
};
use orchbee_local_site_runtime::{
    ActivationState, ModeCheckedLocalSiteRendezvous as Rendezvous, NativeCohort, RendezvousError,
    SiteLifetimeObservation, SiteScope,
};
use std::{
    io, thread,
    time::{Duration, Instant},
};

pub(super) fn run(
    command: &SiteStopCommand,
    id: StopId,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let mut answer = Answer {
        id,
        observation: Observation::Unavailable("runtime"),
        wait: None,
        query: matches!(command, SiteStopCommand::Status { .. }),
    };
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            answer.observation = Observation::Unavailable("signal-registration");
            return render(answer, format, locale);
        }
    };
    observe(command, &mut answer, &mut signals);
    let finalization = match signals.finish() {
        Ok(Finish::Clear) => None,
        Ok(Finish::Cancelled { finalization, .. }) => {
            if matches!(
                answer.observation,
                Observation::Unavailable(_)
                    | Observation::Unknown(_)
                    | Observation::Server(SiteStopStatus::Pending { .. })
            ) {
                answer.wait = Some("cancelled");
            }
            Some(finalization)
        }
        Err(_) => {
            answer.observation = Observation::Unavailable("signal-finalization");
            None
        }
    };
    let mut output = render(answer, format, locale);
    output.finalization = finalization;
    output
}

fn observe(command: &SiteStopCommand, answer: &mut Answer, signals: &mut ArmedSignals) {
    let (action, detached, timeout) = match command {
        SiteStopCommand::Stop {
            retry,
            detach,
            timeout_ms,
            ..
        } => (
            if *retry {
                StopAction::Retry
            } else {
                StopAction::Start
            },
            *detach,
            *timeout_ms,
        ),
        SiteStopCommand::Status { .. } => (StopAction::Status, false, 5_000),
    };
    let deadline = Instant::now() + Duration::from_millis(u64::from(timeout));
    let Ok((base, scope)) = crate::local_activation::runtime_location() else {
        return;
    };
    let Ok(scope) = SiteScope::parse(&scope) else {
        return;
    };
    let site = match Rendezvous::validate_mode_checked(&base, scope) {
        Ok(site) => site,
        Err(RendezvousError::System { source, .. })
            if source.kind() == io::ErrorKind::NotFound && answer.query =>
        {
            answer.observation = Observation::Server(SiteStopStatus::NotRecorded);
            return;
        }
        Err(_) => return,
    };
    match completed(&site, answer.id) {
        Ok(true) => {
            answer.observation = Observation::Stopped;
            return;
        }
        Err(_) => {
            answer.observation = Observation::Unavailable("activation-record");
            return;
        }
        Ok(false) => {}
    }
    if answer.query
        && match site.activation_state() {
            Ok(ActivationState::Absent) => true,
            Ok(ActivationState::Stopped { request_id }) => request_id != answer.id.bytes(),
            _ => false,
        }
    {
        answer.observation = Observation::Server(SiteStopStatus::NotRecorded);
        return;
    }
    let cohort = match NativeCohort::current(deadline) {
        Ok(cohort) => cohort,
        Err(_) => {
            answer.observation = Observation::Unavailable("cohort");
            return;
        }
    };
    let request = SiteStopRequest {
        hello: ClientHello::new(cohort.identity()).expect("validated cohort"),
        id: answer.id,
        action,
    };
    if signals.take().is_ok_and(|signal| signal.is_some()) {
        answer.wait = Some("cancelled");
        return;
    }
    if Instant::now() >= deadline {
        answer.wait = Some("timed-out");
        return;
    }
    let result = exchange_site_stop_before(
        site.client_socket_path(),
        request.clone(),
        deadline.min(Instant::now() + Duration::from_secs(2)),
        signals.poll_fd(),
    );
    match result {
        Ok(result) => answer.observation = Observation::Server(result.status),
        Err(error) => {
            if matches!(error.failure, ConnectClientError::Rejected(_)) {
                answer.observation = Observation::Unavailable("rejected");
                return;
            }
            if !error.submitted || answer.query {
                match completed(&site, answer.id) {
                    Ok(true) => answer.observation = Observation::Stopped,
                    _ => answer.observation = Observation::Unavailable("transport"),
                }
                return;
            }
            answer.observation = Observation::Unknown("transport");
        }
    }
    if answer.query {
        return;
    }
    if Instant::now() >= deadline
        && matches!(
            answer.observation,
            Observation::Server(SiteStopStatus::Pending { .. }) | Observation::Unknown(_)
        )
    {
        answer.wait = Some("timed-out");
        return;
    }
    if detached {
        if matches!(
            answer.observation,
            Observation::Server(SiteStopStatus::Pending { .. })
        ) {
            answer.wait = Some("detached");
        }
        return;
    }
    let request = SiteStopRequest {
        action: StopAction::Status,
        ..request
    };
    while matches!(
        answer.observation,
        Observation::Server(SiteStopStatus::Pending { .. }) | Observation::Unknown(_)
    ) {
        if signals.take().is_ok_and(|signal| signal.is_some()) {
            answer.wait = Some("cancelled");
            return;
        }
        match completed(&site, answer.id) {
            Ok(true) => {
                answer.observation = Observation::Stopped;
                return;
            }
            Err(_) => {
                answer.observation = Observation::Unknown("activation-record");
                return;
            }
            Ok(false) => {}
        }
        if Instant::now() >= deadline {
            answer.wait = Some("timed-out");
            return;
        }
        if let Ok(result) = exchange_site_stop_before(
            site.client_socket_path(),
            request.clone(),
            deadline.min(Instant::now() + Duration::from_secs(2)),
            signals.poll_fd(),
        ) {
            answer.observation = Observation::Server(result.status);
        }
        thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(25)),
        );
    }
}

fn completed(site: &Rendezvous, id: StopId) -> io::Result<bool> {
    Ok(
        matches!(site.activation_state()?, ActivationState::Stopped { request_id } if request_id == id.bytes())
            && site.observe_site_lifetime_lock()? == SiteLifetimeObservation::Free,
    )
}
