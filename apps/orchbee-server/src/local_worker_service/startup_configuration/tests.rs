use super::*;
use orchbee_local_runtime::UserSettings;

#[cfg(windows)]
const SNAPSHOT: &str = "C:/snapshot/config.toml";
#[cfg(not(windows))]
const SNAPSHOT: &str = "/snapshot/config.toml";

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
        let load = parse(
            [
                "--configuration-file",
                SNAPSHOT,
                "--configuration-file-explicit",
                text,
            ]
            .map(OsString::from)
            .into_iter(),
        )
        .unwrap()
        .read(
            || panic!("a frozen location must not resolve the environment"),
            |path, explicit| Ok(located(path, explicit)),
        )
        .unwrap();
        assert_eq!(load.settings.path.as_deref(), Some(Path::new(SNAPSHOT)));
        assert_eq!(load.settings.explicit, explicit);
    }
}

#[test]
fn direct_startup_reads_through_the_shared_settings_owner() {
    let direct = || parse(std::iter::empty()).unwrap();
    assert!(
        direct()
            .read(
                || Ok(located(None, false)),
                |_, _| panic!("direct startup resolves its own location")
            )
            .unwrap()
            .settings
            .path
            .is_none()
    );
    let error = direct()
        .read(
            || Err(SettingsError::new("ORCHBEE_CONFIG_FILE", "invalid-path")),
            |_, _| panic!("direct startup resolves its own location"),
        )
        .unwrap_err();
    assert_eq!(error.field, "ORCHBEE_CONFIG_FILE");
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
            SNAPSHOT,
            "--configuration-file-explicit",
            "false",
            "extra",
        ],
        #[cfg(not(windows))]
        vec![
            "--configuration-file",
            "config.toml",
            "--configuration-file-explicit",
            "false",
        ],
        #[cfg(not(windows))]
        vec![
            "--configuration-file",
            "/snapshot/../config.toml",
            "--configuration-file-explicit",
            "false",
        ],
    ] {
        assert_eq!(
            parse(args.into_iter().map(OsString::from))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
    #[cfg(windows)]
    let unpaired = {
        use std::os::windows::ffi::OsStringExt;
        OsString::from_wide(&[0xd800])
    };
    #[cfg(not(windows))]
    let unpaired = {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(vec![0xff])
    };
    assert!(parse([OsString::from("--configuration-file"), unpaired].into_iter()).is_err());
}
