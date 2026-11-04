//! What one live terminal's own evidence says about closing it without asking. A conditional Tab
//! or Panel Close reads one fresh frame and then one process sample per live terminal; two
//! independent facts decide, and each degrades only toward asking.
//!
//! A command the shell reports as running, or a program holding the terminal's console, asks. A
//! shell that reports its prompt closes even when the sample cannot answer: a refused, unavailable
//! or late sample is an unknown fact, not a running program. A shell that reports nothing, such as
//! `cmd`, is judged by its sample alone and asks when that cannot answer.
use crate::session_coordinator::decoded_argv;
use orchbee_agent_observation_protocol::{ShellRoot, classify_shell_root, is_console_host};
use orchbee_terminal_protocol::{
    MAXIMUM_PROCESS_SAMPLE_ROWS, TerminalProcessConsole, TerminalProcessInfo, TerminalTextFrame,
    TerminalTextRow,
};

/// What a shell's own integration says about the terminal's foreground. Neither fact below is
/// certain to exist: a console host may drop the reader's private markers, as the system ConHost
/// does while the bundled ConPTY delivers them, and a shell may mark nothing at all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::local_terminal_service) enum ShellReport {
    /// Its reader is waiting for input.
    AtPrompt,
    /// It started a command that has not finished, or its cursor left the prompt it marked.
    Running,
    /// Nothing that can decide: no integration, a screen that holds no mark, or a full-screen view.
    Silent,
}

impl ShellReport {
    /// The reader's correlated markers answer first. Otherwise the OSC 133 marks on the screen
    /// rows do: every console host lets them through, and a prompt redrawn after a clear erases its
    /// marks, so an unmarked screen never reads as a running command. Only a complete frame holds
    /// every screen row; a delta frame's rows say nothing about where the cursor's row is.
    pub(in crate::local_terminal_service) fn of(frame: &TerminalTextFrame) -> Self {
        if let Some(observation) = &frame.command_observation {
            if observation.ready {
                return Self::AtPrompt;
            }
            if observation
                .command
                .is_some_and(|command| !command.completed)
            {
                return Self::Running;
            }
        }
        if frame.alternate_screen || frame.route.base_revision.is_some() {
            return Self::Silent;
        }
        // Only the prompt class is the shell's mark. The terminal also stamps the continuation class
        // on rows a prompt's state reached, blank rows and program output among them, and a repaint
        // decides which keep it, so that class says nothing about where the shell is.
        let marked = |row: &TerminalTextRow| row.prompt_class == ROW_PROMPT;
        // Input longer than the row wraps onto rows of the same logical line.
        let mut line = usize::from(frame.cursor.row);
        while line > 0
            && frame
                .rows
                .get(line)
                .is_some_and(|row| !marked(row) && row.flags & ROW_WRAP_CONTINUATION != 0)
        {
            line -= 1;
        }
        if frame.rows.get(line).is_some_and(marked) {
            Self::AtPrompt
        } else if frame.rows.iter().take(line).any(marked) {
            Self::Running
        } else {
            Self::Silent
        }
    }
}

/// The frame row flag of a row continuing the wrapped row above it.
const ROW_WRAP_CONTINUATION: u8 = 2;
/// The `TerminalTextRow::prompt_class` of a row holding a marked prompt.
const ROW_PROMPT: u8 = 1;

/// What one process sample proves about the foreground of a terminal a close would end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::local_terminal_service) enum CloseSample {
    /// A recognized interactive shell, and nothing else holds or may hold its console.
    ShellOnly,
    /// Another program holds the console, or the root is not an interactive shell.
    Program,
    /// The sample cannot tell: it reached its row limit, its root is unreadable, or a row's console
    /// is not observable.
    Unknown,
}

impl CloseSample {
    /// Reads a sample the way the Host's close gate reads its process observation: only a member
    /// that holds the Terminal console is a foreground program. A detached or console-less member
    /// does not hold it, unlike the stricter rule that admits an Agent start.
    pub(in crate::local_terminal_service) fn of(sample: &TerminalProcessInfo) -> Self {
        if sample.processes.len() >= MAXIMUM_PROCESS_SAMPLE_ROWS {
            return Self::Unknown;
        }
        let shell = sample
            .processes
            .iter()
            .find(|row| row.pid == sample.root_pid)
            .map_or(ShellRoot::Unreadable, |root| {
                classify_shell_root(
                    root.image.as_deref(),
                    decoded_argv(root.command_line.as_deref()).as_deref(),
                    &orchbee_local_runtime::windows_interactive_powershell_initialization(),
                )
            });
        match shell {
            ShellRoot::Interactive => {}
            ShellRoot::Other => return Self::Program,
            ShellRoot::Unreadable => return Self::Unknown,
        }
        let mut verdict = Self::ShellOnly;
        for row in &sample.processes {
            if row.pid == sample.root_pid || row.image.as_deref().is_some_and(is_console_host) {
                continue;
            }
            match row.console {
                TerminalProcessConsole::Attached => return Self::Program,
                TerminalProcessConsole::Unknown => verdict = Self::Unknown,
                TerminalProcessConsole::Detached | TerminalProcessConsole::None => {}
            }
        }
        verdict
    }
}

/// What the read of one live terminal produced before the close's budget ended. A fact still
/// missing then counts as the least it can mean: an unread report as silence, an unread sample as
/// unknown. A terminal left unread is therefore closable only if its shell reported its prompt.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::local_terminal_service) struct CloseFacts {
    pub(in crate::local_terminal_service) report: Option<ShellReport>,
    pub(in crate::local_terminal_service) sample: Option<CloseSample>,
}

impl CloseFacts {
    /// Whether closing this terminal ends nothing beyond an idle shell.
    pub(in crate::local_terminal_service) const fn runs_nothing(self) -> bool {
        let report = match self.report {
            Some(report) => report,
            None => ShellReport::Silent,
        };
        let sample = match self.sample {
            Some(sample) => sample,
            None => CloseSample::Unknown,
        };
        match (report, sample) {
            (ShellReport::Running, _)
            | (_, CloseSample::Program)
            | (ShellReport::Silent, CloseSample::Unknown) => false,
            (ShellReport::AtPrompt | ShellReport::Silent, CloseSample::ShellOnly)
            | (ShellReport::AtPrompt, CloseSample::Unknown) => true,
        }
    }
}

#[cfg(test)]
mod tests;
