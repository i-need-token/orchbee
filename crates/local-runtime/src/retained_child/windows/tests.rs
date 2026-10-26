#[path = "tests/registration.rs"]
mod registration;

use super::*;
use std::{
    fs,
    process::Command,
    thread,
    time::{Duration, Instant},
};

const ROOT_ENV: &str = "ORCHBEE_RETAINED_CHILD_FIXTURE";
const ARGUMENTS: [&str; 2] = ["space \"quoted\" slash\\", "中文\\"];

#[test]
fn exact_termination_confirms_exit_and_preserves_another_process() {
    let executable = std::env::current_exe().unwrap();
    let args: Vec<&OsStr> = [
        "--exact",
        "retained_child::tests::termination_fixture",
        "--ignored",
    ]
    .into_iter()
    .map(OsStr::new)
    .collect();
    let mut first = RetainedChild::spawn(&executable, &args).unwrap();
    let second = RetainedChild::spawn(&executable, &args);
    let expired = first.terminate_before(Instant::now());
    let first_live = first.try_wait();
    let ended = first.terminate_before(Instant::now() + Duration::from_secs(3));
    let repeat = first.terminate_before(Instant::now());
    let mut second = second.unwrap();
    let second_live = second.try_wait();
    let second_ended = second.terminate_before(Instant::now() + Duration::from_secs(3));
    // Both exact process owners are settled before any assertion can unwind.
    assert_eq!(expired.unwrap_err().kind(), io::ErrorKind::TimedOut);
    assert!(first_live.unwrap().is_none());
    assert_eq!(ended.unwrap().code(), Some(1));
    assert_eq!(repeat.unwrap().code(), Some(1));
    assert!(second_live.unwrap().is_none());
    assert_eq!(second_ended.unwrap().code(), Some(1));
}

#[test]
#[ignore = "self-expiring process used by the exact termination test"]
fn termination_fixture() {
    thread::sleep(Duration::from_secs(5));
}

#[test]
fn detached_launch_preserves_argv_and_does_not_inherit_caller_capture_pipes() {
    let root = std::env::temp_dir().join(format!("orchbee-retained-test-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let started = Instant::now();
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "retained_child::tests::capture_fixture",
            "--ignored",
            "--nocapture",
        ])
        .env(ROOT_ENV, &root)
        .output()
        .unwrap();
    let elapsed = started.elapsed();
    // Release the exact self-expiring fixture even if an assertion below fails.
    fs::write(root.join("release"), b"").unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    while !root.join("done").exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    let finished = root.join("done").exists();
    let args: Vec<String> =
        serde_json::from_slice(&fs::read(root.join("argv.json")).unwrap()).unwrap();
    let resolved = root.canonicalize().unwrap();
    assert_eq!(
        resolved.parent(),
        Some(std::env::temp_dir().canonicalize().unwrap().as_path())
    );
    fs::remove_dir_all(resolved).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(finished, "fixture must release its own resources");
    assert!(
        elapsed < Duration::from_secs(4),
        "capture waited for detached child: {elapsed:?}"
    );
    assert!(args.windows(2).any(|pair| pair == ["--skip", ARGUMENTS[0]]));
    assert!(args.windows(2).any(|pair| pair == ["--skip", ARGUMENTS[1]]));
}

#[test]
#[ignore = "subprocess fixture invoked with an isolated runtime by the parent test"]
fn capture_fixture() {
    let root = std::path::PathBuf::from(std::env::var_os(ROOT_ENV).unwrap());
    let executable = std::env::current_exe().unwrap();
    let args: Vec<&OsStr> = [
        "--exact",
        "retained_child::tests::detached_fixture",
        "--ignored",
        "--nocapture",
        "--skip",
        ARGUMENTS[0],
        "--skip",
        ARGUMENTS[1],
    ]
    .into_iter()
    .map(OsStr::new)
    .collect();
    let mut child = RetainedChild::spawn(&executable, &args).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !root.join("argv.json").exists() && Instant::now() < deadline {
        assert!(child.try_wait().unwrap().is_none());
        thread::sleep(Duration::from_millis(10));
    }
    assert!(root.join("argv.json").exists());
    drop(child);
}

#[test]
#[ignore = "self-expiring detached subprocess fixture"]
fn detached_fixture() {
    let root = std::path::PathBuf::from(std::env::var_os(ROOT_ENV).unwrap());
    fs::write(
        root.join("argv.json"),
        serde_json::to_vec(&std::env::args().collect::<Vec<_>>()).unwrap(),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    while !root.join("release").exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    fs::write(root.join("done"), b"").unwrap();
}

#[test]
fn exit_code_259_is_a_completed_process_not_still_active() {
    let executable = std::env::current_exe().unwrap();
    let args: Vec<&OsStr> = [
        "--exact",
        "retained_child::tests::exit_259_fixture",
        "--ignored",
    ]
    .into_iter()
    .map(OsStr::new)
    .collect();
    let mut child = RetainedChild::spawn(&executable, &args).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert_eq!(status.code(), Some(259));
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "subprocess exit-status fixture"]
fn exit_259_fixture() {
    std::process::exit(259);
}
