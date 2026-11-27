//! Input is fed while output drains: a command that reads megabytes before it writes anything gets
//! all of them, without either side waiting on the other.
use super::*;
use crate::GIT_PROGRAM;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

const BOUNDS: OutputBounds = OutputBounds {
    stdout: 4096,
    stderr_tail: 4096,
};

#[test]
fn input_larger_than_a_pipe_buffer_reaches_the_command_whole() {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let file = std::env::temp_dir().join(format!(
        "orchbee-git-run-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let input = (0..8 * 1024 * 1024)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state.to_le_bytes()[0]
        })
        .collect::<Vec<_>>();
    fs::write(&file, &input).unwrap();
    // The Git configuration of whoever runs the test, which could filter the file, is never read:
    // no system or global configuration or attributes, and a home that does not exist.
    let home = std::env::temp_dir().join("orchbee-git-process-tests-no-home");
    let hash = |arguments: &[&std::ffi::OsStr], input: Vec<u8>| {
        let mut command = Command::new(GIT_PROGRAM);
        command
            .args(arguments)
            .env_remove("GIT_CONFIG_PARAMETERS")
            .env_remove("GIT_CONFIG_COUNT")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", home.join(".gitconfig"))
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", &home);
        let output = Run::new(command)
            .input(input)
            .wait(BOUNDS, || Ok::<(), ()>(()))
            .unwrap();
        assert!(output.status.success());
        output.stdout
    };
    let fed = hash(&["hash-object".as_ref(), "--stdin".as_ref()], input);
    let read = hash(&["hash-object".as_ref(), file.as_os_str()], Vec::new());
    let _ = fs::remove_file(&file);
    assert_eq!(fed, read);
    assert_eq!(fed.len(), 41, "{}", String::from_utf8_lossy(&fed));
}
