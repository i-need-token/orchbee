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
    /// The selected Session, which also names its runtime folder.
    pub(super) name: SessionName,
}

impl Location {
    pub(super) fn scope(&self) -> &str {
        self.name.as_str()
    }
}

/// The selected Session, spelled as its existing folder is, so `work` reaches the Session created
/// as `Work` through the same runtime folder.
pub(super) fn location() -> io::Result<Location> {
    location_of(SELECTED.get().and_then(Option::as_ref))
}

/// The Session `selected` names, or the invocation's inherited or default one when absent, spelled
/// as [`location`] spells it. The link helper names a Session per channel, not per invocation.
pub(super) fn location_of(selected: Option<&SessionName>) -> io::Result<Location> {
    let mut location = resolve(selected, routed_environment(), default_base()?)?;
    let root = orchbee_local_runtime::DataRoot::resolve(|key| std::env::var_os(key))
        .map_err(|_| io::Error::from(io::ErrorKind::NotFound))?;
    location.name = orchbee_local_runtime::SessionCatalog::spelling(&root, &location.name)?;
    Ok(location)
}

/// The name of the Session this invocation targets, when it can be known: the selected, inherited
/// or default name, spelled as the Session's folder is. Absent when the location is invalid.
pub fn target_session_name() -> Option<SessionName> {
    location().ok().map(|location| location.name)
}

/// Same-Session nested TUIs would be able to select their own hosting terminal.
/// CLI commands and explicit routes to another Session do not use this guard.
pub fn inside_selected_session() -> io::Result<bool> {
    inside_session(
        SELECTED.get().and_then(Option::as_ref),
        routed_environment(),
        default_base()?,
    )
}

/// The process environment with another environment's inherited Session route removed.
fn routed_environment() -> impl Fn(&str) -> Option<OsString> {
    orchbee_local_runtime::session_route_environment(|key| std::env::var_os(key))
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
    let hosting = scope
        .to_str()
        .and_then(|scope| SessionName::parse(scope).ok());
    Ok(hosting == Some(resolve(selected, environment, fallback)?.name))
}

/// Selection precedence: explicit `--session`, the managed terminal's `ORCHBEE_RUNTIME_SCOPE`,
/// `ORCHBEE_SESSION`, then `default`. A scope names its Session's runtime folder, so it is that
/// Session's name.
fn resolve(
    selected: Option<&SessionName>,
    environment: impl Fn(&str) -> Option<OsString>,
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
        SessionName::parse(text).map_err(|_| invalid("ORCHBEE_RUNTIME_SCOPE"))?
    } else {
        let value = environment("ORCHBEE_SESSION").unwrap_or_else(|| OsString::from("default"));
        let text = value.to_str().ok_or_else(|| invalid("ORCHBEE_SESSION"))?;
        SessionName::parse(text).map_err(|_| invalid("ORCHBEE_SESSION"))?
    };
    Ok(Location {
        base,
        default_base,
        name,
    })
}

fn invalid(variable: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, format!("invalid {variable}"))
}

#[cfg(test)]
mod tests;
