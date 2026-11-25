//! The session stores of Agents that keep no run table: a run's session is the one session the store
//! holds for the run's directory that was written since the run started. Several such sessions, or
//! none, give nothing; so does a directory another published run of the same provider shares, which
//! the caller withholds. Only sessions a run adds or rewrites count: a store layout that files new
//! sessions under dated or per-directory folders is read through the folders written since the run
//! started, so a session reopened from an older folder inside the Agent is not seen.
//!
//! - Codex: `~/.codex/sessions/<year>/<month>/<day>/rollout-*.jsonl`, whose first line is the
//!   `session_meta` of a user thread with its `id` and `cwd`.
//! - Pi: `~/.pi/agent/sessions/<folder>/*.jsonl`, and Oh My Pi: `~/.omp/agent/sessions/<folder>/`,
//!   whose `session` line carries `id` and `cwd`.
//! - Droid: `~/.factory/sessions/<folder>/*.jsonl`, whose first line carries `id` and `cwd`.
//! - Qoder CLI: `~/.qoder/projects/<folder>/*.jsonl`, whose first line carries `sessionId` and the
//!   `directories` it opened, the first being its own.
//! - Kimi: `~/.kimi-code/sessions/<folder>/session_<id>/state.json` with `id` and `cwd`.
//! - Gemini: `~/.gemini/tmp/<project>/chats/session-*.jsonl`, the project named for the directory in
//!   `~/.gemini/projects.json`, whose first line is a `main` session's `sessionId`.
//! - Antigravity: the conversation `~/.gemini/antigravity-cli/cache/last_conversations.json` names
//!   for the directory, while its `presence/<id>.lock` was written since the run started.
//! - OpenCode: the one tab `~/.local/state/opencode/latest/tui/tabs.json` keeps for the directory,
//!   when that file was written since the run started.

use super::{
    directories_since, files_since, json_file, leading_lines, same_directory, session, sole, text,
    written_since,
};
use orchbee_server::{AgentResumeProvider, RetainedAgentSession};
use std::path::{Path, PathBuf};

pub(super) fn reads(provider: AgentResumeProvider) -> bool {
    use AgentResumeProvider as P;
    matches!(
        provider,
        P::Codex
            | P::Pi
            | P::Omp
            | P::Droid
            | P::QoderCli
            | P::Kimi
            | P::Gemini
            | P::Antigravity
            | P::OpenCode
    )
}

/// The sole session of `provider`'s store in `directory` written since `started`, Unix
/// milliseconds.
pub(super) fn read(
    home: &Path,
    provider: AgentResumeProvider,
    directory: &str,
    started: u64,
) -> Option<RetainedAgentSession> {
    use AgentResumeProvider as P;
    let found = match provider {
        P::Codex => codex(home, directory, started),
        P::Pi => headed(
            &home.join(".pi").join("agent").join("sessions"),
            provider,
            directory,
            started,
        ),
        P::Omp => headed(
            &home.join(".omp").join("agent").join("sessions"),
            provider,
            directory,
            started,
        ),
        P::Droid => headed(
            &home.join(".factory").join("sessions"),
            provider,
            directory,
            started,
        ),
        P::QoderCli => qoder(home, directory, started),
        P::Kimi => kimi(home, directory, started),
        P::Gemini => gemini(home, directory, started),
        P::Antigravity => return antigravity(home, directory, started),
        P::OpenCode => return opencode(home, directory, started),
        _ => return None,
    };
    sole(found)
}

/// The JSON Lines session files under `root`'s per-directory folders written since `started`.
fn folder_files(root: &Path, started: u64) -> Vec<PathBuf> {
    directories_since(root, started)
        .into_iter()
        .flat_map(|(_, folder)| files_since(&folder, ".jsonl", started))
        .collect()
}

/// Pi, Oh My Pi and Droid: the first of a file's two leading lines that carries both `id` and
/// `cwd` is its header.
fn headed(
    root: &Path,
    provider: AgentResumeProvider,
    directory: &str,
    started: u64,
) -> Vec<RetainedAgentSession> {
    folder_files(root, started)
        .iter()
        .filter_map(|path| {
            let header = leading_lines(path, 2)
                .into_iter()
                .find(|line| text(line, "id").is_some() && text(line, "cwd").is_some())?;
            same_directory(text(&header, "cwd")?, directory)
                .then(|| session(provider, text(&header, "id")?))
                .flatten()
        })
        .collect()
}

fn codex(home: &Path, directory: &str, started: u64) -> Vec<RetainedAgentSession> {
    let root = home.join(".codex").join("sessions");
    let days: Vec<PathBuf> = super::listed(&root)
        .into_iter()
        .filter(|(_, _, metadata)| metadata.is_dir())
        .flat_map(|(_, year, _)| super::listed(&year))
        .filter(|(_, _, metadata)| metadata.is_dir())
        .flat_map(|(_, month, _)| directories_since(&month, started))
        .map(|(_, day)| day)
        .collect();
    days.iter()
        .flat_map(|day| files_since(day, ".jsonl", started))
        .filter_map(|path| {
            let line = leading_lines(&path, 1).into_iter().next()?;
            let payload = line.get("payload")?;
            let user = text(payload, "thread_source").is_none_or(|source| source == "user");
            (text(&line, "type")? == "session_meta"
                && user
                && same_directory(text(payload, "cwd")?, directory))
            .then(|| session(AgentResumeProvider::Codex, text(payload, "id")?))
            .flatten()
        })
        .collect()
}

fn qoder(home: &Path, directory: &str, started: u64) -> Vec<RetainedAgentSession> {
    folder_files(&home.join(".qoder").join("projects"), started)
        .iter()
        .filter_map(|path| {
            let line = leading_lines(path, 1).into_iter().next()?;
            let own = line.get("directories")?.as_array()?.first()?.as_str()?;
            same_directory(own, directory)
                .then(|| session(AgentResumeProvider::QoderCli, text(&line, "sessionId")?))
                .flatten()
        })
        .collect()
}

fn kimi(home: &Path, directory: &str, started: u64) -> Vec<RetainedAgentSession> {
    directories_since(&home.join(".kimi-code").join("sessions"), started)
        .into_iter()
        .flat_map(|(_, folder)| super::listed(&folder))
        .filter(|(name, _, metadata)| metadata.is_dir() && name.starts_with("session_"))
        .filter_map(|(name, folder, _)| {
            let path = folder.join("state.json");
            if !written_since(&path, started) {
                return None;
            }
            let state: serde_json::Value = json_file(&path)?;
            (text(&state, "id")? == name && same_directory(text(&state, "cwd")?, directory))
                .then(|| session(AgentResumeProvider::Kimi, &name))
                .flatten()
        })
        .collect()
}

fn gemini(home: &Path, directory: &str, started: u64) -> Vec<RetainedAgentSession> {
    let root = home.join(".gemini");
    let Some(projects) = json_file::<serde_json::Value>(&root.join("projects.json")) else {
        return Vec::new();
    };
    let Some(project) = projects
        .get("projects")
        .and_then(serde_json::Value::as_object)
        .and_then(|projects| {
            sole_text(
                projects
                    .iter()
                    .filter(|(path, _)| same_directory(path, directory))
                    .filter_map(|(_, name)| name.as_str()),
            )
        })
        .filter(|name| plain_name(name))
    else {
        return Vec::new();
    };
    let chats = root.join("tmp").join(project).join("chats");
    super::listed(&chats)
        .into_iter()
        .filter(|(name, _, metadata)| {
            metadata.is_file()
                && name.starts_with("session-")
                && super::modified(metadata).is_some_and(|written| super::since(written, started))
        })
        .filter_map(|(_, path, _)| {
            let line = leading_lines(&path, 1).into_iter().next()?;
            let main = text(&line, "kind").is_none_or(|kind| kind == "main");
            main.then(|| session(AgentResumeProvider::Gemini, text(&line, "sessionId")?))
                .flatten()
        })
        .collect()
}

fn antigravity(home: &Path, directory: &str, started: u64) -> Option<RetainedAgentSession> {
    let root = home.join(".gemini").join("antigravity-cli");
    let last: serde_json::Value = json_file(&root.join("cache").join("last_conversations.json"))?;
    let id = sole_text(
        last.as_object()?
            .iter()
            .filter(|(path, _)| same_directory(path, directory))
            .filter_map(|(_, id)| id.as_str()),
    )?;
    let reference = session(AgentResumeProvider::Antigravity, id)?;
    written_since(&root.join("presence").join(format!("{id}.lock")), started).then_some(reference)
}

fn opencode(home: &Path, directory: &str, started: u64) -> Option<RetainedAgentSession> {
    let path = home
        .join(".local")
        .join("state")
        .join("opencode")
        .join("latest")
        .join("tui")
        .join("tabs.json");
    if !written_since(&path, started) {
        return None;
    }
    let tabs: serde_json::Value = json_file(&path)?;
    let entries: Vec<_> = tabs
        .get("cwd")?
        .as_object()?
        .iter()
        .filter(|(path, _)| same_directory(path, directory))
        .collect();
    let [(_, entry)] = entries.as_slice() else {
        return None;
    };
    let [tab] = entry.get("tabs")?.as_array()?.as_slice() else {
        return None;
    };
    session(AgentResumeProvider::OpenCode, text(tab, "sessionID")?)
}

/// The one distinct text of `found`, or none.
fn sole_text<'a>(found: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let mut found = found.into_iter();
    let first = found.next()?;
    found.all(|other| other == first).then_some(first)
}

/// A folder name that stays inside its parent.
fn plain_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', ':'])
}
