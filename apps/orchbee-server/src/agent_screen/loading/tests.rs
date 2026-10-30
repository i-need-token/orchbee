use super::*;
use crate::agent_screen::ScreenInput;
use orchbee_agent_observation_protocol::AgentReportProvider as Provider;
use std::{fs, path::PathBuf};

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().join(format!(
            "orchbee-agent-rules-{:032x}",
            u128::from_le_bytes(nonce)
        ));
        fs::create_dir(&path).unwrap();
        fs::create_dir_all(path.join("agent-detection/remote")).unwrap();
        Self(path)
    }
    fn local(&self) -> PathBuf {
        self.0.join("agent-detection").join("codex.toml")
    }
    fn remote(&self) -> PathBuf {
        self.0
            .join("agent-detection")
            .join("remote")
            .join("codex.toml")
    }
    fn explain(&self) -> orchbee_workbench_protocol::AgentScreenExplanation {
        ScreenDetector::load(Some(&self.0), RemoteCache::Admitted)
            .unwrap()
            .explain(Provider::Codex, input())
            .unwrap()
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let path = self.0.canonicalize().unwrap();
        assert!(path.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-agent-rules-")
        );
        fs::remove_dir_all(path).unwrap();
    }
}
fn input() -> ScreenInput<'static> {
    ScreenInput {
        screen: "local-ready remote-ready",
        title: "",
        progress: "",
    }
}
fn manifest(version: &str, id: &str, state: &str) -> String {
    format!(
        "id='codex'\nversion='{version}'\nmin_engine_version=3\n[[rules]]\nid='{id}'\ncontains=['{id}-ready']\nstate='{state}'\n"
    )
}
#[test]
fn installed_rules_prioritize_local_then_valid_remote_and_leave_old_snapshot_immutable() {
    let root = Root::new();
    let initial = root.explain();
    assert_eq!(initial.manifest.as_ref().unwrap().source, Source::Bundled);
    let bundled_version = initial.manifest_version.unwrap();
    fs::write(
        root.remote(),
        manifest(&bundled_version, "remote", "working"),
    )
    .unwrap();
    let remote = root.explain();
    assert_eq!(
        remote.manifest.as_ref().unwrap().source,
        Source::RemoteCache,
        "equal remote version is admissible"
    );
    assert_eq!(remote.matched_rule.unwrap().id, "remote");
    // Without a configured catalog the same cache is neither selected nor reported.
    let ignored = ScreenDetector::load(Some(&root.0), RemoteCache::Ignored)
        .unwrap()
        .explain(Provider::Codex, input())
        .unwrap();
    let info = ignored.manifest.unwrap();
    assert_eq!(info.source, Source::Bundled);
    assert!(info.cached_remote_version.is_none() && info.warnings.is_empty());
    fs::write(root.local(), manifest("0", "local", "blocked")).unwrap();
    let installed = ScreenDetector::load(Some(&root.0), RemoteCache::Admitted).unwrap();
    let local = installed.explain(Provider::Codex, input()).unwrap();
    assert_eq!(local.matched_rule.unwrap().id, "local");
    let info = local.manifest.unwrap();
    assert_eq!(info.source, Source::LocalOverride);
    assert_eq!(info.path.as_deref(), root.local().to_str());
    assert_eq!(info.cached_remote_version, Some(bundled_version));
    assert!(info.local_override_shadowing_remote);
    fs::write(root.local(), manifest("1", "local", "working")).unwrap();
    assert_eq!(
        installed.explain(Provider::Codex, input()).unwrap().state,
        orchbee_workbench_protocol::PanelAgentState::Blocked
    );
    assert_eq!(
        root.explain().state,
        orchbee_workbench_protocol::PanelAgentState::Working
    );
}
#[test]
fn invalid_inputs_warn_fall_back_and_do_not_invent_cached_versions() {
    let root = Root::new();
    fs::write(root.remote(), manifest("9999.1", "remote", "working")).unwrap();
    for (bytes, reason) in [
        (b"not toml".to_vec(), Reason::InvalidManifest),
        (vec![b'x'; 256 * 1024 + 1], Reason::TooLarge),
        (vec![255], Reason::InvalidUtf8),
        (
            manifest("1", "local", "blocked")
                .replace("id='codex'", "id='claude'")
                .into_bytes(),
            Reason::InvalidManifest,
        ),
    ] {
        fs::write(root.local(), bytes).unwrap();
        let result = root.explain();
        let info = result.manifest.unwrap();
        assert_eq!(info.source, Source::RemoteCache);
        assert_eq!(
            info.warnings,
            vec![Warning {
                source: Source::LocalOverride,
                kind: reason
            }]
        );
        assert!(info.local_override_shadowing_remote);
        assert_eq!(result.matched_rule.unwrap().id, "remote");
    }
    fs::remove_file(root.local()).unwrap();
    fs::write(root.remote(), manifest("0", "remote", "working")).unwrap();
    let info = root.explain().manifest.unwrap();
    assert_eq!(info.source, Source::Bundled);
    assert_eq!(info.cached_remote_version.as_deref(), Some("0"));
    assert_eq!(info.warnings[0].kind, Reason::OlderThanBundled);
    fs::write(root.remote(), manifest("invalid", "remote", "working")).unwrap();
    let info = root.explain().manifest.unwrap();
    assert_eq!(info.source, Source::Bundled);
    assert!(info.cached_remote_version.is_none());
    assert_eq!(info.warnings[0].kind, Reason::InvalidManifest);
    fs::write(
        root.remote(),
        manifest("9999.1", "remote", "working").replace("min_engine_version=3\n", ""),
    )
    .unwrap();
    assert_eq!(root.explain().manifest.unwrap().source, Source::Bundled);
    fs::write(
        root.remote(),
        manifest("9999.1", "remote", "working").replace("version='9999.1'\n", ""),
    )
    .unwrap();
    assert_eq!(
        root.explain().manifest.unwrap().warnings[0].kind,
        Reason::InvalidVersion
    );
    fs::write(
        root.local(),
        manifest("1", "local", "blocked").replace(
            "id='codex'",
            "id='team-rules'\naliases=[' C:\\\\tools\\\\CODEX.EXE ']",
        ),
    )
    .unwrap();
    assert_eq!(
        root.explain().manifest.unwrap().source,
        Source::LocalOverride
    );
    fs::write(root.local(), manifest("invalid", "local", "blocked")).unwrap();
    assert_eq!(root.explain().manifest.unwrap().source, Source::Bundled);
}
#[test]
fn numeric_versions_match_reference_ordering_and_validate_entire_input() {
    for (a, b) in [("1", "1.0.0"), ("01.02", "1.2"), ("0", "0.0")] {
        assert!(Version::parse(a) == Version::parse(b));
    }
    assert!(Version::parse("1.10") > Version::parse("1.2"));
    assert!(Version::parse("2") > Version::parse("1.99"));
    for value in [
        "",
        ".1",
        "1.",
        "1..2",
        "-1",
        "1.beta",
        "18446744073709551616",
    ] {
        assert!(Version::parse(value).is_none(), "{value}");
    }
    assert!(Version::parse(&"1".repeat(129)).is_none());
}
