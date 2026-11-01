//! Closes one Tab or Panel at once when every terminal it ends is a shell at its prompt.
//!
//! The confirmation opens first and stays hidden while one process sample per terminal is read.
//! Only a complete set of idle samples, while this Client controls the Workspace, admits the close
//! without a draw. A sample that is refused, unavailable or not an idle shell, a terminal without a
//! live projection, a ReadOnly Client, a set or a Resume, and a budget that runs out all present the
//! confirmation unchanged.

use super::WorkbenchClientUi;
use crate::workbench_client::mailbox::{AdmissionError, QueuedCommand, Tagged};
use crate::workbench_state::{
    CloseConfirmationKind, CloseConfirmationToken, ControllerMode, PaneRef,
    PanelCloseConfirmationToken,
};
use std::time::{Duration, Instant};

/// How long a close waits for its samples before it presents the confirmation.
const PROBE_BUDGET: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Target {
    Tab(CloseConfirmationToken),
    Panel(PanelCloseConfirmationToken),
}

struct Probe {
    serial: u64,
    target: Target,
    pending: Vec<PaneRef>,
    until: Instant,
}

pub(super) struct IdleClose {
    /// Whether a close samples its terminals first. Harnesses that script every Host exchange
    /// present each confirmation instead.
    enabled: bool,
    serial: u64,
    probe: Option<Probe>,
}

impl IdleClose {
    pub(super) const fn new(enabled: bool) -> Self {
        Self {
            enabled,
            serial: 0,
            probe: None,
        }
    }

    /// Whether a confirmation stays hidden while its terminals are sampled.
    pub(super) const fn probing(&self) -> bool {
        self.probe.is_some()
    }
}

impl WorkbenchClientUi {
    /// Samples every terminal the just-opened confirmation of `target` ends, keeping it hidden.
    pub(super) fn begin_idle_close(&mut self, target: Target) {
        self.idle_close.probe = None;
        if !self.idle_close.enabled {
            return;
        }
        let (Some(panes), Some(mailbox)) = (self.idle_close_panes(target), self.mailbox.as_ref())
        else {
            return;
        };
        let Some(serial) = self.idle_close.serial.checked_add(1) else {
            return;
        };
        self.idle_close.serial = serial;
        for pane in &panes {
            match mailbox.try_enqueue(QueuedCommand::ProcessProbe(Tagged::new(
                pane.clone(),
                serial,
            ))) {
                Ok(_) => {}
                Err(AdmissionError::Closed(closed)) => {
                    self.retire(closed.failure());
                    return;
                }
                Err(_) => return,
            }
        }
        if let Some(until) = self.clock.now().checked_add(PROBE_BUDGET) {
            self.idle_close.probe = Some(Probe {
                serial,
                target,
                pending: panes,
                until,
            });
        }
    }

    /// Every pane the target ends, when this Client controls the Workspace and holds a live
    /// projection of each one; otherwise `None`, and the confirmation presents.
    fn idle_close_panes(&self, target: Target) -> Option<Vec<PaneRef>> {
        let state = self.state.as_ref()?;
        if !matches!(
            state.controller_mode(),
            Some(ControllerMode::Controlling(_))
        ) {
            return None;
        }
        let view = state.view();
        let (tab_id, panel_id) = match target {
            Target::Tab(token) => {
                let confirmation = view.close_confirmation().filter(|confirmation| {
                    confirmation.token() == token
                        && confirmation.set().is_none()
                        && confirmation.kind() == CloseConfirmationKind::Close
                })?;
                (confirmation.target_tab_id(), None)
            }
            Target::Panel(token) => {
                let confirmation = view
                    .panel_close_confirmation()
                    .filter(|confirmation| confirmation.token() == token)?;
                (confirmation.tab_id(), Some(confirmation.panel_id()))
            }
        };
        let tab = state
            .cache()
            .tabs()
            .iter()
            .find(|tab| tab.tab_id() == tab_id)?;
        let expected = if panel_id.is_some() {
            1
        } else {
            tab.panel_count()
        };
        let panes: Vec<PaneRef> = self
            .projections
            .iter()
            .map(|projection| &projection.key)
            .filter(|key| {
                key.pane().anchor().tab_id() == tab_id
                    && panel_id.is_none_or(|panel| key.pane().panel() == panel)
            })
            .cloned()
            .collect();
        (expected > 0 && panes.len() == expected).then_some(panes)
    }

    /// Settles one sample: a pane that runs a program presents the confirmation at once; the last
    /// idle sample closes the target without presenting it.
    pub(super) fn apply_idle_close_probe(
        &mut self,
        key: &PaneRef,
        serial: u64,
        idle: bool,
    ) -> bool {
        let Some(probe) = self
            .idle_close
            .probe
            .as_mut()
            .filter(|probe| probe.serial == serial)
        else {
            return false;
        };
        if !idle {
            self.idle_close.probe = None;
            return true;
        }
        probe.pending.retain(|pane| pane != key);
        if !probe.pending.is_empty() {
            return false;
        }
        let target = probe.target;
        self.idle_close.probe = None;
        let admitted = self.state.as_mut().is_some_and(|state| match target {
            Target::Tab(token) => state.admit_unconfirmed_close(token),
            Target::Panel(token) => state.admit_unconfirmed_panel_close(token),
        });
        if admitted {
            // A refused admission leaves the confirmation open, and it presents as usual.
            let _ = match target {
                Target::Tab(_) => self.confirm_close(),
                Target::Panel(_) => self.confirm_panel_close(),
            };
        }
        true
    }

    /// Presents the confirmation when its budget runs out, and forgets a probe whose
    /// confirmation was cancelled or replaced.
    pub(super) fn drive_idle_close(&mut self) -> bool {
        let Some(probe) = &self.idle_close.probe else {
            return false;
        };
        let current = self.state.as_ref().is_some_and(|state| {
            let view = state.view();
            match probe.target {
                Target::Tab(token) => view
                    .close_confirmation()
                    .is_some_and(|confirmation| confirmation.token() == token),
                Target::Panel(token) => view
                    .panel_close_confirmation()
                    .is_some_and(|confirmation| confirmation.token() == token),
            }
        });
        if current && self.clock.now() < probe.until {
            return false;
        }
        self.idle_close.probe = None;
        true
    }
}
