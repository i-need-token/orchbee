//! Application-owned Agent execution ports and shared observation facts.
//! Native startup consumes this boundary; CLI adapters reuse its pure facts and cancellation.
use orchbee_agent_observation_protocol::{AgentSafeInteger, AgentSessionDetail, ObapRequestId};
use orchbee_local_ipc::{LocalIo as BorrowedFd, PollFd, PollFlags, PollTimeout, poll};
use std::time::Duration;
pub mod processes;
pub mod startup;

/// Which stderr diagnostic a typed send answer carries. The send envelope on
/// stdout always reports both axes verbatim; this only picks the localized
/// guidance line — most importantly the cases where resending could type
/// twice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendVerdict {
    /// `accepted` + `confirmed`: nothing to warn about.
    Quiet,
    /// The input landed but detach never confirmed; resending could type twice.
    AcceptedCleanupUnconfirmed,
    /// A typed refusal; the input provably did not reach the Host queue.
    Rejected,
    /// A determinate stop after a confirmed prefix: the receipt names the
    /// frontier and resending the whole sequence could type twice.
    PartialAccepted,
    /// Never submitted; no Host write was attempted.
    Unavailable,
    /// The exchange never settled; the input may already have landed.
    OutcomeUnknown,
}

pub struct AgentObservationRead {
    pub request_id: ObapRequestId,
    pub revision: AgentSafeInteger,
    pub session: Box<AgentSessionDetail>,
}

/// The interruptible private backoff: poll the borrowed wake descriptor
/// for up to `duration`; `true` means it reported readiness or an
/// unrecoverable poll fault, and the owner's arbitration must run before
/// any further step.
pub fn wait_backoff(wake: Option<BorrowedFd<'_>>, duration: Duration) -> bool {
    let Some(fd) = wake else {
        if !duration.is_zero() {
            std::thread::sleep(duration);
        }
        return false;
    };
    if duration.is_zero() {
        return false;
    }
    let mut descriptors = [PollFd::new(fd, PollFlags::POLLIN)];
    let rounded_milliseconds = duration.as_millis().saturating_add(u128::from(
        !duration.subsec_nanos().is_multiple_of(1_000_000),
    ));
    let timeout = PollTimeout::try_from(rounded_milliseconds).unwrap_or(PollTimeout::MAX);
    match poll(&mut descriptors, timeout) {
        Ok(0) => false,
        Ok(_) => descriptors[0]
            .revents()
            .unwrap_or_else(PollFlags::empty)
            .intersects(
                PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR | PollFlags::POLLNVAL,
            ),
        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => false,
        // A descriptor-level poll fault is an internal condition: stop so
        // the owner's arbitration can land it on `internal-failure`.
        Err(_) => true,
    }
}
