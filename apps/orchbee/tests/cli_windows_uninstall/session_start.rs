//! Session start by a Client outside the installed store, against the same test package and
//! profile: while the store holds an active version, the Session's Server and Worker run from that
//! version's folder and none from beside the outside Client; without the pointer the same command
//! starts them beside it; and two outside Clients and the store's Client starting at once all
//! succeed with one Server. Only a process shows which folder a Server image came from.

use super::{Profile, describe};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};
use winsafe::{HPROCESS, HPROCESSLIST, co};

const START: [&str; 4] = ["server", "start", "--format", "json"];

#[test]
#[ignore = "requires ORCHBEE_TEST_UNINSTALL_CLIENT naming a test-signed release directory's Client; run explicitly"]
fn an_outside_client_starts_the_session_from_the_store_and_falls_back_without_it() {
    let client = PathBuf::from(
        std::env::var_os("ORCHBEE_TEST_UNINSTALL_CLIENT")
            .expect("set ORCHBEE_TEST_UNINSTALL_CLIENT to a staged test package's Client"),
    );
    let profile = Profile::new(&client);
    let result = std::panic::catch_unwind(|| profile.start_from_the_store());
    profile.release();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

impl Profile {
    fn start_from_the_store(&self) {
        let launcher = self.install();
        let release = only_entry(&self.store().join("versions"));
        let beside = self.client.parent().unwrap().to_owned();
        let server = format!("{}-server.exe", self.stem);

        let (output, document) = self.run(&self.client, &START);
        assert_eq!(output.status.code(), Some(0), "{}", describe(&output));
        assert_eq!(document["result"]["outcome"], "started", "{document}");
        self.assert_session_runs_from(&release);
        assert_eq!(count_in(&beside, &server), 0);
        self.stop(&release, &server);

        // Without a pointer there is no active version: the outside Client starts its own cohort.
        let pointer = self.store().join("active.json");
        let aside = self.store().join("active.json.aside");
        fs::rename(&pointer, &aside).unwrap();
        let (output, document) = self.run(&self.client, &START);
        assert_eq!(output.status.code(), Some(0), "{}", describe(&output));
        assert_eq!(document["result"]["outcome"], "started", "{document}");
        self.assert_session_runs_from(&beside);
        self.stop(&beside, &server);
        fs::rename(&aside, &pointer).unwrap();

        let starters: Vec<Child> = [&self.client, &self.client, &launcher]
            .into_iter()
            .map(|program| {
                self.command(program)
                    .args(START)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap()
            })
            .collect();
        let mut started = 0;
        for starter in starters {
            let output = starter.wait_with_output().unwrap();
            assert_eq!(output.status.code(), Some(0), "{}", describe(&output));
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            if document["result"]["outcome"] == "started" {
                started += 1;
            }
        }
        assert_eq!(
            started, 1,
            "only the start that spawned the Server says started"
        );
        assert_eq!(count_in(&release, &server), 1);
        assert_eq!(count_in(&beside, &server), 0);
        self.assert_session_runs_from(&release);
    }

    /// The running Session's Server and Worker, as `status server` lists them, run from `folder`.
    fn assert_session_runs_from(&self, folder: &Path) {
        let (output, status) = self.run(&self.client, &["status", "server", "--format", "json"]);
        assert_eq!(output.status.code(), Some(0), "{}", describe(&output));
        let processes = status["result"]["server"]["processes"].as_array().unwrap();
        for role in ["server", "worker"] {
            assert!(
                processes.iter().any(|process| process["role"] == role),
                "{status}"
            );
        }
        let folder = fs::canonicalize(folder).unwrap();
        for process in processes {
            let pid = u32::try_from(process["processId"].as_u64().unwrap()).unwrap();
            let image = image_folder(pid).unwrap();
            assert_eq!(image, folder, "{process}");
        }
    }

    /// Stops the Session and waits until no Server runs from `folder`.
    fn stop(&self, folder: &Path, server: &str) {
        let (output, _) = self.run(
            &self.client,
            &["server", "stop", "--confirm", "--format", "json"],
        );
        assert_eq!(output.status.code(), Some(0), "{}", describe(&output));
        let started = Instant::now();
        while count_in(folder, server) != 0 {
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "the Server stays"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// The one entry of `folder`.
fn only_entry(folder: &Path) -> PathBuf {
    let entries: Vec<PathBuf> = fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(entries.len(), 1, "{entries:?}");
    entries.into_iter().next().unwrap()
}

/// The canonical folder of the running process `pid`'s image.
fn image_folder(pid: u32) -> Option<PathBuf> {
    let image = HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, pid)
        .ok()?
        .QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
        .ok()?;
    fs::canonicalize(Path::new(&image).parent()?).ok()
}

/// How many running processes named `image` run from `folder`.
fn count_in(folder: &Path, image: &str) -> usize {
    let folder = fs::canonicalize(folder).unwrap();
    let mut list = HPROCESSLIST::CreateToolhelp32Snapshot(co::TH32CS::SNAPPROCESS, None).unwrap();
    let ids: Vec<u32> = list
        .iter_processes()
        .filter_map(Result::ok)
        .filter(|entry| entry.szExeFile().eq_ignore_ascii_case(image))
        .map(|entry| entry.th32ProcessID)
        .collect();
    ids.into_iter()
        .filter(|pid| image_folder(*pid).is_some_and(|found| found == folder))
        .count()
}
