//! Immutable invocation selection; environment is consulted only during runtime discovery.
use orchbee_local_site_runtime::{SiteScope, WindowsSiteName};
use std::{ffi::OsString, io, path::PathBuf, sync::OnceLock};

static SELECTED: OnceLock<Option<WindowsSiteName>> = OnceLock::new();

pub(crate) fn select_site(name: Option<WindowsSiteName>) {
    SELECTED
        .set(name)
        .expect("invocation selection is initialized once after parsing");
}

/// Return only the explicit, parser-validated `--session` selection. An
/// inherited environment route remains inherited by the startup command and
/// does not need to be copied into a hint.
pub(crate) fn selected_session_name() -> Option<&'static str> {
    SELECTED
        .get()
        .and_then(Option::as_ref)
        .map(WindowsSiteName::as_str)
}

pub(super) struct Location {
    pub(super) base: PathBuf,
    pub(super) scope: String,
    pub(super) name: Option<WindowsSiteName>,
}

pub(super) fn location() -> io::Result<Location> {
    resolve(
        SELECTED.get().and_then(Option::as_ref),
        |key| std::env::var_os(key),
        &orchbee_local_ipc::current_user_sid()?,
        std::env::temp_dir(),
    )
}

/// Same-Site nested TUIs would be able to select their own hosting terminal.
/// CLI commands and explicit routes to another Site do not use this guard.
pub(crate) fn inside_selected_session() -> io::Result<bool> {
    inside_session(
        SELECTED.get().and_then(Option::as_ref),
        |key| std::env::var_os(key),
        &orchbee_local_ipc::current_user_sid()?,
        std::env::temp_dir(),
    )
}

fn inside_session(
    selected: Option<&WindowsSiteName>,
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
    selected: Option<&WindowsSiteName>,
    environment: impl Fn(&str) -> Option<OsString>,
    sid: &str,
    fallback: PathBuf,
) -> io::Result<Location> {
    let base = environment("ORCHBEE_RUNTIME_BASE")
        .map(PathBuf::from)
        .unwrap_or(fallback);
    let name = if let Some(name) = selected {
        name.clone()
    } else if let Some(scope) = environment("ORCHBEE_RUNTIME_SCOPE") {
        let text = scope
            .to_str()
            .ok_or_else(|| invalid("ORCHBEE_RUNTIME_SCOPE"))?;
        SiteScope::parse(text).map_err(|_| invalid("ORCHBEE_RUNTIME_SCOPE"))?;
        return Ok(Location {
            base,
            scope: text.to_owned(),
            name: None,
        });
    } else {
        let value = environment("ORCHBEE_SESSION").unwrap_or_else(|| OsString::from("default"));
        let text = value.to_str().ok_or_else(|| invalid("ORCHBEE_SESSION"))?;
        WindowsSiteName::parse(text).map_err(|_| invalid("ORCHBEE_SESSION"))?
    };
    Ok(Location {
        base,
        scope: name.scope_for_user(sid),
        name: Some(name),
    })
}

fn invalid(variable: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, format!("invalid {variable}"))
}

#[cfg(test)]
mod tests;
