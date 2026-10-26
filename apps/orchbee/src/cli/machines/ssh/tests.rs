use super::*;
use serde_json::json;
use std::process::Command;
use std::{
    fs,
    time::{Duration, Instant},
};
fn client_document() -> Value {
    json!({"schema":"orchbee.status.client","version":1,"result":{
        "version":env!("CARGO_PKG_VERSION"),"platform":"windows","architecture":std::env::consts::ARCH,
        "protocols":{"admission":orchbee_local_client_protocol::PROTOCOL_VERSION,
            "terminal":orchbee_client_server_protocol::PROTOCOL_VERSION},
        "cohort":{"state":"available","identity":"a".repeat(64)},
        "executable":{"path":"C:/remote/orchbee.exe"}}})
}
#[test]
fn installed_client_admission_requires_current_document_version() {
    let mut document = client_document();
    let admitted = client(&document).unwrap();
    assert_eq!(admitted.executable, "C:/remote/orchbee.exe");
    assert_eq!(admitted.cohort, "a".repeat(64));
    document["version"] = json!(2);
    assert!(client(&document).is_err());
}

fn server_document() -> Value {
    json!({"schema":"orchbee.status.server","version":1,"result":{
        "server":{"state":"running","stopping":false,"version":env!("CARGO_PKG_VERSION"),
            "cohort":"a".repeat(64),"protocols":client_document()["result"]["protocols"]},
        "localCohort":{"identity":"a".repeat(64)},"compatibility":{"compatible":true}}})
}
#[test]
fn preparation_requires_a_fresh_compatible_observation_and_retains_partial_state() {
    for failure in 0..=3 {
        let mut calls = Vec::new();
        let mut remote = Remote::NotContacted;
        let result = prepare_session(&mut remote, |path, action| {
            calls.push(action.to_owned());
            if calls.len() == failure {
                return Err(Error::Cancelled);
            }
            Ok(match action {
                "client" => {
                    assert!(path.is_none());
                    client_document()
                }
                "start" => {
                    assert_eq!(path, Some("C:/remote/orchbee.exe"));
                    json!({"schema":"orchbee.site.start","version":1,"result":{"outcome":"already-running"}})
                }
                "server" => server_document(),
                _ => panic!("unexpected call"),
            })
        });
        if failure == 0 {
            assert_eq!(result, Ok(()));
            assert_eq!(remote.token(), "ready");
            assert_eq!(calls, ["client", "start", "server"]);
        } else {
            assert_eq!(result, Err(Error::Cancelled));
            assert_eq!(calls.len(), failure);
            assert_eq!(
                remote.token(),
                if failure == 1 {
                    "unchanged"
                } else {
                    "possibly-started"
                }
            );
        }
    }
    let client = client(&client_document()).unwrap();
    for (field, value) in [
        ("state", json!("not-running")),
        ("stopping", json!(true)),
        ("cohort", json!("b".repeat(64))),
        ("version", json!("old")),
        ("protocols", json!({"admission":0,"terminal":0})),
    ] {
        let mut v = server_document();
        v["result"]["server"][field] = value;
        assert_eq!(server(&v, &client), Err(Error::RemoteNotReady));
    }
    let mut v = client_document();
    v["result"]["platform"] = json!("linux");
    assert!(matches!(super::client(&v), Err(Error::RemoteIncompatible)));
    v = client_document();
    v["result"]["executable"]["path"] = json!("relative.exe");
    assert!(matches!(super::client(&v), Err(Error::RemoteProtocol)));
}
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
fn actual_powershell_remote_program_preserves_literal_path_site_and_exit_code() {
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
                Some(WindowsCommandFailure::ExitFailure)
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
