use super::*;
use nix::sys::signal::Signal;
use std::{fs, os::unix::fs::PermissionsExt, path::Path};

fn agent(directory: &Path, name: &str) {
    fs::create_dir_all(directory).unwrap();
    let path = directory.join(name);
    fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A name that only the person's login profile or interactive startup file puts on `PATH` is
/// found, an absent name is missing, and the shell starts from a thread that blocks no signal
/// although the probing thread had INT and TERM blocked.
#[test]
fn the_login_shell_path_answers_and_no_signal_stays_blocked() {
    let home = std::env::temp_dir().join(format!(
        "orchbee-agent-probe-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&home).unwrap();
    agent(&home.join("profile"), "profile-agent");
    agent(&home.join("rc"), "rc-agent");
    fs::write(
        home.join(".zprofile"),
        "export PATH=\"$HOME/profile:$PATH\"\n",
    )
    .unwrap();
    fs::write(
        home.join(".zshrc"),
        "export PATH=\"$HOME/rc:$PATH\"\necho noise\n",
    )
    .unwrap();
    fs::write(
        home.join(".bash_profile"),
        "export PATH=\"$HOME/profile:$PATH\"\necho noise\n",
    )
    .unwrap();
    let commands: Vec<String> = ["profile-agent", "rc-agent", "absent-agent"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let mut blocked = SigSet::empty();
    blocked.add(Signal::SIGINT);
    blocked.add(Signal::SIGTERM);
    let cancelled = AtomicBool::new(false);
    for (shell, expected) in [
        (
            "/bin/zsh",
            [
                AgentProbeResult::Found,
                AgentProbeResult::Found,
                AgentProbeResult::Missing,
            ],
        ),
        // A login bash reads its profile; `.bashrc` is the profile's to read.
        (
            "/bin/bash",
            [
                AgentProbeResult::Found,
                AgentProbeResult::Missing,
                AgentProbeResult::Missing,
            ],
        ),
    ] {
        blocked.thread_block().unwrap();
        let mut command = Command::new(shell);
        command
            .env("HOME", &home)
            .env("ZDOTDIR", &home)
            .env_remove("BASH_ENV")
            .env_remove("ENV");
        let results = probe(command, &commands, &cancelled, Instant::now() + DEADLINE);
        assert_eq!(results, expected, "{shell}");
        let mask = SigSet::thread_get_mask().unwrap();
        assert!(!mask.contains(Signal::SIGINT) && !mask.contains(Signal::SIGTERM));
    }
    fs::remove_dir_all(&home).unwrap();
}
