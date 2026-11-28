//! One mutation exchange followed only by bounded, cancellable receipt/status reads.
use super::{
    Answer, Locale, Observation, Output, OutputFormat, ServerStopCommand, StopId,
    platform::exchange_before, render,
};
use crate::cli::console_interrupt::{ArmedSignals, Finish};
use crate::cli_text::CliText;
use orchbee_connection_handshake::RejectionReason as Refusal;
use orchbee_local_client_protocol::{
    ConnectClientError, ExitReply, SessionStopRequest, SessionStopStatus, StopAction,
};
use orchbee_local_runtime::{
    ActivationState, Cohort, ModeCheckedLocalSessionRendezvous as Rendezvous, RendezvousError,
    SessionLifetimeObservation, SessionScope,
};
use std::{
    io::{self, Write},
    thread,
    time::{Duration, Instant},
};

pub(super) fn run(
    command: &ServerStopCommand,
    id: StopId,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let mut answer = Answer {
        id,
        observation: Observation::Unavailable("runtime"),
        wait: None,
        query: matches!(command, ServerStopCommand::Status { .. }),
        elevated_session: false,
    };
    let mut signals = match ArmedSignals::arm_interrupts_or_inherit() {
        Ok(signals) => signals,
        Err(_) => {
            answer.observation = Observation::Unavailable("signal-registration");
            return render(answer, format, locale);
        }
    };
    let waiting = CliText::new(locale).cli_server_stop_waiting(&super::id_text(id));
    observe(command, &mut answer, &mut signals, &waiting);
    let finalization = match signals.finish() {
        Ok(Finish::Clear) => None,
        Ok(Finish::Cancelled { finalization, .. }) => {
            if matches!(
                answer.observation,
                Observation::Unavailable(_)
                    | Observation::Unknown(_)
                    | Observation::Server(SessionStopStatus::Pending { .. })
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

fn observe(
    command: &ServerStopCommand,
    answer: &mut Answer,
    signals: &mut ArmedSignals,
    waiting: &str,
) {
    let (action, detached, timeout) = match command {
        ServerStopCommand::Stop {
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
        ServerStopCommand::Status { .. } => (StopAction::Status, false, 5_000),
    };
    let deadline = Instant::now() + Duration::from_millis(u64::from(timeout));
    let Ok((base, scope)) = crate::local_activation::runtime_location() else {
        return;
    };
    let Ok(scope) = SessionScope::parse(&scope) else {
        return;
    };
    let session = match Rendezvous::validate_mode_checked(&base, scope) {
        Ok(session) => session,
        Err(RendezvousError::System { source, .. })
            if source.kind() == io::ErrorKind::NotFound && answer.query =>
        {
            answer.observation = Observation::Server(SessionStopStatus::NotRecorded);
            return;
        }
        Err(_) => return,
    };
    observe_session(
        &session,
        action,
        detached,
        deadline,
        answer,
        signals,
        Some(waiting),
    );
}

/// The stop or status exchange with one resolved Session and, for a stop, the bounded wait for its
/// end. The wait begins by writing `waiting`, when given, on standard error: an interruption on
/// macOS and Linux ends the invocation with no document, and the line keeps the request identity
/// that stays the accepted stop's handle.
pub(super) fn observe_session(
    session: &Rendezvous,
    action: StopAction,
    detached: bool,
    deadline: Instant,
    answer: &mut Answer,
    signals: &mut ArmedSignals,
    waiting: Option<&str>,
) {
    match completed(session, answer.id) {
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
        && match super::platform::activation_state(session) {
            Ok(ActivationState::Absent) => true,
            Ok(ActivationState::Stopped { request_id }) => request_id != answer.id.bytes(),
            _ => false,
        }
    {
        answer.observation = Observation::Server(SessionStopStatus::NotRecorded);
        return;
    }
    let cohort = match Cohort::current(deadline) {
        Ok(cohort) => cohort,
        Err(_) => {
            answer.observation = Observation::Unavailable("cohort");
            return;
        }
    };
    let offer = crate::local_client::client_offer(Some(cohort.identity()));
    let request = SessionStopRequest {
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
    let result = exchange_before(
        session.client_socket_path(),
        &offer,
        request.clone(),
        deadline.min(Instant::now() + Duration::from_secs(2)),
        signals.poll_fd(),
    );
    match result {
        // A Server records every start while no stop exists, except while a restart holds the
        // Session: nothing was recorded, and the Session takes a stop again once the restart ends.
        Ok(result)
            if action == StopAction::Start && result.status == SessionStopStatus::NotRecorded =>
        {
            answer.observation = Observation::Unavailable("draining");
            return;
        }
        Ok(result) => answer.observation = Observation::Server(result.status),
        Err(error) => {
            answer.elevated_session = error.failure.refused_by_elevated_session();
            match &error.failure {
                // No shim speaks to a Session from before the handshake; its own build stops it.
                ConnectClientError::PredatesHandshake => {
                    answer.observation = Observation::Unavailable("predates-handshake");
                    return;
                }
                ConnectClientError::Refused(rejection)
                    if !answer.query
                        && matches!(
                            rejection.reason,
                            Refusal::InitiatorTooNew
                                | Refusal::FingerprintMismatch
                                | Refusal::Stopping
                        ) =>
                {
                    let draining = rejection.reason == Refusal::Stopping;
                    end_by_exit(session, answer, signals, deadline, draining);
                    return;
                }
                ConnectClientError::Rejected(_) | ConnectClientError::Refused(_) => {
                    answer.observation = Observation::Unavailable("rejected");
                    return;
                }
                _ => {}
            }
            if !error.submitted || answer.query {
                match completed(session, answer.id) {
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
    if Instant::now() >= deadline && unsettled(&answer.observation) {
        answer.wait = Some("timed-out");
        return;
    }
    if detached {
        if matches!(
            answer.observation,
            Observation::Server(SessionStopStatus::Pending { .. })
        ) {
            answer.wait = Some("detached");
        }
        return;
    }
    if unsettled(&answer.observation)
        && let Some(line) = waiting
    {
        let mut stderr = io::stderr().lock();
        let _ = stderr
            .write_all(line.as_bytes())
            .and_then(|()| stderr.flush());
    }
    let request = SessionStopRequest {
        action: StopAction::Status,
        ..request
    };
    while unsettled(&answer.observation) {
        if signals.take().is_ok_and(|signal| signal.is_some()) {
            answer.wait = Some("cancelled");
            return;
        }
        match completed(session, answer.id) {
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
        if let Ok(result) = exchange_before(
            session.client_socket_path(),
            &offer,
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

/// Ends a Session whose Server refused this build's offer, being too old, another development
/// build, or already draining, through the control requests every handshake build answers. `exit`
/// ends only the Server and its Worker and never a terminal, and before P1 nothing else from here
/// can end one; so only a Session with no terminal ends here, and its next start recovers it,
/// counting no crash when that Server recorded its `exit`. Any other Session is left to the build
/// that started it.
fn end_by_exit(
    session: &Rendezvous,
    answer: &mut Answer,
    signals: &mut ArmedSignals,
    deadline: Instant,
    draining: bool,
) {
    let endpoint = session.client_socket_path();
    let terminals = super::platform::server_terminals_before(
        endpoint,
        deadline.min(Instant::now() + Duration::from_secs(2)),
        signals.poll_fd(),
    );
    match terminals {
        Ok(Some(0)) => {}
        Ok(_) => {
            answer.observation = Observation::Unavailable(if draining {
                "draining"
            } else {
                "terminals-open"
            });
            return;
        }
        Err(ConnectClientError::PredatesHandshake) => {
            answer.observation = Observation::Unavailable("predates-handshake");
            return;
        }
        Err(ConnectClientError::Rejected(_) | ConnectClientError::Refused(_)) => {
            answer.observation = Observation::Unavailable("rejected");
            return;
        }
        Err(_) => {
            answer.observation = Observation::Unavailable("transport");
            return;
        }
    }
    if signals.take().is_ok_and(|signal| signal.is_some()) {
        answer.wait = Some("cancelled");
        return;
    }
    match super::platform::exit_before(endpoint, deadline, signals.poll_fd()) {
        Ok(ExitReply::Ending) => {}
        Ok(ExitReply::Unsupported) => {
            answer.observation = Observation::Unavailable("exit-unsupported");
            return;
        }
        Err(error) if !error.submitted => {
            answer.observation = Observation::Unavailable("transport");
            return;
        }
        // The request reached the Server, so it may be ending; only its released lifetime says so.
        Err(_) => answer.observation = Observation::Unknown("transport"),
    }
    loop {
        if session.observe_session_lifetime_lock().ok() == Some(SessionLifetimeObservation::Free) {
            answer.observation = Observation::Exited;
            return;
        }
        if signals.take().is_ok_and(|signal| signal.is_some()) {
            answer.observation = Observation::Unknown("session-lifetime");
            answer.wait = Some("cancelled");
            return;
        }
        if Instant::now() >= deadline {
            answer.observation = Observation::Unknown("session-lifetime");
            answer.wait = Some("timed-out");
            return;
        }
        thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(25)),
        );
    }
}

/// Whether the stop may still end, so a wait goes on.
fn unsettled(observation: &Observation) -> bool {
    matches!(
        observation,
        Observation::Server(SessionStopStatus::Pending { .. }) | Observation::Unknown(_)
    )
}

fn completed(session: &Rendezvous, id: StopId) -> io::Result<bool> {
    Ok(
        matches!(super::platform::activation_state(session)?, ActivationState::Stopped { request_id } if request_id == id.bytes())
            && session.observe_session_lifetime_lock()? == SessionLifetimeObservation::Free,
    )
}
