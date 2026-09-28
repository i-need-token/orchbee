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

pub(super) fn resume_environment_valid(value: Option<OsString>) -> bool {
    value
        .as_deref()
        .is_none_or(|value| matches!(value.to_str(), Some("true" | "false")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_environment_is_closed_before_activation_commit() {
        assert!(resume_environment_valid(None));
        for value in ["true", "false"] {
            assert!(resume_environment_valid(Some(value.into())));
        }
        for value in ["", "1", "FALSE", " false "] {
            assert!(!resume_environment_valid(Some(value.into())));
        }
    }

    #[test]
    fn explicit_default_overrides_inherited_scope_and_name_without_reading_them() {
        let default = WindowsSiteName::parse("default").unwrap();
        let location = resolve(
            Some(&default),
            |key| {
                assert_eq!(key, "ORCHBEE_RUNTIME_BASE");
                Some(OsString::from("C:/runtime"))
            },
            "sid",
            PathBuf::from("C:/fallback"),
        )
        .unwrap();
        assert_eq!(location.scope, default.scope_for_user("sid"));
        assert_eq!(location.base, PathBuf::from("C:/runtime"));
    }

    #[test]
    fn inherited_route_precedes_name_and_invalid_selected_values_do_not_fall_back() {
        let inherited = resolve(
            None,
            |key| match key {
                "ORCHBEE_RUNTIME_SCOPE" => Some(OsString::from("owning-site")),
                "ORCHBEE_SITE" => panic!("inherited route must win"),
                _ => None,
            },
            "sid",
            PathBuf::from("C:/base"),
        )
        .unwrap();
        assert_eq!(inherited.scope, "owning-site");
        assert!(inherited.name.is_none());
        for variable in ["ORCHBEE_RUNTIME_SCOPE", "ORCHBEE_SITE"] {
            for bad in ["", "..", "some/name"] {
                let error = resolve(
                    None,
                    |key| (key == variable).then(|| bad.into()),
                    "sid",
                    PathBuf::from("C:/base"),
                )
                .err()
                .unwrap();
                assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
                assert!(error.to_string().contains(variable));
            }
        }
        let named = resolve(
            None,
            |key| (key == "ORCHBEE_SITE").then(|| "work".into()),
            "sid",
            PathBuf::from("C:/base"),
        )
        .unwrap();
        assert_eq!(
            named.scope,
            WindowsSiteName::parse("work")
                .unwrap()
                .scope_for_user("sid")
        );
    }
}
