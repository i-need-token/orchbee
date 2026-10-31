//! Starts a dedicated retained-close observation task only after the prior Client connection
//! task is absent, preserving its origin and recovery mailbox seed. It does not replay close
//! mutations or replace the Server's retained status authority.

use super::{ClientTask, WorkbenchClient, connection};
use crate::background_thread::BackgroundThread;
use crate::workbench_state;

impl WorkbenchClient {
    pub(super) fn start_status_recovery(&mut self) -> bool {
        if self.task.is_some() {
            return false;
        }
        let Some(origin) = self.origin.clone() else {
            return false;
        };
        let Some(native) = self.backend.native_mut() else {
            return false;
        };
        let (mailbox, clock, deadline, membership) = if let Some((mailbox, clock)) =
            native.begin_status_recovery()
        {
            let Some(deadline) = clock.now().checked_add(workbench_state::OPERATION_TOTAL) else {
                native.close(workbench_state::CarrierFailure::Deadline);
                native.finalize_retirement(
                    None,
                    None,
                    None,
                    workbench_state::CloseWorkerReturn::Returned(None),
                );
                native.release_mailbox();
                return true;
            };
            (mailbox, clock, deadline, false)
        } else if let Some((mailbox, clock, deadline)) = native.begin_retired_membership_recovery()
        {
            (mailbox, clock, deadline, true)
        } else {
            return false;
        };
        let shutdown = connection::RecoveryShutdown::new();
        let worker_shutdown = shutdown.clone();
        match BackgroundThread::spawn("orchbee-close-status-recovery", move || {
            connection::run_status_recovery(
                origin,
                clock,
                mailbox,
                worker_shutdown,
                deadline,
                membership,
            )
        }) {
            Ok(task) => {
                self.recovery_shutdown = Some(shutdown);
                self.task = Some(ClientTask::Native(task));
            }
            Err(_) => {
                shutdown.close();
                native.finish_retired_peer_join(None, true);
                native.close(workbench_state::CarrierFailure::WorkerExited);
                native.finalize_retirement(
                    None,
                    None,
                    None,
                    workbench_state::CloseWorkerReturn::Returned(None),
                );
                native.release_mailbox();
            }
        }
        true
    }
}
