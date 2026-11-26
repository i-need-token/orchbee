use super::*;
#[test]
fn a_development_or_test_build_runs_only_its_own_environment_on_the_far_computer() {
    use base64::Engine as _;
    // Builds never meet across environments: a test build's remote command names only its own
    // store and Client, so it can neither run nor reach a release installation's state there.
    assert!(!orchbee_local_runtime::is_release());
    let line = link_program();
    let encoded = line
        .split_whitespace()
        .skip_while(|word| *word != "-EncodedCommand")
        .nth(1)
        .unwrap_or_default();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .unwrap();
    let units = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    let script = String::from_utf16(&units).unwrap();
    let stem = orchbee_local_runtime::environment_stem();
    assert!(script.contains(&literal(stem)), "{script}");
    assert!(
        script.contains(&literal(&format!("{stem}.exe"))),
        "{script}"
    );
    for release in ["Orchbee", "orchbee.exe"] {
        assert!(!script.contains(&literal(release)), "{script}");
    }
    let posix = posix("--machine-link-v1");
    assert!(
        posix.contains(&format!("/.local/share/{stem}}}")),
        "{posix}"
    );
    assert!(posix.contains(&format!("/Programs/{stem}}}")), "{posix}");
    assert!(
        !posix.contains("/orchbee}") && !posix.contains("/Orchbee}"),
        "{posix}"
    );
}

#[test]
fn the_launch_line_parses_alike_in_every_remote_shell() {
    // `cmd.exe` refuses a command line over 8191 characters; Win32-OpenSSH passes the line to
    // PowerShell without escaping double quotes; the POSIX program sits in one single-quoted word.
    let line = link_program();
    assert!(line.len() < 8191, "{}", line.len());
    assert!(!line.contains('"'), "{line}");
    assert_eq!(line.matches('\'').count(), 2, "{line}");
    assert_eq!(line.lines().count(), 2, "{line}");
    // fish reads `\\` and `\'`, and csh `!`, even inside single quotes.
    for special in ["\\", "!"] {
        assert!(!line.contains(special), "{line}");
    }
    // A shell that globs the first line's word aborts the whole line: zsh does so for `^`, `~` and
    // `#` under `extendedglob`.
    let first = line.lines().next().unwrap_or_default();
    assert!(!first.contains(['*', '?', '[', '^', '~', '#']), "{first}");
}

/// The launch line run by each login shell this computer has, as a macOS or Linux OpenSSH server
/// runs it with `<shell> -c`, toward a store under a temporary `HOME`.
#[cfg(unix)]
#[test]
fn the_launch_line_runs_the_store_client_on_macos_and_linux() {
    use std::os::unix::fs::PermissionsExt as _;
    let home = std::env::temp_dir().join(format!("ob-launch-line-{}", std::process::id()));
    let stem = orchbee_local_runtime::environment_stem();
    let store = home.join(".local/share").join(stem);
    let version = store.join("versions/1.2.3");
    std::fs::create_dir_all(&version).unwrap();
    let client = version.join(stem);
    std::fs::write(&client, "#!/bin/sh\nprintf '%s' \"$*\"\nexit 7\n").unwrap();
    std::fs::set_permissions(&client, std::fs::Permissions::from_mode(0o755)).unwrap();
    // A `uname` that names a system no release runs on.
    let other = home.join("other-system");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("uname"), "#!/bin/sh\necho FreeBSD\n").unwrap();
    std::fs::set_permissions(other.join("uname"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let shells = [
        "/bin/sh",
        "/bin/bash",
        "/bin/zsh",
        "/bin/dash",
        "/bin/ksh",
        "/bin/csh",
        "/usr/bin/fish",
    ]
    .into_iter()
    .filter(|shell| Path::new(shell).is_file());
    let pointer = |product: &str| {
        format!(
            "{{ \"version\": 1, \"active\": {{ \"product\": \"{product}\", \"manifest\": \"{}\" }} }}\n",
            "0".repeat(64)
        )
    };
    for shell in shells {
        let run = |path: Option<&Path>| {
            let mut command = Command::new(shell);
            command
                .arg("-c")
                .arg(link_program())
                .env("HOME", &home)
                .env_remove("ORCHBEE_INSTALL_DIR");
            if let Some(path) = path {
                let inherited = std::env::var("PATH").unwrap_or_default();
                command.env("PATH", format!("{}:{inherited}", path.display()));
            }
            let output = command.output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            (output.status.code(), stdout, shell)
        };
        let _ = std::fs::remove_file(store.join("active.json"));
        assert_eq!(run(None), (Some(PROGRAM_NOT_FOUND), String::new(), shell));
        std::fs::write(store.join("active.json"), pointer("1.2.3")).unwrap();
        assert_eq!(run(None), (Some(7), "--machine-link-v1".to_owned(), shell));
        let unsupported = (Some(PLATFORM_UNSUPPORTED), String::new(), shell);
        assert_eq!(run(Some(&other)), unsupported);
        std::fs::write(store.join("active.json"), pointer("1.2.4")).unwrap();
        assert_eq!(run(None), (Some(PROGRAM_NOT_FOUND), String::new(), shell));
    }
    std::fs::remove_dir_all(&home).unwrap();
}
#[test]
fn only_an_openssh_release_line_of_8_5_or_later_lets_answers_be_remembered() {
    // The text `ssh -V` printed, the release it names, and whether answers may be remembered.
    type Case = (&'static str, Option<(u32, u32)>, bool);
    let cases: [Case; 14] = [
        (
            "OpenSSH_for_Windows_9.5p1, LibreSSL 3.8.2\r\n",
            Some((9, 5)),
            true,
        ),
        (
            "OpenSSH_for_Windows_8.1p1, LibreSSL 3.0.2\r\n",
            Some((8, 1)),
            false,
        ),
        (
            "OpenSSH_8.4p1 Debian-5+deb11u3, OpenSSL 1.1.1w  11 Sep 2023\n",
            Some((8, 4)),
            false,
        ),
        (
            "OpenSSH_8.5p1, OpenSSL 1.1.1k  25 Mar 2021\n",
            Some((8, 5)),
            true,
        ),
        ("OpenSSH_10.0p2, LibreSSL 3.3.6\n", Some((10, 0)), true),
        ("  OpenSSH_9.6\nsecond line\n", Some((9, 6)), true),
        ("", None, false),
        ("ssh: unknown option -- V\r\n", None, false),
        ("OpenSSH_\n", None, false),
        ("OpenSSH_x.y\n", None, false),
        ("OpenSSH_9\n", None, false),
        ("OpenSSH_9.\n", None, false),
        ("OpenSSH_99999999999.5\n", None, false),
        ("PuTTY Release 0.81\nOpenSSH_9.5p1\n", None, false),
    ];
    for (text, expected, remembers) in cases {
        let version = Version::parse(text);
        assert_eq!(
            version.map(|version| (version.major, version.minor)),
            expected,
            "{text:?}"
        );
        assert_eq!(
            version.is_some_and(Version::remembers),
            remembers,
            "{text:?}"
        );
    }
}
