//! Interpretation of a bounded Host Job sample. This never collects processes,
//! expands commands, or creates canonical Agent state.

mod identity;
#[cfg(test)]
mod tests;

use orchbee_terminal_protocol::{TerminalProcess, TerminalProcessInfo};

#[cfg(any(windows, test))]
pub(super) use crate::agent_provider::provider;

pub(super) struct Process<'a> {
    pub(super) facts: &'a TerminalProcess,
    pub(super) argv: Option<Vec<String>>,
    pub(super) provider: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Selection {
    AgentChain,
    Root,
    Ambiguous,
    Unavailable,
}

impl Selection {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::AgentChain => "agent-chain",
            Self::Root => "root",
            Self::Ambiguous => "ambiguous",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug)]
pub(super) struct Foreground {
    pub(super) selection: Selection,
    pub(super) pid: Option<u32>,
    pub(super) provider: Option<&'static str>,
}

pub(super) struct Interpretation<'a> {
    pub(super) processes: Vec<Process<'a>>,
    pub(super) foreground: Foreground,
}

pub(super) fn interpret(sample: &TerminalProcessInfo) -> Interpretation<'_> {
    let processes: Vec<_> = sample
        .processes
        .iter()
        .map(|facts| {
            let argv = facts.command_line.as_deref().and_then(arguments);
            let provider = identity::identify(facts.image.as_deref(), argv.as_deref());
            Process {
                facts,
                argv,
                provider,
            }
        })
        .collect();
    let candidates: Vec<_> = processes.iter().filter(|p| p.provider.is_some()).collect();
    let selected = candidates.iter().find(|candidate| {
        candidates.iter().all(|other| {
            ancestry(sample, other.facts.pid).is_some_and(|chain| {
                candidate.facts.pid == other.facts.pid || chain.contains(&candidate.facts.pid)
            })
        })
    });
    let foreground = if let Some(selected) = selected {
        Foreground {
            selection: Selection::AgentChain,
            pid: Some(selected.facts.pid),
            provider: selected.provider,
        }
    } else if !candidates.is_empty() {
        Foreground {
            selection: Selection::Ambiguous,
            pid: None,
            provider: None,
        }
    } else if processes.iter().any(|p| p.facts.pid == sample.root_pid) {
        Foreground {
            selection: Selection::Root,
            pid: Some(sample.root_pid),
            provider: None,
        }
    } else {
        Foreground {
            selection: Selection::Unavailable,
            pid: None,
            provider: None,
        }
    };
    Interpretation {
        processes,
        foreground,
    }
}

fn ancestry(sample: &TerminalProcessInfo, child: u32) -> Option<Vec<u32>> {
    let mut current = child;
    let mut visited = Vec::with_capacity(sample.processes.len());
    for _ in 0..sample.processes.len() {
        if visited.contains(&current) {
            return None;
        }
        visited.push(current);
        let child = sample.processes.iter().find(|p| p.pid == current)?;
        let Some(next) = child
            .parent_pid
            .and_then(|pid| sample.processes.iter().find(|p| p.pid == pid))
        else {
            return Some(visited);
        };
        if next.creation_time > child.creation_time {
            return None;
        }
        current = next.pid;
    }
    None
}

fn arguments(command: &str) -> Option<Vec<String>> {
    // CommandLineToArgvW("") would return this CLI's executable path.
    if command.trim().is_empty() || command.contains('\0') || command.encode_utf16().count() > 4096
    {
        return None;
    }
    #[cfg(windows)]
    {
        winsafe::CommandLineToArgv(command).ok()
    }
    #[cfg(not(windows))]
    {
        None
    }
}
