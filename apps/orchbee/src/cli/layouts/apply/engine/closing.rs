//! Await retained Close completion; recovery closes only an unchanged workflow-owned Tab.
use super::{Engine, error, ownership};
use crate::{
    cli::{Failure, FailureKind, console_interrupt::SignalWatch, mutation, query},
    local_client::LocalClient,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, SessionId, WorkbenchAction, WorkbenchRequestId, WorkbenchTabCloseRead,
    WorkbenchTabCloseReadOutcome, WorkbenchTabCloseStatus, WorkbenchTabId,
    WorkbenchTerminalContextId,
};
use serde_json::json;
use std::time::{Duration, Instant};

impl Engine<'_> {
    pub(super) fn close_old(&mut self) -> Result<(), Failure> {
        let Some(old) = self.destination.old.clone() else {
            return Ok(());
        };
        let receipt = self.submit(
            "replace",
            WorkbenchAction::TabClose {
                terminal_context_id: self.destination.context.clone(),
                tab_id: old.tab_id.clone(),
            },
        )?;
        let close = receipt
            .close_request_id()
            .and_then(|id| WorkbenchRequestId::parse(id).ok())
            .ok_or_else(|| error(FailureKind::ProtocolInvalid, "invalid-receipt"))?;
        // Keep the acknowledged close ID before checking the signal or deadline.
        loop {
            self.check()?;
            let result = read_close(
                &mut self.client,
                &self.snapshot.session_id,
                &self.destination.context,
                &old.tab_id,
                &close,
                self.deadline,
                self.signals.poll_fd(),
            );
            match result {
                Ok(status) => {
                    self.receipt.record(
                        "replace-status",
                        json!({"closeRequestId":close,"status":status}),
                    );
                    match status {
                        WorkbenchTabCloseStatus::Closed { revision } => {
                            self.destination.old = None;
                            return self.refresh(revision);
                        }
                        WorkbenchTabCloseStatus::Closing { .. } => {
                            self.check()?;
                            self.pause();
                        }
                        WorkbenchTabCloseStatus::OutcomeUnknown { .. } => {
                            return Err(error(FailureKind::OutcomeUnknown, "close-incomplete"));
                        }
                        WorkbenchTabCloseStatus::CleanupBlocked { .. }
                        | WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. } => {
                            return Err(error(FailureKind::ServiceUnavailable, "close-incomplete"));
                        }
                    }
                }
                Err(failure) => {
                    self.check()?;
                    return Err(failure);
                }
            }
        }
    }
    pub(super) fn recover(&mut self) {
        if self.receipt.new_tab.is_none() {
            return;
        }
        let result = self.recover_known();
        if let Err(failure) = result {
            self.receipt.recovery["failure"] = failure.observation();
            self.receipt.recovery["outcome"] = json!("incomplete");
        }
    }
    fn recover_known(&mut self) -> Result<(), Failure> {
        let expected = self
            .expected
            .as_ref()
            .ok_or_else(|| error(FailureKind::Denied, "ownership-unverified"))?;
        if Instant::now() >= self.total_deadline {
            return Err(error(FailureKind::Timeout, "recovery-expired"));
        }
        // A latched user cancellation cannot interrupt the bounded compensating Close itself.
        // A fresh admission avoids replaying on a retired or uncertain original connection.
        let (mut client, snapshot) = query::stream_snapshot(self.total_deadline, None)?;
        if snapshot.session_id != self.snapshot.session_id {
            return Err(error(FailureKind::Denied, "ownership-unverified"));
        }
        ownership::unchanged(&snapshot, &self.destination.context, expected)?;
        let result = mutation::submit_action(
            &mut client,
            &snapshot,
            WorkbenchAction::TabClose {
                terminal_context_id: self.destination.context.clone(),
                tab_id: expected.tab_id.clone(),
            },
            self.total_deadline,
            None,
        );
        match &result {
            Ok(receipt) => self.receipt.record("rollback", receipt.document()),
            Err(failure) => self.receipt.record("rollback", failure.document()),
        }
        let receipt = result?;
        let close = receipt
            .close_request_id()
            .and_then(|id| WorkbenchRequestId::parse(id).ok())
            .ok_or_else(|| error(FailureKind::ProtocolInvalid, "invalid-receipt"))?;
        self.receipt.recovery = json!({"outcome":"closing","closeRequestId":close});
        loop {
            if Instant::now() >= self.total_deadline {
                return Err(error(FailureKind::Timeout, "recovery-expired"));
            }
            let status = read_close(
                &mut client,
                &snapshot.session_id,
                &self.destination.context,
                &expected.tab_id,
                &close,
                self.total_deadline,
                None,
            )?;
            self.receipt.recovery =
                json!({"outcome":"closing","closeRequestId":close,"status":status});
            match status {
                WorkbenchTabCloseStatus::Closed { .. } => {
                    self.receipt.recovery["outcome"] = json!("closed");
                    return Ok(());
                }
                WorkbenchTabCloseStatus::Closing { .. } => std::thread::sleep(
                    Duration::from_millis(30).min(
                        self.total_deadline
                            .saturating_duration_since(Instant::now()),
                    ),
                ),
                WorkbenchTabCloseStatus::OutcomeUnknown { .. } => {
                    return Err(error(FailureKind::OutcomeUnknown, "close-incomplete"));
                }
                WorkbenchTabCloseStatus::CleanupBlocked { .. }
                | WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. } => {
                    return Err(error(FailureKind::ServiceUnavailable, "close-incomplete"));
                }
            }
        }
    }
}
fn read_close(
    client: &mut LocalClient,
    session: &SessionId,
    context: &WorkbenchTerminalContextId,
    tab: &WorkbenchTabId,
    close: &WorkbenchRequestId,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
) -> Result<WorkbenchTabCloseStatus, Failure> {
    let outcome = client
        .submit_workbench_tab_close_read_with_wake(
            WorkbenchTabCloseRead {
                protocol_version: ProtocolVersion,
                request_id: query::request_id()?,
                session_id: session.clone(),
                terminal_context_id: context.clone(),
                tab_id: tab.clone(),
            },
            Some(close),
            deadline,
            wake,
        )
        .map_err(|_| error(FailureKind::ServiceUnavailable, "close-incomplete"))?;
    match outcome {
        WorkbenchTabCloseReadOutcome::Available { status, .. } => Ok(status),
        WorkbenchTabCloseReadOutcome::NotClosing { .. }
        | WorkbenchTabCloseReadOutcome::Denied { .. }
        | WorkbenchTabCloseReadOutcome::Unavailable { .. } => {
            Err(error(FailureKind::ServiceUnavailable, "close-incomplete"))
        }
    }
}
