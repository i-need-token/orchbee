//! Uninstall of an installed test package, in both modes, in a scratch profile: one Agent
//! integration enabled and one Session running beforehand; afterwards the program folder, the
//! user `Path` entry, the installer entries, the integration and the removed data entries are gone,
//! and the person's settings and Sessions stay unless all data was deleted. With all data deleted,
//! an item Orchbee did not write stays in each root, which then stays too, and a second run still
//! finds nothing to refuse. The Agent's own file returns byte for byte. The platform uninstaller's
//! removal empties a user `Path` that held only the store's entry and keeps its registry type.
//!
//! It needs `ORCHBEE_TEST_UNINSTALL_CLIENT` naming the Client of a staged, packaged and test-signed
//! release directory (see the application's Windows CLI configuration). Every process gets its own
//! profile, store root, data root and `PATH` with no Agent program on it, and the registry entries
//! live below a scratch key under `HKCU\Software\ob-test-scratch` that the test deletes.
#![cfg(windows)]

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const AGENT_FILE: &str = r#"{"user":{"enabled":false,"future":123456789012345678901234567890}}"#;
const OTHER_ENTRIES: [&str; 2] = [r"C:\Keep Me", r"D:\Other;"];

#[test]
#[ignore = "requires ORCHBEE_TEST_UNINSTALL_CLIENT naming a test-signed release directory's Client; run explicitly"]
fn uninstall_removes_the_installation_and_keeps_data_unless_asked() {
    let client = PathBuf::from(
        std::env::var_os("ORCHBEE_TEST_UNINSTALL_CLIENT")
            .expect("set ORCHBEE_TEST_UNINSTALL_CLIENT to a staged test package's Client"),
    );
    for delete_data in [false, true] {
        let profile = Profile::new(&client);
        let result = std::panic::catch_unwind(|| profile.uninstall_once(delete_data));
        profile.release();
        if let Err(panic) = result {
            std::panic::resume_unwind(panic);
        }
    }
}

/// On an account whose user `Path` was absent or empty, the PATH step leaves a value holding only
/// the store's entry, `REG_EXPAND_SZ` when it was absent. The platform uninstaller's removal then
/// succeeds and leaves the value empty with its registry type.
#[test]
#[ignore = "requires ORCHBEE_TEST_UNINSTALL_CLIENT naming a test-signed release directory's Client; run explicitly"]
fn installer_uninstall_empties_a_path_that_held_only_the_store_entry() {
    let client = PathBuf::from(
        std::env::var_os("ORCHBEE_TEST_UNINSTALL_CLIENT")
            .expect("set ORCHBEE_TEST_UNINSTALL_CLIENT to a staged test package's Client"),
    );
    for kind in ["REG_SZ", "REG_EXPAND_SZ"] {
        let profile = Profile::new(&client);
        let result = std::panic::catch_unwind(|| profile.remove_the_only_entry(kind));
        profile.release();
        if let Err(panic) = result {
            std::panic::resume_unwind(panic);
        }
    }
}

struct Profile {
    root: PathBuf,
    client: PathBuf,
    stem: String,
    key: String,
}

impl Profile {
    fn new(client: &Path) -> Self {
        let nonce = format!(
            "{:x}-{:x}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(format!("ob-uninstall-{nonce}"));
        for folder in ["home", "local", "roaming", "tmp", "bin"] {
            fs::create_dir_all(root.join(folder)).unwrap();
        }
        let stem = client.file_stem().unwrap().to_str().unwrap().to_owned();
        assert!(stem.starts_with("ob-test-"), "a test package, not {stem}");
        Self {
            root,
            client: client.to_owned(),
            stem,
            key: format!(r"Software\ob-test-scratch\uninstall-{nonce}"),
        }
    }

    fn store(&self) -> PathBuf {
        self.root.join("local").join("Programs").join(&self.stem)
    }

    fn data(&self) -> PathBuf {
        self.root.join("home").join(format!(".{}", self.stem))
    }

    fn agent_file(&self) -> PathBuf {
        self.root.join(r"home\.gemini\config\hooks.json")
    }

    fn shortcut(&self) -> PathBuf {
        self.root.join(format!(
            r"roaming\Microsoft\Windows\Start Menu\Programs\{}.lnk",
            self.stem
        ))
    }

    /// `program` in this profile only: no inherited Orchbee variable, no Agent program on `PATH`.
    fn command(&self, program: &Path) -> Command {
        let system = system_folder();
        let mut command = crate::inherited_environment::orchbee_command(program);
        command
            .env(
                "PATH",
                format!("{};{}", self.root.join("bin").display(), system.display()),
            )
            .env("USERPROFILE", self.root.join("home"))
            .env("HOME", self.root.join("home"))
            .env("APPDATA", self.root.join("roaming"))
            .env("LOCALAPPDATA", self.root.join("local"))
            .env("TEMP", self.root.join("tmp"))
            .env("TMP", self.root.join("tmp"))
            .env("ORCHBEE_HOME", self.data())
            .env("ORCHBEE_INSTALL_DIR", self.store())
            .env("ORCHBEE_TEST_REGISTRY_KEY", &self.key)
            .env("LC_ALL", "en-US")
            .current_dir(&self.root)
            .stdin(Stdio::null());
        command
    }

    fn run(&self, program: &Path, arguments: &[&str]) -> (Output, Value) {
        let output = self.command(program).args(arguments).output().unwrap();
        let document = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
        (output, document)
    }

    fn reg(&self, arguments: &[&str]) -> Output {
        Command::new(system_folder().join("reg.exe"))
            .args(arguments)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    /// The scratch `Path` value's registry type and text; `None` when the value is absent.
    fn path_value(&self) -> Option<(String, String)> {
        let output = self.reg(&[
            "query",
            &format!(r"HKCU\{}\Environment", self.key),
            "/v",
            "Path",
        ]);
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        if !output.status.success() {
            return None;
        }
        text.lines().find_map(|line| {
            let rest = line.trim_start().strip_prefix("Path")?.trim_start();
            let (kind, value) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            kind.starts_with("REG_")
                .then(|| (kind.to_owned(), value.trim().to_owned()))
        })
    }

    fn set_path(&self, kind: &str, value: &str) {
        let environment = format!(r"HKCU\{}\Environment", self.key);
        let arguments = [
            "add",
            environment.as_str(),
            "/v",
            "Path",
            "/t",
            kind,
            "/d",
            value,
            "/f",
        ];
        assert!(self.reg(&arguments).status.success());
    }

    fn registered(&self) -> bool {
        self.reg(&[
            "query",
            &format!(r"HKCU\{}\Uninstall\com.{}.desktop_is1", self.key, self.stem),
        ])
        .status
        .success()
    }

    /// Installs the package and stands in for the PATH step's launcher in `bin`, which it returns.
    fn install(&self) -> PathBuf {
        let (output, install) =
            self.run(&self.client, &["install", "--no-path", "--format", "json"]);
        assert_eq!(output.status.code(), Some(0), "{}", describe(&output));
        assert_eq!(install["result"]["activated"], true);
        let release = PathBuf::from(install["result"]["releasePath"].as_str().unwrap());
        let launcher = self.store().join("bin").join(format!("{}.exe", self.stem));
        fs::create_dir_all(launcher.parent().unwrap()).unwrap();
        fs::copy(
            release.join(format!("{}-launcher.exe", self.stem)),
            &launcher,
        )
        .unwrap();
        launcher
    }

    /// Installs the package, then stands in for the installer and the PATH step: the launcher in
    /// `bin`, the `Path` entry, the registration, the shortcut and a `desktop` folder. Enables one
    /// Agent integration and starts the Session.
    fn prepare(&self) -> PathBuf {
        let launcher = self.install();
        fs::create_dir_all(self.store().join("desktop")).unwrap();
        fs::write(self.store().join(r"desktop\placeholder.txt"), b"desktop").unwrap();
        let path = format!(
            "{};{};{}",
            OTHER_ENTRIES[0],
            self.store().join("bin").display(),
            OTHER_ENTRIES[1]
        );
        self.set_path("REG_EXPAND_SZ", &path);
        let registration = format!(r"HKCU\{}\Uninstall\com.{}.desktop_is1", self.key, self.stem);
        assert!(
            self.reg(&[
                "add",
                registration.as_str(),
                "/v",
                "DisplayName",
                "/d",
                "scratch",
                "/f",
            ])
            .status
            .success()
        );
        fs::create_dir_all(self.shortcut().parent().unwrap()).unwrap();
        fs::write(self.shortcut(), b"shortcut").unwrap();
        // The Agent's own file exists before the enable; its program is a stub on this PATH.
        fs::write(self.root.join(r"bin\agy.cmd"), b"@exit /b 99\r\n").unwrap();
        fs::create_dir_all(self.agent_file().parent().unwrap()).unwrap();
        fs::write(self.agent_file(), AGENT_FILE).unwrap();
        let (output, enable) = self.run(
            &launcher,
            &[
                "agent",
                "integration",
                "enable",
                "--provider",
                "antigravity",
                "--format",
                "json",
            ],
        );
        assert_eq!(output.status.code(), Some(0), "{}", describe(&output));
        assert_eq!(enable["result"]["outcome"], "completed", "{enable}");
        assert_ne!(fs::read_to_string(self.agent_file()).unwrap(), AGENT_FILE);
        let (output, _) = self.run(&launcher, &["server", "start", "--format", "json"]);
        assert_eq!(output.status.code(), Some(0), "{}", describe(&output));
        fs::write(self.data().join("config.toml"), b"# kept\n").unwrap();
        launcher
    }

    fn uninstall_once(&self, delete_data: bool) {
        let launcher = self.prepare();
        // With all data deleted, an item Orchbee did not write stays in each root, as the platform
        // uninstaller's own file does in the store.
        let other: &[&str] = if delete_data { &["unins000.dat"] } else { &[] };
        for root in [self.store(), self.data()] {
            for name in other {
                fs::write(root.join(name), b"not Orchbee's").unwrap();
            }
        }
        let mut arguments = vec!["uninstall", "--confirm", "--format", "json"];
        if delete_data {
            arguments.push("--delete-data");
        }
        let started = Instant::now();
        let (output, document) = self.run(&launcher, &arguments);
        assert_eq!(output.status.code(), Some(0), "{}", describe(&output));
        let schema: Value =
            serde_json::from_str(include_str!("../src/cli/schemas/orchbee.uninstall.json"))
                .unwrap();
        assert!(
            jsonschema::validator_for(&schema)
                .unwrap()
                .is_valid(&document),
            "{document}"
        );
        let result = &document["result"];
        assert_eq!(result["storeRoot"], self.store().display().to_string());
        assert_eq!(result["dataRoot"], self.data().display().to_string());
        assert_eq!(result["error"], Value::Null, "{document}");
        assert_eq!(result["store"], "scheduled", "{document}");
        let sessions = result["sessions"].as_array().unwrap();
        assert!(
            !sessions.is_empty()
                && sessions
                    .iter()
                    .all(|session| session["result"] == "stopped")
        );
        let antigravity = result["integrations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|line| line["provider"] == "antigravity")
            .unwrap();
        assert_eq!(antigravity["result"], "disabled", "{document}");
        assert_eq!(result["pathEntry"]["result"], "removed");
        assert!(
            result["installerEntries"]
                .as_array()
                .unwrap()
                .iter()
                .all(|entry| entry["result"] == "removed")
        );
        let kept: Vec<Value> = if delete_data {
            [self.store(), self.data()]
                .iter()
                .map(|root| serde_json::json!({"path": root.display().to_string(), "items": 1}))
                .collect()
        } else {
            Vec::new()
        };
        assert_eq!(result["kept"], Value::Array(kept), "{document}");
        // Orchbee's program files go once the Client and the launcher have exited; the folder goes
        // with them unless it keeps another item.
        let expected = (!other.is_empty()).then(|| names(other));
        while entries(&self.store()) != expected {
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "the program folder holds {:?}",
                entries(&self.store())
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        assert_eq!(fs::read_to_string(self.agent_file()).unwrap(), AGENT_FILE);
        assert_eq!(
            self.path_value(),
            Some(("REG_EXPAND_SZ".to_owned(), OTHER_ENTRIES.join(";")))
        );
        assert!(!self.registered());
        assert!(!self.shortcut().exists());
        if delete_data {
            assert_eq!(entries(&self.data()), Some(names(other)));
        } else {
            for removed in [
                "logs",
                r"internal\integrations",
                r"internal\cache",
                r"internal\runtime",
                r"internal\tmp",
            ] {
                assert!(!self.data().join(removed).exists(), "{removed}");
            }
            assert!(self.data().join("config.toml").is_file());
            assert!(self.data().join("sessions").is_dir());
        }
        // A second run finds nothing left and changes nothing.
        let (output, again) = self.run(&self.client, &arguments);
        assert_eq!(output.status.code(), Some(0), "{}", describe(&output));
        assert_eq!(again["result"]["store"], "absent", "{again}");
        assert_eq!(again["result"]["pathEntry"]["result"], "absent", "{again}");
    }

    fn remove_the_only_entry(&self, kind: &str) {
        let launcher = self.install();
        self.set_path(kind, &self.store().join("bin").display().to_string());
        let started = Instant::now();
        let output = self
            .command(&launcher)
            .arg("--installer-uninstall-v1")
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{kind}: {}",
            describe(&output)
        );
        assert_eq!(
            self.path_value(),
            Some((kind.to_owned(), String::new())),
            "{kind}"
        );
        while entries(&self.store()).is_some() {
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "the program folder holds {:?}",
                entries(&self.store())
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Stops a Session a failed run left, then deletes the scratch key and the profile.
    fn release(&self) {
        let _ = self.run(
            &self.client,
            &["server", "stop", "--confirm", "--format", "json"],
        );
        let _ = self.reg(&["delete", &format!(r"HKCU\{}", self.key), "/f"]);
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The names in `folder`, sorted; `None` when it is gone.
fn entries(folder: &Path) -> Option<Vec<String>> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .ok()?
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    Some(names)
}

fn names(list: &[&str]) -> Vec<String> {
    let mut names: Vec<String> = list.iter().map(|name| (*name).to_owned()).collect();
    names.sort();
    names
}

fn system_folder() -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32")
}

fn describe(output: &Output) -> String {
    format!(
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}
