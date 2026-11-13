//! Hermes owns activation; Orchbee owns exactly two independently recoverable entry files.
use super::{
    files, hook_configuration_windows,
    inspect::{ResourceOptions, ResourceOwnership, classify_resource_root, owned_elsewhere},
    model::State,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(super) const NAME: &str = "orchbee-agent-observation";
const PREFIX: &str = "# orchbee-agent-observation:hermes\nORCHBEE_ROOT = ";
const BODY: &str = "\n\ndef register(ctx):\n    try:\n        import base64\n        import pathlib\n        import runpy\n        root = pathlib.Path(base64.b64decode(ORCHBEE_ROOT).decode('utf-8'))\n        if root.is_absolute():\n            runpy.run_path(str(root / 'bundle' / 'hermes' / 'plugin.py'))['register'](ctx)\n    except Exception:\n        pass\n";
#[derive(Clone, Copy)]
pub(super) enum Member {
    Entry,
    Manifest,
}
impl Member {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Entry => "__init__.py",
            Self::Manifest => "plugin.yaml",
        }
    }
}
pub(super) fn member_bytes(member: Member, root: &Path) -> Option<Vec<u8>> {
    if !files::path_admitted(root) {
        return None;
    }
    let root_text = root.to_str()?;
    match member {
        Member::Entry => {
            Some(format!("{PREFIX}'{}'{BODY}", STANDARD.encode(root_text)).into_bytes())
        }
        Member::Manifest => serde_json::to_vec(&json!({
            "name":NAME,"version":root.file_name()?.to_str()?,
            "description":"Orchbee session identity observation",
            "provides_hooks":["on_session_start","on_session_reset","pre_llm_call"],
            "orchbee_root":root_text
        }))
        .ok(),
    }
}
fn member_root(member: Member, bytes: &[u8]) -> Option<PathBuf> {
    let root = match member {
        Member::Entry => {
            let text = std::str::from_utf8(bytes).ok()?;
            let value = text
                .strip_prefix(PREFIX)?
                .strip_suffix(BODY)?
                .strip_prefix("'")?
                .strip_suffix("'")?;
            PathBuf::from(String::from_utf8(STANDARD.decode(value).ok()?).ok()?)
        }
        Member::Manifest => {
            let value: Value = serde_json::from_slice(bytes).ok()?;
            PathBuf::from(value.get("orchbee_root")?.as_str()?)
        }
    };
    (member_bytes(member, &root)?.as_slice() == bytes).then_some(root)
}
fn local_state(entry: Option<&[u8]>, manifest: Option<&[u8]>, options: &ResourceOptions) -> State {
    let mut roots = Vec::new();
    for (kind, bytes) in [(Member::Entry, entry), (Member::Manifest, manifest)] {
        if let Some(bytes) = bytes {
            let Some(root) = member_root(kind, bytes) else {
                return State::Conflict;
            };
            roots.push(classify_resource_root(&root.to_string_lossy(), options));
        }
    }
    if roots
        .iter()
        .any(|root| matches!(root, ResourceOwnership::Foreign))
    {
        return State::Conflict;
    }
    if roots.is_empty() {
        return State::NotInstalled;
    }
    if roots.len() != 2
        || roots
            .iter()
            .any(|root| matches!(root, ResourceOwnership::Outdated))
    {
        return State::UpdateRequired;
    }
    State::Enabled
}
/// Whether every entry file present is exactly one Orchbee writes for another environment's
/// Capsule. That environment owns the plugin and its activation, which this environment leaves.
fn elsewhere(entry: Option<&[u8]>, manifest: Option<&[u8]>, options: &ResourceOptions) -> bool {
    let mut present = [(Member::Entry, entry), (Member::Manifest, manifest)]
        .into_iter()
        .filter_map(|(member, bytes)| Some((member, bytes?)))
        .peekable();
    present.peek().is_some()
        && present.all(|(member, bytes)| {
            member_root(member, bytes).is_some_and(|root| owned_elsewhere(&root, options))
        })
}
pub(super) struct Local {
    pub(super) directory: PathBuf,
    pub(super) entry: Option<Vec<u8>>,
    pub(super) manifest: Option<Vec<u8>>,
    /// `conflict` when the entry files are another environment's.
    pub(super) state: State,
    /// The entry files are another environment's.
    pub(super) elsewhere: bool,
}
pub(super) fn local(home: &Path, options: &ResourceOptions) -> Option<Local> {
    let directory = crate::configuration_windows::hermes_home(home)?
        .join("plugins")
        .join(NAME);
    let entry = hook_configuration_windows::read(&directory.join(Member::Entry.name())).ok()?;
    let manifest =
        hook_configuration_windows::read(&directory.join(Member::Manifest.name())).ok()?;
    let alternative = ["plugin.yml", "plugin.json"]
        .iter()
        .any(|name| std::fs::symlink_metadata(directory.join(name)).is_ok());
    let state = if alternative {
        State::Conflict
    } else {
        local_state(entry.as_deref(), manifest.as_deref(), options)
    };
    let elsewhere = !alternative && elsewhere(entry.as_deref(), manifest.as_deref(), options);
    Some(Local {
        directory,
        entry,
        manifest,
        state,
        elsewhere,
    })
}
struct Listed {
    version: String,
    state: State,
}
fn inventory(bytes: &[u8]) -> Result<Option<Listed>, State> {
    let Ok(Value::Array(rows)) = serde_json::from_slice::<Value>(bytes) else {
        return Err(State::Unknown);
    };
    let mut found = None;
    for row in rows {
        if !row.is_object() || row.get("name").and_then(Value::as_str).is_none() {
            return Err(State::Unknown);
        }
        if row["name"] != NAME {
            continue;
        }
        if found.is_some() || row.get("source").and_then(Value::as_str) != Some("user") {
            return Err(State::Conflict);
        }
        let Some(version) = row.get("version").and_then(Value::as_str) else {
            return Err(State::Unknown);
        };
        if !matches!(row.get("removed"), None | Some(Value::Null))
            && row.get("removed").and_then(Value::as_str) != Some("")
        {
            return Err(State::Conflict);
        }
        let state = match row.get("status").and_then(Value::as_str) {
            Some("enabled") => State::Enabled,
            Some("disabled" | "not enabled") => State::Disabled,
            _ => return Err(State::Unknown),
        };
        found = Some(Listed {
            version: version.into(),
            state,
        });
    }
    Ok(found)
}
pub(super) fn activation_disabled(bytes: &[u8]) -> bool {
    matches!(
        inventory(bytes),
        Ok(Some(Listed {
            state: State::Disabled,
            ..
        }))
    )
}
pub(super) fn list_state(bytes: &[u8], local: State, expected_version: &str) -> State {
    if matches!(local, State::Conflict | State::Unknown) {
        return local;
    }
    let found = match inventory(bytes) {
        Ok(value) => value,
        Err(state) => return state,
    };
    if local == State::Enabled
        && found
            .as_ref()
            .is_some_and(|row| row.version != expected_version)
    {
        return State::Conflict;
    }
    match (local, found) {
        (State::NotInstalled, None) => State::NotInstalled,
        (State::NotInstalled, Some(_)) => State::Conflict,
        (State::UpdateRequired, _) => State::UpdateRequired,
        (_, Some(row)) => row.state,
        _ => State::Unknown,
    }
}

pub(super) fn read(
    executable: &Path,
    context: &super::providers::ProviderContext,
) -> super::providers::ReadOutcome {
    use super::{exec, providers::ReadOutcome};
    match exec::run_hermes_command(
        executable,
        &["plugins".into(), "list".into(), "--json".into()],
        context.home,
        context.deadline,
        context.cancel,
    ) {
        exec::CommandResult::Completed { stdout } => ReadOutcome::Stdout(stdout),
        exec::CommandResult::TimedOut => ReadOutcome::TimedOut,
        _ => ReadOutcome::Failed,
    }
}

#[cfg(test)]
mod tests;
