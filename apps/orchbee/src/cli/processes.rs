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
    pub(crate) pid: Option<u32>,
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

/// The program's file name without directories, or `-` when unknown.
pub(crate) fn program_name(process: &TerminalProcess) -> String {
    process
        .image
        .as_deref()
        .map_or("-", |image| {
            image.rsplit(['\\', '/']).next().unwrap_or(image)
        })
        .to_owned()
}

/// The command as a person would type it: the program name and its
/// arguments, without the shell initialization Orchbee appends to an
/// interactive PowerShell. Long commands end in an ellipsis.
pub(crate) fn readable_command(process: &Process<'_>) -> Option<String> {
    const LIMIT: usize = 200;
    let command = match &process.argv {
        Some(argv) => {
            let mut argv: &[String] = argv;
            if let [rest @ .., switch, script] = argv
                && switch.eq_ignore_ascii_case("-Command")
                && (orchbee_local_site_runtime::windows_legacy_powershell_initialization_matches(
                    script,
                ) || *script
                    == orchbee_local_site_runtime::windows_interactive_powershell_initialization())
            {
                argv = rest;
            }
            let (program, arguments) = argv.split_first()?;
            std::iter::once(
                program
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or(program)
                    .to_owned(),
            )
            .chain(arguments.iter().map(|argument| {
                if argument.is_empty() || argument.contains(char::is_whitespace) {
                    format!("\"{argument}\"")
                } else {
                    argument.clone()
                }
            }))
            .collect::<Vec<_>>()
            .join(" ")
        }
        None => process.facts.command_line.clone()?,
    };
    Some(if command.chars().count() > LIMIT {
        command.chars().take(LIMIT - 1).chain(['…']).collect()
    } else {
        command
    })
}

/// A Windows FILETIME (100 ns ticks since 1601) as a UTC timestamp; values
/// before 1970 keep their raw decimal form.
pub(crate) fn filetime_utc(filetime: u64) -> String {
    const UNIX_EPOCH_SECONDS: u64 = 11_644_473_600;
    let Some(seconds) = (filetime / 10_000_000).checked_sub(UNIX_EPOCH_SECONDS) else {
        return filetime.to_string();
    };
    let (days, time) = (seconds / 86_400, seconds % 86_400);
    // Civil date from days since 1970-01-01 (proleptic Gregorian).
    let days = days + 719_468;
    let era = days / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        time / 3_600,
        time % 3_600 / 60,
        time % 60
    )
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
