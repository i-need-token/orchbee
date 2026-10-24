//! Installer boundary with synthetic cohort peers and a real compiled version probe.
//! This proves staging/publication, not Server/Worker/Host behavior or user PATH activation.
use super::*;
use crate::cli::source::ArmedSignals;
use orchbee_local_site_runtime::{WindowsPrivateCache, WindowsUpdateChannel};
use serde_json::json;
use std::{
    fs,
    time::{Duration, Instant},
};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let actual = self.0.canonicalize().unwrap();
        assert_eq!(
            actual.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            actual
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-release-test-")
        );
        fs::remove_dir_all(actual).unwrap();
    }
}
fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target.join(entry.file_name()));
        } else {
            fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
        }
    }
}
fn inventory(directory: &Path, base: &Path, files: &mut Vec<serde_json::Value>) {
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            inventory(&entry.path(), base, files);
        } else {
            let bytes = fs::read(entry.path()).unwrap();
            let path = entry
                .path()
                .strip_prefix(base)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/");
            files.push(json!({"path":path,"url":path,"size":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes))}));
        }
    }
}
fn report() -> Report {
    Report {
        phase: "download",
        channel_saved: false,
        config_path: None,
        published: None,
        reused: false,
        activated: false,
        cleanup_pending: None,
        version: None,
        build_id: None,
    }
}
#[test]
fn offline_install_verifies_before_probe_reuses_without_overwrite_and_cleans_failures() {
    let fixture = Fixture(std::env::temp_dir().join(
        format!("orchbee-release-test-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()),
    ));
    let lock = WindowsPrivateCache::acquire(&fixture.0).unwrap();
    let source = fixture.0.join("source");
    fs::create_dir(&source).unwrap();
    let probe = fixture.0.join("probe.rs");
    fs::write(&probe, "fn main() { println!(\"orchbee 0.1.0\"); }").unwrap();
    let mut command = Command::new("rustc");
    command
        .arg("--crate-name")
        .arg("release_probe")
        .arg(&probe)
        .arg("-o")
        .arg(source.join("orchbee.exe"));
    let output = run_windows_command(
        command,
        Some(Instant::now() + Duration::from_secs(60)),
        || Ok(false),
    );
    assert_eq!(
        output.failure,
        None,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // A compiler-emitted PDB is deliberately part of this fixture's manifest.
    let mut artifacts = serde_json::Map::new();
    for name in [
        "orchbee",
        "orchbee-server",
        "orchbee-worker",
        "orchbee-terminal-host",
    ] {
        let path = source.join(format!("{name}.exe"));
        if name != "orchbee" {
            fs::write(&path, b"synthetic peer; never executed").unwrap();
        }
        artifacts.insert(
            name.into(),
            json!(format!("{:x}", Sha256::digest(fs::read(path).unwrap()))),
        );
    }
    let adapters = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../desktop/resources/agent-observation-adapters");
    copy_tree(&adapters, &source.join("agent-observation-adapters"));
    let capsule: serde_json::Value = serde_json::from_slice(
        &fs::read(source.join("agent-observation-adapters/capsule.json")).unwrap(),
    )
    .unwrap();
    artifacts.insert(
        "agent-observation-adapters".into(),
        capsule["payloadSha256"].clone(),
    );
    fs::write(
        source.join("cohort.json"),
        json!({"schemaVersion":1,"artifacts":artifacts}).to_string(),
    )
    .unwrap();
    fs::create_dir(source.join("conpty")).unwrap();
    for name in [
        "conpty.dll",
        "OpenConsole.exe",
        "Microsoft.ConPTY-LICENSE.txt",
    ] {
        fs::write(
            source.join("conpty").join(name),
            b"synthetic ConPTY distribution; never loaded",
        )
        .unwrap();
    }
    let mut files = Vec::new();
    inventory(&source, &source, &mut files);
    let bytes = serde_json::to_vec(
        &json!({"schemaVersion":1,"product":"orchbee","platform":"windows-x86_64",
        "channel":"stable","version":"0.1.0","buildId":"test","files":files}),
    )
    .unwrap();
    let mut release = Release::parse(&bytes, WindowsUpdateChannel::Stable).unwrap();
    let source_ref = Source::Local(source.join("release.json"));
    let mut context = Context {
        signals: ArmedSignals::arm_interrupts().unwrap(),
        deadline: Instant::now() + Duration::from_secs(60),
        curl_checked: false,
    };
    let root = fixture.0.join("install");
    let mut first = report();
    let published = publish(&root, &source_ref, &release, &mut context, &mut first)
        .unwrap_or_else(|error| panic!("{}", error.code()));
    assert_eq!(first.published, Some(published.clone()));
    assert!(!first.reused && first.cleanup_pending.is_none());
    let mut second = report();
    assert_eq!(
        publish(&root, &source_ref, &release, &mut context, &mut second)
            .unwrap_or_else(|error| panic!("{}", error.code())),
        published
    );
    assert!(second.reused);
    let admitted = NativeCohort::load(&published, context.deadline).unwrap();
    let installed = crate::cli::update::inspect_installation(
        &published.join("orchbee.exe"),
        Some(&admitted),
        "0.1.0",
    );
    assert_eq!(installed.delivery(), "native-release");
    assert_eq!(installed.channel(), Some("stable"));
    assert_eq!(installed.json()["buildId"], "test");
    assert_eq!(
        crate::cli::update::inspect_installation(&published.join("orchbee.exe"), None, "0.1.0")
            .delivery(),
        "unknown"
    );
    assert_eq!(
        crate::cli::update::inspect_installation(
            &published.join("orchbee.exe"),
            Some(&admitted),
            "9.0.0"
        )
        .delivery(),
        "unknown"
    );

    let cohort_before = fs::read(published.join("cohort.json")).unwrap();
    let peer_before = fs::read(published.join("orchbee-server.exe")).unwrap();
    fs::write(published.join("orchbee-server.exe"), b"another cohort peer").unwrap();
    let mut different: serde_json::Value = serde_json::from_slice(&cohort_before).unwrap();
    different["artifacts"]["orchbee-server"] =
        json!(format!("{:x}", Sha256::digest(b"another cohort peer")));
    fs::write(published.join("cohort.json"), different.to_string()).unwrap();
    let stale = NativeCohort::load(&published, context.deadline).unwrap();
    fs::write(published.join("cohort.json"), cohort_before).unwrap();
    fs::write(published.join("orchbee-server.exe"), peer_before).unwrap();
    assert_eq!(
        crate::cli::update::inspect_installation(
            &published.join("orchbee.exe"),
            Some(&stale),
            "0.1.0"
        )
        .delivery(),
        "unknown"
    );
    let executable = fs::read(published.join("orchbee.exe")).unwrap();
    release.identity = "b".repeat(64);
    release.files[0].sha256 = "0".repeat(64);
    let mut failed = report();
    assert!(matches!(
        publish(&root, &source_ref, &release, &mut context, &mut failed),
        Err(Error::Checksum)
    ));
    assert!(failed.published.is_none() && failed.cleanup_pending.is_none());
    assert_eq!(fs::read(published.join("orchbee.exe")).unwrap(), executable);
    assert!(!fs::read_dir(&root).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".stage-")
    }));
    fs::write(published.join("release.json"), b"corrupt metadata").unwrap();
    assert_eq!(
        crate::cli::update::inspect_installation(
            &published.join("orchbee.exe"),
            Some(&admitted),
            "0.1.0"
        )
        .delivery(),
        "unknown"
    );
    drop(context);
    drop(lock);
}
