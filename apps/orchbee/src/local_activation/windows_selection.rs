//! Immutable invocation selection; environment is consulted only during runtime discovery.
use orchbee_local_site_runtime::{SiteScope, WindowsSiteName};
use std::{ffi::OsString, io, path::PathBuf, sync::OnceLock};

static SELECTED: OnceLock<Option<WindowsSiteName>> = OnceLock::new();

pub(crate) fn select_site(name: Option<WindowsSiteName>) {
    SELECTED
        .set(name)
        .expect("invocation selection is initialized once after parsing");
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
        let value = environment("ORCHBEE_SITE").unwrap_or_else(|| OsString::from("default"));
        let text = value.to_str().ok_or_else(|| invalid("ORCHBEE_SITE"))?;
        WindowsSiteName::parse(text).map_err(|_| invalid("ORCHBEE_SITE"))?
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
