//! Typed Agent startup shared by the CLI adapter and native Client surfaces.
//!
//! The workflow owns the reserve -> one input submission -> canonical readiness
//! sequence. Callers inject an exact Panel, one absolute deadline, and an
//! optional cancellation wake. Presentation, process-wide signals, and surface
//! generations remain with their callers.

use std::time::Instant;

use crate::agent_runtime::startup::send;
pub use failure::{ExchangeFailure, Failure, StartFailure};
use orchbee_agent_observation_protocol::{
    AgentLaunchReservation, AgentName, AgentProviderId, AgentSessionId, ObapRequestId,
};
use std::sync::Arc;

mod command;
mod failure;
mod workflow;

/// One already-validated startup intent. Panel selection is deliberately not
/// part of this value: the caller supplies the exact target resolved for the
/// admitted Snapshot.
#[derive(Clone, Debug)]
pub struct Request {
    pub name: AgentName,
    pub provider: AgentProviderId,
    pub(crate) args: Box<[String]>,
}

impl Request {
    pub fn new(name: AgentName, provider: AgentProviderId, args: Vec<String>) -> Self {
        Self {
            name,
            provider,
            args: args.into_boxed_slice(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReservationState {
    None,
    Pending,
    Unknown,
    Released,
    Completed,
}

/// What the startup workflow can prove about the one terminal input
/// submission. This remains separate from the workflow verdict: a later
/// readiness failure cannot erase input that was already accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryDisposition {
    /// No record reached the Host queue, or submission never began.
    NotApplied,
    /// Every record reached the Host queue.
    Applied,
    /// A non-empty prefix reached the Host queue.
    PartiallyApplied,
    /// Transport loss left the delivery frontier unknown.
    Unknown,
}

impl DeliveryDisposition {
    pub(crate) const fn possibly_applied(self) -> bool {
        !matches!(self, Self::NotApplied)
    }
}

impl ReservationState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Pending => "pending",
            Self::Unknown => "unknown",
            Self::Released => "released",
            Self::Completed => "completed",
        }
    }
}

/// Retain the sole input attempt's typed receipt or failure independently of startup readiness.
#[derive(Debug)]
pub enum Submission {
    Sent(Box<send::Receipt>),
    Failed(Arc<send::Error>),
}

/// Startup completion or its terminal failure, with the last failed read retained as a separate cause.
/// Delivery and reservation facts in the Report remain authoritative even when this verdict fails.
#[derive(Debug)]
pub enum Verdict {
    Started {
        session: AgentSessionId,
    },
    Failed {
        failure: Failure,
        read_cause: Option<Box<Failure>>,
    },
}

/// Independently known reservation and delivery facts survive every later observation failure.
#[derive(Debug)]
pub struct Report {
    pub name: AgentName,
    pub provider: AgentProviderId,
    pub reservation: Option<ObapRequestId>,
    pub last_observed: Option<AgentLaunchReservation>,
    pub reservation_state: ReservationState,
    pub submission: Option<Submission>,
    pub(crate) delivery: DeliveryDisposition,
    pub elapsed_ms: u64,
    pub verdict: Verdict,
}
impl Report {
    /// Record a failure before reservation or input, preserving the caller's elapsed-time origin.
    pub(crate) fn preflight(request: Request, started: Instant, failure: Failure) -> Self {
        Self {
            name: request.name,
            provider: request.provider,
            reservation: None,
            last_observed: None,
            reservation_state: ReservationState::None,
            submission: None,
            delivery: DeliveryDisposition::NotApplied,
            elapsed_ms: elapsed_ms(started),
            verdict: Verdict::Failed {
                failure,
                read_cause: None,
            },
        }
    }
}
fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub(crate) use workflow::runs_no_program;
pub use workflow::step_deadline;

/// Run startup against the exact Panel in the caller's admitted Snapshot and owned Client.
/// Native admission precedes startup effects. Every path drops the Client before returning,
/// so the caller may then finalize its process-wide signal source without a live lane.
/// Failures preserve reservation and delivery facts in the returned Report; input is never replayed.
pub fn run_on_client(
    request: Request,
    started: Instant,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    client: crate::local_client::LocalClient,
    snapshot: orchbee_workbench_protocol::Snapshot,
    panel: orchbee_workbench_protocol::WorkbenchPanelId,
) -> Report {
    if crate::agent_runtime::startup::admit().is_err() {
        drop(client);
        return Report::preflight(request, started, Failure::Unsupported);
    }
    workflow::run_admitted(
        &request, started, deadline, wake, client, snapshot, panel, None,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Progress {
    Observing,
    WaitingInput,
}

/// Open one lane on the supplied origin and run startup within the original clock and deadline.
/// Native admission precedes lane creation. Progress callbacks describe observations only;
/// callers retain signal finalization and surface-generation ownership after this returns.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_on_origin(
    request: Request,
    started: Instant,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    origin: &crate::local_client::LocalClientOrigin,
    clock: crate::local_client::LocalOperationClock,
    panel: orchbee_workbench_protocol::WorkbenchPanelId,
    progress: &dyn Fn(Progress),
) -> Report {
    workflow::run_on_origin(
        request, started, deadline, wake, origin, clock, panel, progress,
    )
}
pub fn command_valid(request: &Request) -> bool {
    command::launch(request.provider.as_str(), &request.args).is_some()
}

/// Field-for-field `Report` fixture the application's tests build values from.
#[cfg(any(test, feature = "test-support"))]
pub struct ReportFixture {
    pub name: AgentName,
    pub provider: AgentProviderId,
    pub reservation: Option<ObapRequestId>,
    pub last_observed: Option<AgentLaunchReservation>,
    pub reservation_state: ReservationState,
    pub submission: Option<Submission>,
    pub delivery: DeliveryDisposition,
    pub elapsed_ms: u64,
    pub verdict: Verdict,
}

#[cfg(any(test, feature = "test-support"))]
impl From<ReportFixture> for Report {
    fn from(fixture: ReportFixture) -> Self {
        let ReportFixture {
            name,
            provider,
            reservation,
            last_observed,
            reservation_state,
            submission,
            delivery,
            elapsed_ms,
            verdict,
        } = fixture;
        Self {
            name,
            provider,
            reservation,
            last_observed,
            reservation_state,
            submission,
            delivery,
            elapsed_ms,
            verdict,
        }
    }
}
