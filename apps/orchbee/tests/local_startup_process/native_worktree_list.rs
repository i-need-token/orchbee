//! Real-Site journey witness for `worktree list`: the staged `orchbee` binary
//! drives `project create → workspace create → workspace accept-directory →
//! worktree list` against a live Server and the lazily spawned Worker, with a
//! real private Git repository (committed main worktree plus a detached linked
//! worktree) as the registered directory. A second registered directory is an
//! initialized-but-unborn repository where real Git emits the all-zero HEAD
//! and the unborn branch ref verbatim — asserted as strings, never normalized
//! to null. No TUI client, no tab, and no terminal Host anywhere: the exchange
//! exercises the Worker's pre-Host Worktree Job dispatch directly. JSON output
//! validates against the embedded `orchbee.worktree.list` schema, both text
//! renderings stay localized, and a well-formed absent directory answers the
//! dedicated failure envelope at exit 3.

use super::native_cli_query::{run_cli, run_json, spawn_real_server, validate, validators};
use super::*;
use std::path::Path;
use std::process::Output;

/// Run one CLI invocation under a chosen locale. Identical to `run_cli`
/// except `LANG`; the worktree surface owns no locale flag of its own.
fn run_cli_lang(fixture: &Fixture, lang: &str, arguments: &[&str]) -> Output {
    Command::new(fixture.cohort.join("orchbee"))
        .args(arguments)
        .env("ORCHBEE_RUNTIME_BASE", &fixture.base)
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|error| panic!("spawn query CLI {arguments:?}: {error}"))
}

/// Build a private disposable Git repository: `main` with one real commit and
/// one detached linked worktree. Identity is per-command `-c` only.
fn git_repository(root: &Path) -> Result<(), Box<dyn Error>> {
    let git = |arguments: &[&str]| -> Result<(), Box<dyn Error>> {
        let output = Command::new("git")
            .args(arguments)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .output()?;
        assert!(
            output.status.success(),
            "git {arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    };
    git(&["init", "-b", "main", root.to_str().ok_or("repo UTF-8")?])?;
    fs::write(root.join("seed.txt"), b"seed\n")?;
    let commit = |arguments: &[&str]| -> Result<(), Box<dyn Error>> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["-c", "user.email=fixture@example.invalid"])
            .args(["-c", "user.name=fixture"])
            .args(arguments)
            .output()?;
        assert!(
            output.status.success(),
            "git -C repo {arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    };
    commit(&["add", "seed.txt"])?;
    commit(&["commit", "-m", "seed"])?;
    let linked = root.join("linked-worktree");
    commit(&[
        "worktree",
        "add",
        "--detach",
        linked.to_str().ok_or("linked UTF-8")?,
    ])?;
    Ok(())
}

/// The complete headless journey: a real committed Project and a
/// Server-owned Directory bound to a real Worker resolve `worktree list`
/// through the OBCN job channel against real Git output — all before any
/// terminal Host exists.
#[test]
fn worktree_list_journey_against_a_live_cohort_and_real_git() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let _server = spawn_real_server(&fixture);
    let validators = validators(&fixture);

    // A real private repository: committed main worktree + detached linked
    // worktree, so the listing exercises branch refs and the detached flag.
    let repository = fixture.base.join("real git 仓库");
    fs::create_dir(&repository)?;
    git_repository(&repository)?;
    let repository_path = repository.to_str().ok_or("repo path UTF-8")?.to_owned();
    let linked = repository.join("linked-worktree");
    let linked_path = linked.to_str().ok_or("linked path UTF-8")?.to_owned();
    let head = {
        let output = Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args(["rev-parse", "HEAD"])
            .output()?;
        assert!(output.status.success());
        String::from_utf8(output.stdout)?.trim().to_owned()
    };
    assert_eq!(head.len(), 40, "fixture HEAD: {head}");

    // Commit a Project, Workspace, and the directory — every step a real CLI
    // invocation against the live Server.
    let (output, project) = run_json(
        &fixture,
        &[
            "project", "create", "--label", "journey", "--format", "json",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    let project_id = project["result"]["projectId"].as_str().unwrap().to_owned();
    let (output, workspace) = run_json(
        &fixture,
        &[
            "workspace",
            "create",
            "--project",
            project_id.as_str(),
            "--label",
            "main",
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    let workspace_id = workspace["result"]["workspaceId"]
        .as_str()
        .unwrap()
        .to_owned();
    let (output, accepted) = run_json(
        &fixture,
        &[
            "workspace",
            "accept-directory",
            workspace_id.as_str(),
            "--path",
            repository_path.as_str(),
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "accept-directory: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let directory_id = accepted["result"]["directoryId"]
        .as_str()
        .expect("directoryId")
        .to_owned();

    // The committed directory bound a real Worker; the journey runs entirely
    // before any terminal Host could exist.
    assert_eq!(
        fixture.processes("orchbee-worker").len(),
        1,
        "one real Worker owns the directory"
    );
    assert!(
        fixture.processes("orchbee-terminal-host").is_empty(),
        "no terminal Host may exist on this path"
    );

    // The actual journey: JSON document.
    let (output, document) = run_json(
        &fixture,
        &[
            "worktree",
            "list",
            "--project",
            project_id.as_str(),
            "--directory",
            directory_id.as_str(),
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "worktree list: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(document["schema"], "orchbee.worktree.list");
    validate(&validators, "orchbee.worktree.list", &output);
    assert_eq!(document["source"]["kind"], "worktree-list");
    assert!(
        document["source"]["siteId"]
            .as_str()
            .is_some_and(|id| !id.is_empty()),
        "source carries the Site identity: {document}"
    );
    assert!(
        document["source"]["revision"]
            .as_str()
            .is_some_and(|revision| revision.parse::<u64>().is_ok()),
        "source carries the recheck-time committed revision: {document}"
    );
    assert_eq!(document["result"]["projectId"], project_id.as_str());
    assert_eq!(document["result"]["directoryId"], directory_id.as_str());
    assert_eq!(document["result"]["path"], repository_path.as_str());
    let entries = document["result"]["worktrees"]
        .as_array()
        .expect("worktrees");
    assert_eq!(entries.len(), 2, "main + detached worktree: {entries:?}");
    let main_entry = entries
        .iter()
        .find(|entry| entry["worktreePath"] == repository_path.as_str())
        .expect("registered directory appears as a worktree");
    assert_eq!(main_entry["head"], head.as_str());
    assert_eq!(main_entry["branch"], "refs/heads/main");
    assert_eq!(main_entry["detached"], false);
    assert_eq!(main_entry["bare"], false);
    let detached = entries
        .iter()
        .find(|entry| entry["worktreePath"] == linked_path.as_str())
        .expect("detached linked worktree is listed");
    assert_eq!(detached["head"], head.as_str());
    assert_eq!(detached["branch"], serde_json::Value::Null);
    assert_eq!(detached["detached"], true);

    // English text rendering on the same live path.
    let output = run_cli(
        &fixture,
        &[
            "worktree",
            "list",
            "--project",
            project_id.as_str(),
            "--directory",
            directory_id.as_str(),
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout)?;
    assert!(text.contains("Project ID"), "{text}");
    assert!(text.contains("PATH"), "{text}");
    assert!(text.contains("refs/heads/main"), "{text}");
    assert!(text.contains("detached"), "{text}");

    // Chinese text rendering, same journey, locale-selected.
    let output = run_cli_lang(
        &fixture,
        "zh_CN.UTF-8",
        &[
            "worktree",
            "list",
            "--project",
            project_id.as_str(),
            "--directory",
            directory_id.as_str(),
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout)?;
    assert!(text.contains("路径"), "{text}");
    assert!(text.contains("分支"), "{text}");
    assert!(text.contains("refs/heads/main"), "{text}");

    // Still no terminal Host: the whole journey ran pre-Host.
    assert!(
        fixture.processes("orchbee-terminal-host").is_empty(),
        "worktree list must not start a Host"
    );

    // A well-formed but absent directory id is the Server's not-found: exit 3
    // with the dedicated worktree failure document.
    let (output, failure) = run_json(
        &fixture,
        &[
            "worktree",
            "list",
            "--project",
            project_id.as_str(),
            "--directory",
            "ffffffffffffffffffffffffffffffff",
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(failure["schema"], "orchbee.cli.failure.worktree");
    assert_eq!(failure["failure"]["kind"], "not-found");
    assert_eq!(failure["failure"]["details"]["resource"], "directory");
    assert_eq!(
        failure["failure"]["details"]["target"],
        "ffffffffffffffffffffffffffffffff"
    );
    validate(&validators, "orchbee.cli.failure.worktree", &output);

    // A second registered directory backed by an initialized but unborn
    // repository: real Git reports the all-zero HEAD and the unborn branch
    // ref verbatim, and neither may be normalized to null.
    let unborn = fixture.base.join("unborn repo");
    fs::create_dir(&unborn)?;
    let init = Command::new("git")
        .args(["init", "-b", "main"])
        .arg(&unborn)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()?;
    assert!(
        init.status.success(),
        "git init unborn: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    let unborn_path = unborn.to_str().ok_or("unborn path UTF-8")?.to_owned();

    // Pin the real Git bytes first so the CLI answer is compared against an
    // observed wire value, not an invented constant.
    let probe = Command::new("git")
        .arg("-C")
        .arg(&unborn)
        .args(["worktree", "list", "--porcelain", "-z"])
        .output()?;
    assert!(probe.status.success());
    let zero_head = "0".repeat(40);
    let raw = String::from_utf8_lossy(&probe.stdout);
    assert!(
        raw.contains(&format!("HEAD {zero_head}")),
        "real Git emits the all-zero HEAD for an unborn repository: {raw:?}"
    );
    assert!(
        raw.contains("branch refs/heads/main"),
        "real Git emits the unborn branch ref verbatim: {raw:?}"
    );

    let (output, workspace) = run_json(
        &fixture,
        &[
            "workspace",
            "create",
            "--project",
            project_id.as_str(),
            "--label",
            "unborn",
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    let unborn_workspace = workspace["result"]["workspaceId"]
        .as_str()
        .unwrap()
        .to_owned();
    let (output, accepted) = run_json(
        &fixture,
        &[
            "workspace",
            "accept-directory",
            unborn_workspace.as_str(),
            "--path",
            unborn_path.as_str(),
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "accept-directory unborn: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let unborn_directory = accepted["result"]["directoryId"]
        .as_str()
        .expect("unborn directoryId")
        .to_owned();

    let (output, document) = run_json(
        &fixture,
        &[
            "worktree",
            "list",
            "--project",
            project_id.as_str(),
            "--directory",
            unborn_directory.as_str(),
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "worktree list unborn: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(&validators, "orchbee.worktree.list", &output);
    assert_eq!(document["result"]["directoryId"], unborn_directory.as_str());
    assert_eq!(document["result"]["path"], unborn_path.as_str());
    let entries = document["result"]["worktrees"]
        .as_array()
        .expect("unborn worktrees");
    assert_eq!(
        entries.len(),
        1,
        "unborn repository lists itself: {entries:?}"
    );
    let entry = &entries[0];
    assert_eq!(entry["worktreePath"], unborn_path.as_str());
    assert_eq!(
        entry["head"],
        serde_json::Value::String(zero_head.clone()),
        "the all-zero HEAD stays a string, never normalized to null: {entry}"
    );
    assert_eq!(
        entry["branch"], "refs/heads/main",
        "the unborn branch ref stays verbatim: {entry}"
    );
    assert_eq!(entry["detached"], false);
    assert_eq!(entry["bare"], false);

    // Still no terminal Host after the second directory's listing.
    assert!(
        fixture.processes("orchbee-terminal-host").is_empty(),
        "the unborn listing must not start a Host"
    );
    Ok(())
}
