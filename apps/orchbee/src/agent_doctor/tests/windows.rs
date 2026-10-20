use super::super::{Budget, fs, probes};
use std::{ffi::OsStr, path::Path};

#[test]
fn all_windows_provider_rows_fit_v6_without_widening_legacy_versions() {
    use serde_json::{Value, json};
    let run = super::super::DoctorRun {
        providers: probes::PROVIDERS
            .iter()
            .map(|provider| {
                json!({
                    "id": crate::agent_integrations::IntegrationId::parse(provider.id).unwrap().provider(), "executable":{"state":"absent"},
                    "runtime":{"node":{"state":"absent"}},
                    "hook":{"state":"unknown","reason":"unreadable","detail":{}}
                })
            })
            .collect(),
        roots: vec![],
        endpoint: json!({"socket":{"state":"not-inspected","reason":"windows-endpoint-not-inspected"}}),
        environment: json!({"observationSocketEnv":{"state":"unset"}}),
        complete: true,
    };
    assert_eq!(run.providers.len(), 20);
    let output = super::super::emit(
        run,
        crate::invocation::OutputFormat::Json,
        crate::localization::Locale::EnUs,
    );
    let mut document: Value = serde_json::from_slice(&output.stdout).unwrap();
    let definition: Value =
        serde_json::from_str(include_str!("../../cli/schemas/orchbee.agent.doctor.json")).unwrap();
    let validator = jsonschema::validator_for(&definition).unwrap();
    validator.validate(&document).unwrap();
    document["version"] = 3.into();
    assert!(!validator.is_valid(&document));
    document["result"]["providers"]
        .as_array_mut()
        .unwrap()
        .truncate(14);
    validator.validate(&document).unwrap();
    document["version"] = 4.into();
    let row = document["result"]["providers"][0].clone();
    document["result"]["providers"] = json!(vec![row; 21]);
    assert!(!validator.is_valid(&document));
}

#[test]
fn windows_paths_admit_drive_and_unc_but_refuse_devices_and_relative_names() {
    for path in [
        r"C:\Users\中文 user\config.toml",
        r"\\host\share\capsule",
        r"\\?\C:\assets",
    ] {
        assert!(fs::path_admitted(Path::new(path)), "{path}");
    }
    for path in [
        "",
        r"relative\config",
        r"C:config",
        r"\config",
        r"\\.\pipe\name",
        r"\\server\pipe\name",
        r"\\?\GLOBALROOT\Device\x",
        r"C:\tmp\NUL.txt",
        r"C:\tmp\a:stream",
    ] {
        assert!(!fs::path_admitted(Path::new(path)), "{path}");
    }
    assert!(matches!(
        fs::read_regular(Path::new(r"\\.\pipe\unused"), 10, &mut Budget::new()),
        fs::FileRead::UnsafeKind
    ));
}

#[test]
fn windows_reads_obey_caps_and_do_not_follow_junctions() {
    use std::{fs as disk, os::windows::process::CommandExt, process::Command};
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let target = disk::canonicalize(&self.0).unwrap();
            assert!(target.starts_with(disk::canonicalize(std::env::temp_dir()).unwrap()));
            assert!(
                self.0
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("orchbee-doctor-fs-")
            );
            disk::remove_dir_all(target).unwrap();
        }
    }
    let root = Scratch(std::env::temp_dir().join(format!(
            "orchbee-doctor-fs-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    disk::create_dir(&root.0).unwrap();
    let file = root.0.join("literal 中文.txt");
    disk::write(&file, b"abcd").unwrap();
    assert_eq!(
        fs::read_regular(&file, 4, &mut Budget::new()),
        fs::FileRead::Bytes(b"abcd".to_vec())
    );
    assert_eq!(
        fs::read_regular(&file, 3, &mut Budget::new()),
        fs::FileRead::OverLimit
    );
    let mut budget = Budget::new();
    budget.consume(super::super::MAX_TOTAL_READ_BYTES - 3);
    assert_eq!(
        fs::read_regular(&file, 10, &mut budget),
        fs::FileRead::BudgetExhausted
    );
    assert_eq!(budget.remaining_bytes(), 3);
    assert_eq!(
        fs::read_regular(&root.0, 100, &mut Budget::new()),
        fs::FileRead::UnsafeKind
    );
    let source = root.0.join("source");
    disk::create_dir(&source).unwrap();
    disk::write(source.join("keep"), b"preserve").unwrap();
    let link = root.0.join("junction");
    let powershell = Path::new(&std::env::var_os("SystemRoot").unwrap())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let output = Command::new(powershell).args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:DOCTOR_TEST_LINK -Target $env:DOCTOR_TEST_SOURCE -ErrorAction Stop | Out-Null"])
        .env("DOCTOR_TEST_LINK", &link).env("DOCTOR_TEST_SOURCE", &source).creation_flags(0x0800_0000).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::lstat(&link), fs::Lstat::Symlink);
    assert_eq!(
        fs::read_regular(&link, 100, &mut Budget::new()),
        fs::FileRead::Symlink
    );
    assert!(matches!(
        fs::capsule_walk(&link, &mut Budget::new()),
        fs::CapsuleWalk::Symlink
    ));
    assert!(matches!(
        fs::capsule_walk(&root.0, &mut Budget::new()),
        fs::CapsuleWalk::UnsafeKind
    ));
    disk::remove_dir(&link).unwrap();
    assert_eq!(disk::read(source.join("keep")).unwrap(), b"preserve");
}

#[test]
fn windows_codex_literal_paths_and_file_urls_preserve_the_native_root() {
    let scan = probes::scan_codex_config(
        br#"[marketplaces.orchbee-local]
source_type = 'local'
source = 'C:\Users\a b\capsule'
[plugins."orchbee-agent-observation@orchbee-local"]
enabled = true
"#,
    );
    assert_eq!(scan.source.as_deref(), Some(r"C:\Users\a b\capsule"));
    assert_eq!(scan.source_type.as_deref(), Some("local"));
    assert!(
        matches!(probes::derive_module_root("file:///C:/a%20b/bundle/opencode/orchbee-agent-observation.mjs", "/bundle/opencode/orchbee-agent-observation.mjs"), probes::ModuleUrlRoot::Root(root) if root == b"C:/a b")
    );
    assert!(
        fs::scan_path_for("../codex", OsStr::new(r"C:\bin"))
            .found
            .is_none()
    );
}
