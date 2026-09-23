//! Stop observation rendering keeps known facts separate from wait disposition.
use super::{Locale, Output, OutputFormat, StopId, emit, id_text};
use orchbee_local_client_protocol::{SiteStopStatus, StopFailure, StopPhase};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub(super) enum Observation {
    Stopped,
    Server(SiteStopStatus),
    Unavailable(&'static str),
    Unknown(&'static str),
}

pub(super) struct Answer {
    pub(super) id: StopId,
    pub(super) observation: Observation,
    pub(super) wait: Option<&'static str>,
    pub(super) query: bool,
}

fn phase(phase: StopPhase) -> &'static str {
    match phase {
        StopPhase::Admission => "admission",
        StopPhase::BackgroundJobs => "background-jobs",
        StopPhase::Inventory => "inventory",
        StopPhase::Force => "force",
        StopPhase::Dispose => "dispose",
        StopPhase::HostExit => "host-exit",
        StopPhase::Worker => "worker",
        StopPhase::WorkerExit => "worker-exit",
        StopPhase::Ingress => "ingress",
        StopPhase::Clients => "clients",
        StopPhase::Completion => "completion",
    }
}
fn reason(reason: StopFailure) -> &'static str {
    match reason {
        StopFailure::Deadline => "deadline",
        StopFailure::Unavailable => "unavailable",
        StopFailure::OutcomeUnknown => "outcome-unknown",
        StopFailure::Protocol => "protocol",
        StopFailure::Refused => "refused",
        StopFailure::ProcessWait => "process-wait",
        StopFailure::RecordWrite => "record-write",
    }
}

impl Answer {
    fn result(&self) -> Value {
        let mut value = match self.observation {
            Observation::Stopped => json!({"outcome":"stopped"}),
            Observation::Server(SiteStopStatus::NotRecorded) => json!({"outcome":"not-recorded"}),
            Observation::Server(SiteStopStatus::Pending {
                attempt,
                phase: stage,
            }) => json!({"outcome":"pending","attempt":attempt.get(),"phase":phase(stage)}),
            Observation::Server(SiteStopStatus::Failed {
                attempt,
                phase: stage,
                failure,
                disposed_terminals,
            }) => {
                json!({"outcome":"failed","attempt":attempt.get(),"phase":phase(stage),"reason":reason(failure),"disposedTerminals":disposed_terminals})
            }
            Observation::Server(SiteStopStatus::Busy { active_id }) => {
                json!({"outcome":"busy","activeRequestId":id_text(active_id)})
            }
            Observation::Server(SiteStopStatus::Unmanaged) => json!({"outcome":"unmanaged"}),
            Observation::Unavailable(reason) => json!({"outcome":"unavailable","reason":reason}),
            Observation::Unknown(reason) => json!({"outcome":"unknown","reason":reason}),
        };
        value["requestId"] = json!(id_text(self.id));
        if let Some(wait) = self.wait {
            value["wait"] = json!(wait);
        }
        value
    }
    fn status(&self) -> u8 {
        if matches!(self.observation, Observation::Unknown(_)) {
            return 8;
        }
        if self.wait == Some("cancelled") {
            return 130;
        }
        if self.wait == Some("timed-out") {
            return 7;
        }
        match self.observation {
            Observation::Stopped => 0,
            Observation::Unavailable(_) => 4,
            Observation::Unknown(_) => 8,
            Observation::Server(_) if self.query => 0,
            Observation::Server(SiteStopStatus::Pending { .. }) => 0,
            Observation::Server(_) => 3,
        }
    }
}

pub(super) fn render(answer: Answer, format: OutputFormat, locale: Locale) -> Output {
    emit(
        answer.id,
        answer.result(),
        answer.status(),
        answer.wait,
        format,
        locale,
    )
}
