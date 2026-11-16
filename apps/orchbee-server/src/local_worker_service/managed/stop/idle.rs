//! The managed Windows Session's idle move: each tick of the stop loop looks at the Session, and a
//! completed countdown begins the ordinary stop under an identity of the Server's own. A failed idle
//! stop is tried again, as a recorded stop refuses every Client until it ends.
use super::{Owners, own_stop_id};
use crate::{
    automatic_updates::AutomaticUpdates,
    idle_move::{Activity, Closing, Countdown},
    local_client_service::ManagedClients,
    session_shutdown::SessionShutdown,
};
use orchbee_local_client_protocol::{SessionStopRequest, StopAction, StopId};
use std::time::Instant;

/// The idle move's state across ticks.
#[derive(Default)]
pub(super) struct IdleMove {
    countdown: Countdown,
    /// The Client connection mark of the previous look.
    mark: Option<u64>,
    /// The stop this owner began.
    began: Option<StopId>,
}

impl IdleMove {
    /// One tick. Only a Server that schedules automatic updates moves, so the build's switch and
    /// its store version folder decide here too.
    pub(super) fn tick(
        &mut self,
        owners: &Owners,
        shutdown: &SessionShutdown,
        clients: &ManagedClients,
    ) {
        if let Some(id) = self.began {
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
        let mark = clients.idle_mark();
        let activity = Activity {
            terminal: owners.terminals.terminal_activity(),
            client: mark.is_none() || mark != self.mark,
            helper: updates.helper_running(),
            operation: owners.terminals.operation_pending(),
        };
        self.mark = mark;
        let mut closing = Close {
            owners,
            updates,
            shutdown,
            clients,
            mark,
            began: None,
        };
        if self
            .countdown
            .observe(Instant::now(), activity, &mut closing)
        {
            self.began = closing.began;
        }
    }
}

struct Close<'a> {
    owners: &'a Owners,
    updates: &'a AutomaticUpdates,
    shutdown: &'a SessionShutdown,
    clients: &'a ManagedClients,
    mark: Option<u64>,
    began: Option<StopId>,
}

impl Closing for Close<'_> {
    fn superseded(&mut self) -> bool {
        self.updates.superseded()
    }

    fn worker_idle(&mut self) -> bool {
        self.owners.terminals.worker_processes_idle()
    }

    /// Looks again at what may have started while the Worker answered, then begins only if no
    /// Client connection opened since the last look. A plugin hook job started in the moment
    /// between the Worker's answer and this begin goes unseen, and the stop cancels it with the
    /// Worker.
    fn begin(&mut self) -> bool {
        let terminals = &self.owners.terminals;
        if terminals.terminal_activity()
            || terminals.operation_pending()
            || self.updates.helper_running()
        {
            return false;
        }
        let Some(id) = own_stop_id() else {
            return false;
        };
        let (clients, mark) = (self.clients, self.mark);
        if !self.shutdown.begin_idle(id, || clients.idle_mark() == mark) {
            return false;
        }
        self.began = Some(id);
        true
    }
}
