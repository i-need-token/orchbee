//! Closes one Tab or Panel at once when no program runs in any terminal it ends.
//!
//! The confirmation opens first and stays hidden while each live terminal is read. Two independent
//! facts decide: the shell's own report in the terminal's frames, and one process sample. A test
//! must produce each of them as a real session does; see `docs/native-tab-close-sets.md`.
//!
//! A command the shell reports as running, or a program holding the terminal's console, presents
//! the confirmation. A shell that reports its prompt closes even when the sample cannot answer: a
//! refused, unavailable or late sample is an unknown fact, not a running program. A shell that
//! reports nothing, such as `cmd`, is judged by its sample alone and asks when that cannot answer.
//! A ReadOnly Client, a set of several Tabs and a Resume always present the confirmation.

use super::WorkbenchClientUi;
use crate::agent_start::CloseSample;
use crate::terminal_projection::ShellReport;
use crate::workbench_client::mailbox::{AdmissionError, CloseProbe, QueuedCommand};
use crate::workbench_projection::ClientTerminalBinding;
use crate::workbench_state::{
    CloseConfirmationToken, CloseTerminal, ControllerMode, PanelCloseConfirmationToken,
};
use std::time::{Duration, Instant};

/// How long a close waits for its reads before it decides without them. This matches the Worker's
/// own bound on one process sample, so a slow answer still arrives.
const PROBE_BUDGET: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Target {
    Tab(CloseConfirmationToken),
    Panel(PanelCloseConfirmationToken),
}

/// Whether a terminal may close unasked once its sample is known.
const fn runs_nothing(report: ShellReport, sample: CloseSample) -> bool {
    match (report, sample) {
        (ShellReport::Running, _)
        | (_, CloseSample::Program)
        | (ShellReport::Silent, CloseSample::Unknown) => false,
        (ShellReport::AtPrompt | ShellReport::Silent, CloseSample::ShellOnly)
        | (ShellReport::AtPrompt, CloseSample::Unknown) => true,
    }
}

struct Probe {
    serial: u64,
    target: Target,
    pending: Vec<CloseTerminal>,
    until: Instant,
}

pub(super) struct IdleClose {
    /// Whether a close reads its terminals first. Harnesses that script every Host exchange
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

    /// Whether a confirmation stays hidden while its terminals are read.
    pub(super) const fn probing(&self) -> bool {
        self.probe.is_some()
    }
}

impl WorkbenchClientUi {
    /// Reads every live terminal the just-opened confirmation of `target` ends, keeping it hidden.
    pub(super) fn begin_idle_close(&mut self, target: Target) {
        self.idle_close.probe = None;
        if !self.idle_close.enabled {
            return;
        }
        let Some(terminals) = self.idle_close_terminals(target) else {
            return;
        };
        // Everything ended: nothing is left to read and nothing runs.
        if terminals.is_empty() {
            self.admit_idle_close(target);
            return;
        }
        if terminals
            .iter()
            .any(|terminal| self.shown_shell_report(terminal) == ShellReport::Running)
        {
            return;
        }
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_ref()) else {
            return;
        };
        let session = state.cache().session_id().clone();
        let (Some(serial), Some(until)) = (
            self.idle_close.serial.checked_add(1),
            self.clock.now().checked_add(PROBE_BUDGET),
        ) else {
            return;
        };
        self.idle_close.serial = serial;
        for terminal in &terminals {
            let probe = CloseProbe {
                serial,
                session: session.clone(),
                terminal: terminal.clone(),
            };
            // A read the mailbox refuses stays pending, and the budget decides without it.
            if let Err(AdmissionError::Closed(closed)) =
                mailbox.try_enqueue(QueuedCommand::CloseProbe(probe))
            {
                self.retire(closed.failure());
                return;
            }
        }
        self.idle_close.probe = Some(Probe {
            serial,
            target,
            pending: terminals,
            until,
        });
    }

    /// Every live terminal the target ends, when this Client controls the Workspace and can name
    /// them all; otherwise `None`, and the confirmation presents.
    fn idle_close_terminals(&self, target: Target) -> Option<Vec<CloseTerminal>> {
        let state = self.state.as_ref()?;
        if !matches!(
            state.controller_mode(),
            Some(ControllerMode::Controlling(_))
        ) {
            return None;
        }
        let view = state.view();
        let settled = |pane: &_| {
            self.projections
                .iter()
                .any(|projection| &projection.key == pane && projection.value.end().is_some())
        };
        match target {
            Target::Tab(token) => {
                let confirmation = view
                    .close_confirmation()
                    .filter(|confirmation| confirmation.token() == token)?;
                state.tab_live_terminals(confirmation.sole_tab_id()?, &settled)
            }
            Target::Panel(token) => {
                let confirmation = view
                    .panel_close_confirmation()
                    .filter(|confirmation| confirmation.token() == token)?;
                state.panel_live_terminals(confirmation.tab_id(), confirmation.panel_id(), &settled)
            }
        }
    }

    /// The report in the frames of the pane showing `terminal`, freshest at each decision.
    fn shown_shell_report(&self, terminal: &CloseTerminal) -> ShellReport {
        terminal
            .pane
            .as_ref()
            .and_then(|pane| {
                self.projections
                    .iter()
                    .find(|projection| &projection.key == pane)
            })
            .map_or(ShellReport::Silent, |projection| {
                projection.value.shell_report()
            })
    }

    /// Settles one terminal: one running a program presents the confirmation at once; the last one
    /// running nothing closes the target without presenting it.
    pub(super) fn apply_idle_close_probe(
        &mut self,
        binding: &ClientTerminalBinding,
        serial: u64,
        sample: CloseSample,
        unshown_report: ShellReport,
    ) -> bool {
        let Some(terminal) = self
            .idle_close
            .probe
            .as_ref()
            .filter(|probe| probe.serial == serial)
            .and_then(|probe| {
                probe
                    .pending
                    .iter()
                    .find(|terminal| &terminal.binding == binding)
            })
            .cloned()
        else {
            return false;
        };
        let report = if terminal.pane.is_some() {
            self.shown_shell_report(&terminal)
        } else {
            unshown_report
        };
        if !runs_nothing(report, sample) {
            self.idle_close.probe = None;
            return true;
        }
        let Some(probe) = self.idle_close.probe.as_mut() else {
            return false;
        };
        probe.pending.retain(|pending| pending.binding != *binding);
        if !probe.pending.is_empty() {
            return false;
        }
        let target = probe.target;
        self.idle_close.probe = None;
        self.admit_idle_close(target);
        true
    }

    /// Runs the armed close without presenting it. A refused admission leaves the confirmation
    /// open, and it presents as usual.
    fn admit_idle_close(&mut self, target: Target) {
        let admitted = self.state.as_mut().is_some_and(|state| match target {
            Target::Tab(token) => state.admit_unconfirmed_close(token),
            Target::Panel(token) => state.admit_unconfirmed_panel_close(token),
        });
        if admitted {
            let _ = match target {
                Target::Tab(_) => self.confirm_close(),
                Target::Panel(_) => self.confirm_panel_close(),
            };
        }
    }

    /// Decides when the budget runs out: the terminals still unread close only if each shell
    /// reports its prompt. Forgets a probe whose confirmation was cancelled or replaced.
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
        let target = probe.target;
        let at_prompt = current
            && probe
                .pending
                .iter()
                .all(|terminal| self.shown_shell_report(terminal) == ShellReport::AtPrompt);
        self.idle_close.probe = None;
        if at_prompt {
            self.admit_idle_close(target);
        }
        true
    }
}
