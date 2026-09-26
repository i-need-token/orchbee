//! `agent doctor` unit and fixture coverage: pure classification mappings,
//! bounded-IO adapter outcomes, and the orchestration fold — real
//! filesystem only inside each test's scratch directory.

use super::*;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;

fn scratch(tag: &str) -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "obcli-agent-doctor-{}-{tag}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).expect("create scratch base");
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
        .expect("chmod scratch base");
    directory
}

fn provider(id: &str) -> &'static Provider {
    probes::PROVIDERS
        .iter()
        .find(|provider| provider.id == id)
        .expect("canonical provider id")
}

/// The closed `--provider` grammar vocabulary and the probe table agree in
/// order and content — completion, validation, and row emission share one
/// canonical ordering.
#[test]
fn provider_ids_match_the_probe_table() {
    let table: Vec<&str> = probes::PROVIDERS.iter().map(|p| p.id).collect();
    assert_eq!(table, PROVIDER_IDS);
}

// ---------------------------------------------------------------------------
// moduleUrl → recorded root (C-1 hardening, E-4 derivation).
// ---------------------------------------------------------------------------

#[test]
fn module_url_root_derivation() {
    let suffix = "/extensions/opencode/dist/index.mjs";
    // Happy path: raw bytes preserved, percent-decoded once.
    let ModuleUrlRoot::Root(root) = probes::derive_module_root(
        "file:///home/u/.cache/cap%20sule/extensions/opencode/dist/index.mjs",
        suffix,
    ) else {
        panic!("valid file URL must derive")
    };
    assert_eq!(root, b"/home/u/.cache/cap sule");
    // Non-UTF8 decoded bytes stay raw — never lossy.
    let ModuleUrlRoot::Root(root) =
        probes::derive_module_root("file:///r%FFx/extensions/opencode/dist/index.mjs", suffix)
    else {
        panic!("non-UTF8 bytes still derive")
    };
    assert_eq!(root, b"/r\xffx");
    // Non-file schemes and any authority are not candidates.
    for url in [
        "https://h/extensions/opencode/dist/index.mjs",
        "file://localhost/x/extensions/opencode/dist/index.mjs",
        "file://h/x/extensions/opencode/dist/index.mjs",
        "file:x/extensions/opencode/dist/index.mjs",
        "FILE:///x/extensions/opencode/dist/index.mjs",
    ] {
        assert_eq!(
            probes::derive_module_root(url, suffix),
            ModuleUrlRoot::NonFile,
            "{url}"
        );
    }
    // Query, fragment, malformed escapes, and decoded NUL bytes are
    // invalid — never a candidate. (A decoded non-absolute path is
    // unreachable: `file:///…` already forces a leading `/`.)
    for url in [
        "file:///x/extensions/opencode/dist/index.mjs?y=1",
        "file:///x/extensions/opencode/dist/index.mjs#f",
        "file:///x/extensions/opencode/dist/index.mjs%2",
        "file:///x/extensions/opencode/dist/index.mjs%zz",
        "file:///%00/extensions/opencode/dist/index.mjs",
        "file:///x/extensions/opencode/dist/index.mjs%",
    ] {
        assert_eq!(
            probes::derive_module_root(url, suffix),
            ModuleUrlRoot::Invalid,
            "{url}"
        );
    }
    // Wrong or rootless suffix contributes nothing — no ancestor guessing.
    for url in [
        "file:///x/extensions/other/dist/index.mjs",
        "file:///extensions/opencode/dist/index.mjs",
        "file:///x/extensions/opencode/dist/index.mjs/extra",
    ] {
        assert_eq!(
            probes::derive_module_root(url, suffix),
            ModuleUrlRoot::SuffixUnrecognized,
            "{url}"
        );
    }
    // Failure reasons carry their typed spellings for detail rows.
    assert_eq!(
        ModuleUrlRoot::NonFile.failure_reason(),
        Some("module-url-non-file")
    );
    assert_eq!(
        ModuleUrlRoot::Invalid.failure_reason(),
        Some("module-url-invalid")
    );
    assert_eq!(
        ModuleUrlRoot::SuffixUnrecognized.failure_reason(),
        Some("module-url-suffix-unrecognized")
    );
    assert_eq!(
        ModuleUrlRoot::Root(vec![]).failure_reason(),
        None,
        "a derived root has no failure reason"
    );
}

// ---------------------------------------------------------------------------
// Bounded JSON (C-2 structural limits).
// ---------------------------------------------------------------------------

#[test]
fn bounded_json_enforces_depth_and_member_caps() {
    // Values sit at depth ≤32 (the root is depth 1): 31 nested arrays put
    // the leaf at 32 and admit; 32 put it at 33 and refuse as malformed —
    // never truncated.
    let nested = |depth: usize| {
        let mut text = String::from("1");
        for _ in 0..depth {
            text = format!("[{text}]");
        }
        text
    };
    assert!(probes::bounded_json(nested(31).as_bytes()).is_some());
    assert!(probes::bounded_json(nested(32).as_bytes()).is_none());
    // 256 object members admit; 257 refuse.
    let members = |count: usize| {
        format!(
            "{{{}}}",
            (0..count)
                .map(|index| format!("\"k{index}\":0"))
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    assert!(probes::bounded_json(members(256).as_bytes()).is_some());
    assert!(probes::bounded_json(members(257).as_bytes()).is_none());
    // Arrays count members identically.
    let array = |count: usize| format!("[{}]", "0,".repeat(count).trim_end_matches(','));
    assert!(probes::bounded_json(array(256).as_bytes()).is_some());
    assert!(probes::bounded_json(array(257).as_bytes()).is_none());
    // Malformed bytes are not documents.
    assert!(probes::bounded_json(b"{").is_none());
    assert!(probes::bounded_json(b"\xff\xfe").is_none());
}

// ---------------------------------------------------------------------------
// Codex config scan — the C-3 ladder's scanner half.
// ---------------------------------------------------------------------------

/// The real codex section spellings — the scanner keys on these literal
/// headers exactly like the owning adapter.
const PLUGIN_SECTION: &[u8] = br#"[plugins."orchbee-agent-observation@orchbee-local"]"#;
const MARKETPLACE_SECTION: &[u8] = b"[marketplaces.orchbee-local]";

fn codex_config(plugin: Option<&[u8]>, marketplace: Option<&[u8]>) -> Vec<u8> {
    let mut bytes = Vec::new();
    if let Some(body) = marketplace {
        bytes.extend_from_slice(MARKETPLACE_SECTION);
        bytes.push(b'\n');
        bytes.extend_from_slice(body);
    }
    if let Some(body) = plugin {
        bytes.extend_from_slice(PLUGIN_SECTION);
        bytes.push(b'\n');
        bytes.extend_from_slice(body);
    }
    bytes
}

#[test]
fn codex_scan_fields() {
    // Well-formed local+absolute marketplace with explicit enablement.
    let scan = probes::scan_codex_config(&codex_config(
        Some(b"enabled = true\n"),
        Some(b"source_type = \"local\"\nsource = \"/opt/capsule\"\n"),
    ));
    assert!(scan.plugin_present && scan.marketplace_present);
    assert_eq!(scan.enabled, Some(Some(true)));
    assert_eq!(scan.source_type.as_deref(), Some("local"));
    assert_eq!(scan.source.as_deref(), Some("/opt/capsule"));
    assert!(!scan.duplicate_key && !scan.duplicate_section);
    // `enabled` present but not bool-parseable → `Some(None)` (the
    // `unknown/malformed` signal, never `conflict` by itself).
    let scan = probes::scan_codex_config(&codex_config(
        Some(b"enabled = \"yes\"\n"),
        Some(b"source_type = \"local\"\nsource = \"/opt/capsule\"\n"),
    ));
    assert_eq!(scan.enabled, Some(None));
    // Key absent → `None` (the `enabledFlag:"absent"` projection).
    let scan = probes::scan_codex_config(&codex_config(
        Some(b""),
        Some(b"source_type = \"local\"\nsource = \"/opt/capsule\"\n"),
    ));
    assert_eq!(scan.enabled, None);
    // Duplicate key and duplicate section are separate contradiction flags.
    let scan = probes::scan_codex_config(&codex_config(
        Some(b"enabled = true\nenabled = false\n"),
        None,
    ));
    assert!(scan.duplicate_key);
    // Two empty plugin sections: no key repeats, the section count alone
    // is the contradiction.
    let mut doubled = codex_config(Some(b""), None);
    doubled.extend_from_slice(&codex_config(Some(b""), None));
    let scan = probes::scan_codex_config(&doubled);
    assert!(scan.duplicate_section && !scan.duplicate_key);
    // Non-local source type and non-absolute source are recorded as facts;
    // classification decides they are contradictions.
    let scan = probes::scan_codex_config(&codex_config(
        Some(b""),
        Some(b"source_type = \"github\"\nsource = \"x/y\"\n"),
    ));
    assert_eq!(scan.source_type.as_deref(), Some("github"));
    assert_eq!(scan.source.as_deref(), Some("x/y"));
    let scan = probes::scan_codex_config(&codex_config(
        Some(b""),
        Some(b"source_type = \"local\"\nsource = \"relative/path\"\n"),
    ));
    assert_eq!(scan.source.as_deref(), Some("relative/path"));
    // Marketplace-only (no plugin section at all) parses for
    // classification as `absent`.
    let scan = probes::scan_codex_config(&codex_config(
        None,
        Some(b"source_type = \"local\"\nsource = \"/opt/capsule\"\n"),
    ));
    assert!(!scan.plugin_present && scan.marketplace_present);
    // Unsupported syntax lines are ignored exactly like the consumer
    // scanner — never invented TOML semantics.
    let scan = probes::scan_codex_config(b"??? not toml at all\n");
    assert!(!scan.plugin_present && !scan.marketplace_present && !scan.duplicate_key);
}

// ---------------------------------------------------------------------------
// Codex hook ladder — Root43's total order, through the real probe.
// ---------------------------------------------------------------------------

fn codex_probe(home: &Path, config: &[u8]) -> (Vec<RootCandidate>, HookProbe) {
    let codex = home.join(provider("codex").home_target);
    std::fs::create_dir_all(codex.parent().expect("parent")).expect("mkdir");
    if !config.is_empty() {
        std::fs::write(&codex, config).expect("write config");
    }
    let mut recorded = Vec::new();
    let mut budget = Budget::new();
    let probe = probe_codex(provider("codex"), home, &mut recorded, &mut budget);
    (recorded, probe)
}

fn verdict(probe: &HookProbe) -> (HookState, HookReason) {
    match probe {
        HookProbe::Final { state, reason, .. } => (*state, *reason),
        HookProbe::AwaitsRoot { .. } => panic!("awaits root, not a verdict"),
    }
}

#[test]
fn codex_hook_total_order() {
    let local_marketplace = b"source_type = \"local\"\nsource = \"/opt/capsule\"\n".as_slice();

    // CDX-01: no file → absent/target-missing.
    let home = scratch("codex-absent");
    let (_recorded, probe) = codex_probe(&home, b"");
    assert_eq!(
        verdict(&probe),
        (HookState::Absent, HookReason::TargetMissing)
    );

    // Recognized contradictions → conflict with the matching reason.
    for (config, reason, tag) in [
        (
            codex_config(
                Some(b"enabled = true\nenabled = false\n"),
                Some(local_marketplace),
            ),
            HookReason::DuplicateKey,
            "dup-key",
        ),
        (
            {
                let mut bytes = codex_config(Some(b""), None);
                bytes.extend_from_slice(&codex_config(Some(b""), None));
                bytes
            },
            HookReason::DuplicateSection,
            "dup-section",
        ),
        (
            codex_config(Some(b"enabled = true\n"), None),
            HookReason::PluginWithoutMarketplace,
            "no-marketplace",
        ),
        (
            codex_config(
                Some(b""),
                Some(b"source_type = \"github\"\nsource = \"x/y\"\n"),
            ),
            HookReason::NonLocalSource,
            "non-local",
        ),
        (
            codex_config(
                Some(b""),
                Some(b"source_type = \"local\"\nsource = \"rel/path\"\n"),
            ),
            HookReason::SourceNotAbsolute,
            "rel-source",
        ),
        (
            // A marketplace without `source_type` is not provably local —
            // the same contradiction arm as an explicit non-local type.
            codex_config(Some(b""), Some(b"source = \"/opt/capsule\"\n")),
            HookReason::NonLocalSource,
            "no-source-type",
        ),
    ] {
        let home = scratch(tag);
        let (_recorded, probe) = codex_probe(&home, &config);
        assert_eq!(verdict(&probe), (HookState::Conflict, reason), "{tag}");
    }

    // CDX-03: `enabled` present but non-bool → unknown/malformed — the
    // scanner's unrecognized syntax is not a recognized contradiction.
    let home = scratch("codex-malformed");
    let (_recorded, probe) = codex_probe(
        &home,
        &codex_config(Some(b"enabled = \"yes\"\n"), Some(local_marketplace)),
    );
    assert_eq!(verdict(&probe), (HookState::Unknown, HookReason::Malformed));

    // Marketplace-only is `absent`, not contradiction — step 3 covers it.
    let home = scratch("codex-marketplace-only");
    let (_recorded, probe) = codex_probe(&home, &codex_config(None, Some(local_marketplace)));
    assert_eq!(
        verdict(&probe),
        (HookState::Absent, HookReason::NoManagedInstall)
    );

    // Well-formed plugin: awaits the recorded root; the local absolute
    // source registered exactly one candidate.
    let home = scratch("codex-awaits");
    let (recorded, probe) = codex_probe(
        &home,
        &codex_config(Some(b"enabled = true\n"), Some(local_marketplace)),
    );
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].provider, "codex");
    assert_eq!(recorded[0].field, "source");
    assert_eq!(recorded[0].path, b"/opt/capsule");
    let HookProbe::AwaitsRoot {
        consistent: ConsistentOutcome::Codex {
            enabled_flag: Some(true),
        },
        ..
    } = probe
    else {
        panic!("enabled=true must await its recorded root")
    };
    // Plugin present but no `enabled` key → `enabledFlag:"absent"`, still
    // awaiting the recorded-root outcome.
    let home = scratch("codex-unspecified");
    let (_recorded, probe) = codex_probe(&home, &codex_config(Some(b""), Some(local_marketplace)));
    let HookProbe::AwaitsRoot {
        consistent: ConsistentOutcome::Codex { enabled_flag: None },
        detail,
        ..
    } = probe
    else {
        panic!("unspecified enablement must await its recorded root")
    };
    assert_eq!(detail["enabledFlag"], json!("absent"));

    // CDX-06: a local absolute source pointing anywhere is admitted — the
    // probe never imports Desktop's foreign-source authority.
    let home = scratch("codex-anywhere");
    let (recorded, probe) = codex_probe(
        &home,
        &codex_config(
            Some(b""),
            Some(b"source_type = \"local\"\nsource = \"/anywhere/at/all\"\n"),
        ),
    );
    assert_eq!(recorded.len(), 1);
    assert!(matches!(probe, HookProbe::AwaitsRoot { .. }));
}

/// CDX-05/the terminal fold: `enabled=false` is `disabled` yet the recorded
/// root's observed state still lands in `detail.recordedRoot`.
#[test]
fn codex_disabled_retains_root_facts() {
    let mut detail = Map::from_iter([("enabledFlag".to_owned(), json!(false))]);
    let recorded = vec![RootCandidate {
        provider: "codex",
        field: "source",
        path: b"/opt/capsule".to_vec(),
    }];
    let (state, reason, detail) = finalize_hook(
        HookProbe::AwaitsRoot {
            root_index: Some(0),
            consistent: ConsistentOutcome::Codex {
                enabled_flag: Some(false),
            },
            detail: std::mem::take(&mut detail),
        },
        &recorded,
        0,
        &[RootState::Consistent],
    );
    assert_eq!(state, HookState::Disabled);
    assert_eq!(reason, HookReason::DisabledFlag);
    assert_eq!(detail["enabledFlag"], json!(false));
    assert_eq!(detail["recordedRoot"]["state"], json!("consistent"));
    assert_eq!(detail["recordedRoot"]["path"], json!("/opt/capsule"));
    // An inconsistent root under `enabled=false` still surfaces its fact —
    // disabled never erases the observed root problem.
    let (_state, _reason, detail) = finalize_hook(
        HookProbe::AwaitsRoot {
            root_index: Some(0),
            consistent: ConsistentOutcome::Codex {
                enabled_flag: Some(false),
            },
            detail: Map::new(),
        },
        &recorded,
        0,
        &[RootState::Inconsistent],
    );
    assert_eq!(detail["recordedRoot"]["state"], json!("inconsistent"));
}

/// The recorded-root fold for `enabled=true`/unspecified and Tier-D
/// `Present` outcomes: consistent roots proceed to the provider's own
/// present verdict; contradictions and uninspectable outcomes map
/// uniformly.
#[test]
fn recorded_root_fold_uniform() {
    let recorded = vec![RootCandidate {
        provider: "codex",
        field: "source",
        path: b"/opt/capsule".to_vec(),
    }];
    let fold = |outcome: ConsistentOutcome, state: RootState| {
        finalize_hook(
            HookProbe::AwaitsRoot {
                root_index: Some(0),
                consistent: outcome,
                detail: Map::new(),
            },
            &recorded,
            0,
            &[state],
        )
    };
    // Consistent root → the declaring provider's own present verdict.
    let (state, reason, _detail) = fold(
        ConsistentOutcome::Codex {
            enabled_flag: Some(true),
        },
        RootState::Consistent,
    );
    assert_eq!(state, HookState::Present);
    assert_eq!(reason, HookReason::ConfigDeclaresInstall);
    let (state, reason, _detail) = fold(
        ConsistentOutcome::Present {
            reason: HookReason::BootstrapMarkerConsistent,
        },
        RootState::Consistent,
    );
    assert_eq!(state, HookState::Present);
    assert_eq!(reason, HookReason::BootstrapMarkerConsistent);
    // Proven contradiction / positively-missing referent → conflict.
    let (state, reason, _detail) = fold(
        ConsistentOutcome::Codex {
            enabled_flag: Some(true),
        },
        RootState::Inconsistent,
    );
    assert_eq!(state, HookState::Conflict);
    assert_eq!(reason, HookReason::RecordedRootInconsistent);
    let (state, reason, _detail) = fold(
        ConsistentOutcome::Codex {
            enabled_flag: Some(true),
        },
        RootState::Absent,
    );
    assert_eq!(state, HookState::Conflict);
    assert_eq!(reason, HookReason::RecordedRootAbsent);
    // Every uninspectable root outcome → unknown/recorded-root-uninspectable.
    for root_state in [
        RootState::Unreadable,
        RootState::Unsafe,
        RootState::PathTooLong,
        RootState::BudgetExhausted,
        RootState::NotInspected,
    ] {
        let (state, reason, detail) = fold(
            ConsistentOutcome::Codex {
                enabled_flag: Some(true),
            },
            root_state,
        );
        assert_eq!(state, HookState::Unknown, "{root_state:?}");
        assert_eq!(reason, HookReason::RecordedRootUninspectable);
        assert_eq!(
            detail["recordedRoot"]["state"],
            json!(root_state.as_str()),
            "the specific outcome survives in detail"
        );
    }
    // A declared root the recorded-root cap refused is uninspectable —
    // `unknown`, with no recordedRoot fact row attached.
    let (state, reason, detail) = finalize_hook(
        HookProbe::AwaitsRoot {
            root_index: None,
            consistent: ConsistentOutcome::Codex {
                enabled_flag: Some(true),
            },
            detail: Map::new(),
        },
        &recorded,
        0,
        &[],
    );
    assert_eq!(state, HookState::Unknown);
    assert_eq!(reason, HookReason::RecordedRootUninspectable);
    assert!(detail.get("recordedRoot").is_none());
}

// ---------------------------------------------------------------------------
// Guarded marker / fingerprint (gemini, qwen-code).
// ---------------------------------------------------------------------------

#[test]
fn guarded_marker_parse() {
    let marker = |version: &str, files: &str| {
        format!(
            r#"{{"kind":"guarded-extension","integrationId":"gemini","contractVersion":{version},"files":[{files}]}}"#
        )
    };
    // Valid marker — numeric `3.0` accepted like JS `=== 3`.
    let valid_sha = "a".repeat(64);
    let MarkerParse::Valid {
        contract_version,
        files,
    } = probes::parse_guarded_marker(
        &marker(
            "3.0",
            &format!(r#"{{"path":"a.js","sha256":"{valid_sha}"}}"#),
        )
        .into_bytes(),
        "gemini",
    )
    else {
        panic!("valid marker")
    };
    assert_eq!(contract_version, 3);
    assert_eq!(files.len(), 1);
    // A newer contract version reports the declared number — the doctor
    // build is the stale side, never "broken".
    assert_eq!(
        probes::parse_guarded_marker(&marker("4", "").into_bytes(), "gemini"),
        MarkerParse::UnrecognizedVersion(4)
    );
    // Wrong integration id → name-mismatch.
    assert_eq!(
        probes::parse_guarded_marker(&marker("3", "").into_bytes(), "qwen-code"),
        MarkerParse::NameMismatch
    );
    // Any other shape contradiction → invalid.
    for bytes in [
        b"{}".as_slice(),
        br#"{"kind":"guarded-extension"}"#.as_slice(),
        br#"{"kind":"guarded-extension","integrationId":"gemini","contractVersion":0,"files":[]}"#
            .as_slice(),
        marker("3", r#"{"path":"a.js"}"#).into_bytes().as_slice(),
        marker("3", r#"{"sha256":"x","path":"a.js","extra":1}"#)
            .into_bytes()
            .as_slice(),
    ] {
        assert_eq!(
            probes::parse_guarded_marker(bytes, "gemini"),
            MarkerParse::Invalid,
            "{bytes:?}"
        );
    }
    // Marker `files[]` over 64 → malformed (C-2 file bound).
    let over = (0..65)
        .map(|index| format!(r#"{{"path":"f{index}","sha256":""}}"#))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        probes::parse_guarded_marker(&marker("3", &over).into_bytes(), "gemini"),
        MarkerParse::Invalid
    );
}

#[test]
fn fingerprint_matches_utf16_order() {
    let entry = |path: &str| probes::MarkerFileEntry {
        path: path.to_owned(),
        sha256: String::new(),
    };
    // Order-insensitive through the UTF-16 code-unit sort — equal sets match.
    let mut computed = vec![entry("b.js"), entry("a.js")];
    let declared = vec![entry("a.js"), entry("b.js")];
    assert!(probes::fingerprint_matches(&mut computed, &declared));
    let mut computed = vec![entry("a.js"), entry("c.js")];
    assert!(!probes::fingerprint_matches(&mut computed, &declared));
}

/// Guarded-directory probe through the real filesystem: consistent marker →
/// `present`; fingerprint contradiction → `conflict/marker-mismatch`.
#[test]
fn guarded_probe_through_filesystem() {
    let home = scratch("guarded");
    let target = home.join(provider("gemini").home_target);
    std::fs::create_dir_all(&target).expect("mkdir");
    let payload = b"payload-bytes";
    std::fs::write(target.join("index.js"), payload).expect("write payload");
    let sha = probes::hex_digest(payload);
    let marker = format!(
        r#"{{"kind":"guarded-extension","integrationId":"gemini","contractVersion":3,"files":[{{"path":"index.js","sha256":"{sha}"}}]}}"#
    );
    std::fs::write(target.join(probes::GUARDED_MARKER_NAME), marker).expect("write marker");
    let mut budget = Budget::new();
    let probe = probe_guarded(provider("gemini"), &home, &mut budget);
    assert_eq!(
        verdict(&probe),
        (HookState::Present, HookReason::MarkerConsistent)
    );
    // Marker declaring a payload that differs → contradiction.
    std::fs::write(target.join("index.js"), b"different").expect("rewrite payload");
    let mut budget = Budget::new();
    let probe = probe_guarded(provider("gemini"), &home, &mut budget);
    assert_eq!(
        verdict(&probe),
        (HookState::Conflict, HookReason::MarkerMismatch)
    );
    // Missing target → absent; symlinked target → unknown/symlink-uninspected.
    let empty = scratch("guarded-absent");
    let mut budget = Budget::new();
    let probe = probe_guarded(provider("gemini"), &empty, &mut budget);
    assert_eq!(
        verdict(&probe),
        (HookState::Absent, HookReason::TargetMissing)
    );
}

// ---------------------------------------------------------------------------
// Cline package.json.
// ---------------------------------------------------------------------------

#[test]
fn cline_package_versions() {
    let package = |name: &str, version: &str| {
        format!(r#"{{"name":"{name}","orchbeeAgentObservation":{{"contractVersion":{version}}}}}"#)
    };
    assert_eq!(
        probes::parse_cline_package(&package("orchbee-agent-observation", "3").into_bytes()),
        ClinePackage::Current
    );
    assert_eq!(
        probes::parse_cline_package(&package("orchbee-agent-observation", "2").into_bytes()),
        ClinePackage::UpdateRequired
    );
    assert_eq!(
        probes::parse_cline_package(&package("orchbee-agent-observation", "4").into_bytes()),
        ClinePackage::UnrecognizedVersion
    );
    for bytes in [
        b"{}".as_slice(),
        package("other-name", "3").into_bytes().as_slice(),
        package("orchbee-agent-observation", "\"3\"")
            .into_bytes()
            .as_slice(),
        package("orchbee-agent-observation", "0")
            .into_bytes()
            .as_slice(),
        b"not json".as_slice(),
    ] {
        assert_eq!(
            probes::parse_cline_package(bytes),
            ClinePackage::Conflict,
            "{bytes:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Bootstrap module scan (opencode, pi, omp).
// ---------------------------------------------------------------------------

#[test]
fn bootstrap_scan_verdicts() {
    let shape = provider("opencode")
        .bootstrap
        .as_ref()
        .expect("opencode bootstrap");
    let module_url = "file:///home/u/capsule/extensions/opencode/dist/index.mjs";
    let bytes = format!(
        "// orchbee-agent-observation-bootstrap-v3:opencode\nconst orchbeeAgentObservationModuleUrl = {module_url_json};\n{body}",
        module_url_json = serde_json::to_string(module_url).expect("url json"),
        body = String::from_utf8_lossy(shape.body),
    );
    let scan = probes::scan_bootstrap(bytes.as_bytes(), "opencode", shape);
    assert_eq!(scan.verdict, BootstrapVerdict::Present);
    assert_eq!(scan.module_url.as_deref(), Some(module_url));
    // Foreign content: no owned marker.
    let scan = probes::scan_bootstrap(b"// unrelated\n", "opencode", shape);
    assert_eq!(scan.verdict, BootstrapVerdict::ForeignContent);
    // Owned prefix naming another integration → name-mismatch.
    let scan = probes::scan_bootstrap(
        b"// orchbee-agent-observation-bootstrap-v3:pi\nrest",
        "opencode",
        shape,
    );
    assert_eq!(scan.verdict, BootstrapVerdict::NameMismatch);
    // Owned marker with a different body → template-mismatch; the moduleUrl
    // literal still contributes its recorded-root candidate.
    let bytes = format!(
        "// orchbee-agent-observation-bootstrap-v3:opencode\nconst orchbeeAgentObservationModuleUrl = {};\nbody-differs",
        serde_json::to_string(module_url).expect("url json"),
    );
    let scan = probes::scan_bootstrap(bytes.as_bytes(), "opencode", shape);
    assert_eq!(scan.verdict, BootstrapVerdict::TemplateMismatch);
    assert_eq!(scan.module_url.as_deref(), Some(module_url));
}

// ---------------------------------------------------------------------------
// Capsule manifest + payload digest.
// ---------------------------------------------------------------------------

#[test]
fn capsule_manifest_shape_and_version() {
    let manifest = |contract: &str, payload_version: &str, sha: &str, extra: &str| {
        format!(
            r#"{{"contractVersion":{contract},"payloadVersion":{payload_version},"payloadSha256":"{sha}"{extra}}}"#
        )
    };
    let digest = "a".repeat(64);
    // Strict three-key set + supported versions → valid.
    let scan =
        probes::scan_capsule_manifest(&manifest("3", "\"0.4.16\"", &digest, "").into_bytes())
            .expect("bounded json record");
    assert!(scan.valid_shape && !scan.version_mismatch);
    assert_eq!(
        scan.declared_payload_sha256.as_deref(),
        Some(digest.as_str())
    );
    // Declared ≠ supported → version-mismatch (uninspectable, not broken).
    let scan =
        probes::scan_capsule_manifest(&manifest("4", "\"0.4.16\"", &digest, "").into_bytes())
            .expect("bounded json record");
    assert!(scan.version_mismatch && !scan.valid_shape);
    let scan = probes::scan_capsule_manifest(&manifest("3", "\"9.9.9\"", &digest, "").into_bytes())
        .expect("bounded json record");
    assert!(scan.version_mismatch && !scan.valid_shape);
    // Missing or extra keys → shape-invalid, never version-mismatch.
    let scan = probes::scan_capsule_manifest(
        &manifest("3", "\"0.4.16\"", &digest, ",\"extra\":1").into_bytes(),
    )
    .expect("bounded json record");
    assert!(!scan.valid_shape && !scan.version_mismatch);
    let scan =
        probes::scan_capsule_manifest(br#"{"contractVersion":3}"#).expect("bounded json record");
    assert!(!scan.valid_shape && !scan.version_mismatch);
    // Bad digest spelling → shape-invalid.
    let scan =
        probes::scan_capsule_manifest(&manifest("3", "\"0.4.16\"", "not-hex", "").into_bytes())
            .expect("bounded json record");
    assert!(!scan.valid_shape);
    // Non-record documents do not scan at all.
    assert!(probes::scan_capsule_manifest(b"[1,2]").is_none());
}

#[test]
fn capsule_payload_digest_orders_and_serializes() {
    // UTF-16 order: 'Z' (U+005A) precedes 'a' (U+0061); multi-byte paths sort
    // by code units, not bytes.
    let mut entries = vec![
        probes::CapsuleEntry {
            directory: false,
            path: "a.txt".to_owned(),
            sha256: Some("b".repeat(64)),
            size: Some(3),
        },
        probes::CapsuleEntry {
            directory: true,
            path: "Z".to_owned(),
            sha256: None,
            size: None,
        },
    ];
    let digest = probes::capsule_payload_digest(&mut entries);
    // The sort ran in place and serialized the exact TS shapes.
    assert_eq!(entries[0].path, "Z");
    let expected_serialized = format!(
        "[{{\"kind\":\"directory\",\"path\":\"Z\"}},{{\"kind\":\"file\",\"path\":\"a.txt\",\"sha256\":\"{}\",\"size\":3}}]",
        "b".repeat(64)
    );
    assert_eq!(digest, probes::hex_digest(expected_serialized.as_bytes()));
    // Deterministic under input-order permutation.
    let mut reversed = entries.clone();
    reversed.reverse();
    assert_eq!(probes::capsule_payload_digest(&mut reversed), digest);
}

// ---------------------------------------------------------------------------
// Asset-root probe through the filesystem.
// ---------------------------------------------------------------------------

#[test]
fn supplied_root_probe_states() {
    // A consistent capsule: manifest + payload walk agree.
    let root = scratch("capsule-good");
    std::fs::write(root.join("payload.bin"), b"payload").expect("write payload");
    // Digest the same serialized walk the probe itself collects.
    let mut budget = Budget::new();
    let CapsuleWalk::Entries(mut walked) = fs::capsule_walk(&root, &mut budget) else {
        panic!("fixture capsule must walk")
    };
    let digest = probes::capsule_payload_digest(&mut walked);
    let manifest =
        format!(r#"{{"contractVersion":3,"payloadVersion":"0.4.16","payloadSha256":"{digest}"}}"#);
    std::fs::write(root.join(probes::CAPSULE_MANIFEST_NAME), manifest).expect("write manifest");
    let mut budget = Budget::new();
    let outcome = probe_root(root.as_os_str().as_bytes(), &mut budget);
    assert_eq!(outcome.state, RootState::Consistent);
    // A tampered payload → inconsistent (proven contradiction).
    std::fs::write(root.join("payload.bin"), b"tampered").expect("tamper");
    let mut budget = Budget::new();
    let outcome = probe_root(root.as_os_str().as_bytes(), &mut budget);
    assert_eq!(outcome.state, RootState::Inconsistent);
    // Missing root → absent.
    let mut budget = Budget::new();
    let outcome = probe_root(b"/definitely/not/present", &mut budget);
    assert_eq!(outcome.state, RootState::Absent);
    // A non-directory / unsafe kind → unsafe.
    let file_root = scratch("capsule-file");
    let file_path = file_root.join("plain");
    std::fs::write(&file_path, b"x").expect("write file");
    let mut budget = Budget::new();
    let outcome = probe_root(file_path.as_os_str().as_bytes(), &mut budget);
    assert_eq!(outcome.state, RootState::Unsafe);
    // A symlinked root → unsafe, never followed.
    let link = file_root.join("link");
    std::os::unix::fs::symlink(&root, &link).expect("symlink");
    let mut budget = Budget::new();
    let outcome = probe_root(link.as_os_str().as_bytes(), &mut budget);
    assert_eq!(outcome.state, RootState::Unsafe);
}

// ---------------------------------------------------------------------------
// Observation socket fact (private probe — lstat admission, never connect).
// ---------------------------------------------------------------------------

#[test]
fn observation_socket_fact_states() {
    let euid = nix::unistd::geteuid().as_raw();
    // Absent when nothing exists.
    let directory = scratch("socket");
    let path = directory.join("agent-observation-v3.sock");
    assert_eq!(fs::observation_socket(&path, euid), fs::SocketFact::Absent);
    // A real same-euid socket → present.
    let _listener = UnixListener::bind(&path).expect("bind scratch socket");
    match fs::observation_socket(&path, euid) {
        fs::SocketFact::Present { .. } => {}
        other => panic!("expected present, got {other:?}"),
    }
    // Foreign euid → wrong-owner.
    match fs::observation_socket(&path, euid + 1) {
        fs::SocketFact::WrongOwner { owner, .. } => assert_eq!(owner, euid),
        other => panic!("expected wrong-owner, got {other:?}"),
    }
    // A regular file at the path → not-a-socket.
    let plain = directory.join("plain");
    std::fs::write(&plain, b"x").expect("write file");
    assert!(matches!(
        fs::observation_socket(&plain, euid),
        fs::SocketFact::NotASocket { .. }
    ));
    // A symlink at the path → unsafe-symlink, never followed.
    let link = directory.join("link");
    std::os::unix::fs::symlink(&plain, &link).expect("symlink");
    assert_eq!(
        fs::observation_socket(&link, euid),
        fs::SocketFact::UnsafeSymlink
    );
}

// ---------------------------------------------------------------------------
// Vocabulary closure — emitted spellings stay inside the schema's enums.
// ---------------------------------------------------------------------------

#[test]
fn emitted_vocabularies_are_closed() {
    // Hook reasons: every emitted spelling is one of the schema's literals.
    let reasons = [
        "target-missing",
        "no-managed-install",
        "disabled-flag",
        "marker-consistent",
        "bootstrap-marker-consistent",
        "config-declares-install",
        "foreign-content",
        "marker-mismatch",
        "template-mismatch",
        "name-mismatch",
        "duplicate-key",
        "duplicate-section",
        "non-local-source",
        "source-not-absolute",
        "plugin-without-marketplace",
        "recorded-root-inconsistent",
        "recorded-root-absent",
        "contract-version-below-supported",
        "provider-command-inspection-required",
        "unreadable",
        "malformed",
        "unsafe-file-kind",
        "symlink-uninspected",
        "budget-exhausted",
        "not-inspected",
        "contract-version-unrecognized",
        "recorded-root-uninspectable",
    ];
    let mut emitted: Vec<&str> = Vec::new();
    for variant in [
        HookReason::TargetMissing,
        HookReason::NoManagedInstall,
        HookReason::DisabledFlag,
        HookReason::MarkerConsistent,
        HookReason::BootstrapMarkerConsistent,
        HookReason::ConfigDeclaresInstall,
        HookReason::ForeignContent,
        HookReason::MarkerMismatch,
        HookReason::TemplateMismatch,
        HookReason::NameMismatch,
        HookReason::DuplicateKey,
        HookReason::DuplicateSection,
        HookReason::NonLocalSource,
        HookReason::SourceNotAbsolute,
        HookReason::PluginWithoutMarketplace,
        HookReason::RecordedRootInconsistent,
        HookReason::RecordedRootAbsent,
        HookReason::ContractVersionBelowSupported,
        HookReason::ProviderCommandInspectionRequired,
        HookReason::Unreadable,
        HookReason::Malformed,
        HookReason::UnsafeFileKind,
        HookReason::SymlinkUninspected,
        HookReason::BudgetExhausted,
        HookReason::NotInspected,
        HookReason::ContractVersionUnrecognized,
        HookReason::RecordedRootUninspectable,
    ] {
        emitted.push(variant.as_str());
    }
    assert_eq!(emitted.len(), reasons.len(), "every variant is pinned");
    for spelling in emitted {
        assert!(reasons.contains(&spelling), "{spelling}");
    }
    // Root states and hook states are similarly closed.
    for (variant, spelling) in [
        (RootState::Consistent, "consistent"),
        (RootState::Inconsistent, "inconsistent"),
        (RootState::Unreadable, "unreadable"),
        (RootState::Unsafe, "unsafe"),
        (RootState::Absent, "absent"),
        (RootState::PathTooLong, "path-too-long"),
        (RootState::BudgetExhausted, "budget-exhausted"),
        (RootState::NotInspected, "not-inspected"),
    ] {
        assert_eq!(variant.as_str(), spelling);
    }
    for (variant, spelling) in [
        (HookState::Present, "present"),
        (HookState::Absent, "absent"),
        (HookState::Disabled, "disabled"),
        (HookState::Conflict, "conflict"),
        (HookState::UpdateRequired, "update-required"),
        (HookState::Unknown, "unknown"),
        (HookState::Unsupported, "unsupported"),
    ] {
        assert_eq!(variant.as_str(), spelling);
    }
}

/// Every provider's manual-command hint is present for the Tier-B
/// provider-command kind only — never executed, only rendered.
#[test]
fn provider_command_manual_commands() {
    for provider in probes::PROVIDERS {
        match provider.kind {
            ProbeKind::ProviderCommand => assert!(
                provider.manual_command.is_some(),
                "{} needs its manual command",
                provider.id
            ),
            _ => assert!(
                provider.manual_command.is_none(),
                "{} must not carry a manual command",
                provider.id
            ),
        }
    }
}
