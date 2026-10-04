//! A separate mutation connection preserves heartbeat delivery and owns shutdown/join.
use super::*;
use crate::local_client::workbench::split_ratio::SplitRatioFailure;
use orchbee_workbench_protocol::{WorkbenchCommand, WorkbenchRevision};

pub(super) struct Mutation {
    wake: LocalStream,
    task: Option<JoinHandle<()>>,
}
impl Mutation {
    pub(super) fn start(
        shared: Shared,
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
        command: WorkbenchCommand,
    ) -> io::Result<Self> {
        let (wake_read, wake) = LocalStream::pair()?;
        let task = thread::Builder::new()
            .name("orchbee-split-ratio".into())
            .spawn(move || {
                let deadline = clock.now() + Duration::from_secs(2);
                let result = origin.reconnect_before_cancellable(
                    deadline,
                    clock.clone(),
                    Some(wake_read.as_local_io()),
                );
                let reply = match result {
                    Ok(mut client) => client.set_split_ratio(
                        command.clone(),
                        deadline,
                        Some(wake_read.as_local_io()),
                    ),
                    Err(_) => Err(SplitRatioFailure {
                        may_have_committed: false,
                        reason:
                            orchbee_workbench_protocol::ClientViewControlReason::ClientDisconnected,
                    }),
                };
                let (lock, signal) = &*shared;
                if let Ok(mut handoff) = lock.lock() {
                    handoff.layout_pending = false;
                    handoff.layout_result = Some(LayoutCompletion {
                        operation_id: command.request_id,
                        reply,
                    });
                }
                signal.notify_all();
            })?;
        Ok(Self {
            wake,
            task: Some(task),
        })
    }
    pub(super) fn finished(&self) -> bool {
        self.task.as_ref().is_none_or(JoinHandle::is_finished)
    }
}
impl Drop for Mutation {
    fn drop(&mut self) {
        let _ = self.wake.shutdown(std::net::Shutdown::Both);
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}
pub(in crate::terminal_session::client_control) struct LayoutCompletion {
    pub(in crate::terminal_session::client_control) operation_id: WorkbenchRequestId,
    pub(in crate::terminal_session::client_control) reply:
        Result<WorkbenchRevision, SplitRatioFailure>,
}
