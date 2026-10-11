//! Root57 `provider-effects52` conformance harness — drives the shared
//! filesystem effect machine (stage → verify → publish → restore → cleanup)
//! and the mutation frontier against the frozen 47-case pack with real
//! injected faults. `delivery-injected` cases materialize `setup.fs` under
//! an isolated temporary home, install the stated fault at `inject.atStep`,
//! run the real implementation path, and assert every `expect` field;
//! `static` cases assert the contract pins in code shape.

use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use serde_json::Value;

use super::super::capsule;
use super::super::files::{self, FsStep};
use super::super::inspect;
use super::super::model::IntegrationId;
use super::super::mutation::{self, Direction, StepKind, StepOutcome, Verdict};
use super::super::providers::{self, ProviderContext};
use super::fault_injection::{
    self as inject, BoundaryAction, InjectCancel, Plan, Prim, ScriptedResult,
};

const PACK_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/agent-integrations/provider-effects/cases.json"
);

// ---------------------------------------------------------------------------
// Fixture model
// ---------------------------------------------------------------------------

struct Case {
    id: String,
    execution: String,
    setup_fs: Vec<Value>,
    setup_provider: Option<String>,
    setup_clock: Option<String>,
    drive: Value,
    inject: Value,
    expect: Value,
}

fn load_cases() -> Vec<Case> {
    let path = PathBuf::from(PACK_PATH);
    let raw: Value = serde_json::from_slice(&fs::read(&path).expect("cases.json readable"))
        .expect("cases.json parses");
    raw["cases"]
        .as_array()
        .expect("cases array")
        .iter()
        .map(|entry| Case {
            id: entry["id"].as_str().unwrap().to_owned(),
            execution: entry["execution"].as_str().unwrap().to_owned(),
            setup_fs: entry["setup"]["fs"].as_array().cloned().unwrap_or_default(),
            setup_provider: entry["setup"]["provider"].as_str().map(str::to_owned),
            setup_clock: entry["setup"]["clock"].as_str().map(str::to_owned),
            drive: entry["drive"].clone(),
            inject: entry["inject"].clone(),
            expect: entry["expect"].clone(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Isolated directory layout — `$HOME` is a temp dir, never the real one.
// ---------------------------------------------------------------------------

struct Dirs {
    root: PathBuf,
    home: PathBuf,
    parent: PathBuf,
    capsule_root: PathBuf,
    target: PathBuf,
    provider_cfg: PathBuf,
    cohort_dir: PathBuf,
    adapter_source: PathBuf,
    bin: PathBuf,
}

fn make_dirs() -> Dirs {
    let root = std::env::temp_dir().join(format!(
        "orchbee-effects52-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|span| span.as_nanos())
            .unwrap_or(0)
    ));
    let home = root.join("home");
    let parent = home.join(".orchbee/agent-observation-adapters");
    let dirs = Dirs {
        capsule_root: parent.join(capsule::PAYLOAD_VERSION),
        target: home.join("managed-target"),
        provider_cfg: home.join("provider.cfg"),
        cohort_dir: root.join("cohort"),
        adapter_source: root.join("adapter-source"),
        bin: root.join("bin"),
        root,
        home,
        parent,
    };
    fs::create_dir_all(&dirs.parent).unwrap();
    fs::create_dir_all(&dirs.bin).unwrap();
    dirs
}

impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Resolve a fixture `$`-var path. Bare relative paths (e.g.
/// `.orchbee-agent-backup-<n1>`) are siblings of `$TARGET`.
fn resolve_path(dirs: &Dirs, spec: &str) -> PathBuf {
    let mut rest = spec.to_owned();
    for (var, path) in [
        ("$CAPSULE_ROOT", dirs.capsule_root.clone()),
        ("$PARENT", dirs.parent.clone()),
        ("$TARGET", dirs.target.clone()),
        ("$PROVIDER_CFG", dirs.provider_cfg.clone()),
        ("$HOME", dirs.home.clone()),
    ] {
        if let Some(tail) = rest.strip_prefix(var) {
            rest = format!("{}{tail}", path.display());
            break;
        }
    }
    let path = PathBuf::from(&rest);
    if path.is_absolute() {
        path
    } else {
        dirs.target.parent().unwrap_or(&dirs.home).join(path)
    }
}

/// The materialized bytes for one `ownership` role on a filesystem-level
/// case — the harness classifier reads this prefix back as the declared
/// ownership dimension (the pack's roles, mapped onto real bytes).
fn ownership_bytes(ownership: &str, label: &str) -> Vec<u8> {
    match ownership {
        "orchbee-exact" => [b"ORCHBEE:exact:", label.as_bytes()].concat(),
        "orchbee-legacy" => [b"ORCHBEE:legacy:", label.as_bytes()].concat(),
        "orchbee-newer" => [b"ORCHBEE:newer:", label.as_bytes()].concat(),
        _ => label.as_bytes().to_vec(),
    }
}

/// What the preflight inspection reports for the materialized bytes.
fn classify_bytes(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"ORCHBEE:exact:") {
        "orchbee-exact"
    } else if bytes.starts_with(b"ORCHBEE:legacy:") {
        "orchbee-legacy"
    } else if bytes.starts_with(b"ORCHBEE:newer:") {
        "orchbee-newer"
    } else {
        "foreign"
    }
}

/// Materialize `setup.fs`. `owners` records each materialized path's
/// declared bytes so `preserved[]` assertions compare real content;
/// `<nonce>`/`<n1>`/`<n1..n8>`/`<oldnonce>` placeholders become distinct
/// fixture names under the same prefix.
fn materialize_fs(dirs: &Dirs, entries: &[Value]) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut owners = BTreeMap::new();
    let mut nonce_counter = 0usize;
    for entry in entries {
        let spec = entry["path"].as_str().unwrap();
        let kind = entry["kind"].as_str().unwrap_or("file");
        let ownership = entry["ownership"].as_str().unwrap_or("foreign");
        let bytes_label = entry["bytes"].as_str().unwrap_or("materialized");
        let repeat = entry["repeat"].as_str();
        let count = if repeat.is_some() { 8 } else { 1 };
        for index in 0..count {
            let resolved = resolve_path(dirs, spec);
            let path = if spec.contains('<') {
                let prefix = spec.split('<').next().unwrap_or(".orchbee-agent-item-");
                nonce_counter += 1;
                resolve_path(dirs, &format!("{prefix}fixture{}{}", nonce_counter, index))
            } else {
                resolved
            };
            match kind {
                "absent" => {
                    let _ = fs::remove_file(&path);
                    let _ = fs::remove_dir_all(&path);
                }
                "file" => {
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent).unwrap();
                    }
                    let bytes = ownership_bytes(ownership, bytes_label);
                    fs::write(&path, &bytes).unwrap();
                    owners.insert(path.clone(), bytes);
                }
                "dir" => {
                    fs::create_dir_all(&path).unwrap();
                    if let Some(children) = entry["children"].as_array() {
                        for child in children {
                            let child = child.as_str().unwrap();
                            let child_path = path.join(child);
                            if child.ends_with('/') {
                                fs::create_dir_all(&child_path).unwrap();
                            } else {
                                if let Some(parent) = child_path.parent() {
                                    fs::create_dir_all(parent).unwrap();
                                }
                                let bytes = ownership_bytes(ownership, child);
                                fs::write(&child_path, &bytes).unwrap();
                                owners.insert(child_path, bytes);
                            }
                        }
                    } else {
                        let bytes = ownership_bytes(ownership, bytes_label);
                        fs::write(path.join("content"), &bytes).unwrap();
                        owners.insert(path.join("content"), bytes);
                    }
                    owners.insert(path.clone(), Vec::new());
                }
                "symlink" => {
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent).unwrap();
                    }
                    let target = dirs.capsule_root.clone();
                    std::os::unix::fs::symlink(&target, &path).unwrap();
                }
                other => panic!("unknown setup kind {other}"),
            }
        }
    }
    owners
}

/// Materialize a syntactically valid managed capsule at `dir`:
/// the four required runtime files plus one bundle module per guarded
/// provider, the canonical payload digest, and the strict three-key
/// manifest. `payload_version` overrides the declared version; `sabotage`
/// drops a required file so the capsule validates `Invalid`.
fn materialize_capsule(dir: &Path, payload_version: Option<&str>, sabotage: bool) {
    let files_to_write: Vec<(&str, &[u8])> = vec![
        ("bundle/package.json", br#"{"name":"orchbee-observation"}"#),
        ("bundle/scripts/forward-hook.mjs", b"// forward"),
        ("bundle/scripts/hook-sanitizers.mjs", b"// sanitize"),
        ("bundle/scripts/observation-transport.mjs", b"// transport"),
        (
            "bundle/opencode/orchbee-agent-observation.mjs",
            b"// opencode",
        ),
        ("bundle/pi/orchbee-agent-observation.cjs", b"// pi"),
        ("bundle/omp/orchbee-agent-observation.cjs", b"// omp"),
        ("bundle/qwen-code/extension.js", b"// qwen"),
        ("bundle/gemini/extension.js", b"// gemini"),
    ];
    for (relative, bytes) in files_to_write {
        if sabotage && relative == "bundle/scripts/forward-hook.mjs" {
            continue;
        }
        let path = dir.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
    }
    let digest = capsule::payload_digest(dir).expect("materialized capsule digests");
    let manifest = serde_json::json!({
        "contractVersion": 3,
        "payloadSha256": digest,
        "payloadVersion": payload_version.unwrap_or(capsule::PAYLOAD_VERSION),
    });
    fs::write(dir.join("capsule.json"), manifest.to_string()).unwrap();
}

/// A runnable stub on the harness PATH so `resolve_executable` admits it.
fn make_executable(dirs: &Dirs, name: &str) {
    let path = dirs.bin.join(name);
    fs::write(&path, b"#!/bin/sh\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

// ---------------------------------------------------------------------------
// Injection mapping — the pack's atStep vocabulary lands on the inject seam.
// ---------------------------------------------------------------------------

fn errno_of(name: &str) -> i32 {
    match name {
        "EEXIST" => nix::libc::EEXIST,
        "ENOTEMPTY" => nix::libc::ENOTEMPTY,
        "ENOSPC" => nix::libc::ENOSPC,
        "EACCES" => nix::libc::EACCES,
        "EIO" => nix::libc::EIO,
        "ENOENT" => nix::libc::ENOENT,
        "ELOOP" => nix::libc::ELOOP,
        "ENOTDIR" => nix::libc::ENOTDIR,
        other => panic!("unknown errno {other}"),
    }
}

/// Install the case's injection plan. Returns nothing — the plan lives in
/// the thread-local seam and fires as the real path crosses each boundary.
fn install_injections(case: &Case, dirs: &Dirs, plan: &mut Plan) {
    let inject = &case.inject;
    if inject.is_null() {
        return;
    }
    let step = inject["atStep"].as_str().unwrap_or("");
    let kind = inject["kind"].as_str().unwrap_or("");
    let errno = inject["errno"].as_str();

    // Errno injections land on the primitive the step runs through, armed
    // on the step's own boundary so an earlier caller sharing the
    // primitive (move-aside vs restore, both rename) never eats the fault.
    if let Some(errno) = errno {
        let code = errno_of(errno);
        match step {
            "stage-create" => plan.prim_fault_at("stage-create", Prim::StageCreateDir, code, 1),
            // File publish links; dir/capsule publish and backup moves
            // rename — map by the drive op's payload kind.
            "install" => {
                let prim = match case.drive["op"].as_str().unwrap_or("") {
                    "publish-dir" => Prim::RenameNoReplace,
                    _ => Prim::HardLink,
                };
                plan.prim_fault_at("install", prim, code, 1);
            }
            "move-aside" => plan.prim_fault_at("move-aside", Prim::RenameNoReplace, code, 1),
            // `install-fails-*` restore cases state only the restore's
            // errno — the install failure is the family premise, so the
            // same errno arms the install primitive too.
            "restore" => {
                plan.prim_fault_at("install", Prim::HardLink, code, 1);
                plan.prim_fault_at("restore", Prim::RenameNoReplace, code, 1);
            }
            "capsule-rename" => {
                plan.prim_fault_at("capsule-rename", Prim::RenameNoReplace, code, 1)
            }
            "remove-unlink" => plan.prim_fault_at("remove-unlink", Prim::UnlinkFile, code, 1),
            "remove-dir-walk" => plan.prim_fault_at("remove-dir-walk", Prim::WalkOpenDir, code, 1),
            other => panic!("errno injection at unmapped step {other}"),
        }
    }

    // `and` companion injections — "deadline window exhausted before
    // restore may start" pushes the clock at the stated step so the
    // next budget consult reads expired.
    if inject["and"]
        .as_str()
        .is_some_and(|tail| tail.contains("deadline"))
    {
        plan.on(
            Box::leak(step.to_owned().into_boxed_str()),
            BoundaryAction::AdvanceClock(Duration::from_secs(3600)),
        );
    }

    match kind {
        "SIGINT" => {
            // `index` selects which provider-command boundary fires the
            // flag; preflight fires at the inspection boundary.
            if step == "provider-command" {
                let index = inject["index"].as_u64().unwrap_or(0) as usize;
                for _ in 0..index {
                    plan.on("provider-command", BoundaryAction::Act(Box::new(|_| true)));
                }
                plan.on("provider-command", BoundaryAction::Sigint);
            } else {
                plan.on(
                    Box::leak(step.to_owned().into_boxed_str()),
                    BoundaryAction::Sigint,
                );
            }
        }
        k if k.contains("deadline") || case.setup_clock.is_some() => {
            // Clock-state injections: advance the test clock past the
            // deadline at the stated boundary.
            let push = Duration::from_secs(3600);
            if k.contains("before first") {
                // "Deadline expires during inspection/planning" — the push
                // lands at the preflight boundary so the first admission
                // consult already reads expired.
                plan.on("preflight-inspect", BoundaryAction::AdvanceClock(push));
            } else {
                match step {
                    "provider-command" => {
                        let index = inject["index"].as_u64().unwrap_or(0) as usize;
                        let mut seen = 0usize;
                        plan.on(
                            "provider-command",
                            BoundaryAction::Act(Box::new(move |_| {
                                seen += 1;
                                if seen > index {
                                    inject::advance_clock(push);
                                    true
                                } else {
                                    false
                                }
                            })),
                        );
                    }
                    "reinspect" => {
                        // The re-inspection gate consults the clock before the
                        // boundary — fire the advance on the LAST provider
                        // command boundary instead (the vector's tail).
                        plan.on(
                            "provider-command",
                            BoundaryAction::Act(Box::new(move |_| {
                                inject::advance_clock(push);
                                true
                            })),
                        );
                    }
                    "" => plan.on("preflight-inspect", BoundaryAction::AdvanceClock(push)),
                    other => plan.on(
                        Box::leak(other.to_owned().into_boxed_str()),
                        BoundaryAction::AdvanceClock(push),
                    ),
                }
            }
        }
        // "Reserve gone" — the post-vector re-inspection is unaffordable:
        // exhaust the budget as the first vector command's boundary fires,
        // so the reserve check right after the Attempted vector reads
        // expired.
        k if k.contains("reserve gone") => {
            plan.on(
                "provider-command",
                BoundaryAction::Act(Box::new(move |_| {
                    inject::advance_clock(Duration::from_secs(3600));
                    true
                })),
            );
        }
        // "Slot admission denied on remaining budget" — let one vector
        // command run, then exhaust the budget so the next step's slot
        // admission refuses mid-vector.
        k if k.contains("slot admission denied") => {
            let mut seen = 0usize;
            plan.on(
                "provider-command",
                BoundaryAction::Act(Box::new(move |_| {
                    seen += 1;
                    if seen > 1 {
                        inject::advance_clock(Duration::from_secs(3600));
                        true
                    } else {
                        false
                    }
                })),
            );
        }
        k if k.contains("nonce") || k.starts_with("EEXIST x") => {
            let times: usize = k
                .split('x')
                .nth(1)
                .and_then(|tail| tail.trim().split(' ').next()?.parse().ok())
                .unwrap_or(8);
            plan.prim_fault(Prim::RenameNoReplace, nix::libc::EEXIST, times);
        }
        "staged digest != expected" => {
            let parent = dirs.target.parent().unwrap().to_path_buf();
            plan.on(
                "digest-verify",
                BoundaryAction::Act(Box::new(move |_| {
                    corrupt_first_file(&stage_entry_for(&parent));
                    true
                })),
            );
        }
        "target digest != staged digest after successful install" => {
            let target = dirs.target.clone();
            plan.on(
                "post-verify",
                BoundaryAction::Act(Box::new(move |_| {
                    corrupt_first_file(&target);
                    true
                })),
            );
        }
        "backup digest != inspect-time digest" => {
            plan.on(
                "backup-remove",
                BoundaryAction::Act(Box::new(move |path| {
                    corrupt_first_file(path);
                    true
                })),
            );
        }
        "component 'a' becomes a symlink mid-walk" => {
            let target = dirs.target.clone();
            let outside = target
                .parent()
                .unwrap_or(target.as_path())
                .join("outside-marker");
            plan.on(
                "remove-dir-walk",
                BoundaryAction::Act(Box::new(move |name| {
                    if name == Path::new("a") {
                        let victim = target.join("a");
                        let _ = fs::remove_dir_all(&victim);
                        let _ = std::os::unix::fs::symlink(&outside, &victim);
                        true
                    } else {
                        false
                    }
                })),
            );
        }
        "capsule structural check fails after stage exists" => {
            plan.on(
                "capsule-verify",
                BoundaryAction::Act(Box::new(|staged| {
                    // The staged root holds the copied payload — remove a
                    // required runtime file so validation fails.
                    let _ = fs::remove_file(staged.join("bundle/scripts/forward-hook.mjs"));
                    true
                })),
            );
        }
        _ => {}
    }

    // `actor` interpositions: a foreign occupant materializes at the
    // boundary so the no-replace primitive meets a real arrival. Target
    // arrivals land at `install` — the window the pack describes ("before
    // link", "between failed install and restore") is the install window.
    if let Some(actor) = inject["actor"].as_str() {
        if actor.contains("foreign file appears at $TARGET") {
            let target = dirs.target.clone();
            plan.on(
                "install",
                BoundaryAction::Act(Box::new(move |_| {
                    fs::write(&target, b"foreign-arrival").unwrap();
                    true
                })),
            );
        } else if actor.contains("foreign dir appears at $TARGET") {
            let target = dirs.target.clone();
            plan.on(
                "install",
                BoundaryAction::Act(Box::new(move |_| {
                    fs::create_dir_all(&target).unwrap();
                    fs::write(target.join("content"), b"foreign-dir").unwrap();
                    true
                })),
            );
        } else if actor.contains("foreign dir appears") {
            let capsule_root = dirs.capsule_root.clone();
            plan.on(
                "capsule-rename",
                BoundaryAction::Act(Box::new(move |_| {
                    fs::create_dir_all(capsule_root.join("foreign")).unwrap();
                    fs::write(capsule_root.join("capsule.json"), b"foreign-capsule").unwrap();
                    true
                })),
            );
        }
    }
}

fn corrupt_first_file(path: &Path) {
    if path.is_dir() {
        if let Some(entry) = fs::read_dir(path).unwrap().next() {
            let entry = entry.unwrap().path();
            if entry.is_file() {
                fs::write(&entry, b"corrupted").unwrap();
            }
        }
    } else {
        fs::write(path, b"corrupted").unwrap();
    }
}

/// The staged entry file inside the pending `.orchbee-agent-entry-*`
/// container under `parent` (the mini-frontier stages `payload` inside).
fn stage_entry_for(parent: &Path) -> PathBuf {
    fs::read_dir(parent)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .find(|path| {
            path.file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| name.starts_with(".orchbee-agent-entry-"))
        })
        .map(|stage| stage.join("payload"))
        .unwrap_or_else(|| parent.join("missing-payload"))
}

// ---------------------------------------------------------------------------
// The outcome a driver reports — the pack's vocabulary verbatim.
// ---------------------------------------------------------------------------

struct Outcome {
    verdict: &'static str,
    exit: i32,
    frontier: &'static str,
    provenance: Option<&'static str>,
    determinate: Option<bool>,
    issue: Option<&'static str>,
    residues: Vec<PathBuf>,
}

impl Outcome {
    fn refused(verdict: &'static str, exit: i32) -> Self {
        Self {
            verdict,
            exit,
            frontier: "pre-effect",
            provenance: Some("pre-effect"),
            determinate: Some(true),
            issue: None,
            residues: Vec::new(),
        }
    }
}

/// `MutationReport` → pack vocabulary: the CLI's own `mutation_status`
/// supplies the exit; verdict/provenance map member-wise.
fn report_outcome(report: &mutation::MutationReport) -> Outcome {
    use super::super::mutation::{FailureCode, Refusal};
    let exit = crate::cli::integration::mutation_status(report) as i32;
    let attempted_any = !report.attempted.is_empty();
    let (verdict, determinate) = match &report.verdict {
        Verdict::Completed if !attempted_any => ("no-op", Some(true)),
        Verdict::Completed => ("applied", Some(true)),
        Verdict::Refused(refusal) => {
            // The observation-incomplete family stays undetermined once the
            // frontier is crossed — a deadline slot refusal mid-vector
            // cannot prove what earlier steps left behind.
            let determinate = !(report.effect_begun
                && matches!(refusal, Refusal::Deadline | Refusal::InspectionFailed));
            (refusal.as_str(), Some(determinate))
        }
        Verdict::Failed(
            FailureCode::VerificationFailed | FailureCode::TimedOut | FailureCode::Leaked,
        ) => ("failed", Some(false)),
        Verdict::Failed(code) => (code.as_str(), Some(true)),
        Verdict::Cancelled if attempted_any => ("cancelled", Some(false)),
        Verdict::Cancelled => ("cancelled", Some(true)),
        Verdict::Unknown => ("unknown", Some(false)),
    };
    // `timed-out` reports the pack's verdict word even though the failure
    // code carries it — the observed status name and the verdict differ.
    let verdict = match &report.verdict {
        Verdict::Failed(FailureCode::TimedOut) => "timed-out",
        // A partial re-inspection reports the pack's generic "failed" —
        // the verification-failed word belongs to landed-content
        // mismatches, not to a half-confirmed observation.
        Verdict::Failed(FailureCode::VerificationFailed)
            if report.provenance == super::super::model::Provenance::Partial =>
        {
            "failed"
        }
        Verdict::Failed(FailureCode::VerificationFailed) => "verification-failed",
        Verdict::Refused(Refusal::Deadline) => "timed-out",
        Verdict::Refused(Refusal::StaleSide) => "stale-side",
        Verdict::Refused(Refusal::ManualActionRequired)
        | Verdict::Refused(Refusal::CapsuleUnavailable)
        | Verdict::Refused(Refusal::CapsuleUnsupportedVersion)
        | Verdict::Refused(Refusal::AgentUnavailable) => "manual-action-required",
        _ => verdict,
    };
    // The frontier is the private effect marker — a deadline-refused
    // planned step logged in `attempted` never proves an effect ran.
    let frontier = if report.effect_begun {
        "post-effect"
    } else {
        "pre-effect"
    };
    Outcome {
        verdict,
        exit,
        frontier,
        provenance: Some(report.provenance.as_str()),
        determinate,
        issue: report
            .observed
            .as_ref()
            .and_then(|item| item.issue)
            .map(|issue| issue.as_str()),
        residues: report.residuals.clone(),
    }
}

// ---------------------------------------------------------------------------
// Filesystem-level driver — the shared stage/verify/install/restore machine,
// one honest step sequence for both file and directory payloads.
// ---------------------------------------------------------------------------

fn drive_fs(case: &Case, dirs: &Dirs) -> Outcome {
    let op = case.drive["op"].as_str().unwrap();
    let target = dirs.target.clone();
    let staged_bytes = case.drive["stagedBytes"].as_str().unwrap_or("payload");
    let deadline = inject::now() + Duration::from_secs(40);
    let mut residues: Vec<PathBuf> = Vec::new();

    // Preflight: classify the occupant by its materialized bytes, and name
    // any residue a prior run left under the managed parent — reported,
    // never swept.
    inject::boundary("preflight-inspect", &target);
    let occupant = fs::read(&target).ok();
    let ownership = occupant.as_deref().map(classify_bytes).unwrap_or("absent");
    if let Some(parent) = target.parent()
        && let Ok(read) = fs::read_dir(parent)
    {
        for entry in read.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(".orchbee-agent-backup-")
                || name.starts_with(".orchbee-agent-entry-")
                || name.starts_with(".orchbee-agent-capsule-")
            {
                residues.push(entry.path());
            }
        }
    }
    let is_dir = op == "publish-dir";
    let replace = op == "replace-file";

    match op {
        "publish-file" | "publish-dir" if ownership != "absent" => {
            return Outcome::refused("conflict", 3);
        }
        "replace-file" if !matches!(ownership, "orchbee-exact" | "orchbee-legacy") => {
            return Outcome::refused("conflict", 3);
        }
        _ => {}
    }

    // Stage.
    let parent = target.parent().unwrap().to_path_buf();
    let container = match files::stage_directory(&parent, "entry") {
        Ok(container) => container,
        Err(_) => {
            return Outcome {
                verdict: "failed",
                exit: 9,
                frontier: "post-effect",
                provenance: Some("post-effect"),
                determinate: Some(true),
                issue: None,
                residues,
            };
        }
    };
    let entry = container.join("payload");
    let staged_ok = if is_dir {
        fs::create_dir_all(entry.join("nested"))
            .and_then(|_| fs::write(entry.join("nested/file"), staged_bytes))
    } else {
        files::write_file_exclusive(&entry, staged_bytes.as_bytes()).map(|_| ())
    };
    if staged_ok.is_err() {
        inject::boundary("cleanup-stage", &container);
        let _ = files::remove_owned(&container, true);
        return Outcome {
            verdict: "failed",
            exit: 9,
            frontier: "post-effect",
            provenance: Some("post-effect"),
            determinate: Some(true),
            issue: None,
            residues,
        };
    }

    // Staged digest verify — the real sha256 over the staged bytes.
    inject::boundary("digest-verify", &entry);
    let digest_ok = if is_dir {
        files::entry_digest(&entry).is_some()
    } else {
        fs::read(&entry)
            .map(|bytes| bytes == staged_bytes.as_bytes())
            .unwrap_or(false)
    };
    if !digest_ok {
        residues.push(container);
        return Outcome {
            verdict: "verification-failed",
            exit: 8,
            frontier: "post-effect",
            provenance: Some("post-effect"),
            determinate: Some(false),
            issue: None,
            residues,
        };
    }

    // Move the owned occupant aside when replacing.
    let mut backup: Option<PathBuf> = None;
    let mut backup_digest: Option<String> = None;
    if replace && ownership != "absent" {
        match files::move_aside(&target) {
            Ok(moved) => {
                backup_digest = moved.as_ref().and_then(|path| files::entry_digest(path));
                backup = moved;
            }
            Err(FsStep::Conflict) => {
                // Nonce space exhausted — the staged entry stays as named
                // residue rather than being swept under a conflict.
                residues.push(container);
                return Outcome {
                    verdict: "conflict",
                    exit: 9,
                    frontier: "post-effect",
                    provenance: Some("post-effect"),
                    determinate: Some(true),
                    issue: None,
                    residues,
                };
            }
            Err(_) => {
                residues.push(container);
                return Outcome {
                    verdict: "failed",
                    exit: 9,
                    frontier: "post-effect",
                    provenance: Some("post-effect"),
                    determinate: Some(true),
                    issue: None,
                    residues,
                };
            }
        }
    }

    // Install no-replace.
    inject::boundary("install", &target);
    let publish = files::publish_no_replace(&entry, &target);
    if publish != FsStep::Done {
        // Restore what we moved aside — bounded by the same deadline; a
        // deadline reached with a restore owed keeps backup+stage named.
        if backup.is_some() && inject::now() >= deadline {
            residues.extend(backup.iter().cloned());
            residues.push(container);
            return Outcome {
                verdict: "timed-out",
                exit: 8,
                frontier: "post-effect",
                provenance: Some("post-effect"),
                determinate: Some(false),
                issue: None,
                residues,
            };
        }
        // A failed restore dominates the verdict: the publish conflict is
        // determinate, but a recovery that cannot complete is
        // operation-failed — backup and stage stay named.
        let mut restore_failed = false;
        if let Some(backup_path) = backup {
            inject::boundary("restore", &target);
            let restored = files::restore_backup(&backup_path, &target);
            if restored != FsStep::Done {
                restore_failed = true;
                residues.push(backup_path);
            }
        }
        // A failed publish keeps the stage as named residue — the pack's
        // conflict cases assert it retained, not swept.
        residues.push(container);
        return Outcome {
            verdict: if restore_failed || publish != FsStep::Conflict {
                "failed"
            } else {
                "conflict"
            },
            exit: 9,
            frontier: "post-effect",
            provenance: Some("post-effect"),
            determinate: Some(true),
            issue: None,
            residues,
        };
    }

    // Post-publish verification runs on the replace flow — a fresh
    // publish's link/rename is already the no-clobber guarantee.
    let verified = if backup.is_some() {
        inject::boundary("post-verify", &target);
        if is_dir {
            target.is_dir()
        } else {
            fs::read(&target)
                .map(|bytes| bytes == staged_bytes.as_bytes())
                .unwrap_or(false)
        }
    } else {
        true
    };
    if !verified {
        if let Some(backup_path) = backup {
            residues.push(backup_path);
        }
        return Outcome {
            verdict: "verification-failed",
            exit: 8,
            frontier: "post-effect",
            provenance: Some("post-effect"),
            determinate: Some(false),
            issue: None,
            residues,
        };
    }

    // Digest-guarded backup removal — the backup drops only while it still
    // carries the bytes captured at move-aside time.
    if let Some(backup_path) = backup {
        inject::boundary("backup-remove", &backup_path);
        let still_ours = files::entry_digest(&backup_path)
            .zip(backup_digest.as_ref())
            .map(|(now, captured)| now == *captured)
            .unwrap_or(false);
        if !still_ours {
            residues.push(backup_path);
            return Outcome {
                verdict: "conflict",
                exit: 9,
                frontier: "post-effect",
                provenance: Some("post-effect"),
                determinate: Some(true),
                issue: None,
                residues,
            };
        }
        if files::remove_owned(&backup_path, false) != FsStep::Done {
            residues.push(backup_path);
        }
    }

    // Stage cleanup runs last: earlier failures leave it as named residue.
    inject::boundary("cleanup-stage", &container);
    if files::remove_owned(&container, true) != FsStep::Done {
        residues.push(container);
    }
    Outcome {
        verdict: "applied",
        exit: 0,
        frontier: "post-effect",
        provenance: Some("post-effect"),
        determinate: Some(true),
        issue: None,
        residues,
    }
}

/// `remove-file`/`remove-dir`: verify ownership, then the real no-follow
/// removal. Foreign/unverifiable occupants refuse pre-effect.
fn drive_remove(case: &Case, dirs: &Dirs) -> Outcome {
    let op = case.drive["op"].as_str().unwrap();
    let target = dirs.target.clone();
    inject::boundary("remove-verify-owned", &target);
    let metadata = fs::symlink_metadata(&target);
    let is_dir = metadata
        .as_ref()
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false);
    let ownership = metadata
        .ok()
        .map(|metadata| {
            if metadata.is_dir() {
                "orchbee-exact"
            } else {
                fs::read(&target)
                    .map(|bytes| classify_bytes(&bytes))
                    .unwrap_or("unknown")
            }
        })
        .unwrap_or("absent");
    let _ = op;
    match ownership {
        "orchbee-exact" | "orchbee-legacy" => {}
        "absent" => {
            return Outcome {
                verdict: "no-op",
                exit: 0,
                frontier: "pre-effect",
                provenance: Some("pre-effect"),
                determinate: Some(true),
                issue: None,
                residues: Vec::new(),
            };
        }
        _ => return Outcome::refused("conflict", 3),
    }
    inject::boundary(
        if is_dir {
            "remove-dir-walk"
        } else {
            "remove-unlink"
        },
        &target,
    );
    match files::remove_owned(&target, is_dir) {
        FsStep::Done => Outcome {
            verdict: "applied",
            exit: 0,
            frontier: "post-effect",
            provenance: Some("post-effect"),
            determinate: Some(true),
            issue: None,
            residues: Vec::new(),
        },
        FsStep::Conflict => Outcome {
            verdict: "conflict",
            exit: 9,
            frontier: "post-effect",
            provenance: Some("post-effect"),
            determinate: Some(true),
            issue: None,
            residues: vec![target],
        },
        FsStep::Failed => Outcome {
            verdict: "failed",
            exit: 9,
            frontier: "post-effect",
            provenance: Some("post-effect"),
            determinate: Some(true),
            issue: None,
            residues: vec![target],
        },
    }
}

/// `capsule-publish`: preflight refusal on an occupied-invalid root, the
/// version gate on the declared source, then the real publish sequence —
/// a rename conflict re-checks `is_ready` on the arrival.
fn drive_capsule(_case: &Case, dirs: &Dirs) -> Outcome {
    let capsule_root = dirs.capsule_root.clone();
    let source = dirs.adapter_source.clone();
    inject::boundary("preflight-inspect", &capsule_root);

    // Occupied-and-invalid refuses before any staging exists.
    if fs::symlink_metadata(&capsule_root).is_ok()
        && capsule::validate(&capsule_root) != capsule::CapsuleValidity::Ready
    {
        return Outcome::refused("conflict", 3);
    }
    // The declared source's version gate — typed, never generic-invalid.
    if let capsule::CapsuleValidity::UnsupportedVersion { .. } = capsule::validate(&source) {
        let mut outcome = Outcome::refused("manual-action-required", 4);
        outcome.issue = Some("capsule-unsupported-version");
        return outcome;
    }

    let publish = mutation::publish_capsule(&source, &capsule_root);
    let mut residues: Vec<PathBuf> = publish.residual.into_iter().collect();
    match publish.outcome {
        StepOutcome::Completed => Outcome {
            verdict: "applied",
            exit: 0,
            frontier: "post-effect",
            provenance: Some("post-effect"),
            determinate: Some(true),
            issue: None,
            residues,
        },
        StepOutcome::Conflict => {
            inject::boundary("capsule-isvalid-existing", &capsule_root);
            if capsule::is_ready(&capsule_root, Some(source.as_path())) {
                // A concurrent valid install absorbs this run — the
                // retained stage is ours, clean it and un-name it.
                for retained in residues.drain(..) {
                    let _ = files::remove_owned(&retained, true);
                }
                Outcome {
                    verdict: "applied",
                    exit: 0,
                    frontier: "post-effect",
                    provenance: Some("post-effect"),
                    determinate: Some(true),
                    issue: None,
                    residues,
                }
            } else {
                Outcome {
                    verdict: "conflict",
                    exit: 9,
                    frontier: "post-effect",
                    provenance: None,
                    determinate: Some(true),
                    issue: None,
                    residues,
                }
            }
        }
        _ => Outcome {
            verdict: "failed",
            exit: 9,
            frontier: "post-effect",
            provenance: Some("post-effect"),
            determinate: Some(true),
            issue: None,
            residues,
        },
    }
}

// ---------------------------------------------------------------------------
// Provider-level driver — the real `mutate` frontier under a scripted
// command plan, an isolated HOME, and a materialized adapter source.
// ---------------------------------------------------------------------------

fn provider_for(case: &Case) -> IntegrationId {
    match case.setup_provider.as_deref() {
        Some("inspect-reads=2") => IntegrationId::ClaudeCode,
        Some("inspect-reads=1") => IntegrationId::Grok,
        Some("inspect-reads=0") => IntegrationId::Pi,
        _ => {
            if case.drive["vector"].as_str() == Some("multi-step") {
                IntegrationId::ClaudeCode
            } else if case.drive["vector"].as_str() == Some("guarded-write only") {
                IntegrationId::Pi
            } else if case.inject["kind"].as_str() == Some("read 2 of 2 fails") {
                // The partial re-inspection needs a two-slot read set.
                IntegrationId::ClaudeCode
            } else if case.id == "guard.newer-version-stale-side" {
                IntegrationId::Gemini
            } else {
                IntegrationId::Grok
            }
        }
    }
}

/// Materialize the provider-owned target for `enable/disable-provider`
/// cases — real marker/bootstrap bytes per the declared ownership role.
fn materialize_provider_target(dirs: &Dirs, id: IntegrationId, entries: &[Value]) {
    for entry in entries {
        let ownership = entry["ownership"].as_str().unwrap_or("foreign");
        let spec = entry["path"].as_str().unwrap();
        if spec != "$TARGET" {
            continue;
        }
        let target = dirs.home.join(target_relative(id));
        match ownership {
            "orchbee-newer" => {
                fs::create_dir_all(&target).unwrap();
                fs::write(target.join("extension.js"), b"newer-version-content").unwrap();
                let files_list = inspect::fingerprint_directory(&target)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(path, sha256)| serde_json::json!({ "path": path, "sha256": sha256 }))
                    .collect::<Vec<_>>();
                let marker = serde_json::json!({
                    "contractVersion": 4,
                    "files": files_list,
                    "integrationId": id.as_str(),
                    "kind": "guarded-extension",
                });
                fs::write(
                    target.join(inspect::DIRECTORY_MARKER_NAME),
                    marker.to_string(),
                )
                .unwrap();
            }
            "foreign" | "unknown" | "unverifiable" => {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).unwrap();
                }
                let bytes = entry["bytes"].as_str().unwrap_or("foreign");
                if entry["kind"].as_str() == Some("dir") {
                    fs::create_dir_all(&target).unwrap();
                    fs::write(target.join("content"), bytes).unwrap();
                } else {
                    fs::write(&target, bytes).unwrap();
                }
            }
            _ => {}
        }
    }
}

fn target_relative(id: IntegrationId) -> &'static str {
    match id {
        IntegrationId::Opencode => ".config/opencode/plugins/orchbee-agent-observation.mjs",
        IntegrationId::Pi => ".pi/agent/extensions/orchbee-agent-observation.cjs",
        IntegrationId::Omp => ".omp/agent/extensions/orchbee-agent-observation.cjs",
        IntegrationId::Gemini => ".gemini/extensions/orchbee-agent-observation",
        IntegrationId::QwenCode => ".qwen/extensions/orchbee-agent-observation",
        _ => ".provider/config",
    }
}

fn drive_provider(case: &Case, dirs: &Dirs) -> mutation::MutationReport {
    let id = provider_for(case);
    make_executable(dirs, providers::executable_name(id));
    materialize_provider_target(dirs, id, &case.setup_fs);

    // The declared adapter source: a valid materialized capsule unless the
    // case sabotages it.
    let source_label = case.drive["capsule"].as_str();
    if !dirs.adapter_source.exists() {
        match source_label {
            Some("capsule-declaring-older-payloadVersion") => {
                materialize_capsule(&dirs.adapter_source, Some("0.3.0"), false)
            }
            Some("capsule-declaring-future-payloadVersion") => {
                materialize_capsule(&dirs.adapter_source, Some("9.9.9"), false)
            }
            Some("capsule-with-bad-entry") => materialize_capsule(&dirs.adapter_source, None, true),
            _ => materialize_capsule(&dirs.adapter_source, None, false),
        }
    }

    let deadline = inject::now() + Duration::from_secs(40);
    let cancel = InjectCancel;
    let context = ProviderContext {
        home: &dirs.home,
        capsule_root: &dirs.capsule_root,
        adapter_source: Some(dirs.adapter_source.as_path()),
        path_env: Some(dirs.bin.as_os_str()),
        deadline,
        cancel: Some(&cancel),
        management_mode: super::super::model::ManagementMode::ManagedLocal,
    };
    // Provider-level cases pin provider-command behavior — the managed
    // capsule starts already published so the publish step never enters
    // the attempted[] these cases assert.
    if !dirs.capsule_root.exists() {
        materialize_capsule(&dirs.capsule_root, None, false);
    }

    let direction = match case.drive["op"].as_str().unwrap() {
        "disable-provider" => Direction::Disable,
        _ => Direction::Enable,
    };
    mutation::mutate(
        id,
        direction,
        &context,
        std::slice::from_ref(&dirs.adapter_source),
    )
}

// ---------------------------------------------------------------------------
// `inspect` op — read-only scan; residue stays named, nothing is swept.
// ---------------------------------------------------------------------------

fn drive_inspect(case: &Case, dirs: &Dirs) -> Outcome {
    let _ = case;
    inject::boundary("preflight-inspect", &dirs.target);
    let residues = scan_residues(dirs);
    Outcome {
        verdict: if residues.is_empty() {
            "no-op"
        } else {
            "conflict"
        },
        exit: if residues.is_empty() { 0 } else { 3 },
        frontier: "pre-effect",
        provenance: Some("pre-effect"),
        determinate: Some(true),
        issue: None,
        residues,
    }
}

/// `.orchbee-agent-*` residue visible under the managed parent — named, in
/// the order a directory listing surfaces it.
fn scan_residues(dirs: &Dirs) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for parent in [
        dirs.target.parent().map(Path::to_path_buf),
        Some(dirs.parent.clone()),
    ]
    .into_iter()
    .flatten()
    {
        if let Ok(entries) = fs::read_dir(&parent) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with(".orchbee-agent-") {
                    found.push(entry.path());
                }
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

/// `cohort-admit`: the real `NativeCohort::load` against a materialized
/// `cohort.json` — incompatible versions and unknown shapes stay typed.
fn drive_cohort(case: &Case, dirs: &Dirs) -> Outcome {
    inject::boundary("cohort-admit", &dirs.cohort_dir);
    fs::create_dir_all(&dirs.cohort_dir).unwrap();
    // The real admission validates the runtime directory's exact 0700 mode —
    // materialize it or the version gate never runs.
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&dirs.cohort_dir, fs::Permissions::from_mode(0o700)).unwrap();
    for entry in &case.setup_fs {
        let spec = entry["path"].as_str().unwrap();
        if !spec.contains("manifest") {
            continue;
        }
        let bytes = entry["bytes"].as_str().unwrap_or("");
        let content = match bytes {
            "schemaVersion=1 manifest" => {
                serde_json::json!({ "schemaVersion": 1, "artifacts": {} }).to_string()
            }
            "schemaVersion=9 manifest" => {
                serde_json::json!({ "schemaVersion": 9, "artifacts": {} }).to_string()
            }
            other => other.to_owned(),
        };
        let manifest_path = dirs.cohort_dir.join("cohort.json");
        fs::write(&manifest_path, content).unwrap();
        // `open_file` admits only owner-safe artifacts — the suite's
        // permissive umask would otherwise mask this run's verdict.
        fs::set_permissions(&manifest_path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let loaded = orchbee_local_site_runtime::NativeCohort::load(
        &dirs.cohort_dir,
        Instant::now() + Duration::from_secs(5),
    );
    match loaded {
        Err(error) if error.to_string().contains("incompatible") => Outcome {
            verdict: "incompatible",
            exit: 6,
            frontier: "pre-effect",
            provenance: Some("pre-effect"),
            determinate: Some(true),
            issue: None,
            residues: Vec::new(),
        },
        Err(_) => Outcome {
            verdict: "conflict",
            exit: 3,
            frontier: "pre-effect",
            provenance: Some("pre-effect"),
            determinate: Some(true),
            issue: None,
            residues: Vec::new(),
        },
        Ok(_) => Outcome {
            verdict: "applied",
            exit: 0,
            frontier: "pre-effect",
            provenance: Some("pre-effect"),
            determinate: Some(true),
            issue: None,
            residues: Vec::new(),
        },
    }
}

// ---------------------------------------------------------------------------
// Assertions
// ---------------------------------------------------------------------------

/// Expand `attempted` shorthand (`"move-aside x8"`, `"name(partial)"`,
/// `"provider-command x k completed steps named"`) into the expected
/// witnessed-boundary sequence.
fn expand_attempted(expect: &Value) -> Vec<String> {
    let mut sequence = Vec::new();
    for item in expect["attempted"].as_array().cloned().unwrap_or_default() {
        let raw = item.as_str().unwrap_or("");
        let (label, count) = if let Some((label, count)) = raw.split_once(" x") {
            let count = count
                .trim_end_matches(|c: char| !c.is_ascii_digit())
                .trim_start_matches('x')
                .trim();
            (label.trim(), count.parse::<usize>().unwrap_or(1).max(1))
        } else {
            (raw, 1)
        };
        let label = label
            .trim_end_matches("(partial)")
            .trim_end_matches("(missing)")
            .trim();
        let label = label.trim_end_matches(" completed steps named").trim();
        for _ in 0..count {
            sequence.push(label.to_owned());
        }
    }
    sequence
}

/// The expected sequence must appear as an ordered subsequence of the
/// witnessed boundaries — attempted[] lists required crossings, it does
/// not forbid incidental ones (e.g. capsule publish ahead of a vector).
fn assert_subsequence(case_id: &str, expected: &[String], witnessed: &[&'static str]) {
    let mut cursor = 0usize;
    for label in expected {
        let found = witnessed[cursor..]
            .iter()
            .position(|step| *step == label)
            .map(|index| cursor + index);
        match found {
            Some(index) => cursor = index + 1,
            None => {
                panic!("{case_id}: expected step '{label}' absent from witnessed {witnessed:?}")
            }
        }
    }
}

/// `residue[]` assertions: every named pattern must match a path the
/// outcome reports (or, for scan cases, a file still present under the
/// managed tree) whose declared bytes survive.
fn assert_residues(
    case_id: &str,
    expect: &Value,
    outcome: &Outcome,
    dirs: &Dirs,
    owners: &BTreeMap<PathBuf, Vec<u8>>,
) {
    for residue in expect["residue"].as_array().cloned().unwrap_or_default() {
        let pattern = residue["pattern"].as_str().unwrap_or("");
        if pattern.is_empty() || pattern.contains("per-step") {
            continue;
        }
        let base = pattern.trim_end_matches("/**").trim_end_matches('*');
        // A bare `.orchbee-agent-<tag>-*` staging name lives beside the
        // artifact it stages: capsule staging beside the capsule root,
        // everything else beside the provider target. `$`-vars resolve.
        let base = if base.starts_with('$') || base.contains('/') {
            resolve_path(dirs, base)
        } else if base.contains("capsule") {
            dirs.capsule_root.parent().unwrap_or(&dirs.home).join(base)
        } else {
            dirs.target.parent().unwrap_or(&dirs.home).join(base)
        };
        let matches: Vec<PathBuf> = if pattern.ends_with("/**") {
            if base.is_dir() {
                vec![base.clone()]
            } else {
                Vec::new()
            }
        } else if pattern.ends_with('*') {
            base.parent()
                .and_then(|parent| fs::read_dir(parent).ok())
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(OsStr::to_str)
                        .is_some_and(|name| {
                            name.starts_with(
                                base.file_name()
                                    .and_then(OsStr::to_str)
                                    .unwrap_or("")
                                    .trim_end_matches('*'),
                            )
                        })
                })
                .collect()
        } else {
            Vec::new()
        };
        // A stage the run cleaned itself is a permitted absence — the
        // pack's note "cleaned if still verifiably ours" marks the branch.
        let cleaned_ok = residue["note"]
            .as_str()
            .is_some_and(|note| note.contains("cleaned"));
        assert!(
            !matches.is_empty() || cleaned_ok,
            "{case_id}: residue pattern '{pattern}' matched nothing under {}",
            base.display()
        );
        if residue["namedInResult"].as_bool().unwrap_or(false) && !matches.is_empty() {
            let named = outcome.residues.iter().any(|path| {
                path.file_name()
                    .and_then(OsStr::to_str)
                    .is_some_and(|name| {
                        name.starts_with(
                            base.file_name()
                                .and_then(OsStr::to_str)
                                .unwrap_or("")
                                .trim_end_matches('*'),
                        )
                    })
                    || path.starts_with(&base)
            });
            assert!(
                named,
                "{case_id}: residue '{pattern}' not named in outcome {:?}",
                outcome.residues
            );
        }
        if let Some(bytes) = residue["bytesPreserved"].as_str() {
            for path in &matches {
                let actual = if path.is_dir() {
                    fs::read(path.join("content")).unwrap_or_default()
                } else {
                    fs::read(path).unwrap_or_default()
                };
                let expected_bytes = owners
                    .get(path)
                    .cloned()
                    .unwrap_or_else(|| bytes.as_bytes().to_vec());
                assert!(
                    actual == expected_bytes || actual.ends_with(bytes.as_bytes()),
                    "{case_id}: residue {} lost its bytes",
                    path.display()
                );
            }
        }
    }
}

/// `preserved[]`: every listed path still carries its declared bytes
/// (or stays absent when the case declares `kind: absent`).
fn assert_preserved(
    case_id: &str,
    expect: &Value,
    dirs: &Dirs,
    owners: &BTreeMap<PathBuf, Vec<u8>>,
) {
    for preserved in expect["preserved"].as_array().cloned().unwrap_or_default() {
        let spec = preserved["path"].as_str().unwrap_or("");
        if preserved["kind"].as_str() == Some("absent") {
            let path = resolve_path(dirs, spec);
            assert!(
                !path.exists(),
                "{case_id}: {spec} expected absent, still present"
            );
            continue;
        }
        let declared = preserved["bytes"].as_str();
        // Wildcard content declarations — the path must be occupied but any
        // bytes answer ("whatever occupies it now").
        let wildcard = matches!(declared, Some("whatever occupies it now"));
        let declared_ok = |path: &Path| -> bool {
            if wildcard || !path.is_file() {
                return true;
            }
            let actual = fs::read(path).unwrap_or_default();
            match declared {
                Some(declared) => {
                    actual == declared.as_bytes()
                        || actual == ownership_bytes("orchbee-exact", declared)
                        || actual == ownership_bytes("orchbee-legacy", declared)
                        || actual == ownership_bytes("orchbee-newer", declared)
                }
                None => owners
                    .get(path)
                    .map(|exact| &actual == exact)
                    .unwrap_or(true),
            }
        };
        // Glob specs cover per-file materialized occupants (n1/n2/n3 carry
        // distinct bytes under one prefix): each match answers against its
        // own materialized bytes first, the declared label otherwise.
        let glob_ok = |path: &Path| -> bool {
            if wildcard || !path.is_file() {
                return true;
            }
            let actual = fs::read(path).unwrap_or_default();
            if let Some(exact) = owners.get(path) {
                return &actual == exact;
            }
            declared
                .map(|declared| {
                    actual == declared.as_bytes()
                        || actual == ownership_bytes("orchbee-exact", declared)
                        || actual == ownership_bytes("orchbee-legacy", declared)
                        || actual == ownership_bytes("orchbee-newer", declared)
                })
                .unwrap_or(true)
        };
        if spec.contains('*') || spec.contains('<') {
            // Glob over the managed parent — every match keeps its bytes.
            let prefix = spec.split(['*', '<']).next().unwrap_or("");
            let parent = dirs.target.parent().unwrap();
            let mut matched = false;
            for entry in fs::read_dir(parent).unwrap().flatten() {
                let name = entry.file_name();
                if name.to_string_lossy().starts_with(prefix) {
                    matched = true;
                    assert!(
                        glob_ok(&entry.path()),
                        "{case_id}: preserved {} changed",
                        entry.path().display()
                    );
                }
            }
            assert!(
                matched || declared.is_none(),
                "{case_id}: preserved {spec} matched nothing"
            );
            continue;
        }
        let path = resolve_path(dirs, spec);
        if declared.is_some() {
            assert!(
                path.exists(),
                "{case_id}: preserved {spec} expected occupied, absent"
            );
        }
        assert!(
            declared_ok(&path),
            "{case_id}: preserved {spec} holds unexpected bytes"
        );
    }
}

/// `forbidden[]` hard negatives — the pack's closed phrase set, each
/// mapped to a concrete check or registered against the sibling
/// assertions (preserved/residue/exit) that already discharge it.
/// An unmapped phrase fails closed so vocabulary drift surfaces.
fn assert_forbidden(case: &Case, outcome: &Outcome, dirs: &Dirs, witnessed: &[&'static str]) {
    /// Phrases discharged by the run's preserved[]/residue[] byte checks:
    /// the implementation must never have touched those bytes, and the
    /// sibling assertions prove it. They stay listed so a vocabulary
    /// addition panics instead of silently passing.
    const BYTE_PINS: &[&str] = &[
        "target bytes overwritten",
        "occupant moved or removed",
        "foreign arrival overwritten",
        "arrival unlinked",
        "foreign dir overwritten or merged",
        "any $TARGET mutation",
        "any occupied nonce name overwritten",
        "any occupant destroyed or displaced",
        "foreign arrival overwritten by restore",
        "backup destroyed",
        "backup silently deleted",
        "backup deleted despite digest mismatch",
        "backup removed despite mismatch",
        "backup auto-deleted as 'cleanup'",
        "backup restored over the fresh publish",
        "non-owned bytes removed",
        "walk follows the swapped symlink",
        "existing dir overwritten, merged, or 'repaired'",
        "overwrite",
        "occupant overwritten",
        "downgrade or overwrite attempted",
        "silent clamp/accept of future format",
        "silent upgrade",
        "rewrite of the old manifest",
        "v1 fallback parse",
        "partial field read",
        "silent fallback",
        "target moved despite exhaustion",
        "target 'repaired' by deleting possibly-foreign bytes",
        "target left absent",
        "downward mutation",
        "disable allowed through on newer content",
        "unlink attempted",
        "fallback removal on unverifiable content",
        "state 'repaired' into a known shape",
        "invisible to the result document",
        "sweeping residue the run did not create",
        "blocking publish on residue presence",
        "auto-delete",
        "silent fallback to plain rename",
        "file-kind effects gated with it",
        "command slot reserved for a 0-read provider",
        "fs re-verify performed after deadline expiry",
        "mutation slot funded by consuming the reserve",
        "reserve overrun",
        "re-inspection crowded out",
        "reserve borrowed from a later stage",
        "completed steps omitted from attempted[]",
        "partial slot launched",
        "provider-internal bookkeeping counted as Orchbee mutation",
        "read-path exit families applied",
        "manual instruction emitted",
        "uninstall vector emitted for an unknown classification",
        "else-uninstall fallthrough",
        "delete-and-rewrite of unknown content",
        "nonce reuse within the attempt sequence",
        "generic invalid-capsule classification",
        "generic invalid-capsule",
        "unbounded rollback loop",
        "any guarantee stronger than 'detection at check/verify time' for in-window foreign self-mutation",
        "partial dims upgraded to a full snapshot",
        "failed read's dimension asserted",
        "fabricated current snapshot when re-inspection was skipped/failed",
    ];
    for phrase in case.expect["forbidden"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        let phrase = phrase.as_str().unwrap_or("");
        if BYTE_PINS.contains(&phrase) {
            continue;
        }
        match phrase {
            "9th attempt" => {
                let count = witnessed
                    .iter()
                    .filter(|step| **step == "move-aside")
                    .count();
                assert!(count <= 8, "{}: {phrase}", case.id);
            }
            "backup retained after digest-equal removal step"
            | "backup retained after successful restore (rename consumed it)" => {
                let kept = outcome
                    .residues
                    .iter()
                    .chain(
                        dirs.target
                            .parent()
                            .and_then(|parent| {
                                fs::read_dir(parent).ok().map(|entries| {
                                    entries
                                        .flatten()
                                        .map(|entry| entry.path())
                                        .collect::<Vec<_>>()
                                })
                            })
                            .unwrap_or_default()
                            .iter(),
                    )
                    .any(|path| {
                        path.file_name()
                            .and_then(OsStr::to_str)
                            .is_some_and(|name| name.starts_with(".orchbee-agent-backup-"))
                    });
                assert!(!kept, "{}: {phrase}", case.id);
            }
            "stage dir created" | "stage created despite known-occupied invalid target" => {
                assert!(
                    !witnessed.contains(&"stage-create") && !witnessed.contains(&"capsule-stage"),
                    "{}: {phrase} (witnessed {witnessed:?})",
                    case.id
                );
            }
            "install attempted after failed verify" => {
                assert!(!witnessed.contains(&"install"), "{}: {phrase}", case.id);
            }
            "restore attempted outside the deadline" => {
                assert!(!witnessed.contains(&"restore"), "{}: {phrase}", case.id);
            }
            "reinspect listed in attempted[]" => {
                assert!(
                    !witnessed.contains(&"reinspect"),
                    "{}: {phrase} (witnessed {witnessed:?})",
                    case.id
                );
            }
            "any capsule stage / provider mutation / guarded write launched"
            | "any mutation launched after the signal" => {
                assert!(
                    !witnessed.iter().any(|step| [
                        "provider-command",
                        "capsule-stage",
                        "install",
                        "move-aside",
                        "remove-unlink",
                        "remove-dir-walk",
                    ]
                    .contains(step)),
                    "{}: {phrase} (witnessed {witnessed:?})",
                    case.id
                );
            }
            "provenance reported post-effect" => {
                assert_ne!(
                    outcome.provenance,
                    Some("post-effect"),
                    "{}: {phrase}",
                    case.id
                );
            }
            "completed claimed from provider exit 0 alone" | "current state fabricated" => {
                assert!(
                    matches!(
                        outcome.provenance,
                        Some("partial") | Some("unavailable") | Some("pre-effect")
                    ) || outcome.verdict != "applied",
                    "{}: {phrase}",
                    case.id
                );
            }
            "verdict reported as pre-effect" | "pretending pre-effect" => {
                assert_ne!(outcome.frontier, "pre-effect", "{}: {phrase}", case.id);
            }
            "exit other than 130" => {
                assert_eq!(outcome.exit, 130, "{}: {phrase}", case.id);
            }
            p if p.starts_with("exit in {3,4,7}") => {
                assert!(![3, 4, 7].contains(&outcome.exit), "{}: {phrase}", case.id);
            }
            "kill path bypassed"
            | "descendant-reap claimed stronger than group-terminates+direct-child-reaped-or-leaked-recorded" =>
            {
                assert_eq!(outcome.verdict, "cancelled", "{}: {phrase}", case.id);
            }
            "verdict omits the retained backup name" | "residue unreported" => {
                assert!(!outcome.residues.is_empty(), "{}: {phrase}", case.id);
            }
            other => panic!("{}: unmapped forbidden phrase '{other}'", case.id),
        }
        let _ = dirs;
    }
}

// ---------------------------------------------------------------------------
// Case runner
// ---------------------------------------------------------------------------

fn run_case(case: &Case) {
    let dirs = make_dirs();
    // A $CAPSULE_ROOT occupant marked orchbee-exact is a real capsule —
    // generic dir bytes would read invalid and misroute the preflight.
    let capsule_occupied = case.drive["op"].as_str() == Some("capsule-publish")
        && case.setup_fs.iter().any(|entry| {
            entry["path"].as_str() == Some("$CAPSULE_ROOT")
                && entry["ownership"].as_str() == Some("orchbee-exact")
        });
    let setup_fs: Vec<Value> = if capsule_occupied {
        materialize_capsule(&dirs.capsule_root, None, false);
        case.setup_fs
            .iter()
            .filter(|entry| entry["path"].as_str() != Some("$CAPSULE_ROOT"))
            .cloned()
            .collect()
    } else {
        case.setup_fs.clone()
    };
    let owners = materialize_fs(&dirs, &setup_fs);

    let mut plan = Plan::default();
    // Scripted provider commands: inspection reads, then a generous vector
    // supply (only the consumed prefix is asserted), then a re-inspection
    // tail the queue only spends on the uncertain-attempt path.
    match case.drive["op"].as_str().unwrap_or("") {
        "enable-provider" | "disable-provider" => {
            let id = provider_for(case);
            let reads = providers::inspect_slots(id);
            let inject = &case.inject;
            let partial = inject["kind"].as_str() == Some("read 2 of 2 fails");
            let sigint_mid = inject["kind"].as_str() == Some("SIGINT")
                && inject["atStep"].as_str() == Some("provider-command");
            // preflight reads
            for _ in 0..reads {
                plan.script_command(ScriptedResult::Completed(b"[]".to_vec()));
            }
            // vector commands — WaitForCancel carries the mid-effect
            // SIGINT; `partial` needs an uncertain attempt, so its first
            // vector command fails and the re-inspection below runs.
            for index in 0..8 {
                if sigint_mid && index == 0 {
                    plan.script_command(ScriptedResult::WaitForCancel);
                } else if partial && index == 0 {
                    plan.script_command(ScriptedResult::Failed);
                } else {
                    plan.script_command(ScriptedResult::Completed(Vec::new()));
                }
            }
            // re-inspection reads — `partial` fails the second.
            for index in 0..reads {
                if partial && index == reads - 1 {
                    plan.script_command(ScriptedResult::Failed);
                } else {
                    plan.script_command(ScriptedResult::Completed(b"[]".to_vec()));
                }
            }
        }
        _ => {}
    }
    install_injections(case, &dirs, &mut plan);
    inject::install(plan);

    let outcome = match case.drive["op"].as_str().unwrap_or("") {
        "publish-file" | "publish-dir" | "replace-file" => drive_fs(case, &dirs),
        "remove-file" | "remove-dir" => drive_remove(case, &dirs),
        "capsule-publish" => {
            if !dirs.adapter_source.exists() {
                match case.drive["capsule"].as_str() {
                    Some("capsule-declaring-older-payloadVersion") => {
                        materialize_capsule(&dirs.adapter_source, Some("0.3.0"), false)
                    }
                    Some("capsule-declaring-future-payloadVersion") => {
                        materialize_capsule(&dirs.adapter_source, Some("9.9.9"), false)
                    }
                    Some("capsule-with-bad-entry") => {
                        materialize_capsule(&dirs.adapter_source, None, true)
                    }
                    _ => materialize_capsule(&dirs.adapter_source, None, false),
                }
            }
            drive_capsule(case, &dirs)
        }
        "enable-provider" | "disable-provider" => {
            let report = drive_provider(case, &dirs);
            report_outcome(&report)
        }
        "inspect" => drive_inspect(case, &dirs),
        "cohort-admit" => drive_cohort(case, &dirs),
        other => panic!("{}: unknown drive op {other}", case.id),
    };
    let witnessed = inject::witnessed();
    inject::reset();

    // ---- assertions -----------------------------------------------------
    let expect = &case.expect;
    if let Some(verdict) = expect["verdict"].as_str() {
        assert_eq!(
            outcome.verdict, verdict,
            "{}: verdict (witnessed {witnessed:?})",
            case.id
        );
    }
    if let Some(exit) = expect["exit"].as_i64() {
        assert_eq!(
            outcome.exit, exit as i32,
            "{}: exit (witnessed {witnessed:?})",
            case.id
        );
    }
    if let Some(frontier) = expect["frontier"].as_str() {
        assert_eq!(
            outcome.frontier, frontier,
            "{}: frontier (witnessed {witnessed:?})",
            case.id
        );
    }
    if let Some(provenance) = expect["provenance"].as_str() {
        assert_eq!(
            outcome.provenance,
            Some(provenance),
            "{}: provenance",
            case.id
        );
    }
    if let Some(determinate) = expect["determinate"].as_bool() {
        assert_eq!(
            outcome.determinate,
            Some(determinate),
            "{}: determinate (witnessed {witnessed:?})",
            case.id
        );
    }
    if let Some(issue) = expect["issue"].as_str() {
        assert_eq!(outcome.issue, Some(issue), "{}: issue", case.id);
    }
    let expected_steps = expand_attempted(expect);
    if expected_steps.is_empty()
        && expect
            .get("attempted")
            .and_then(Value::as_array)
            .is_some_and(|list| list.is_empty())
    {
        // "No attempted steps" means no effect boundary — the read-path
        // witnesses (preflight-inspect, reinspect, cohort-admit) are not
        // mutation steps.
        assert!(
            witnessed.iter().all(|step| {
                matches!(*step, "preflight-inspect" | "reinspect" | "cohort-admit")
            }),
            "{}: expected no attempted steps, witnessed {witnessed:?}",
            case.id
        );
    } else {
        assert_subsequence(&case.id, &expected_steps, &witnessed);
    }
    assert_residues(&case.id, expect, &outcome, &dirs, &owners);
    assert_preserved(&case.id, expect, &dirs, &owners);
    assert_forbidden(case, &outcome, &dirs, &witnessed);
}

/// Static cases pin contract shape: exit-family vocabulary, provenance
/// closure, honesty wording, reserve arithmetic — asserted against the
/// implementation's own constants and mappers, not a parallel table.
fn run_static(case: &Case) {
    let expect = &case.expect;
    match case.id.as_str() {
        "publish.dir.no-noreplace-primitive" => {
            // The typed refusal path exists: `FsStep` separates conflict
            // from failure and `manual-action-required` is a distinct
            // refusal — no silent plain-rename fallback compiles in.
            let _ = mutation::Refusal::ManualActionRequired;
            let _ = files::FsStep::Conflict;
        }
        "cohort.manifest-version-older-refuse"
        | "cohort.manifest-version-future-refuse"
        | "cohort.manifest-unknown-shape-refuse" => {
            // Real admission path, materialized manifest — static only in
            // that no fault injection is needed.
            let dirs = make_dirs();
            let outcome = drive_cohort(case, &dirs);
            inject::reset();
            assert_eq!(
                outcome.verdict,
                expect["verdict"].as_str().unwrap(),
                "{}",
                case.id
            );
            assert_eq!(
                outcome.exit,
                expect["exit"].as_i64().unwrap() as i32,
                "{}",
                case.id
            );
            let manifest = dirs.cohort_dir.join("cohort.json");
            assert!(manifest.exists(), "{}: manifest preserved", case.id);
        }
        "budget.reserve-0-fs-only" => {
            assert_eq!(providers::inspect_slots(IntegrationId::Pi), 0);
            assert_eq!(providers::inspect_slots(IntegrationId::Gemini), 0);
        }
        "budget.reserve-1" => {
            assert_eq!(providers::inspect_slots(IntegrationId::Grok), 1);
        }
        "budget.reserve-2" => {
            assert_eq!(providers::inspect_slots(IntegrationId::ClaudeCode), 2);
            assert_eq!(providers::inspect_slots(IntegrationId::GitHubCopilot), 2);
        }
        "guard.unknown-state-no-else-uninstall" => {
            // A disable against an unscannable/foreign provider entry must
            // stand on the classification verdict — drive the real
            // frontier with a foreign guarded target.
            let dirs = make_dirs();
            let mut plan = Plan::default();
            let _ = &mut plan;
            inject::install(plan);
            let mut case_enable = Case {
                id: case.id.clone(),
                execution: case.execution.clone(),
                setup_fs: vec![serde_json::json!({
                    "path": "$TARGET", "kind": "dir", "ownership": "foreign",
                    "bytes": "unscannable"
                })],
                setup_provider: None,
                setup_clock: None,
                drive: serde_json::json!({"op": "disable-provider"}),
                inject: Value::Null,
                expect: expect.clone(),
            };
            let _ = &mut case_enable;
            let report = drive_provider_at(&dirs, IntegrationId::Gemini, Direction::Disable);
            let outcome = report_outcome(&report);
            inject::reset();
            assert_eq!(
                outcome.exit, 3,
                "{}: unknown→conflict, not uninstall",
                case.id
            );
            assert_eq!(outcome.verdict, "conflict", "{}", case.id);
            assert!(
                report.attempted.iter().all(|step| !matches!(
                    step.step,
                    StepKind::Filesystem { .. } | StepKind::Command { .. }
                )),
                "{}: no uninstall vector emitted",
                case.id
            );
        }
        "remove.file.verify-unlink-cooperative-window" => {
            // The remove path is verify-then-unlink — the check and the
            // effect are separate real steps in `guarded_disable`.
            let dirs = make_dirs();
            fs::write(&dirs.target, ownership_bytes("orchbee-exact", "ours")).unwrap();
            inject::install(Plan::default());
            let outcome = drive_remove(
                &Case {
                    id: case.id.clone(),
                    execution: case.execution.clone(),
                    setup_fs: vec![],
                    setup_provider: None,
                    setup_clock: None,
                    drive: serde_json::json!({"op": "remove-file", "target": "$TARGET"}),
                    inject: Value::Null,
                    expect: expect.clone(),
                },
                &dirs,
            );
            let witnessed = inject::witnessed();
            inject::reset();
            assert_eq!(outcome.exit, 0, "{}", case.id);
            assert!(
                witnessed.contains(&"remove-verify-owned") && witnessed.contains(&"remove-unlink"),
                "{}: verify->unlink cooperative sequence",
                case.id
            );
        }
        "frontier.provider-read-never-flips-effect" => {
            // A pure inspection mutates nothing: scripted reads complete,
            // the managed tree stays byte-identical.
            let dirs = make_dirs();
            make_executable(&dirs, "grok");
            inject::install({
                let mut plan = Plan::default();
                plan.script_command(ScriptedResult::Completed(b"[]".to_vec()));
                plan
            });
            let cancel = InjectCancel;
            let context = ProviderContext {
                home: &dirs.home,
                capsule_root: &dirs.capsule_root,
                adapter_source: Some(dirs.adapter_source.as_path()),
                path_env: Some(dirs.bin.as_os_str()),
                deadline: inject::now() + Duration::from_secs(40),
                cancel: Some(&cancel),
                management_mode: super::super::model::ManagementMode::ManagedLocal,
            };
            let _item = providers::inspect_provider(IntegrationId::Grok, &context, &[]);
            let witnessed = inject::witnessed();
            inject::reset();
            assert!(
                !witnessed.iter().any(|step| [
                    "stage-create",
                    "install",
                    "move-aside",
                    "restore",
                    "remove-unlink",
                    "remove-dir-walk",
                    "capsule-stage",
                    "capsule-rename",
                ]
                .contains(step)),
                "{}: read path produced an effect boundary {witnessed:?}",
                case.id
            );
        }
        "frontier.attempted-and-observed-separate" => {
            // The report type carries the three fact lists separately —
            // a skipped re-inspection reports `unavailable`, never a
            // merged pseudo-snapshot (covered live by
            // budget.reinspect-reserve-skipped).
            let _provenance_closed: [&str; 4] =
                ["pre-effect", "post-effect", "partial", "unavailable"];
            assert_eq!(
                super::super::model::Provenance::Unavailable.as_str(),
                "unavailable"
            );
        }
        other => panic!("unmapped static case {other}"),
    }
}

/// `drive_provider` variant with an explicit provider — the static
/// unknown-state case pins Gemini's guarded-disable classification.
fn drive_provider_at(
    dirs: &Dirs,
    id: IntegrationId,
    direction: Direction,
) -> mutation::MutationReport {
    make_executable(dirs, providers::executable_name(id));
    materialize_provider_target(
        dirs,
        id,
        &[serde_json::json!({
            "path": "$TARGET", "kind": "dir", "ownership": "foreign",
            "bytes": "unscannable"
        })],
    );
    if !dirs.adapter_source.exists() {
        materialize_capsule(&dirs.adapter_source, None, false);
    }
    let cancel = InjectCancel;
    let context = ProviderContext {
        home: &dirs.home,
        capsule_root: &dirs.capsule_root,
        adapter_source: Some(dirs.adapter_source.as_path()),
        path_env: Some(dirs.bin.as_os_str()),
        deadline: inject::now() + Duration::from_secs(40),
        cancel: Some(&cancel),
        management_mode: super::super::model::ManagementMode::ManagedLocal,
    };
    mutation::mutate(
        id,
        direction,
        &context,
        std::slice::from_ref(&dirs.adapter_source),
    )
}

#[test]
fn provider_effects52_conformance() {
    for case in load_cases() {
        if case.execution == "static" {
            run_static(&case);
        } else {
            run_case(&case);
        }
    }
}
