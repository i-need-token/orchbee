//! The run tables Agents keep for their running processes, each read for one probe's processes:
//! the run and its descendants, since some Agents record a child they start. A record counts only
//! for the exact process it names: its process id is one of the probe's, and the time it was written
//! or the start it records is that process's creation time or later, so a record a dead process left
//! behind never names a later process that reuses its id. The directories are the ones the Server's
//! home gives: process samples carry no environment, so an Agent pointed elsewhere for itself alone
//! is not seen.
//!
//! - Claude Code: `~/.claude/sessions/<pid>.json`, matched on its exact `procStart`.
//! - Grok: `~/.grok/active_sessions.json`, the newest entry opened by the process.
//! - Qwen Code: `~/.qwen/sessions/<pid>.json`, the entry of its interactive interface.
//! - GitHub Copilot: `~/.copilot/session-state/<session>/inuse.<pid>.lock`, the one session the
//!   process holds open, once its `events.jsonl` holds a conversation to resume.
//! - Cline: `~/.cline/data/sessions/<session>/<session>.json`, the newest session the process
//!   started.

use super::super::{RunAnchor, RunProbe};
use super::{
    directories_since, json_file, process_started, rfc3339_milliseconds, session, since, sole,
    text, unix_milliseconds,
};
use orchbee_server::{AgentResumeProvider, RetainedAgentSession};
use serde::Deserialize;
use std::path::Path;

pub(super) fn reads(provider: AgentResumeProvider) -> bool {
    use AgentResumeProvider as P;
    matches!(
        provider,
        P::ClaudeCode | P::Grok | P::QwenCode | P::GitHubCopilot | P::Cline
    )
}

pub(super) fn read(home: &Path, probe: &RunProbe) -> Option<RetainedAgentSession> {
    use AgentResumeProvider as P;
    let processes = probe.processes.as_slice();
    match probe.provider {
        P::ClaudeCode => super::claude::read(processes),
        P::Grok => grok(home, processes),
        P::QwenCode => qwen(home, processes),
        P::GitHubCopilot => copilot(home, processes),
        P::Cline => cline(home, processes),
        _ => None,
    }
}

#[derive(Deserialize)]
struct GrokEntry {
    session_id: String,
    pid: u64,
    opened_at: String,
}

/// Grok keeps one entry per open session; a process that opened several holds the newest.
fn grok(home: &Path, processes: &[RunAnchor]) -> Option<RetainedAgentSession> {
    let entries: Vec<GrokEntry> = json_file(&home.join(".grok").join("active_sessions.json"))?;
    entries
        .into_iter()
        .filter_map(|entry| {
            let opened = rfc3339_milliseconds(&entry.opened_at)?;
            since(opened, process_started(processes, entry.pid)?).then_some((opened, entry))
        })
        .max_by_key(|(opened, _)| *opened)
        .and_then(|(_, entry)| session(AgentResumeProvider::Grok, &entry.session_id))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QwenEntry {
    pid: u64,
    session_id: String,
    started_at: u64,
    kind: String,
}

fn qwen(home: &Path, processes: &[RunAnchor]) -> Option<RetainedAgentSession> {
    let directory = home.join(".qwen").join("sessions");
    sole(processes.iter().filter_map(|process| {
        let entry: QwenEntry = json_file(&directory.join(format!("{}.json", process.pid)))?;
        let started = unix_milliseconds(process.created)?;
        (entry.pid == u64::from(process.pid)
            && entry.kind == "tui"
            && since(entry.started_at, started))
        .then(|| session(AgentResumeProvider::QwenCode, &entry.session_id))
        .flatten()
    }))
}

/// Copilot locks each session it holds open with `inuse.<pid>.lock`, holding its process id, in
/// that session's folder; a new lock adds an entry there, so only folders written since the run
/// started are looked in. Copilot writes the session's `events.jsonl` with its first message and
/// cannot resume a session without one.
fn copilot(home: &Path, processes: &[RunAnchor]) -> Option<RetainedAgentSession> {
    let state = home.join(".copilot").join("session-state");
    let started = unix_milliseconds(processes.first()?.created)?;
    sole(
        directories_since(&state, started)
            .into_iter()
            .filter_map(|(name, folder)| {
                processes.iter().find_map(|process| {
                    let lock = folder.join(format!("inuse.{}.lock", process.pid));
                    let metadata = std::fs::metadata(&lock).ok()?;
                    let created = super::created(&metadata)?;
                    let holder = std::fs::read_to_string(&lock).ok()?;
                    (metadata.len() <= 32
                        && holder.trim_end() == process.pid.to_string()
                        && since(created, unix_milliseconds(process.created)?)
                        && folder.join("events.jsonl").is_file())
                    .then(|| session(AgentResumeProvider::GitHubCopilot, &name))
                    .flatten()
                })
            }),
    )
}

/// Cline names each session by its start in Unix milliseconds, so only sessions named for a time
/// since the run started are read; a process that started several holds the newest.
fn cline(home: &Path, processes: &[RunAnchor]) -> Option<RetainedAgentSession> {
    let sessions = home.join(".cline").join("data").join("sessions");
    let started = unix_milliseconds(processes.first()?.created)?;
    super::listed(&sessions)
        .into_iter()
        .filter(|(name, _, metadata)| {
            metadata.is_dir()
                && name
                    .get(..13)
                    .and_then(|digits| digits.parse::<u64>().ok())
                    .is_some_and(|named| since(named, started))
        })
        .filter_map(|(name, folder, _)| {
            let record: serde_json::Value = json_file(&folder.join(format!("{name}.json")))?;
            let pid = record.get("pid")?.as_u64()?;
            let begun = rfc3339_milliseconds(text(&record, "started_at")?)?;
            let own = text(&record, "session_id")? == name
                && since(begun, process_started(processes, pid)?);
            own.then(|| Some((begun, session(AgentResumeProvider::Cline, &name)?)))
                .flatten()
        })
        .max_by_key(|(begun, _)| *begun)
        .map(|(_, session)| session)
}
