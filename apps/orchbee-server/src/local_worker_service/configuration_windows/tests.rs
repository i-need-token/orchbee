use super::*;
use orchbee_local_runtime::UserSettings;

fn located(path: Option<&Path>, explicit: bool) -> UserSettingsLoad {
    UserSettingsLoad {
        settings: UserSettings {
            path: path.map(Path::to_owned),
            explicit,
            ..UserSettings::default()
        },
        ..UserSettingsLoad::default()
    }
}

#[test]
fn frozen_location_is_read_once_without_resolving_the_environment() {
    for (text, explicit) in [("true", true), ("false", false)] {
        let load = resolve(
            [
                "--configuration-file",
                "C:/snapshot/config.toml",
                "--configuration-file-explicit",
                text,
            ]
            .map(OsString::from)
            .into_iter(),
            || panic!("a frozen location must not resolve the environment"),
            |path, explicit| Ok(located(path, explicit)),
        )
        .unwrap();
        assert_eq!(
            load.settings.path.as_deref(),
            Some(Path::new("C:/snapshot/config.toml"))
        );
        assert_eq!(load.settings.explicit, explicit);
    }
}

#[test]
fn direct_startup_reads_through_the_shared_settings_owner() {
    assert!(
        resolve(
            std::iter::empty(),
            || Ok(located(None, false)),
            |_, _| panic!("direct startup resolves its own location")
        )
        .unwrap()
        .settings
        .path
        .is_none()
    );
    let error = resolve(
        std::iter::empty(),
        || Err(SettingsError::new("ORCHBEE_CONFIG_FILE", "invalid-path")),
        |_, _| panic!("direct startup resolves its own location"),
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("ORCHBEE_CONFIG_FILE"));
}

#[test]
fn private_argument_is_closed_and_never_falls_back() {
    for args in [
        vec!["--unknown", "true"],
        vec!["--manifest-check", "true"],
        vec!["--configuration-file"],
        vec!["--configuration-file", "", "--configuration-file-explicit"],
        vec![
            "--configuration-file",
            "",
            "--configuration-file-explicit",
            "TRUE",
        ],
        vec![
            "--configuration-file",
            "",
            "--configuration-file-explicit",
            "true",
        ],
        vec![
            "--configuration-file",
            "C:/config.toml",
            "--configuration-file-explicit",
            "false",
            "extra",
        ],
    ] {
        assert_eq!(
            resolve(
                args.into_iter().map(OsString::from),
                || panic!("no fallback"),
                |_, _| panic!("no fallback")
            )
            .unwrap_err()
            .kind(),
            io::ErrorKind::InvalidInput
        );
    }
    use std::os::windows::ffi::OsStringExt;
    assert!(
        resolve(
            [
                OsString::from("--configuration-file"),
                OsString::from_wide(&[0xd800])
            ]
            .into_iter(),
            || panic!("no fallback"),
            |_, _| panic!("no fallback")
        )
        .is_err()
    );
}
