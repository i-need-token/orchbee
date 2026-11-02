//! Interpretation of a bounded Host Job sample. This never collects processes,
//! expands commands, or creates canonical Agent state. The foreground Agent chain is the
//! Server's: [`foreground`] projects its identity result and run anchor, never the sample.
//! Input needs more than that display: [`input_provider`] also requires the sample to hold it.

use orchbee_agent_observation_protocol::{
    AgentIdentityUnknownReason, AgentProcessAnchor, AgentProcessIdentity, AgentProcessRow,
    AgentReportProvider, AgentSampleVerdict, classify_agent_process, evaluate_agent_sample,
};
#[cfg(test)]
mod tests;

use orchbee_terminal_protocol::{
    MAXIMUM_PROCESS_SAMPLE_ROWS, TerminalProcess, TerminalProcessConsole, TerminalProcessInfo,
};

pub use crate::agent_provider::provider;

pub struct Process<'a> {
    pub facts: &'a TerminalProcess,
    pub argv: Option<Vec<String>>,
    pub provider: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    AgentChain,
    Root,
    Ambiguous,
    Unavailable,
}

impl Selection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AgentChain => "agent-chain",
            Self::Root => "root",
            Self::Ambiguous => "ambiguous",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Foreground {
    pub selection: Selection,
    pub pid: Option<u32>,
    pub provider: Option<&'static str>,
}

/// The Server's answer for one Terminal generation: its identity result and the anchor of the
/// run Panel reports attribute to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServerIdentity {
    pub identity: AgentProcessIdentity,
    pub anchor: Option<AgentProcessAnchor>,
}

/// Why the Terminal's Agent must not receive input now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputRefusal {
    /// The Server publishes no run, or this sample no longer holds the run's anchor.
    NotRunning,
    /// Several candidates, or the Server and this sample name different chains.
    Ambiguous,
    /// The Server's result or this sample cannot show an interactive Agent at the anchor.
    Unidentified,
}

/// Per-process facts for diagnostics: decoded arguments and the provider each row names alone.
pub fn interpret(sample: &TerminalProcessInfo) -> Vec<Process<'_>> {
    sample
        .processes
        .iter()
        .map(|facts| {
            let argv = facts.command_line.as_deref().and_then(arguments);
            let provider = row(facts, argv.as_deref())
                .anchor_provider()
                .map(AgentReportProvider::native_provider_id);
            Process {
                facts,
                argv,
                provider,
            }
        })
        .collect()
}

/// The identity row the Server's sampler decodes from the same Host facts.
fn row(facts: &TerminalProcess, argv: Option<&[String]>) -> AgentProcessRow {
    AgentProcessRow {
        pid: facts.pid,
        parent_pid: facts.parent_pid,
        creation_time: facts.creation_time,
        opaque: facts.image.is_none(),
        detached: matches!(
            facts.console,
            TerminalProcessConsole::Detached | TerminalProcessConsole::None
        ),
        recognition: classify_agent_process(facts.image.as_deref(), argv),
    }
}

/// The provider that may receive input. A published run only shows ([`foreground`]): the Server
/// keeps it until a later sample of its own ends it, and keeps it under unknown or ambiguous
/// results. Input also requires the Server's current result to name the run's provider as an
/// interactive Agent, and this sample, read before that result, to hold the run's anchor and name
/// it on its own as the only Agent by the verdict the Server applies.
///
/// # Errors
/// Refuses without a run, without its anchor in this sample, or when either answer is unknown
/// or names another chain.
pub fn input_provider(
    sample: &TerminalProcessInfo,
    server: ServerIdentity,
) -> Result<&'static str, InputRefusal> {
    let ambiguous = AgentProcessIdentity::Unknown(AgentIdentityUnknownReason::AmbiguousCandidates);
    let Some(anchor) = server.anchor else {
        return Err(if server.identity == ambiguous {
            InputRefusal::Ambiguous
        } else {
            InputRefusal::NotRunning
        });
    };
    let rows: Vec<_> = sample
        .processes
        .iter()
        .map(|facts| {
            row(
                facts,
                facts.command_line.as_deref().and_then(arguments).as_deref(),
            )
        })
        .collect();
    if !rows
        .iter()
        .any(|row| row.pid == anchor.pid && row.anchor_provider() == Some(anchor.provider))
    {
        return Err(InputRefusal::NotRunning);
    }
    match server.identity {
        AgentProcessIdentity::Agent { provider, .. } if provider == anchor.provider => {}
        AgentProcessIdentity::Agent { .. } => return Err(InputRefusal::Ambiguous),
        identity if identity == ambiguous => return Err(InputRefusal::Ambiguous),
        _ => return Err(InputRefusal::Unidentified),
    }
    let at_limit = sample.processes.len() >= MAXIMUM_PROCESS_SAMPLE_ROWS;
    match evaluate_agent_sample(&rows, at_limit) {
        AgentSampleVerdict::Anchor(found) if found.pid == anchor.pid => match found.result() {
            AgentProcessIdentity::Agent { .. } => Ok(anchor.provider.native_provider_id()),
            _ => Err(InputRefusal::Unidentified),
        },
        AgentSampleVerdict::Unresolved { reason, .. }
            if reason != AgentIdentityUnknownReason::AmbiguousCandidates =>
        {
            Err(InputRefusal::Unidentified)
        }
        _ => Err(InputRefusal::Ambiguous),
    }
}

/// The foreground the Server attributes to the sampled Terminal: its published run is the Agent
/// chain; without one, ambiguity stays explicit and otherwise the sample's retained root (or
/// nothing) is in front. The anchor can name a process the earlier sample does not hold.
pub fn foreground(sample: &TerminalProcessInfo, server: ServerIdentity) -> Foreground {
    let (selection, pid, provider) = match (server.anchor, server.identity) {
        (Some(anchor), _) => (
            Selection::AgentChain,
            Some(anchor.pid),
            Some(anchor.provider.native_provider_id()),
        ),
        (None, AgentProcessIdentity::Unknown(AgentIdentityUnknownReason::AmbiguousCandidates)) => {
            (Selection::Ambiguous, None, None)
        }
        (None, _) if sample.processes.iter().any(|p| p.pid == sample.root_pid) => {
            (Selection::Root, Some(sample.root_pid), None)
        }
        (None, _) => (Selection::Unavailable, None, None),
    };
    Foreground {
        selection,
        pid,
        provider,
    }
}
/// The program's file name without directories, or `-` when unknown.
pub fn program_name(process: &TerminalProcess) -> String {
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
pub fn readable_command(process: &Process<'_>) -> Option<String> {
    const LIMIT: usize = 200;
    let command = match &process.argv {
        Some(argv) => {
            let mut argv: &[String] = argv;
            if let [rest @ .., switch, script] = argv
                && switch.eq_ignore_ascii_case("-Command")
                && *script == orchbee_local_runtime::windows_interactive_powershell_initialization()
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
pub fn filetime_utc(filetime: u64) -> String {
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
