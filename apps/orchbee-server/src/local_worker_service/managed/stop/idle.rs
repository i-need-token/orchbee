//! The managed Windows and macOS Session's idle move: each tick of the stop loop looks at the
//! Session, and a completed countdown begins the ordinary stop under an identity of the Server's
//! own. A failed idle stop is tried again, as a recorded stop refuses every Client until it ends.
//! The stop's record names the version the Session moved to, so a connected Client starts it.
use super::Owners;
use crate::{
    automatic_updates::AutomaticUpdates,
    idle_move::{Activity, Closing, Countdown},
    session_shutdown::SessionShutdown,
};
use orchbee_local_client_protocol::{SessionStopRequest, StopAction, StopId};
use std::time::Instant;

/// The idle move's state across ticks.
#[derive(Default)]
pub(super) struct IdleMove {
    countdown: Countdown,
    /// The stop this owner began, and the store's active version it moves the Session to.
    began: Option<(StopId, String)>,
    /// The identity every stop of a POSIX Server uses; a Windows idle stop draws a fresh one.
    identity: Option<StopId>,
}

impl IdleMove {
    /// An idle move whose stop uses `identity`, the one the Server's signals and own stops use.
    #[cfg(unix)]
    pub(super) fn under(identity: StopId) -> Self {
        Self {
            identity: Some(identity),
            ..Self::default()
        }
    }
}

impl IdleMove {
    /// One tick. Only a Server that schedules automatic updates moves, so the build's switch and
    /// its store version folder decide here too.
    pub(super) fn tick(&mut self, owners: &Owners, shutdown: &SessionShutdown) {
        if let Some((id, _)) = &self.began {
            let id = *id;
            // A retry queues only a failed attempt; one that is pending or running stays as it is.
            shutdown.request(&SessionStopRequest {
                id,
                action: StopAction::Retry,
            });
            return;
        }
        let Some(updates) = owners.automatic_updates.as_ref() else {
            return;
        };
        // Another stop or an `exit` already ends this Server.
        if shutdown.stopping() {
            return;
        }
        let activity = Activity {
            terminal: owners.terminals.terminal_activity(),
            helper: updates.helper_running(),
            operation: owners.terminals.operation_pending(),
        };
        let mut closing = Close {
            owners,
            updates,
            shutdown,
            identity: self.identity,
            version: None,
            began: None,
        };
        if self
            .countdown
            .observe(Instant::now(), activity, &mut closing)
        {
            self.began = closing.began.zip(closing.version);
        }
    }

    /// The version the stop `id` moves the Session to, when this owner began it.
    pub(super) fn moved_to(&self, id: StopId) -> Option<&str> {
        self.began
            .as_ref()
            .filter(|(began, _)| *began == id)
            .map(|(_, version)| version.as_str())
    }
}

struct Close<'a> {
    owners: &'a Owners,
    updates: &'a AutomaticUpdates,
    shutdown: &'a SessionShutdown,
    identity: Option<StopId>,
    /// The newer active version the supersession check found.
    version: Option<String>,
    began: Option<StopId>,
}

impl Closing for Close<'_> {
    fn superseded(&mut self) -> bool {
        self.version = self.updates.superseding();
        self.version.is_some()
    }

    fn worker_idle(&mut self) -> bool {
        self.owners.terminals.worker_processes_idle()
    }

    /// Looks again at what may have started while the Worker answered, then begins. A plugin hook
    /// job started in the moment between the Worker's answer and this begin goes unseen, and the
    /// stop cancels it with the Worker.
    fn begin(&mut self) -> bool {
        let terminals = &self.owners.terminals;
        if terminals.terminal_activity()
            || terminals.operation_pending()
            || self.updates.helper_running()
        {
            return false;
        }
        let Some(id) = self.identity.or_else(stop_id) else {
            return false;
        };
        if !self.shutdown.begin_idle(id) {
            return false;
        }
        self.began = Some(id);
        true
    }
}

/// A fresh stop identity for a Windows idle stop.
#[cfg(windows)]
fn stop_id() -> Option<StopId> {
    super::own_stop_id()
}

/// A POSIX idle move always carries its Server's identity.
#[cfg(unix)]
fn stop_id() -> Option<StopId> {
    None
}
