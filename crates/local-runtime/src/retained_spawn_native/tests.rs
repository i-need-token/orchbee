//! Boundary evidence for the reviewed `posix_spawn` adapter: what a child receives, and what its
//! wait reports.
use super::{spawn, wait};
use nix::fcntl::{FcntlArg, FdFlag, fcntl};
use std::{
    ffi::{OsStr, OsString},
    fs,
    io::Read,
    os::unix::{net::UnixStream, process::ExitStatusExt},
    path::Path,
    process::ExitStatus,
    time::Duration,
};

fn search_path() -> Vec<(OsString, OsString)> {
    vec![("PATH".into(), "/usr/bin:/bin".into())]
}

#[test]
fn a_descriptor_open_across_exec_reaches_no_child() {
    let (kept, mut peer) = UnixStream::pair().unwrap();
    // Without close-on-exec, every child the standard library starts would hold this end.
    fcntl(&kept, FcntlArg::F_SETFD(FdFlag::empty())).unwrap();
    // macOS refuses socket options once the peer has closed, so set the bound first.
    peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    let child = spawn(
        Path::new("/bin/sleep"),
        &[OsStr::new("2")],
        &search_path(),
        Path::new("/"),
        None,
        false,
    )
    .unwrap();
    drop(kept);
    assert_eq!(peer.read(&mut [0; 1]).unwrap(), 0);
    assert_eq!(
        wait(child, false).unwrap(),
        None,
        "the child must outlive the close"
    );
    let status = ExitStatus::from_raw(wait(child, true).unwrap().unwrap());
    assert!(status.success());
}

#[test]
fn a_child_runs_in_its_directory_with_its_arguments_and_exactly_its_environment() {
    let output =
        std::env::temp_dir().join(format!("orchbee-retained-spawn-{}", std::process::id()));
    let script =
        r#"printf '%s\n' "$(pwd)" "$#" "$2" "$3" "${PROBE-unset}" "${HOME-unset}" > "$1"; exit 7"#;
    let mut environment = search_path();
    environment.push(("PROBE".into(), "a b".into()));
    let child = spawn(
        Path::new("/bin/sh"),
        &[
            OsStr::new("-c"),
            OsStr::new(script),
            OsStr::new("sh"),
            output.as_os_str(),
            OsStr::new("two words"),
            OsStr::new(""),
        ],
        &environment,
        Path::new("/tmp"),
        None,
        false,
    )
    .unwrap();
    let status = ExitStatus::from_raw(wait(child, true).unwrap().unwrap());
    let written = fs::read_to_string(&output).unwrap();
    fs::remove_file(&output).unwrap();
    assert_eq!(status.code(), Some(7));
    assert_eq!(written, "/private/tmp\n3\ntwo words\n\na b\nunset\n");
}

#[test]
fn a_missing_executable_starts_nothing() {
    let error = spawn(
        Path::new("/nonexistent/orchbee-retained-spawn"),
        &[],
        &search_path(),
        Path::new("/"),
        None,
        false,
    )
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}
