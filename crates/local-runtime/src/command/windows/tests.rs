use super::*;
#[cfg(windows)]
#[test]
fn raw_command_launch_preserves_environment_and_exit_without_script_interpolation() {
    let root = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap());
    let shell = root.join("System32/cmd.exe");
    let encoded = windows_raw_command_encoded(
        shell.to_str().unwrap(),
        "if \"%ORCHBEE_TEST%\"==\"a'$b\" (exit /b 7) else (exit /b 9)",
        &[("ORCHBEE_TEST".into(), "a'$b".into())],
    )
    .unwrap();
    let status =
        std::process::Command::new(root.join("System32/WindowsPowerShell/v1.0/powershell.exe"))
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-EncodedCommand",
                &encoded,
            ])
            .status()
            .unwrap();
    assert_eq!(status.code(), Some(7));
    assert!(windows_raw_command_encoded("cmd.exe", "x\0y", &[]).is_none());
    assert!(
        windows_raw_command_encoded(
            "cmd.exe",
            "echo hello",
            &[("PATH".into(), "untrusted".into())]
        )
        .is_none()
    );
}
#[test]
fn encoding_has_independent_vectors_and_bounded_input() {
    for (plain, expected) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(base64(plain.as_bytes()), expected);
    }
    assert_eq!(literal("a'$b"), "'a''$b'");
    assert!(windows_encoded_command("codex", &["a\nb".into()]).is_none());
    assert!(windows_encoded_command("", &[]).is_none());
    assert!(windows_encoded_command("pi", &["'".repeat(4096)]).is_some());
    assert!(windows_encoded_command("pi", &["x".repeat(31 * 1024)]).is_none());
    assert!(windows_encoded_command("pi", &vec![String::new(); 129]).is_none());
}
