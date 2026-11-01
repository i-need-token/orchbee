//! Reads one terminal's facts for an idle close. A Viewer that already shows it is borrowed;
//! otherwise a temporary one attaches, reads its first frame and a process sample, and detaches.
//! A read that cannot answer reports `Unknown` instead of failing the Client.
use super::{
    CarrierFailure, ClientExecutor, CommandLease, EventError, Instant, PaneRef, ReadyResult,
    Result, TerminalViewerAction, TerminalViewerOutcome, ViewerUnavailableReason, WorkerEvent,
};
use crate::agent_start::{CloseSample, close_sample};
use crate::primary_directory::decode_process_info;
use crate::terminal_projection::{ShellReport, TerminalProjection};
use crate::workbench_client::mailbox::CloseProbe;
use crate::workbench_projection::ClientTerminalBinding;
use orchbee_client_server_protocol::{HostEpoch, HostTerminalId, ViewerId};
use std::time::Duration;

/// How long a temporary Viewer waits for its first frame.
const FIRST_FRAME: Duration = Duration::from_millis(400);
/// Bounds the temporary Viewer's exchanges, and so how long it holds up the next queued command.
const TEMPORARY_VIEWER: Duration = Duration::from_secs(2);
const FRAME_RETRY: Duration = Duration::from_millis(10);

impl ClientExecutor {
    pub(super) fn run_close_probe(
        &mut self,
        lease: &CommandLease,
        probe: &CloseProbe,
        deadline: Instant,
    ) -> ReadyResult<()> {
        let (sample, report) = match &probe.terminal.pane {
            Some(pane) => (
                self.sample_shown_terminal(lease, pane, deadline)?,
                ShellReport::Silent,
            ),
            None => self.read_unshown_terminal(probe)?,
        };
        let published = self.mailbox.publish_notice_before(
            WorkerEvent::CloseProbeOutcome {
                probe: probe.serial,
                binding: probe.terminal.binding.clone(),
                sample,
                report,
            },
            deadline,
            &self.clock,
        );
        match published {
            // The UI's budget has decided by then, so a late outcome owes nobody anything.
            Ok(_) | Err(EventError::ForwardExpired) => Ok(()),
            Err(EventError::Closed(closed)) => Err(closed.failure().into()),
        }
    }

    /// One process sample through the live Viewer of this exact pane incarnation.
    fn sample_shown_terminal(
        &mut self,
        lease: &CommandLease,
        key: &PaneRef,
        deadline: Instant,
    ) -> ReadyResult<CloseSample> {
        if self.attached_ref(key).is_err() || !self.pane_admitted(lease, key)? {
            return Ok(CloseSample::Unknown);
        }
        let action = TerminalViewerAction::ReadProcessInfo {
            attachment_id: self.viewer_id_for(key)?,
        };
        let Some((_, outcome)) = self.viewer_for_ready_lease(lease, key, action, deadline)? else {
            return Ok(CloseSample::Unknown);
        };
        if matches!(
            outcome,
            TerminalViewerOutcome::Unavailable {
                reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
            }
        ) {
            self.dispose_lost_viewer(lease, key, deadline)?;
            return Ok(CloseSample::Unknown);
        }
        if matches!(outcome, TerminalViewerOutcome::Unavailable { .. }) {
            self.settle_pane_viewer_lost(lease, key, deadline)?;
            return Ok(CloseSample::Unknown);
        }
        Ok(self
            .attached_ref(key)
            .map_or(CloseSample::Unknown, |attached| {
                classify(
                    outcome,
                    &attached.attachment_id,
                    &attached.host_epoch,
                    &attached.host_terminal_id,
                )
            }))
    }

    /// Every fallible read returns through Detach; a lost Detach retires this Client, as it does
    /// for the other temporary Viewers.
    fn read_unshown_terminal(&mut self, probe: &CloseProbe) -> Result<(CloseSample, ShellReport)> {
        let unknown = (CloseSample::Unknown, ShellReport::Silent);
        let deadline = self
            .clock
            .now()
            .checked_add(TEMPORARY_VIEWER)
            .ok_or(CarrierFailure::Deadline)?;
        let Some(action) = attach_action(&probe.session, &probe.terminal.binding) else {
            return Ok(unknown);
        };
        let TerminalViewerOutcome::Attached {
            attachment_id,
            host_epoch,
            host_terminal_id,
            ..
        } = self.source_viewer(action, deadline)?
        else {
            return Ok(unknown);
        };
        let report =
            self.first_frame_report(&attachment_id, &host_epoch, &host_terminal_id, deadline);
        let sample = self.source_viewer(
            TerminalViewerAction::ReadProcessInfo {
                attachment_id: attachment_id.clone(),
            },
            deadline,
        );
        let detached = self.source_viewer(
            TerminalViewerAction::Detach {
                attachment_id: attachment_id.clone(),
            },
            deadline,
        );
        // A terminal that ended meanwhile took its Viewer with it.
        if !matches!(
            detached?,
            TerminalViewerOutcome::Detached {}
                | TerminalViewerOutcome::Rejected { .. }
                | TerminalViewerOutcome::Unavailable { .. }
        ) {
            return Err(CarrierFailure::Protocol);
        }
        let sample = classify(sample?, &attachment_id, &host_epoch, &host_terminal_id);
        Ok((sample, report?))
    }

    /// The shell's report in this Viewer's first frame; silent when no frame arrives in time.
    fn first_frame_report(
        &mut self,
        attachment_id: &ViewerId,
        host_epoch: &HostEpoch,
        host_terminal_id: &HostTerminalId,
        deadline: Instant,
    ) -> Result<ShellReport> {
        let (Ok(attachment), Ok(epoch)) = (
            orchbee_terminal_protocol::AttachmentId::parse(attachment_id.as_str()),
            orchbee_terminal_protocol::HostEpoch::parse(host_epoch.as_str()),
        ) else {
            return Ok(ShellReport::Silent);
        };
        let mut projection =
            TerminalProjection::new(attachment, epoch).with_host_terminal(host_terminal_id.clone());
        let cutoff = self
            .clock
            .now()
            .checked_add(FIRST_FRAME)
            .map_or(deadline, |cutoff| cutoff.min(deadline));
        while self.clock.now() < cutoff {
            let action = TerminalViewerAction::Poll {
                attachment_id: attachment_id.clone(),
            };
            match self.source_viewer(action, deadline)? {
                TerminalViewerOutcome::Empty {} => std::thread::sleep(FRAME_RETRY),
                TerminalViewerOutcome::Record(record) => {
                    match projection.receive_for_geometry(&record, None) {
                        Ok(Some(TerminalViewerAction::FrameApplied { .. })) => {
                            return Ok(projection.shell_report());
                        }
                        Ok(Some(feedback)) => {
                            let _accepted = self.source_viewer(feedback, deadline)?;
                        }
                        Ok(None) => {}
                        Err(_) => return Ok(ShellReport::Silent),
                    }
                }
                _ => return Ok(ShellReport::Silent),
            }
        }
        Ok(ShellReport::Silent)
    }
}

fn attach_action(
    session: &orchbee_workbench_protocol::SessionId,
    binding: &ClientTerminalBinding,
) -> Option<TerminalViewerAction> {
    Some(TerminalViewerAction::AttachBinding {
        session_id: orchbee_client_server_protocol::SessionId::parse(session.as_str()).ok()?,
        worker_id: orchbee_client_server_protocol::WorkerId::parse(binding.worker_id().as_str())
            .ok()?,
        terminal_id: orchbee_client_server_protocol::TerminalId::parse(
            binding.terminal_id().as_str(),
        )
        .ok()?,
    })
}

fn classify(
    outcome: TerminalViewerOutcome,
    attachment_id: &ViewerId,
    host_epoch: &HostEpoch,
    host_terminal_id: &HostTerminalId,
) -> CloseSample {
    decode_process_info(outcome, attachment_id, host_epoch, host_terminal_id)
        .ok()
        .flatten()
        .map_or(CloseSample::Unknown, |info| close_sample(&info))
}
