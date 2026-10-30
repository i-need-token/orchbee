//! Report64 `provider-conformance64` conformance harness — replays the
//! current 221-case corpus (ten providers, inspect/enable/disable) against
//! the real `inspect_full`/`mutate` paths under an isolated HOME, scripted
//! command queue, and materialized capsules. `expect.commands` pins the
//! exact argv sequence (read slots first, then the mutation tail — a
//! strict prefix of `planCommands`); `expect.cli` pins exit/frontier/
//! verdict/provenance; `fsEffect`/`mustNotMutate` pin the filesystem
//! outcome. No real provider binary is ever executed; no real HOME is
//! ever touched.

use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
};

use serde_json::Value;
use sha2::Digest;

use super::super::capsule;
use super::super::inspect;
use super::super::model::{IntegrationId, ManagementMode};
use super::super::mutation::{self, Direction, StepKind, StepOutcome, Verdict};
use super::super::providers::{self, ProviderContext};
use super::CliStatus;
use super::fault_injection::{
    self as inject, BoundaryAction, InjectCancel, Plan, Prim, ScriptedResult,
};

const PACK_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/provider-conformance"
);

// ---------------------------------------------------------------------------
// Fixture model
// ---------------------------------------------------------------------------

struct Case {
    id: String,
    provider: IntegrationId,
    read_slots: usize,
    operation: Direction,
    inspect_only: bool,
    management_mode: ManagementMode,
    exe: Option<String>,
    capsule: String,
    capsule_sources_missing: Vec<String>,
    capsule_bundle: Option<(String, Value)>,
    deadline_ms: Option<u64>,
    fs: Vec<Value>,
    commands: Vec<Value>,
    inject: Value,
    expect: Value,
}

fn provider_id(name: &str) -> IntegrationId {
    IntegrationId::parse(name).unwrap_or_else(|| panic!("unknown provider {name}"))
}

fn load_cases() -> Vec<Case> {
    let root = PathBuf::from(PACK_DIR);
    let mut cases = Vec::new();
    let mut entries: Vec<PathBuf> = fs::read_dir(root.join("cases"))
        .expect("cases dir readable")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    entries.sort();
    for path in entries {
        let raw: Value = serde_json::from_slice(&fs::read(&path).expect("case file readable"))
            .expect("case file parses");
        let provider = provider_id(raw["provider"].as_str().expect("provider id"));
        let read_slots = raw["readSlots"].as_u64().expect("readSlots") as usize;
        for entry in raw["cases"].as_array().expect("cases array") {
            let given = &entry["given"];
            let operation = match entry["operation"].as_str().expect("operation") {
                "inspect" => (Direction::Enable, true),
                "enable" => (Direction::Enable, false),
                "disable" => (Direction::Disable, false),
                other => panic!("unknown operation {other}"),
            };
            cases.push(Case {
                id: entry["id"].as_str().expect("id").to_owned(),
                provider,
                read_slots,
                operation: operation.0,
                inspect_only: operation.1,
                management_mode: match given["managementMode"].as_str().unwrap_or("managed-local") {
                    "manual-only" => ManagementMode::ManualOnly,
                    _ => ManagementMode::ManagedLocal,
                },
                exe: given["exe"].as_str().map(str::to_owned),
                capsule: given["capsule"].as_str().unwrap_or("ready").to_owned(),
                capsule_sources_missing: given["capsuleSourcesMissing"]
                    .as_array()
                    .map(|list| {
                        list.iter()
                            .filter_map(|item| item.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default(),
                capsule_bundle: given["capsuleBundle"].as_object().map(|bundle| {
                    (
                        bundle["path"]
                            .as_str()
                            .expect("capsuleBundle.path")
                            .to_owned(),
                        bundle["contentJson"].clone(),
                    )
                }),
                deadline_ms: given["deadlineMs"].as_u64(),
                fs: given["fs"].as_array().cloned().unwrap_or_default(),
                commands: given["commands"].as_array().cloned().unwrap_or_default(),
                inject: given["inject"].clone(),
                expect: entry["expect"].clone(),
            });
        }
    }
    cases
}

// ---------------------------------------------------------------------------
// Isolated layout — temp HOME, capsule root, declared adapter source, PATH.
// ---------------------------------------------------------------------------

struct Dirs {
    root: PathBuf,
    home: PathBuf,
    capsule_root: PathBuf,
    adapter_source: PathBuf,
    bin: PathBuf,
}

fn make_dirs() -> Dirs {
    let root = std::env::temp_dir().join(format!(
        "orchbee-conformance64-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|span| span.as_nanos())
            .unwrap_or(0)
    ));
    let home = root.join("home");
    let dirs = Dirs {
        capsule_root: home
            .join(".orchbee/agent-observation-adapters")
            .join("a".repeat(64)),
        adapter_source: root.join("adapter-source"),
        bin: root.join("bin"),
        root,
        home,
    };
    fs::create_dir_all(&dirs.home).unwrap();
    fs::create_dir_all(&dirs.bin).unwrap();
    dirs
}

impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

// ---------------------------------------------------------------------------
// ${VAR} substitution + @payload bodies
// ---------------------------------------------------------------------------

fn substitutions(dirs: &Dirs, id: IntegrationId) -> Vec<(String, String)> {
    let capsule_parent = dirs.capsule_root.parent().unwrap().to_path_buf();
    vec![
        ("${HOME}".into(), dirs.home.display().to_string()),
        (
            "${CAPSULE_ROOT}".into(),
            dirs.capsule_root.display().to_string(),
        ),
        (
            "${BUNDLE}".into(),
            dirs.capsule_root.join("bundle").display().to_string(),
        ),
        (
            "${BUNDLE_URL}".into(),
            format!("file://{}", dirs.capsule_root.join("bundle").display()),
        ),
        // A different managed content identity, without ordering between siblings.
        (
            "${OUTDATED_ROOT}".into(),
            capsule_parent.join("b".repeat(64)).display().to_string(),
        ),
        (
            "${OTHER_SIBLING}".into(),
            capsule_parent.join("b".repeat(64)).display().to_string(),
        ),
        (
            "${FOREIGN_ROOT}".into(),
            "/third-party/agent-observation-adapters".into(),
        ),
        ("${VERSION}".into(), "1.0.0".to_owned()),
        ("${OLDER_VERSION}".into(), "0.3.0".into()),
        (
            "${MARKETPLACE}".into(),
            inspect::MARKETPLACE_NAME.to_owned(),
        ),
        (
            "${PLUGIN_ID}".into(),
            "orchbee-agent-observation@orchbee-local".into(),
        ),
        ("${PLUGIN_NAME}".into(), "orchbee-agent-observation".into()),
        ("${INTEGRATION_ID}".into(), id.as_str().to_owned()),
    ]
}

fn substitute(text: &str, vars: &[(String, String)]) -> String {
    let mut out = text.to_owned();
    for (token, value) in vars {
        out = out.replace(token.as_str(), value);
    }
    out
}

fn payloads() -> BTreeMap<String, Value> {
    let root = PathBuf::from(PACK_DIR);
    let raw: Value = serde_json::from_slice(
        &fs::read(root.join("payloads.json")).expect("payloads.json readable"),
    )
    .expect("payloads.json parses");
    raw["payloads"]
        .as_object()
        .expect("payloads map")
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

/// Resolve a `content`/`stdout`/`target` field: a literal string (vars
/// substituted) or an `@name` payload reference (substituted inside too).
fn resolve_text(
    field: &Value,
    vars: &[(String, String)],
    payloads: &BTreeMap<String, Value>,
) -> String {
    let raw = field.as_str().expect("text field is a string");
    if let Some(name) = raw.strip_prefix('@') {
        let body = payloads
            .get(name)
            .unwrap_or_else(|| panic!("payload @{name} exists"));
        let rendered = match body {
            Value::String(text) => text.clone(),
            other => serde_json::to_string(other).expect("payload serializes"),
        };
        substitute(&rendered, vars)
    } else {
        substitute(raw, vars)
    }
}

/// `contentJson`/`stdoutJson` — an `@name` JSON body or an inline JSON
/// value; either renders compact after `${VAR}` substitution.
fn resolve_json(
    field: &Value,
    vars: &[(String, String)],
    payloads: &BTreeMap<String, Value>,
) -> Vec<u8> {
    let body = match field {
        Value::String(raw) => match raw.strip_prefix('@') {
            Some(name) => payloads
                .get(name)
                .unwrap_or_else(|| panic!("payload @{name} exists"))
                .clone(),
            None => Value::String(raw.clone()),
        },
        inline => inline.clone(),
    };
    let rendered = substitute(&body.to_string(), vars);
    serde_json::from_str::<Value>(&rendered)
        .unwrap_or_else(|_| panic!("{field} renders as json"))
        .to_string()
        .into_bytes()
}

// ---------------------------------------------------------------------------
// Materialization
// ---------------------------------------------------------------------------

/// The full bundle set every capsule materializes — per-provider sources
/// plus the shared runtime files the capsule contract requires.
fn capsule_files() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("bundle/package.json", br#"{"name":"orchbee-agent-observation","orchbeeAgentObservation":{"contractVersion":1}}"#),
        ("bundle/scripts/forward-hook.mjs", b"// forward"),
        ("bundle/scripts/hook-sanitizers.mjs", b"// sanitize"),
        ("bundle/scripts/observation-transport.mjs", b"// transport"),
        ("bundle/opencode/orchbee-agent-observation.mjs", b"// opencode module"),
        ("bundle/pi/orchbee-agent-observation.cjs", b"// pi module"),
        ("bundle/omp/orchbee-agent-observation.cjs", b"// omp module"),
        ("bundle/gemini/extension.js", b"// gemini extension"),
        ("bundle/gemini/runtime.js", b"// gemini runtime"),
        ("bundle/qwen-code/extension.js", b"// qwen extension"),
        ("bundle/qwen-code/hook.js", b"// qwen hook"),
    ]
}

/// Does `missing` glob cover `relative`? `dir/**` covers the whole subtree.
fn covered(missing: &[String], relative: &str) -> bool {
    missing.iter().any(|pattern| {
        pattern
            .strip_suffix("/**")
            .map(|prefix| relative.starts_with(&format!("{prefix}/")) || relative == prefix)
            .unwrap_or_else(|| relative == pattern)
    })
}

/// Materialize a capsule directory: the bundle set minus the case's
/// `capsuleSourcesMissing`, plus the optional `capsuleBundle` override,
/// then a manifest whose digest covers exactly what was written.
/// `sabotage` drops a required file entirely so the capsule validates
/// Invalid — the publish path's staged verification then fails.
fn materialize_capsule(
    root: &Path,
    missing: &[String],
    bundle: Option<&(String, Vec<u8>)>,
    sabotage: bool,
) {
    for (relative, bytes) in capsule_files() {
        if covered(missing, relative) {
            continue;
        }
        if sabotage && relative == "bundle/scripts/forward-hook.mjs" {
            continue;
        }
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
    }
    if let Some((relative, bytes)) = bundle {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
    }
    let digest = capsule::payload_digest(root).expect("materialized capsule digests");
    let manifest = serde_json::json!({
        "contractVersion": 1,
        "payloadSha256": digest,
    });
    fs::write(root.join("capsule.json"), manifest.to_string()).unwrap();
}

fn make_executable(dirs: &Dirs, name: &str) {
    let path = dirs.bin.join(name);
    fs::write(&path, b"#!/bin/sh\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// Materialize a `dir-owned` extension-directory entry: copy the capsule's
/// bundle source plus the shared runtime scripts, fingerprint the real
/// bytes, write the marker, then apply the case's `mutate` perturbation.
fn materialize_owned_dir(
    dirs: &Dirs,
    id: IntegrationId,
    target: &Path,
    entry: &Value,
    vars: &[(String, String)],
    payloads: &BTreeMap<String, Value>,
) {
    let spec = providers::guarded_spec(id).expect("dir-owned needs a guarded provider");
    let source = dirs.capsule_root.join(spec.source_relative);
    if source.is_dir() {
        copy_contents(&source, target);
    } else {
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(&source, target).unwrap();
    }
    if let Some(runtime) = spec.runtime_relative {
        copy_contents(&dirs.capsule_root.join(runtime), &target.join("scripts"));
    }
    let files = inspect::fingerprint_directory(target).expect("owned dir fingerprints");
    let marker = serde_json::json!({
        "contractVersion": entry["markerContractVersion"].as_u64().unwrap_or(1),
        "files": files
            .into_iter()
            .map(|(path, sha256)| serde_json::json!({ "path": path, "sha256": sha256 }))
            .collect::<Vec<_>>(),
        "integrationId": id.as_str(),
        "kind": "guarded-extension",
    });
    fs::write(
        target.join(inspect::DIRECTORY_MARKER_NAME),
        marker.to_string(),
    )
    .unwrap();
    if let Some(perturb) = entry.get("mutate") {
        if let Some(link) = perturb.get("addSymlink") {
            std::os::unix::fs::symlink(
                substitute(link["target"].as_str().unwrap(), vars),
                target.join(link["path"].as_str().unwrap()),
            )
            .unwrap();
        } else if let Some(add) = perturb.get("addFile") {
            // `"addFile": "name"` is the bare drift file; an object form
            // carries explicit content.
            let (name, bytes) = match add {
                Value::String(name) => (name.clone(), b"// user addition\n".to_vec()),
                other => (
                    other["path"].as_str().expect("addFile.path").to_owned(),
                    resolve_text(&other["content"], vars, payloads).into_bytes(),
                ),
            };
            fs::write(target.join(name), bytes).unwrap();
        } else {
            fs::write(
                target.join(perturb["path"].as_str().unwrap()),
                resolve_text(&perturb["content"], vars, payloads),
            )
            .unwrap();
        }
    }
}

fn copy_contents(source: &Path, target: &Path) {
    if !source.exists() {
        return;
    }
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap().flatten() {
        let destination = target.join(entry.file_name());
        if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            copy_contents(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), &destination).unwrap();
        }
    }
}

/// Materialize the case's `fs` entries under the fixture HOME. Entries
/// carrying `mutateDuringOp` are NOT created — they describe the
/// mid-window mutation the plan replays at the stage→re-inspection
/// boundary instead.
fn materialize_fs(
    dirs: &Dirs,
    id: IntegrationId,
    entries: &[Value],
    vars: &[(String, String)],
    payloads: &BTreeMap<String, Value>,
    plan: &mut Plan,
) {
    for entry in entries {
        let path = dirs.home.join(entry["path"].as_str().expect("fs.path"));
        if let Some(during_op) = entry.get("mutateDuringOp").and_then(Value::as_str) {
            assert_eq!(
                during_op, "write-foreign",
                "{}: mutateDuringOp",
                entry["path"]
            );
            plan.on(
                "reinspect-target",
                BoundaryAction::Act(Box::new(move |target| {
                    fs::write(target, b"// foreign mid-window write\n").is_ok()
                })),
            );
            continue;
        }
        match entry["kind"].as_str().expect("fs.kind") {
            "file" => {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                let bytes = if let Some(size) = entry["contentBytes"].as_u64() {
                    vec![b'#'; size as usize]
                } else if let Some(content) = entry.get("content") {
                    resolve_text(content, vars, payloads).into_bytes()
                } else {
                    resolve_json(&entry["contentJson"], vars, payloads)
                };
                fs::write(&path, bytes).unwrap();
            }
            "dir" => {
                fs::create_dir_all(&path).unwrap();
            }
            "dir-with-entries" => {
                let count = entry["entries"].as_u64().expect("entries") as usize;
                for index in 0..count {
                    fs::create_dir_all(path.join(format!("entry-{index}"))).unwrap();
                }
            }
            "dir-owned" => {
                materialize_owned_dir(dirs, id, &path, entry, vars, payloads);
            }
            "symlink" => {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                let target = substitute(entry["target"].as_str().unwrap(), vars);
                std::os::unix::fs::symlink(target, &path).unwrap();
            }
            other => panic!("unknown fs.kind {other}"),
        }
        if let Some(mode) = entry["mode"]
            .as_str()
            .filter(|_| entry["kind"] != "symlink")
        {
            fs::set_permissions(
                &path,
                fs::Permissions::from_mode(u32::from_str_radix(mode, 8).unwrap()),
            )
            .unwrap();
        }
    }
}

/// A snapshot of the provider-visible filesystem — every path under HOME
/// except the Orchbee-owned capsule/staging space (`.orchbee/**` and any
/// `.orchbee-agent-*` residue), which `mustNotMutate` excludes by
/// definition (the publish residue is Orchbee's own and reported).
fn home_scan(home: &Path) -> BTreeMap<String, String> {
    let mut scan = BTreeMap::new();
    let mut pending = vec![home.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            if path == home.join(".orchbee") || name.starts_with(".orchbee-agent-") {
                continue;
            }
            let relative = path
                .strip_prefix(home)
                .unwrap()
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            let file_type = entry.file_type().unwrap();
            if file_type.is_symlink() {
                scan.insert(
                    relative,
                    format!("symlink:{}", fs::read_link(&path).unwrap().display()),
                );
            } else if file_type.is_dir() {
                scan.insert(relative, "dir".to_owned());
                pending.push(path);
            } else {
                let bytes = fs::read(&path).unwrap_or_default();
                scan.insert(relative, format!("file:{:x}", sha2::Sha256::digest(&bytes)));
            }
        }
    }
    scan
}

// ---------------------------------------------------------------------------
// Scripted command queue
// ---------------------------------------------------------------------------

/// Install the case's `inject` plan — mid-window interposition plus
/// step-scoped primitive faults, mapped once onto the pack's named kinds.
fn install_injections(case: &Case, dirs: &Dirs, plan: &mut Plan) {
    let inject = &case.inject;
    if inject.is_null() {
        return;
    }
    match inject["kind"].as_str().expect("inject.kind") {
        // A concurrent install lands a valid capsule between the staged
        // verify and the no-replace publish; the retained stage's same-call
        // cleanup then fails — the residue must survive the report.
        "concurrent-valid-capsule-cleanup-fails" => {
            let capsule_root = dirs.capsule_root.clone();
            plan.on(
                "capsule-verify",
                BoundaryAction::Act(Box::new(move |_| {
                    materialize_capsule(&capsule_root, &[], None, false);
                    true
                })),
            );
            let errno = match inject["errno"].as_str().expect("inject.errno") {
                "EIO" => nix::libc::EIO,
                "EACCES" => nix::libc::EACCES,
                "EPERM" => nix::libc::EPERM,
                "ENOTEMPTY" => nix::libc::ENOTEMPTY,
                other => panic!("{}: unknown inject.errno {other}", case.id),
            };
            plan.prim_fault_at("capsule-isvalid-existing", Prim::WalkUnlink, errno, 1);
        }
        other => panic!("{}: unknown inject.kind {other}", case.id),
    }
}

fn script_commands(
    case: &Case,
    vars: &[(String, String)],
    payloads: &BTreeMap<String, Value>,
    plan: &mut Plan,
) {
    let mut completion_advance_seen = false;
    for (command_index, command) in case.commands.iter().enumerate() {
        let result = &command["result"];
        match result["kind"].as_str().expect("result.kind") {
            "completed" => {
                let stdout = if let Some(json) = result.get("stdoutJson") {
                    resolve_json(json, vars, payloads)
                } else {
                    resolve_text(&result["stdout"], vars, payloads).into_bytes()
                };
                plan.script_command(ScriptedResult::Completed(stdout));
                if let Some(advance_ms) = result["afterCommandAdvanceMs"].as_u64() {
                    assert!(
                        !completion_advance_seen,
                        "{}: only one afterCommandAdvanceMs is supported",
                        case.id
                    );
                    completion_advance_seen = true;
                    let mut remaining_completions = command_index
                        .checked_sub(case.read_slots)
                        .unwrap_or_else(|| {
                            panic!(
                                "{}: afterCommandAdvanceMs belongs to a mutation command",
                                case.id
                            )
                        });
                    let advance = Duration::from_millis(advance_ms);
                    // This is external scheduling time after the named
                    // command legally completed, not command wall time. The
                    // mutation boundary fires after attempted/effect state is
                    // recorded and before the next slot-admission consult;
                    // inspection reads never consume it.
                    plan.on(
                        "provider-command-completed",
                        BoundaryAction::Act(Box::new(move |_| {
                            if remaining_completions == 0 {
                                inject::advance_clock(advance);
                                true
                            } else {
                                remaining_completions -= 1;
                                false
                            }
                        })),
                    );
                }
            }
            "failed" => plan.script_command(ScriptedResult::Failed),
            "timed-out" => plan.script_command(ScriptedResult::TimedOut),
            "output-too-large" => plan.script_command(ScriptedResult::OutputTooLarge),
            other => panic!("{}: unknown result.kind {other}", case.id),
        }
    }
}

// ---------------------------------------------------------------------------
// Assertion helpers
// ---------------------------------------------------------------------------

/// The canonical attempt vocabulary for a finished report — the fixture's
/// `expect.attempt` space maps verdicts onto `attempted`/`conflict`/
/// `failed`/`manual-action-required`.
fn attempt_word(report: &mutation::MutationReport) -> &'static str {
    match report.verdict {
        Verdict::Completed => "attempted",
        Verdict::Cancelled => "cancelled",
        Verdict::Unknown => "unknown",
        Verdict::Refused(refusal) => match refusal {
            mutation::Refusal::Conflict => "conflict",
            mutation::Refusal::ManualActionRequired => "manual-action-required",
            _ => "failed",
        },
        Verdict::Failed(code) => match code {
            mutation::FailureCode::TimedOut | mutation::FailureCode::ManualActionRequired => {
                "manual-action-required"
            }
            _ => "failed",
        },
    }
}

fn verdict_word(report: &mutation::MutationReport) -> &'static str {
    match report.verdict {
        Verdict::Completed => "completed",
        Verdict::Cancelled => "cancelled",
        Verdict::Unknown => "unknown",
        Verdict::Refused(refusal) => refusal.as_str(),
        Verdict::Failed(code) => code.as_str(),
    }
}

/// `cli.attemptedSteps` rendering: a command step is named by its leading
/// subcommand words (values stop the walk); a non-completed outcome
/// suffixes ` (<outcome>)`.
fn attempted_step_names(report: &mutation::MutationReport) -> Vec<String> {
    const SUBCOMMANDS: &[&str] = &[
        "plugin",
        "plugins",
        "marketplace",
        "list",
        "install",
        "uninstall",
        "remove",
        "add",
        "enable",
        "disable",
        "get",
        "set",
    ];
    report
        .attempted
        .iter()
        .map(|step| {
            let name = match &step.step {
                StepKind::Command { argv, .. } => argv
                    .iter()
                    .take_while(|arg| SUBCOMMANDS.contains(&arg.as_str()))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" "),
                StepKind::Filesystem { operation, .. } => operation.to_string(),
            };
            if step.outcome == StepOutcome::Completed {
                name
            } else {
                format!("{} ({})", name, step.outcome.as_str())
            }
        })
        .collect()
}

fn substitute_argv(list: &[Value], vars: &[(String, String)]) -> Vec<Vec<String>> {
    list.iter()
        .map(|argv| {
            argv.as_array()
                .expect("argv array")
                .iter()
                .map(|arg| substitute(arg.as_str().expect("argv arg"), vars))
                .collect()
        })
        .collect()
}

fn assert_snapshot(case: &Case, item: &super::super::model::SnapshotItem) {
    let expect = &case.expect["snapshot"];
    if expect.is_null() {
        return;
    }
    if let Some(state) = expect["state"].as_str() {
        assert_eq!(item.state.as_str(), state, "{}: snapshot.state", case.id);
    }
    if let Some(availability) = expect["availability"].as_str() {
        assert_eq!(
            item.availability.as_str(),
            availability,
            "{}: snapshot.availability",
            case.id
        );
    }
    if let Some(mode) = expect["managementMode"].as_str() {
        assert_eq!(
            item.management_mode.as_str(),
            mode,
            "{}: snapshot.managementMode",
            case.id
        );
    }
    if expect.get("issue").is_some() {
        let actual = item.issue.map(|issue| issue.as_str());
        let expected = expect["issue"].as_str();
        assert_eq!(actual, expected, "{}: snapshot.issue", case.id);
    }
}

fn assert_commands(case: &Case, vars: &[(String, String)]) {
    let expect = &case.expect;
    let log: Vec<Vec<String>> = inject::argv_log()
        .into_iter()
        .map(|mut argv| {
            argv.remove(0);
            argv
        })
        .collect();
    if let Some(commands) = expect.get("commands") {
        let expected = substitute_argv(commands.as_array().expect("commands array"), vars);
        assert_eq!(log, expected, "{}: expect.commands argv sequence", case.id);
    }
    if let Some(plan) = expect.get("planCommands") {
        // The mutation tail (argv beyond the read slots) must be a strict
        // prefix of the planned vector — a mid-plan timeout stops early.
        let expected = substitute_argv(plan.as_array().expect("planCommands array"), vars);
        let tail: Vec<Vec<String>> = log
            .iter()
            .skip(case.read_slots.min(log.len()))
            .cloned()
            .collect();
        assert!(
            tail.len() <= expected.len() && tail == expected[..tail.len()],
            "{}: planCommands prefix\n  tail={tail:?}\n  plan={expected:?}",
            case.id
        );
    }
}

fn assert_fs_effect(
    case: &Case,
    dirs: &Dirs,
    vars: &[(String, String)],
    report: &mutation::MutationReport,
) {
    let Some(effect) = case.expect.get("fsEffect") else {
        return;
    };
    let kind = effect["kind"].as_str().expect("fsEffect.kind");
    let path = effect["path"]
        .as_str()
        .map(|relative| dirs.home.join(relative));
    match kind {
        "none" => {
            // No publish/remove step completed against the named target —
            // refusals never touched it, a mid-window write keeps its own
            // foreign bytes.
            if let Some(target) = &path {
                let touched = report.attempted.iter().any(|step| {
                    matches!(
                        &step.step,
                        StepKind::Filesystem { operation, path }
                            if *path == *target
                                && step.outcome == StepOutcome::Completed
                                && matches!(
                                    *operation,
                                    "publish-entry" | "remove-entry" | "move-aside" | "restore-backup"
                                )
                    )
                });
                assert!(!touched, "{}: fsEffect none — target mutated", case.id);
            }
        }
        "publish-file" | "replace" | "publish-dir" => {
            let target = path.expect("fsEffect.path");
            let metadata = fs::symlink_metadata(&target)
                .unwrap_or_else(|_| panic!("{}: {} exists", case.id, target.display()));
            if let Some(content) = effect.get("content") {
                // File payload: exact bytes and mode.
                assert!(metadata.is_file(), "{}: fsEffect file is a file", case.id);
                let expected = resolve_text(content, vars, &payloads());
                assert_eq!(
                    fs::read(&target).unwrap(),
                    expected.into_bytes(),
                    "{}: published file bytes",
                    case.id
                );
                if let Some(mode) = effect["mode"].as_str() {
                    let actual = format!("{:04o}", metadata.permissions().mode() & 0o777);
                    assert_eq!(actual, mode, "{}: published file mode", case.id);
                }
            } else {
                // Directory payload: the marker + fingerprint must verify
                // the freshly staged bytes as ours.
                assert!(metadata.is_dir(), "{}: fsEffect dir is a dir", case.id);
                assert_eq!(
                    inspect::inspect_owned_directory(&target, case.provider),
                    inspect::TargetState::Current,
                    "{}: published dir verifies current",
                    case.id
                );
            }
        }
        "unlink" | "remove-dir" => {
            let target = path.expect("fsEffect.path");
            let error = fs::symlink_metadata(&target).map(|_| ()).unwrap_err();
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::NotFound,
                "{}: {} removed",
                case.id,
                target.display()
            );
        }
        other => panic!("{}: unknown fsEffect.kind {other}", case.id),
    }
}

// ---------------------------------------------------------------------------
// The driver
// ---------------------------------------------------------------------------

fn run_case(case: &Case, cli_status: CliStatus) {
    let dirs = make_dirs();
    let vars = substitutions(&dirs, case.provider);
    let payloads = payloads();
    let mut plan = Plan::default();

    // Executable: `exe: null` resolves nothing; anything else admits the
    // registered name on the harness PATH.
    if case.exe.is_some() {
        make_executable(&dirs, providers::executable_name(case.provider));
    }

    // The capsule contract: `ready` materializes the published root AND an
    // identical valid source; `unpublished` leaves the root missing but the
    // source intact, so publish is attempted and SUCCEEDS — every later
    // verdict sits behind an Orchbee-owned effect; `absent`/`invalid` leaves
    // the source invalid, so a publish is attempted and fails.
    match case.capsule.as_str() {
        "ready" => {
            let bundle = case
                .capsule_bundle
                .as_ref()
                .map(|(path, body)| (path.clone(), resolve_json(body, &vars, &payloads)));
            materialize_capsule(
                &dirs.capsule_root,
                &case.capsule_sources_missing,
                bundle.as_ref(),
                false,
            );
            materialize_capsule(
                &dirs.adapter_source,
                &case.capsule_sources_missing,
                bundle.as_ref(),
                false,
            );
        }
        "unpublished" => {
            materialize_capsule(&dirs.adapter_source, &[], None, false);
        }
        _ => {
            materialize_capsule(&dirs.adapter_source, &[], None, true);
        }
    }

    materialize_fs(&dirs, case.provider, &case.fs, &vars, &payloads, &mut plan);
    install_injections(case, &dirs, &mut plan);
    script_commands(case, &vars, &payloads, &mut plan);
    inject::install(plan);

    let before = if case.expect["mustNotMutate"].as_bool().unwrap_or(false) {
        Some(home_scan(&dirs.home))
    } else {
        None
    };

    let deadline = inject::now()
        + case
            .deadline_ms
            .map(Duration::from_millis)
            .unwrap_or_else(|| Duration::from_secs(360));
    let cancel = InjectCancel;
    let context = ProviderContext {
        home: &dirs.home,
        capsule_root: &dirs.capsule_root,
        adapter_source: Some(dirs.adapter_source.as_path()),
        path_env: Some(dirs.bin.as_os_str()),
        deadline,
        cancel: Some(&cancel),
        management_mode: case.management_mode,
    };

    if case.inspect_only {
        let item = providers::inspect_provider(case.provider, &context);
        assert_snapshot(case, &item);
        assert_commands(case, &vars);
    } else {
        let report = mutation::mutate(case.provider, case.operation, &context);
        if let Some(observed) = &report.observed {
            assert_snapshot(case, observed);
        } else {
            assert!(
                case.expect.get("snapshot").is_none(),
                "{}: snapshot asserted but report.observed is empty",
                case.id
            );
        }
        let expect = &case.expect;
        if let Some(attempt) = expect["attempt"].as_str() {
            assert_eq!(
                attempt_word(&report),
                attempt,
                "{}: expect.attempt (verdict {:?}, attempted {:?})",
                case.id,
                report.verdict,
                report.attempted
            );
        }
        assert_commands(case, &vars);
        if let Some(cli) = expect.get("cli") {
            if let Some(exit) = cli["exit"].as_u64() {
                assert_eq!(
                    cli_status(&report) as u64,
                    exit,
                    "{}: cli.exit (verdict {:?})",
                    case.id,
                    report.verdict
                );
            }
            if let Some(frontier) = cli["frontier"].as_str() {
                let actual = if report.effect_begun {
                    "post-effect"
                } else {
                    "pre-effect"
                };
                assert_eq!(actual, frontier, "{}: cli.frontier", case.id);
            }
            if let Some(verdict) = cli["verdict"].as_str() {
                assert_eq!(verdict_word(&report), verdict, "{}: cli.verdict", case.id);
            }
            if let Some(code) = cli["code"].as_str() {
                assert_eq!(verdict_word(&report), code, "{}: cli.code", case.id);
            }
            if let Some(provenance) = cli["observedProvenance"].as_str() {
                let actual = report.provenance.as_str();
                let ok = if provenance == "partial-or-unavailable" {
                    matches!(actual, "partial" | "unavailable")
                } else {
                    actual == provenance
                };
                assert!(
                    ok,
                    "{}: cli.observedProvenance {actual} != {provenance}",
                    case.id
                );
            }
            if let Some(steps) = cli["attemptedSteps"].as_array() {
                let expected: Vec<String> = steps
                    .iter()
                    .map(|step| substitute(step.as_str().unwrap(), &vars))
                    .collect();
                assert_eq!(
                    attempted_step_names(&report),
                    expected,
                    "{}: cli.attemptedSteps",
                    case.id
                );
            }
        }
        assert_fs_effect(case, &dirs, &vars, &report);
        // `residue[]`: each pattern must name a real survivor — present on
        // disk AND still named in the report. A cleanup that failed but
        // vanished from `residuals` is exactly the dishonesty this pins.
        for residue in case.expect["residue"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            let pattern = residue["pattern"].as_str().expect("residue.pattern");
            let prefix = pattern.trim_end_matches('*');
            let named = report.residuals.iter().any(|path| {
                path.file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .is_some_and(|name| name.starts_with(prefix))
            });
            assert!(
                named,
                "{}: residue pattern '{pattern}' not named in {:?}",
                case.id, report.residuals
            );
            let base = dirs
                .capsule_root
                .parent()
                .unwrap_or(&dirs.home)
                .to_path_buf();
            let survivor = fs::read_dir(&base)
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| entry.path())
                .any(|path| {
                    path.file_name()
                        .and_then(std::ffi::OsStr::to_str)
                        .is_some_and(|name| name.starts_with(prefix))
                });
            assert!(
                survivor,
                "{}: residue pattern '{pattern}' matched nothing under {}",
                case.id,
                base.display()
            );
        }
        if let Some(before) = before {
            let after = home_scan(&dirs.home);
            assert_eq!(before, after, "{}: mustNotMutate", case.id);
        }
    }
    inject::reset();
}

#[test]
fn capsule_digest_uses_javascript_utf16_path_order() {
    let dirs = make_dirs();
    let root = dirs.root.join("unicode-digest");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("\u{e000}"), b"B").unwrap();
    fs::write(root.join("\u{10000}"), b"A").unwrap();
    // Node JSON.stringify puts U+10000 before U+E000; UTF-8 byte order reverses it.
    assert_eq!(
        capsule::payload_digest(&root).as_deref(),
        Some("34231f4c10b448ca3b14d2ef5668bdcf0302b3155c0b357d5d5f597b2d870460")
    );
}

/// Replays the frozen `provider-conformance64` corpus; `cli_status` is the owning application's
/// integration exit mapping.
pub fn provider_conformance64_replay(cli_status: CliStatus) {
    let cases = load_cases();
    assert_eq!(cases.len(), 221, "current corpus is 221 cases");
    let mut failures = Vec::new();
    for case in &cases {
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_case(case, cli_status)));
        inject::reset();
        if let Err(payload) = outcome {
            failures.push(format!("{}: {payload:?}", case.id));
        }
    }
    assert!(
        failures.is_empty(),
        "{} conformance failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
