use super::*;
fn powershell(script: &str) -> Command {
    let path = std::path::PathBuf::from(winsafe::GetSystemDirectory().unwrap())
        .join("WindowsPowerShell/v1.0/powershell.exe");
    let mut command = Command::new(path);
    command.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        script,
    ]);
    command
}
#[test]
fn full_duplex_exchange_drains_before_input_and_delivers_eof_and_exit() {
    let script = "$o=[Console]::OpenStandardOutput();$b=[byte[]]::new(131072);$o.Write($b,0,$b.Length);$i=[Console]::OpenStandardInput();$i.CopyTo($o);exit 42";
    let input = (0..1024 * 1024)
        .map(|i| (i % 256) as u8)
        .collect::<Vec<_>>();
    let output = run_windows_command_exchange(
        powershell(script),
        Some(&input),
        2 * 1024 * 1024,
        Some(Instant::now() + Duration::from_secs(15)),
        || Ok(false),
    );
    assert_eq!(
        output.failure,
        Some(CommandFailure::ExitFailure),
        "{output:?}"
    );
    assert_eq!(output.exit_code, Some(42));
    assert!(!output.stdout_truncated);
    assert_eq!(&output.stdout[..131072], vec![0; 131072]);
    assert_eq!(&output.stdout[131072..], input);
}
#[test]
fn exchange_bounds_both_outputs_and_settles_stalled_input() {
    let output = run_windows_command_exchange(
        powershell(
            "$b=[byte[]]::new(100000);[Console]::OpenStandardOutput().Write($b,0,$b.Length);[Console]::OpenStandardError().Write($b,0,$b.Length)",
        ),
        None,
        100,
        Some(Instant::now() + Duration::from_secs(10)),
        || Ok(false),
    );
    assert_eq!(output.failure, None);
    assert_eq!((output.stdout.len(), output.stderr.len()), (100, 100));
    assert!(output.stdout_truncated && output.stderr_truncated);
    let output = run_windows_command_exchange(
        powershell("Start-Sleep -Seconds 30"),
        Some(&vec![0; 1024 * 1024]),
        100,
        Some(Instant::now() + Duration::from_secs(1)),
        || Ok(false),
    );
    assert_eq!(output.failure, Some(CommandFailure::Timeout));
}
#[test]
fn exchange_cancels_before_spawn_and_during_blocked_write() {
    let mut invalid = Command::new("orchbee-nonexistent-command.exe");
    invalid.arg("never");
    assert_eq!(
        run_windows_command_exchange(invalid, Some(b"x"), 10, None, || Ok(true)).failure,
        Some(CommandFailure::Cancelled)
    );
    let start = Instant::now();
    let output = run_windows_command_exchange(
        powershell("Start-Sleep -Seconds 30"),
        Some(&vec![0; 1024 * 1024]),
        100,
        Some(start + Duration::from_secs(10)),
        || Ok(start.elapsed() > Duration::from_millis(700)),
    );
    assert_eq!(output.failure, Some(CommandFailure::Cancelled));
}
