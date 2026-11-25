//! The exact processes of one published run as a committed sample's rows give them: the run, the
//! processes descending from it, and its directory. It reads no file and decides no capture.

#[cfg(any(windows, target_os = "macos"))]
use orchbee_terminal_protocol::{TerminalProcess, TerminalProcessInfo};

/// At most this many processes of one run are kept for a process source to look up.
#[cfg(any(windows, target_os = "macos"))]
const MAXIMUM_RUN_PROCESSES: usize = 32;

/// One process: its pid and its creation time in sample row units, which defeats pid reuse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RunAnchor {
    pub(crate) pid: u32,
    pub(crate) created: u64,
}

/// One published run's processes and directory as a committed sample's rows gave them.
#[cfg(any(windows, target_os = "macos"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RunProcesses {
    pub(crate) run: RunAnchor,
    pub(crate) processes: Vec<RunAnchor>,
    pub(crate) directory: Option<String>,
}

#[cfg(any(windows, target_os = "macos"))]
impl RunProcesses {
    /// The run's row and the rows descending from it through parents created no later than their
    /// children, so a reused parent id never adopts a process; none without the run's row.
    pub(crate) fn of(run: RunAnchor, sample: &TerminalProcessInfo) -> Option<Self> {
        let rows = &sample.processes;
        let row = rows
            .iter()
            .find(|row| row.pid == run.pid && row.creation_time == run.created)?;
        let processes = std::iter::once(run)
            .chain(
                rows.iter()
                    .filter(|row| row.pid != run.pid && descends(rows, run, row))
                    .map(|row| RunAnchor {
                        pid: row.pid,
                        created: row.creation_time,
                    }),
            )
            .take(MAXIMUM_RUN_PROCESSES)
            .collect();
        Some(Self {
            run,
            processes,
            directory: row.current_directory.clone(),
        })
    }
}

/// Whether `row` descends from `run` through parents created no later than their children.
#[cfg(any(windows, target_os = "macos"))]
fn descends(rows: &[TerminalProcess], run: RunAnchor, row: &TerminalProcess) -> bool {
    let mut current = row;
    for _ in 0..rows.len() {
        let Some(parent) = current.parent_pid else {
            return false;
        };
        if parent == run.pid {
            return run.created <= current.creation_time;
        }
        match rows.iter().find(|row| row.pid == parent) {
            Some(row) if row.creation_time <= current.creation_time => current = row,
            _ => return false,
        }
    }
    false
}
