//! Interpretation of a bounded Host Job sample. This never collects processes,
//! expands commands, or creates canonical Agent state.

use orchbee_agent_observation_protocol::{
    AgentProcessCandidate, AgentProcessSelection, AgentReportProvider, identify_agent_process,
    select_agent_process,
};
#[cfg(test)]
mod tests;

use orchbee_terminal_protocol::{TerminalProcess, TerminalProcessInfo};

#[cfg(any(windows, test))]
pub(super) use crate::agent_provider::provider;

pub(crate) struct Process<'a> {
    pub(crate) facts: &'a TerminalProcess,
    pub(crate) argv: Option<Vec<String>>,
    pub(super) provider: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Selection {
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
pub(crate) struct Foreground {
    pub(crate) selection: Selection,
    pub(super) pid: Option<u32>,
    pub(crate) provider: Option<&'static str>,
}

pub(crate) struct Interpretation<'a> {
    pub(crate) processes: Vec<Process<'a>>,
    pub(crate) foreground: Foreground,
}

pub(crate) fn interpret(sample: &TerminalProcessInfo) -> Interpretation<'_> {
    let processes: Vec<_> = sample
        .processes
        .iter()
        .map(|facts| {
            let argv = facts.command_line.as_deref().and_then(arguments);
            let provider = identify_agent_process(facts.image.as_deref(), argv.as_deref());
            Process {
                facts,
                argv,
                provider,
            }
        })
        .collect();
    let candidates: Vec<_> = processes
        .iter()
        .map(|p| AgentProcessCandidate {
            pid: p.facts.pid,
            parent_pid: p.facts.parent_pid,
            creation_time: p.facts.creation_time,
            provider: p.provider.and_then(AgentReportProvider::lookup),
        })
        .collect();
    let foreground = match select_agent_process(&candidates) {
        AgentProcessSelection::Selected { pid, provider, .. } => Foreground {
            selection: Selection::AgentChain,
            pid: Some(pid),
            provider: Some(provider.native_provider_id()),
        },
        AgentProcessSelection::Ambiguous => Foreground {
            selection: Selection::Ambiguous,
            pid: None,
            provider: None,
        },
        AgentProcessSelection::None if processes.iter().any(|p| p.facts.pid == sample.root_pid) => {
            Foreground {
                selection: Selection::Root,
                pid: Some(sample.root_pid),
                provider: None,
            }
        }
        AgentProcessSelection::None => Foreground {
            selection: Selection::Unavailable,
            pid: None,
            provider: None,
        },
    };
    Interpretation {
        processes,
        foreground,
    }
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
