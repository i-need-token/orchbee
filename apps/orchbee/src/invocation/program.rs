//! Executable intent stays inert until the final CLI process has admitted the whole command.
use super::{
    ExpectedValue, FailureEnvelope, Options, ParseFailure, UsageContext, UsageReason,
    launch_option, usage_with,
};
use orchbee_workbench_protocol::{LaunchSelection, WorkbenchDirectoryPath};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ParsedLaunch {
    Absolute(LaunchSelection),
    Path {
        name: String,
        arguments: Vec<String>,
    },
}

pub(super) fn parse(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<ParsedLaunch>, ParseFailure> {
    let Some(exec) = options.exec.as_deref() else {
        return launch_option(options, context, envelope)
            .map(|value| value.map(ParsedLaunch::Absolute));
    };
    if WorkbenchDirectoryPath::parse(exec).is_ok() {
        return launch_option(options, context, envelope)
            .map(|value| value.map(ParsedLaunch::Absolute));
    }
    if !valid_name(exec) {
        return Err(invalid(
            exec,
            context,
            envelope,
            ExpectedValue::ExecutableName,
        ));
    }
    // Validate the argument contract independently of a filesystem lookup. Recheck the final
    // executable+argv size after resolution, before configuration, discovery or mutation.
    LaunchSelection::new("/".into(), options.args.clone())
        .map_err(|_| invalid("--arg", context, envelope, ExpectedValue::LaunchArguments))?;
    Ok(Some(ParsedLaunch::Path {
        name: exec.to_owned(),
        arguments: options.args.clone(),
    }))
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !matches!(name, "." | "..")
        && !name
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '/' | '\\' | ':'))
}

pub(crate) fn resolve(
    selection: Option<ParsedLaunch>,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<LaunchSelection>, ParseFailure> {
    let (name, arguments) = match selection {
        None => return Ok(None),
        Some(ParsedLaunch::Absolute(selection)) => return Ok(Some(selection)),
        Some(ParsedLaunch::Path { name, arguments }) => (name, arguments),
    };
    let path = std::env::var_os("PATH");
    let executable = lookup(&name, path.as_deref(), executable_file)
        .and_then(|path| path.to_str().map(str::to_owned))
        .ok_or_else(|| invalid(&name, context, envelope, ExpectedValue::ExecutableOnPath))?;
    LaunchSelection::new(executable, arguments)
        .map(Some)
        .map_err(|_| invalid(&name, context, envelope, ExpectedValue::LaunchArguments))
}

fn invalid(
    value: &str,
    context: UsageContext,
    envelope: FailureEnvelope,
    expected: ExpectedValue,
) -> ParseFailure {
    usage_with(
        UsageReason::InvalidValue {
            context,
            option: "--exec",
            value: value.to_owned(),
            expected,
        },
        envelope,
    )
}

fn lookup(
    name: &str,
    path: Option<&OsStr>,
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
        #[cfg(windows)]
        let names = if Path::new(name).extension().is_some() {
            vec![name.to_owned()]
        } else {
            vec![format!("{name}.exe"), format!("{name}.com")]
        };
        #[cfg(not(windows))]
        let names = vec![name.to_owned()];
        for name in names {
            let candidate = directory.join(name);
            if executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        nix::unistd::access(path, nix::unistd::AccessFlags::X_OK).is_ok()
    }
    #[cfg(windows)]
    {
        path.extension()
            .and_then(OsStr::to_str)
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe") || ext.eq_ignore_ascii_case("com"))
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
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
            lookup("tool", Some(OsStr::new(path)), |p| p == Path::new(first)
                || p == Path::new(second)),
            Some(PathBuf::from(first))
        );
        assert_eq!(
            lookup("tool", Some(OsStr::new(path)), |p| p == Path::new(second)),
            Some(PathBuf::from(second))
        );
        assert!(lookup("tool", Some(OsStr::new(path)), |_| false).is_none());
        assert!(lookup("tool", None, |_| true).is_none());
        for name in ["", ".", "..", "../tool", r".\tool", "C:tool", "tool\n"] {
            assert!(
                lookup(name, Some(OsStr::new(path)), |_| panic!(
                    "invalid name reached filesystem"
                ))
                .is_none()
            );
        }
    }
}
