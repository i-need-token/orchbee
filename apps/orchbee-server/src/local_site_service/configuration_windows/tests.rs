use super::*;

#[test]
fn immutable_snapshot_does_not_reread_mutable_file_or_environment() {
    for (text, expected) in [("true", true), ("false", false)] {
        let settings = resolve(
            [
                "--resume-agents-on-restore",
                text,
                "--manifest-check",
                "true",
                "--manifest-check-source",
                "default",
                "--configuration-file",
                "",
                "--configuration-file-explicit",
                "false",
                "--default-shell",
                "C:/snapshot/shell.exe",
                "--shell-mode",
                "login",
            ]
            .map(OsString::from)
            .into_iter(),
            || panic!("committed startup must not reload configuration"),
        )
        .unwrap();
        assert_eq!(settings.resume_agents_on_restore, expected);
        assert_eq!(settings.shell.mode().as_str(), "login");
        assert_eq!(
            std::path::Path::new(settings.shell.executable()),
            std::path::Path::new("C:/snapshot/shell.exe")
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
        .resume_agents_on_restore
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
