//! Connects kept terminal output to the Session: it reads the setting, captures each live
//! terminal's recent output on a temporary viewer before a save, prunes files after a committed
//! save, and builds the seed a restored terminal shows before its program starts. The format,
//! caps and storage belong to `crate::terminal_output`.

use super::LocalTerminalService;
use crate::localization::{self, Locale};
use crate::session_coordinator::ProcessObservationTarget;
use crate::terminal_output::{self, TerminalOutput};
use orchbee_workbench_protocol::{PresentationBinding, TerminalPresentationId};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
use super::viewer_connection::process_sampling::screen::{ReplayCapture, replay_capture};
#[cfg(windows)]
use std::{sync::atomic::AtomicBool, time::Instant};

impl LocalTerminalService {
    pub(crate) fn install_terminal_output(&self, output: Arc<TerminalOutput>) -> bool {
        self.terminal_output.set(output).is_ok()
    }

    pub(crate) fn terminal_output(&self) -> Option<&Arc<TerminalOutput>> {
        self.terminal_output.get()
    }

    /// Whether this Session keeps and replays terminal output, read from the last valid settings
    /// before the caller takes any lock. Before startup installs configuration it reads as off.
    pub(crate) fn output_replay_enabled(&self) -> bool {
        self.configuration
            .with_preferences(|values| values.restore_terminal_output)
            .unwrap_or(false)
    }

    fn output_locale(&self) -> Locale {
        let preference = self
            .configuration
            .with_preferences(|values| values.locale)
            .unwrap_or_default();
        Locale::configured(preference, |name| std::env::var_os(name))
    }

    /// The seed a restored `presentation` shows before its program starts: its kept output and
    /// the divider in the configured language. A missing, unreadable or foreign file yields none.
    pub(crate) fn restored_output_seed(
        &self,
        presentation: &TerminalPresentationId,
        token: u64,
    ) -> Option<Vec<u8>> {
        let (content, saved_at) = self.terminal_output()?.restored(presentation, token)?;
        let divider = localization::output_before_restart(
            self.output_locale(),
            orchbee_local_runtime::local_clock(saved_at),
        );
        Some(terminal_output::seed(&content, &divider))
    }

    /// After a committed save, removes every kept file the checkpoint does not name. With the
    /// setting off the checkpoint names none, so every file goes.
    pub(crate) fn terminal_output_after_save(&self, bytes: &[u8]) {
        let Some(output) = self.terminal_output() else {
            return;
        };
        let named = orchbee_server::WorkbenchCheckpoint::parse(bytes)
            .map(|checkpoint| checkpoint.terminal_output_tokens());
        let pruned = match named {
            Ok(named) => output.after_publish(&named),
            Err(_) => return,
        };
        if let Err(error) = pruned {
            diagnostic("prune", &error);
        }
    }

    /// Removes every kept file at once, when the setting has just been turned off.
    pub(crate) fn remove_terminal_output(&self) {
        if let Some(output) = self.terminal_output()
            && let Err(error) = output.remove_all()
        {
            diagnostic("remove", &error);
        }
    }

    /// The tokens the next checkpoint names for `targets`, or `None` when replay is off.
    pub(crate) fn checkpoint_outputs(
        &self,
        enabled: bool,
        targets: &[ProcessObservationTarget],
    ) -> Option<Vec<(PresentationBinding, u64)>> {
        enabled.then(|| {
            let bindings: Vec<_> = targets.iter().map(|t| t.binding.clone()).collect();
            self.terminal_output()
                .map(|output| output.tokens_for(&bindings))
                .unwrap_or_default()
        })
    }

    /// Captures every live terminal's recent output before a background save, one temporary
    /// viewer at a time through the shared observer. A busy observer, a failed capture or the
    /// deadline keeps that terminal's previous file.
    #[cfg(windows)]
    pub(crate) fn capture_terminal_output(&self, deadline: Instant) {
        let Some(output) = self.terminal_output() else {
            return;
        };
        let Ok(targets) = self.output_targets() else {
            return;
        };
        let stop = AtomicBool::new(false);
        for (target, presentation) in targets {
            if Instant::now() >= deadline {
                eprintln!(
                    "orchbee-server diagnostic component=checkpoint operation=terminal-output stage=capture cause=deadline"
                );
                return;
            }
            let Ok(mut pending) = self.observer.claim(self) else {
                continue;
            };
            let mut captured = None;
            *pending = self.with_observation_viewer(&target, &stop, |viewer, template| {
                captured = Some(replay_capture(
                    viewer,
                    template,
                    &stop,
                    terminal_output::LINES,
                    output.known(&target.binding),
                ));
            });
            if pending.is_some() {
                return;
            }
            drop(pending);
            self.keep_capture(output, &target.binding, &presentation, captured);
        }
    }

    /// Captures one terminal during the final save, whose viewer cleanup the caller settles.
    #[cfg(windows)]
    pub(super) fn capture_final_output(
        &self,
        target: &ProcessObservationTarget,
        presentation: Option<&TerminalPresentationId>,
    ) -> Option<super::viewer_connection::PendingCleanup> {
        let output = self.terminal_output()?;
        let presentation = presentation?;
        let stop = AtomicBool::new(false);
        let mut captured = None;
        let pending = self.with_observation_viewer(target, &stop, |viewer, template| {
            captured = Some(replay_capture(
                viewer,
                template,
                &stop,
                terminal_output::LINES,
                output.known(&target.binding),
            ));
        });
        self.keep_capture(output, &target.binding, presentation, captured);
        pending
    }

    #[cfg(windows)]
    fn keep_capture(
        &self,
        output: &TerminalOutput,
        binding: &PresentationBinding,
        presentation: &TerminalPresentationId,
        captured: Option<Result<ReplayCapture, orchbee_workbench_protocol::SemanticEventsFailure>>,
    ) {
        if let Some(Ok(ReplayCapture::Lines { version, lines })) = captured
            && let Err(error) = output.record(binding, presentation, version, &lines, now())
        {
            diagnostic("write", &error);
        }
    }

    /// Every live checkpoint terminal and the Presentation it shows.
    #[cfg(windows)]
    pub(crate) fn output_targets(
        &self,
    ) -> std::io::Result<Vec<(ProcessObservationTarget, TerminalPresentationId)>> {
        let coordinator = self
            .coordinator
            .lock()
            .map_err(|_| std::io::Error::other("coordinator failed"))?;
        let (_, targets) = coordinator.checkpoint_observation_targets()?;
        let presentations = coordinator.checkpoint_presentations();
        Ok(targets
            .into_iter()
            .filter_map(|target| {
                let presentation = presentations
                    .iter()
                    .find(|(binding, _)| binding == &target.binding)?
                    .1
                    .clone();
                Some((target, presentation))
            })
            .collect())
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(0)
}

fn diagnostic(stage: &str, error: &std::io::Error) {
    eprintln!(
        "orchbee-server diagnostic component=checkpoint operation=terminal-output stage={stage} cause=io:{:?}",
        error.kind()
    );
}
