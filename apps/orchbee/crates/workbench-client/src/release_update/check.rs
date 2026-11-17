//! About's Check for updates. A press asks the Session Server, through `workbench.update-check`, to
//! run its scheduled automatic-update job now, and About follows that run to its end by reading
//! again while it shows. The run, its helper and its install stay the Server's; a version it
//! installs reaches this TUI as the Update control, through the same store reading as a scheduled
//! run's. Only a check this TUI asked for reports its end, so a failed background run still
//! changes nothing on screen.
use super::Build;
use crate::local_client::{LocalClientOrigin, LocalOperationClock, subscription::Request};
use crate::session_capabilities::{SessionAction, serves};
use orchbee_local_runtime::EnvironmentKind;
use orchbee_workbench_protocol::{
    ProtocolVersion, SessionId, UpdateCheckAction, UpdateCheckFinish, UpdateCheckOutcome,
    WorkbenchRequest, WorkbenchResponse, WorkbenchUpdateCheckRequest,
};
use std::time::{Duration, Instant};

/// How long one request may wait for its answer.
const EXCHANGE: Duration = Duration::from_secs(5);
/// How often About reads a run that is queued or running.
const READ_PAUSE: Duration = Duration::from_secs(1);
/// How long About waits for the store to name a version the run installed before it reads the
/// store's answer as final.
const STORE_WAIT: Duration = Duration::from_secs(15);

/// What About's Updates row reports.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum UpdateCheck {
    /// Nothing to report: no check was asked for here, or it ended in the Update control.
    #[default]
    Idle,
    /// A run is queued or running, or the version it installed is being read from the store.
    Checking,
    UpToDate,
    Failed,
    /// This TUI cannot ask, for this reason.
    Unavailable(CheckUnavailable),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckUnavailable {
    /// A development build never updates.
    Development,
    /// The Session runs a version that does not take the request.
    Session,
    /// Nothing updates this TUI's store from here: the platform keeps none, the TUI belongs to
    /// another computer's Session, or the Session's Server runs no automatic updates.
    Installation,
}

impl CheckUnavailable {
    /// The reason for a TUI that has no update state at all, as one attached to a saved machine.
    pub(crate) const fn without_updater() -> Self {
        if matches!(
            orchbee_local_runtime::environment_kind(),
            EnvironmentKind::Development
        ) {
            Self::Development
        } else {
            Self::Installation
        }
    }
}

/// What the TUI around the check supplies on every drive.
pub(crate) struct CheckContext<'a> {
    pub(crate) origin: Option<&'a LocalClientOrigin>,
    pub(crate) session: Option<SessionId>,
    pub(crate) clock: &'a LocalOperationClock,
    /// About is on screen, so a run in progress is read again.
    pub(crate) visible: bool,
}

/// One correlated request; only its own answer settles it.
struct Exchange {
    request: WorkbenchUpdateCheckRequest,
    pending: Request,
}

enum Answer {
    Outcome(UpdateCheckOutcome),
    /// No answer, or one this TUI cannot match to its request.
    Lost,
}

impl Exchange {
    fn poll(&self) -> Option<Answer> {
        Some(match self.pending.poll()? {
            Ok(WorkbenchResponse::UpdateCheck(result))
                if result.request_id == self.request.request_id =>
            {
                Answer::Outcome(result.outcome)
            }
            Ok(_) | Err(_) => Answer::Lost,
        })
    }
}

/// What the store reading tells a check that ended in an install.
#[derive(Clone, Copy)]
pub(super) struct Store {
    /// A reading arrived since the last drive.
    pub(super) read: bool,
    /// The Update control is Ready.
    pub(super) ready: bool,
}

#[derive(Default)]
pub(super) struct Check {
    shown: UpdateCheck,
    exchange: Option<Exchange>,
    /// This TUI asked for the run About follows, so its end is reported.
    asked: bool,
    /// When the next read of a queued or running run is due.
    next_read: Option<Instant>,
    /// After an `installed` answer, the moment About stops waiting for the store's reading.
    installed: Option<Instant>,
    /// About was on screen at the last drive.
    visible: bool,
    /// The Session whose Server this state follows; another one starts afresh.
    session: Option<SessionId>,
    /// That Server answered that it runs no automatic updates.
    refused: bool,
}

impl Check {
    pub(super) const fn shown(&self) -> UpdateCheck {
        self.shown
    }

    /// Asks for a run now, or joins the run About already follows.
    pub(super) fn start(&mut self, build: Build, context: &CheckContext<'_>) {
        self.follow_session(context);
        if let Some(reason) = self.unavailable(build, context) {
            self.settle_unavailable(reason);
            return;
        }
        if self.installed.is_some() {
            return;
        }
        // A run already queued or running is the one this press asks for.
        if self.shown == UpdateCheck::Checking {
            self.asked = true;
            return;
        }
        self.asked = true;
        self.shown = UpdateCheck::Checking;
        self.exchange = None;
        if !self.send(UpdateCheckAction::Start, context) {
            self.asked = false;
            self.shown = UpdateCheck::Failed;
        }
    }

    /// Settles answers and sends the reads a visible About needs. Answers whether the store
    /// should be read now, after a run installed a version.
    pub(super) fn drive(&mut self, build: Build, context: &CheckContext<'_>, store: Store) -> bool {
        self.follow_session(context);
        let appeared = context.visible && !self.visible;
        self.visible = context.visible;
        if let Some(reason) = self.unavailable(build, context) {
            self.settle_unavailable(reason);
            return false;
        }
        if matches!(self.shown, UpdateCheck::Unavailable(_)) {
            self.shown = UpdateCheck::Idle;
        }
        let now = context.clock.now();
        if self.wait_for_store(store, now) {
            return false;
        }
        if let Some(exchange) = &self.exchange {
            let Some(answer) = exchange.poll() else {
                return false;
            };
            self.exchange = None;
            if self.settle(answer, now) {
                return true;
            }
        }
        if appeared && self.shown != UpdateCheck::Checking {
            // A result from an earlier visit is stale; a read shows a run that is under way.
            self.shown = UpdateCheck::Idle;
            self.asked = false;
            let _sent = self.send(UpdateCheckAction::Read, context);
            return false;
        }
        if self.shown == UpdateCheck::Checking
            && context.visible
            && self.next_read.is_none_or(|at| now >= at)
            && !self.send(UpdateCheckAction::Read, context)
        {
            self.settle(Answer::Lost, now);
        }
        false
    }

    /// After an install, keeps Checking until the store names the version or the wait ends.
    /// Answers whether it still waits.
    fn wait_for_store(&mut self, store: Store, now: Instant) -> bool {
        let Some(until) = self.installed else {
            return false;
        };
        if !store.read && !store.ready && now < until {
            return true;
        }
        self.installed = None;
        // A newer active version shows as the Update control; otherwise this TUI is already as
        // new as the store's version.
        self.shown = if store.ready {
            UpdateCheck::Idle
        } else {
            UpdateCheck::UpToDate
        };
        false
    }

    /// Applies one answer; answers whether the run installed a version the store now names.
    fn settle(&mut self, answer: Answer, now: Instant) -> bool {
        let asked = std::mem::take(&mut self.asked);
        let reported = |shown| {
            if asked { shown } else { UpdateCheck::Idle }
        };
        match answer {
            Answer::Outcome(UpdateCheckOutcome::Checking {}) => {
                self.asked = asked;
                self.shown = UpdateCheck::Checking;
                self.next_read = Some(now + READ_PAUSE);
            }
            Answer::Outcome(UpdateCheckOutcome::Finished { result }) => match result {
                UpdateCheckFinish::UpToDate => self.shown = reported(UpdateCheck::UpToDate),
                UpdateCheckFinish::Failed => self.shown = reported(UpdateCheck::Failed),
                UpdateCheckFinish::Installed if asked => {
                    self.shown = UpdateCheck::Checking;
                    self.installed = Some(now + STORE_WAIT);
                    return true;
                }
                UpdateCheckFinish::Installed => self.shown = UpdateCheck::Idle,
            },
            Answer::Outcome(UpdateCheckOutcome::Unavailable {}) => {
                self.refused = true;
                self.shown = UpdateCheck::Unavailable(CheckUnavailable::Installation);
            }
            // `idle` after a start means the Server lost the run it queued.
            Answer::Outcome(
                UpdateCheckOutcome::Idle {}
                | UpdateCheckOutcome::Rejected { .. }
                | UpdateCheckOutcome::Denied {},
            )
            | Answer::Lost => self.shown = reported(UpdateCheck::Failed),
        }
        false
    }

    fn send(&mut self, action: UpdateCheckAction, context: &CheckContext<'_>) -> bool {
        let (Some(origin), Some(session), Ok(request_id)) = (
            context.origin,
            context.session.clone(),
            crate::machine_observation::request_id(),
        ) else {
            return false;
        };
        let request = WorkbenchUpdateCheckRequest {
            protocol_version: ProtocolVersion,
            request_id,
            session_id: session,
            action,
        };
        let Ok(pending) = origin.request(
            &WorkbenchRequest::UpdateCheck(request.clone()),
            context.clock.now() + EXCHANGE,
            context.clock.clone(),
        ) else {
            return false;
        };
        self.next_read = None;
        self.exchange = Some(Exchange { request, pending });
        true
    }

    /// Why this TUI cannot ask now, checked before any request.
    fn unavailable(&self, build: Build, context: &CheckContext<'_>) -> Option<CheckUnavailable> {
        if matches!(build.kind, EnvironmentKind::Development) {
            return Some(CheckUnavailable::Development);
        }
        let origin = context.origin.filter(|origin| origin.is_local());
        if !build.offers_updates() || origin.is_none() || self.refused {
            return Some(CheckUnavailable::Installation);
        }
        // Unlike other actions, an unknown handshake answer counts as missing: a request an older
        // Server cannot read must never reach it.
        let served = origin
            .and_then(LocalClientOrigin::session_capabilities)
            .is_some_and(|answer| serves(Some(&answer), SessionAction::CheckForUpdates));
        (!served).then_some(CheckUnavailable::Session)
    }

    fn settle_unavailable(&mut self, reason: CheckUnavailable) {
        self.exchange = None;
        self.asked = false;
        self.next_read = None;
        self.installed = None;
        self.shown = UpdateCheck::Unavailable(reason);
    }

    /// A reconnect to another Session drops what the old one's Server answered.
    fn follow_session(&mut self, context: &CheckContext<'_>) {
        if context.session.is_some() && self.session != context.session {
            *self = Self {
                session: context.session.clone(),
                visible: self.visible,
                ..Self::default()
            };
        }
    }
}

#[cfg(test)]
mod tests;
