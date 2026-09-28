//! Resolve the managed startup snapshot before any process or durable effect.
use orchbee_local_site_runtime::{WindowsSettingsError, WindowsUserSettings};
use std::{ffi::OsString, io};

pub(super) fn resolve(
    mut arguments: impl Iterator<Item = OsString>,
    load: impl FnOnce() -> Result<WindowsUserSettings, WindowsSettingsError>,
) -> io::Result<bool> {
    let Some(flag) = arguments.next() else {
        return load()
            .map(|settings| settings.resume_agents_on_restore)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error));
    };
    if flag != "--resume-agents-on-restore" {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let value = match arguments
        .next()
        .as_deref()
        .and_then(std::ffi::OsStr::to_str)
    {
        Some("true") => true,
        Some("false") => false,
        _ => return Err(io::ErrorKind::InvalidInput.into()),
    };
    if arguments.next().is_some() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immutable_snapshot_does_not_reread_mutable_file_or_environment() {
        for (text, expected) in [("true", true), ("false", false)] {
            assert_eq!(
                resolve(
                    ["--resume-agents-on-restore", text]
                        .map(OsString::from)
                        .into_iter(),
                    || panic!("committed startup must not reload configuration")
                )
                .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn direct_startup_validates_the_shared_settings_owner() {
        assert!(
            !resolve(std::iter::empty(), || Ok(WindowsUserSettings {
                resume_agents_on_restore: false,
                ..WindowsUserSettings::default()
            }))
            .unwrap()
        );
        let error = resolve(std::iter::empty(), || {
            Err(WindowsSettingsError::new("file", "invalid-toml"))
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("invalid-toml"));
    }

    #[test]
    fn private_argument_is_closed_and_never_falls_back() {
        for args in [
            vec!["--unknown", "true"],
            vec!["--resume-agents-on-restore"],
            vec!["--resume-agents-on-restore", "FALSE"],
            vec!["--resume-agents-on-restore", ""],
            vec!["--resume-agents-on-restore", " false "],
            vec!["--resume-agents-on-restore", "false", "extra"],
        ] {
            assert_eq!(
                resolve(args.into_iter().map(OsString::from), || panic!(
                    "no fallback"
                ))
                .unwrap_err()
                .kind(),
                io::ErrorKind::InvalidInput
            );
        }
        use std::os::windows::ffi::OsStringExt;
        assert!(
            resolve(
                [
                    OsString::from("--resume-agents-on-restore"),
                    OsString::from_wide(&[0xd800])
                ]
                .into_iter(),
                || panic!("no fallback")
            )
            .is_err()
        );
    }
}
