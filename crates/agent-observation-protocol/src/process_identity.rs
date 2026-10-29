//! Pure native process identity rules shared by diagnostic and trusted Host adapters:
//! the provider registry, the invocation mode classifier, per-process recognition,
//! the candidate selector, and the identity result vocabulary. Nothing here performs I/O.
mod executable;
mod mode;
mod registry;
mod result;
use crate::AgentReportProvider;
pub use executable::{AgentProcessRecognition, AgentUnresolvedProcess, classify_agent_process};
pub use mode::{AgentInvocationMode, AgentModeGrammar};
pub use registry::{
    AGENT_PROVIDER_REGISTRY, AGENT_PROVIDER_REGISTRY_VERSION, AgentAlias, AgentAliasClass,
    AgentBundledRuntime, AgentInstallRoot, AgentInstallShape, AgentProviderEntry,
};
pub use result::{
    AgentIdentityGrade, AgentIdentityUnknownReason, AgentIdentityUnsupportedReason,
    AgentProcessIdentity,
};

/// Extracts the native executable basename and one recognized suffix.
pub fn native_agent_basename(path: &str) -> String {
    let value = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    for suffix in [
        ".exe", ".com", ".cmd", ".bat", ".ps1", ".js", ".mjs", ".cjs", ".py",
    ] {
        if let Some(stem) = value.strip_suffix(suffix) {
            return stem.to_owned();
        }
    }
    value
}

/// One already-decoded process identity; only the OS adapter supplies these facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentProcessCandidate {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub creation_time: u64,
    pub provider: Option<AgentReportProvider>,
}
/// A unique outer Agent chain, no recognized Agent, or conflicting/invalid ancestry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentProcessSelection {
    None,
    Ambiguous,
    Selected {
        pid: u32,
        creation_time: u64,
        provider: AgentReportProvider,
    },
}
/// Selects the outer recognized Agent when every candidate belongs to its ancestry chain.
/// Missing ancestors end the chain; cycles and parent PID reuse cannot establish it.
pub fn select_agent_process(processes: &[AgentProcessCandidate]) -> AgentProcessSelection {
    if processes.iter().enumerate().any(|(i, p)| {
        p.pid == 0 || p.creation_time == 0 || processes[..i].iter().any(|q| q.pid == p.pid)
    }) {
        return AgentProcessSelection::Ambiguous;
    }
    let candidates: Vec<_> = processes.iter().filter(|p| p.provider.is_some()).collect();
    if candidates.is_empty() {
        return AgentProcessSelection::None;
    }
    let selected = candidates.iter().find(|candidate| {
        candidates.iter().all(|other| {
            ancestry(processes, other.pid)
                .is_some_and(|chain| candidate.pid == other.pid || chain.contains(&candidate.pid))
        })
    });
    selected.map_or(AgentProcessSelection::Ambiguous, |p| {
        AgentProcessSelection::Selected {
            pid: p.pid,
            creation_time: p.creation_time,
            provider: p.provider.expect("recognized candidate"),
        }
    })
}
fn ancestry(processes: &[AgentProcessCandidate], child: u32) -> Option<Vec<u32>> {
    let mut current = child;
    let mut visited = Vec::with_capacity(processes.len());
    for _ in 0..processes.len() {
        if visited.contains(&current) {
            return None;
        }
        visited.push(current);
        let child = processes.iter().find(|p| p.pid == current)?;
        let Some(next) = child
            .parent_pid
            .and_then(|pid| processes.iter().find(|p| p.pid == pid))
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
#[cfg(test)]
mod tests;
