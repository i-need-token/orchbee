use super::*;

#[test]
fn immutable_snapshot_does_not_reread_mutable_file_or_environment() {
    for (text, expected) in [("true", true), ("false", false)] {
        let settings = resolve(
            [
                "--manifest-check",
                text,
                "--manifest-check-source",
                "file",
                "--configuration-file",
                "C:/snapshot/config.toml",
                "--configuration-file-explicit",
                "false",
            ]
            .map(OsString::from)
            .into_iter(),
            || panic!("committed startup must not reload configuration"),
        )
        .unwrap();
        assert_eq!(settings.manifest_check, expected);
    }
}

#[test]
fn direct_startup_validates_the_shared_settings_owner() {
    assert!(
        !resolve(std::iter::empty(), || Ok(UserSettings {
            manifest_check: false,
            ..UserSettings::default()
        }))
        .unwrap()
        .manifest_check
    );
    let error = resolve(std::iter::empty(), || {
        Err(SettingsError::new("file", "invalid-toml"))
    })
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("invalid-toml"));
}

#[test]
fn private_argument_is_closed_and_never_falls_back() {
    for args in [
        vec!["--unknown", "true"],
        vec!["--manifest-check"],
        vec!["--manifest-check", "FALSE"],
        vec!["--manifest-check", ""],
        vec!["--manifest-check", " false "],
        vec!["--manifest-check", "false", "extra"],
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
                OsString::from("--manifest-check"),
                OsString::from_wide(&[0xd800])
            ]
            .into_iter(),
            || panic!("no fallback")
        )
        .is_err()
    );
}
