//! Windows Worktree operands become absolute before runtime access, without probing filesystem paths.
use orchbee_workbench_protocol::{MAXIMUM_DIRECTORY_PATH_BYTES, WorkbenchDirectoryPath};
use std::{
    ffi::OsString,
    path::{Component, Path, PathBuf, Prefix},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PathFailure {
    Input,
    Context(&'static str),
}

/// Preserves literal absolute operands and resolves only explicitly declared caller-relative inputs.
pub(super) fn normalize(
    value: &str,
    mut environment: impl FnMut(&str) -> Option<OsString>,
    cwd: impl FnOnce() -> std::io::Result<PathBuf>,
) -> Result<WorkbenchDirectoryPath, PathFailure> {
    if value.is_empty() || value.len() > MAXIMUM_DIRECTORY_PATH_BYTES || value.contains('\0') {
        return Err(PathFailure::Input);
    }
    let path = Path::new(value);
    if drive_absolute(path) {
        return WorkbenchDirectoryPath::parse(value).map_err(|_| PathFailure::Input);
    }
    if path.has_root() || matches!(path.components().next(), Some(Component::Prefix(_))) {
        return Err(PathFailure::Input);
    }
    let rest = if value == "~" {
        Some("")
    } else {
        value
            .strip_prefix("~/")
            .or_else(|| value.strip_prefix(r"~\"))
    };
    let absolute = if let Some(rest) = rest {
        // Split separators so a second slash cannot replace the selected home root.
        let mut home = home(&mut environment)?;
        for component in rest.split(['/', '\\']).filter(|part| !part.is_empty()) {
            if component.contains(':') {
                return Err(PathFailure::Input);
            }
            home.push(component);
        }
        home
    } else {
        if value.contains(':') {
            return Err(PathFailure::Input);
        }
        let current = cwd().map_err(|_| PathFailure::Context("current-directory"))?;
        checked_context(&current, "current-directory")?;
        current.join(path)
    };
    if !drive_absolute(&absolute) {
        return Err(PathFailure::Input);
    }
    WorkbenchDirectoryPath::parse(absolute.to_str().ok_or(PathFailure::Input)?)
        .map_err(|_| PathFailure::Input)
}

fn drive_absolute(path: &Path) -> bool {
    path.is_absolute()
        && matches!(
            path.components().next(),
            Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        )
}

fn checked_context(path: &Path, name: &'static str) -> Result<(), PathFailure> {
    let value = path.to_str().ok_or(PathFailure::Context(name))?;
    if !drive_absolute(path) || WorkbenchDirectoryPath::parse(value).is_err() {
        return Err(PathFailure::Context(name));
    }
    Ok(())
}

fn home(environment: &mut impl FnMut(&str) -> Option<OsString>) -> Result<PathBuf, PathFailure> {
    let nonempty = |value: Option<OsString>| value.filter(|value| !value.is_empty());
    if let Some(value) = nonempty(environment("USERPROFILE")) {
        let path = PathBuf::from(value);
        checked_context(&path, "USERPROFILE")?;
        return Ok(path);
    }
    let drive = nonempty(environment("HOMEDRIVE"));
    let path = nonempty(environment("HOMEPATH"));
    match (drive, path) {
        (Some(drive), Some(path)) => {
            let drive = drive.to_str().ok_or(PathFailure::Context("HOMEDRIVE"))?;
            if !matches!(drive.as_bytes(), [letter, b':'] if letter.is_ascii_alphabetic()) {
                return Err(PathFailure::Context("HOMEDRIVE"));
            }
            let path = path.to_str().ok_or(PathFailure::Context("HOMEPATH"))?;
            if !path.starts_with(['/', '\\']) || path.starts_with("//") || path.starts_with(r"\\") {
                return Err(PathFailure::Context("HOMEPATH"));
            }
            let home = PathBuf::from(format!("{drive}{path}"));
            checked_context(&home, "HOMEPATH")?;
            Ok(home)
        }
        (Some(_), None) => Err(PathFailure::Context("HOMEPATH")),
        (None, Some(_)) => Err(PathFailure::Context("HOMEDRIVE")),
        (None, None) => {
            let value = nonempty(environment("HOME"))
                .ok_or(PathFailure::Context("USERPROFILE/HOMEDRIVE/HOMEPATH/HOME"))?;
            let home = PathBuf::from(value);
            checked_context(&home, "HOME")?;
            Ok(home)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(
        value: &str,
        variables: &[(&str, &str)],
    ) -> Result<WorkbenchDirectoryPath, PathFailure> {
        normalize(
            value,
            |key| {
                variables
                    .iter()
                    .find(|(name, _)| *name == key)
                    .map(|(_, value)| OsString::from(value))
            },
            || Ok(PathBuf::from(r"C:\caller\repo")),
        )
    }

    #[test]
    fn pinned_herdr_local_operand_cases_preserve_caller_and_home_ownership() {
        // Herdr e0507237 cli/worktree.rs::normalize_path_arg and worktree.rs::expand_tilde_path:
        // common supported Windows spellings; malformed context refusal is an explicit stricter boundary.
        for (input, expected) in [
            (r"C:\literal $HOME %VAR%", r"C:\literal $HOME %VAR%"),
            ("C:/literal/../path", "C:/literal/../path"),
            (r"\\?\C:\literal", r"\\?\C:\literal"),
            ("relative", r"C:\caller\repo\relative"),
            (r"..\trees\new", r"C:\caller\repo\..\trees\new"),
            ("~", r"D:\User space"),
            (r"~\trees\新", r"D:\User space\trees\新"),
            ("~//trees///new", r"D:\User space\trees\new"),
            ("~/../new", r"D:\User space\..\new"),
            ("$HOME", r"C:\caller\repo\$HOME"),
            ("%USERPROFILE%", r"C:\caller\repo\%USERPROFILE%"),
        ] {
            assert_eq!(
                resolve(input, &[("USERPROFILE", r"D:\User space")])
                    .unwrap()
                    .as_str(),
                expected
            );
        }
        assert_eq!(
            resolve("~", &[("HOMEDRIVE", "E:"), ("HOMEPATH", r"\user")])
                .unwrap()
                .as_str(),
            r"E:\user"
        );
        assert_eq!(
            resolve("~/new", &[("USERPROFILE", ""), ("HOME", r"F:\home")])
                .unwrap()
                .as_str(),
            r"F:\home\new"
        );
    }

    #[test]
    fn absolute_and_relative_operands_never_consult_home_and_invalid_home_never_falls_back() {
        normalize(
            r"C:\does-not-need-to-exist",
            |_| panic!("unused home"),
            || panic!("unused cwd"),
        )
        .unwrap();
        normalize(
            "new",
            |_| panic!("unused home"),
            || Ok(PathBuf::from(r"C:\base")),
        )
        .unwrap();
        for invalid in ["relative", "~", r"\\server\share", "C:relative"] {
            assert_eq!(
                resolve("~", &[("USERPROFILE", invalid), ("HOME", r"C:\valid")]),
                Err(PathFailure::Context("USERPROFILE"))
            );
        }
        assert_eq!(
            resolve("~", &[("HOMEDRIVE", "C:")]),
            Err(PathFailure::Context("HOMEPATH"))
        );
        assert_eq!(
            resolve("~", &[("HOMEDRIVE", "C:"), ("HOMEPATH", "relative")]),
            Err(PathFailure::Context("HOMEPATH"))
        );
        assert_eq!(
            resolve("~", &[]),
            Err(PathFailure::Context("USERPROFILE/HOMEDRIVE/HOMEPATH/HOME"))
        );
    }

    #[test]
    fn bounds_and_ambiguous_roots_refuse_before_runtime_or_path_probing() {
        for input in [
            "",
            "C:relative",
            r"\rooted",
            r"\\server\share",
            r"\\.\device",
            "~/C:/escape",
            "bad\0path",
        ] {
            assert_eq!(
                resolve(input, &[("USERPROFILE", r"C:\home")]),
                Err(PathFailure::Input),
                "{input:?}"
            );
        }
        let max = format!("C:/{}", "x".repeat(MAXIMUM_DIRECTORY_PATH_BYTES - 3));
        assert_eq!(resolve(&max, &[]).unwrap().as_str(), max);
        assert_eq!(resolve(&(max + "x"), &[]), Err(PathFailure::Input));
        assert_eq!(
            resolve(&"x".repeat(MAXIMUM_DIRECTORY_PATH_BYTES), &[]),
            Err(PathFailure::Input)
        );
        assert_eq!(
            normalize("new", |_| None, || Err(std::io::ErrorKind::NotFound.into())),
            Err(PathFailure::Context("current-directory"))
        );
        use std::os::windows::ffi::OsStringExt;
        assert_eq!(
            normalize(
                "~",
                |_| Some(OsString::from_wide(&[0xd800])),
                || panic!("unused cwd")
            ),
            Err(PathFailure::Context("USERPROFILE"))
        );
    }
}
