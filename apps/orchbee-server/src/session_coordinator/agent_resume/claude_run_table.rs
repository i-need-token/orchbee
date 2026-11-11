//! Reads Claude Code's own run table: the `sessions/<pid>.json` entry each running Claude Code
//! process keeps in its configuration directory, naming the conversation that process holds. An
//! entry counts only for the exact process it is read for: its `pid` is that process id and its
//! `procStart` is the creation time the Host sampled, written as the same decimal 100 ns count
//! since 1601, so an entry a dead process left behind never names a later process that reuses its
//! id. The directory is `.claude` in the Server's home: process samples carry no environment, so a
//! `CLAUDE_CONFIG_DIR` set for Claude Code alone is not seen and its entries are not found. Every
//! read is bounded and every parse strict; a missing, unreadable, oversized, malformed, foreign or
//! mismatched entry is no entry, never an error. It reads nothing else and decides no capture.

use super::{RunAnchor, RunTableEntry};
use orchbee_server::{AgentResumeProvider, RetainedAgentSession};
use serde::Deserialize;
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

/// Claude Code 2.1.322 writes entries of under 1 KiB; anything larger is not one.
const MAXIMUM_ENTRY_BYTES: u64 = 64 * 1024;

/// The run table entry of each anchored process that has one naming a valid Claude Code session.
/// Call it with no lock held: it reads one small file per anchor.
pub(crate) fn read(anchors: &[RunAnchor]) -> Vec<RunTableEntry> {
    match directory() {
        Some(directory) if !anchors.is_empty() => read_in(&directory, anchors),
        _ => Vec::new(),
    }
}

/// Claude Code's default run table directory, `~/.claude/sessions`.
fn directory() -> Option<PathBuf> {
    std::env::home_dir().map(|home| home.join(".claude").join("sessions"))
}

fn read_in(directory: &Path, anchors: &[RunAnchor]) -> Vec<RunTableEntry> {
    anchors
        .iter()
        .filter_map(|&anchor| {
            Some(RunTableEntry {
                anchor,
                session: entry(directory, anchor)?,
            })
        })
        .collect()
}

fn entry(directory: &Path, anchor: RunAnchor) -> Option<RetainedAgentSession> {
    let file = File::open(directory.join(format!("{}.json", anchor.pid))).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_ENTRY_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAXIMUM_ENTRY_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if u64::try_from(bytes.len()).ok()? > MAXIMUM_ENTRY_BYTES {
        return None;
    }
    parse(&bytes, anchor)
}

/// The fields an entry must carry; Claude Code writes others beside them, which are ignored.
/// A repeated field is rejected.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    pid: u64,
    session_id: String,
    proc_start: String,
}

/// The session `bytes` names when they are one JSON object, as Claude Code writes it, whose `pid`
/// and `procStart` are `anchor`'s process id and creation time digit for digit and whose
/// `sessionId` is a valid Claude Code id.
fn parse(bytes: &[u8], anchor: RunAnchor) -> Option<RetainedAgentSession> {
    // Serde would also read these fields from a JSON array.
    if bytes.first() != Some(&b'{') {
        return None;
    }
    let entry: Entry = serde_json::from_slice(bytes).ok()?;
    if entry.pid != u64::from(anchor.pid) || entry.proc_start != anchor.created.to_string() {
        return None;
    }
    RetainedAgentSession::new(AgentResumeProvider::ClaudeCode, &entry.session_id)
}
