//! Immutable invocation selection; environment is consulted only during runtime discovery.
use orchbee_local_runtime::{SessionName, SessionScope};
use std::{ffi::OsString, io, path::PathBuf, sync::OnceLock};

static SELECTED: OnceLock<Option<SessionName>> = OnceLock::new();

pub fn select_session(name: Option<SessionName>) {
    SELECTED
        .set(name)
        .expect("invocation selection is initialized once after parsing");
}

/// Return only the explicit, parser-validated `--session` selection. An
/// inherited environment route remains inherited by the startup command and
/// does not need to be copied into a hint.
pub fn selected_session_name() -> Option<&'static str> {
    SELECTED
        .get()
        .and_then(Option::as_ref)
        .map(SessionName::as_str)
}

pub(super) struct Location {
    pub(super) base: PathBuf,
    /// The base is this environment's default rather than `ORCHBEE_RUNTIME_BASE`.
    pub(super) default_base: bool,
    pub(super) scope: String,
    pub(super) name: Option<SessionName>,
}

pub(super) fn location() -> io::Result<Location> {
    resolve(
        SELECTED.get().and_then(Option::as_ref),
        |key| std::env::var_os(key),
        &orchbee_local_ipc::current_user_sid()?,
        default_base()?,
    )
}

/// Same-Session nested TUIs would be able to select their own hosting terminal.
/// CLI commands and explicit routes to another Session do not use this guard.
pub fn inside_selected_session() -> io::Result<bool> {
    inside_session(
        SELECTED.get().and_then(Option::as_ref),
        |key| std::env::var_os(key),
        &orchbee_local_ipc::current_user_sid()?,
        default_base()?,
    )
}

fn default_base() -> io::Result<PathBuf> {
    orchbee_local_runtime::default_runtime_base(|key| std::env::var_os(key)).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "this environment has no default runtime base",
        )
    })
}

fn inside_session(
    selected: Option<&SessionName>,
    environment: impl Fn(&str) -> Option<OsString>,
    sid: &str,
    fallback: PathBuf,
) -> io::Result<bool> {
    let Some(scope) = environment("ORCHBEE_RUNTIME_SCOPE") else {
        return Ok(false);
    };
    let hosted = environment("ORCHBEE_TERMINAL_ID")
        .and_then(|value| {
            value
                .to_str()
                .and_then(|value| orchbee_workbench_protocol::TerminalId::parse(value).ok())
        })
        .is_some()
        && environment("ORCHBEE_WORKER_ID")
            .and_then(|value| {
                value
                    .to_str()
                    .and_then(|value| orchbee_workbench_protocol::WorkerId::parse(value).ok())
            })
            .is_some();
    if !hosted {
        return Ok(false);
    }
    Ok(scope.to_str()
        == Some(
            resolve(selected, environment, sid, fallback)?
                .scope
                .as_str(),
        ))
}

fn resolve(
    selected: Option<&SessionName>,
    environment: impl Fn(&str) -> Option<OsString>,
    sid: &str,
    fallback: PathBuf,
) -> io::Result<Location> {
    let explicit = environment("ORCHBEE_RUNTIME_BASE").map(PathBuf::from);
    let default_base = explicit.is_none();
    let base = explicit.unwrap_or(fallback);
    let name = if let Some(name) = selected {
        name.clone()
    } else if let Some(scope) = environment("ORCHBEE_RUNTIME_SCOPE") {
        let text = scope
            .to_str()
            .ok_or_else(|| invalid("ORCHBEE_RUNTIME_SCOPE"))?;
        SessionScope::parse(text).map_err(|_| invalid("ORCHBEE_RUNTIME_SCOPE"))?;
        return Ok(Location {
            base,
            default_base,
            scope: text.to_owned(),
            name: None,
        });
    } else {
        let value = environment("ORCHBEE_SESSION").unwrap_or_else(|| OsString::from("default"));
        let text = value.to_str().ok_or_else(|| invalid("ORCHBEE_SESSION"))?;
        SessionName::parse(text).map_err(|_| invalid("ORCHBEE_SESSION"))?
    };
    Ok(Location {
        base,
        default_base,
        scope: name.scope_for_user(sid),
        name: Some(name),
    })
}

fn invalid(variable: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, format!("invalid {variable}"))
}

#[cfg(test)]
mod tests;
