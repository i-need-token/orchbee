//! Shared bounded PATH walk behind the selected native executable adapter.
use super::{
    ExpectedValue, FailureEnvelope, ParseFailure, ParsedLaunch, UsageContext, invalid, valid_name,
};
use orchbee_workbench_protocol::{LaunchSelection, WorkbenchDirectoryPath};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};
pub(super) fn resolve(
    selection: Option<ParsedLaunch>,
    context: UsageContext,
    envelope: FailureEnvelope,
    names: fn(&str) -> Vec<String>,
    executable_file: fn(&Path) -> bool,
) -> Result<Option<LaunchSelection>, ParseFailure> {
    let (name, arguments) = match selection {
        None => return Ok(None),
        Some(ParsedLaunch::Absolute(selection)) => return Ok(Some(selection)),
        Some(ParsedLaunch::Path { name, arguments }) => (name, arguments),
    };
    let path = std::env::var_os("PATH");
    let executable = lookup(&name, path.as_deref(), names, executable_file)
        .and_then(|path| path.to_str().map(str::to_owned))
        .ok_or_else(|| invalid(&name, context, envelope, ExpectedValue::ExecutableOnPath))?;
    LaunchSelection::new(executable, arguments)
        .map(Some)
        .map_err(|_| invalid(&name, context, envelope, ExpectedValue::LaunchArguments))
}

fn lookup(
    name: &str,
    path: Option<&OsStr>,
    names: fn(&str) -> Vec<String>,
    mut executable: impl FnMut(&Path) -> bool,
) -> Option<PathBuf> {
    if !valid_name(name) {
        return None;
    }
    let path = path?;
    if path.len() > 1024 * 1024 {
        return None;
    }
    for directory in std::env::split_paths(path).take(256) {
        let Some(text) = directory.to_str() else {
            continue;
        };
        if text.chars().any(char::is_control) || WorkbenchDirectoryPath::parse(text).is_err() {
            continue;
        }
        for name in names(name) {
            let candidate = directory.join(name);
            if executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_lookup_uses_ordered_absolute_entries_and_never_implicit_current_directory() {
        #[cfg(windows)]
        let (path, first, second) = (
            r";relative;C:\one;C:\two",
            r"C:\one\tool.exe",
            r"C:\two\tool.exe",
        );
        #[cfg(not(windows))]
        let (path, first, second) = (":relative:/one:/two", "/one/tool", "/two/tool");
        assert_eq!(
            lookup(
                "tool",
                Some(OsStr::new(path)),
                super::super::native::names,
                |p| p == Path::new(first) || p == Path::new(second)
            ),
            Some(PathBuf::from(first))
        );
        assert_eq!(
            lookup(
                "tool",
                Some(OsStr::new(path)),
                super::super::native::names,
                |p| p == Path::new(second)
            ),
            Some(PathBuf::from(second))
        );
        assert!(
            lookup(
                "tool",
                Some(OsStr::new(path)),
                super::super::native::names,
                |_| false
            )
            .is_none()
        );
        assert!(lookup("tool", None, super::super::native::names, |_| true).is_none());
        for name in ["", ".", "..", "../tool", r".\tool", "C:tool", "tool\n"] {
            assert!(
                lookup(
                    name,
                    Some(OsStr::new(path)),
                    super::super::native::names,
                    |_| panic!("invalid name reached filesystem")
                )
                .is_none()
            );
        }
    }
}
