//! Native PowerShell and process-exit witness, separate from host-independent packet assertions.
use super::*;
use orchbee_local_runtime::run_windows_command;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        if !self.0.exists() {
            return;
        }
        let path = self.0.canonicalize().unwrap();
        assert_eq!(
            path.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-machine-test-")
        );
        fs::remove_dir_all(path).unwrap();
    }
}
#[test]
fn actual_powershell_remote_program_preserves_literal_path_session_and_exit_code() {
    let root = Root(std::env::temp_dir().join(format!(
        "orchbee-machine-test-{}",
        crate::machines::ProfileId::generate().unwrap().as_str()
    )));
    let directory = root.0.join("quote' $(calc)& 中文");
    fs::create_dir_all(&directory).unwrap();
    let source = directory.join("fixture.rs");
    fs::write(&source, r#"fn main() {
        let a:Vec<String>=std::env::args().skip(1).collect();
        if a == ["--session","work-1","status","client","--format","json"] {println!("{{\"literal\":true}}");}
        else if a == ["--session","work-1","server","start","--format","json"] {std::process::exit(42);}
        else {std::process::exit(43);}
    }"#).unwrap();
    let executable = directory.join("orchbee.exe");
    let mut rustc = Command::new("rustc");
    rustc
        .arg(&source)
        .args(["--crate-name", "machine_fixture", "-o"])
        .arg(&executable);
    let build = run_windows_command(
        rustc,
        Some(Instant::now() + Duration::from_secs(45)),
        || Ok(false),
    );
    assert_eq!(build.failure, None);
    assert_eq!(build.exit_code, Some(0), "{build:?}");
    let powershell = PathBuf::from(winsafe::GetSystemDirectory().unwrap())
        .join("WindowsPowerShell/v1.0/powershell.exe");
    for (action, status) in [("client", 0), ("start", 42)] {
        let script = program(Some(executable.to_str().unwrap()), "work-1", action);
        let encoded = script.split_ascii_whitespace().last().unwrap();
        let mut command = Command::new(&powershell);
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
            encoded,
        ]);
        let result = run_windows_command(
            command,
            Some(Instant::now() + Duration::from_secs(15)),
            || Ok(false),
        );
        assert_eq!(
            result.failure,
            if status == 0 {
                None
            } else {
                Some(CommandFailure::ExitFailure)
            }
        );
        assert_eq!(result.exit_code, Some(status), "{result:?}");
        if status == 0 {
            assert_eq!(
                serde_json::from_slice::<Value>(&result.stdout).unwrap(),
                json!({"literal":true})
            );
        }
    }
}
