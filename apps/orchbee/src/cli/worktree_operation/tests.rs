use super::*;
use crate::invocation::{Request, WorktreeStartCommand, parse_request};
use orchbee_workbench_protocol::*;
use serde_json::{Value, json};
use std::ffi::OsString;

fn id(value: u8) -> String {
    format!("{value:032x}")
}
fn command(words: &[&str]) -> WorktreeCommand {
    let Request::Worktree { command, .. } =
        parse_request(words.iter().map(OsString::from)).unwrap()
    else {
        panic!("worktree command")
    };
    #[cfg(windows)]
    let command = command
        .into_resolved(
            OutputFormat::Text,
            |_| panic!("absolute test operand"),
            || panic!("absolute test operand"),
        )
        .unwrap();
    command
}
fn start(words: &[&str]) -> Box<WorktreeStartCommand> {
    let WorktreeCommand::Start(command) = command(words) else {
        panic!("start")
    };
    command
}
fn snapshot() -> Snapshot {
    parse_snapshot(&serde_json::to_vec(&json!({"kind":"workbench.snapshot","protocolVersion":1,"siteId":id(1),"revision":"2",
        "navigation":{ "projects":[{"projectId":id(3), "label":"project"}], "directories":[], "workspaces":[{"workspaceId":id(2),"terminalContextId":id(2),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"},{"workspaceId":id(4), "label":"work", "primaryDirectoryId":null, "terminalContextId":id(5), "projectId":id(3)}]},
        "terminalContexts":[{"terminalContextId":id(2),"groups":[],"tabs":[]},{"terminalContextId":id(5),"groups":[],"tabs":[{
            "tabId":id(6),"label":"shell","groupId":null,"pinned":false,"root":{"kind":"panel","panelId":id(7),
                "primaryPresentation":{"customLabel":null,"presentationId":id(8),"label":"shell","binding":{"terminalId":id(9),"workerId":id(10)}},"childPresentations":[]}}]}]})).unwrap()).unwrap()
}

#[test]
fn grammar_rejects_destructive_or_conflicting_intents_before_runtime_access() {
    for words in [
        vec!["worktree", "remove", "--workspace=work"],
        vec!["worktree", "remove", "--confirm"],
        vec!["worktree", "create", "--confirm"],
        vec!["worktree", "create", "--workspace=work", "--cwd=C:/repo"],
        vec!["worktree", "open"],
        vec!["worktree", "open", "--path=C:/repo", "--branch=main"],
        vec!["worktree", "open", "--branch=main", "--base=HEAD"],
        vec!["worktree", "create", "--branch=-bad"],
        vec!["worktree", "create", "--branch=bad\nref"],
        vec!["worktree", "create", "--force"],
        vec!["worktree", "create", "--focus", "--no-focus"],
        vec!["worktree", "create", "--focus", "--detach"],
        vec![
            "worktree",
            "remove",
            "--workspace=w",
            "--confirm",
            "--focus",
        ],
        vec![
            "worktree",
            "remove",
            "--workspace=w",
            "--confirm",
            "--client=00000000000000000000000000000001",
        ],
        vec!["project", "list", "--trust-repository"],
        vec!["worktree", "create", "extra"],
        vec!["worktree", "status", "bad"],
        vec![
            "worktree",
            "status",
            "00000000000000000000000000000001",
            "--detach",
        ],
        vec![
            "worktree",
            "resume",
            "00000000000000000000000000000001",
            "--generation=1",
        ],
        vec![
            "worktree",
            "resume",
            "00000000000000000000000000000001",
            "--generation=0",
            "--confirm",
        ],
    ] {
        assert!(
            parse_request(words.iter().map(OsString::from)).is_err(),
            "{words:?}"
        );
    }
    let create = start(&[
        "worktree",
        "create",
        "--branch=feature/中文",
        "--base=HEAD~2",
        "--path=C:/trees/new",
        "--request-id=00000000000000000000000000000001",
        "--detach",
        "--timeout=7",
    ]);
    assert!(create.detach);
    assert_eq!(create.timeout_ms, 7);
    assert_eq!(create.operation_id.unwrap().as_str(), id(1));
    assert!(matches!(
        command(&["worktree", "resume", &id(1), "--generation=64", "--confirm"]),
        WorktreeCommand::Resume {
            expected_attempt: 64,
            ..
        }
    ));
}

#[test]
fn canonical_workspace_source_keeps_identity_and_directory_source_has_optional_project() {
    for grouped in [true, false] {
        let mut snapshot = snapshot();
        if !grouped {
            snapshot.navigation.workspaces[1].project_id = None;
        }
        let explicit = start(&["worktree", "create", "--workspace=work"]);
        let (project, intent) = selection::intent(&explicit, &snapshot, || {
            panic!("explicit source must not read cwd")
        })
        .unwrap();
        assert_eq!(project, None);
        assert!(
            matches!(intent, WorktreeIntent::Create { source: WorktreeSource::Workspace { workspace_id }, .. } if workspace_id.as_str() == id(4))
        );
    }
    let mut snapshot = snapshot();
    snapshot.navigation.projects.push(Project {
        project_id: WorkbenchProjectId::parse(&id(11)).unwrap(),
        label: "other".into(),
    });
    for (words, expected) in [
        (vec!["worktree", "create", "--cwd=C:/explicit"], None),
        (
            vec!["worktree", "create", "--cwd=C:/explicit", "--project=other"],
            Some(id(11)),
        ),
    ] {
        let explicit = start(&words);
        let (project, intent) = selection::intent(&explicit, &snapshot, || {
            panic!("explicit source must not read cwd")
        })
        .unwrap();
        assert_eq!(
            project.as_ref().map(|project| project.to_string()),
            expected
        );
        assert!(
            matches!(intent, WorktreeIntent::Create { source: WorktreeSource::Directory { path }, .. } if path.as_str() == "C:/explicit")
        );
    }
}

fn answer(status: WorktreeOperationStatus) -> Answer {
    Answer {
        operation_id: WorkbenchRequestId::parse(&id(12)).unwrap(),
        wait: None,
        post_open: None,
        observation: Observation::Server {
            reply: WorkbenchWorktreeResult {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&id(13)).unwrap(),
                operation_id: WorkbenchRequestId::parse(&id(12)).unwrap(),
                outcome: WorkbenchWorktreeOutcome::Available {
                    project_id: None,
                    attempt: 2,
                    status: Box::new(status),
                },
            },
        },
    }
}

#[test]
fn partial_git_failure_keeps_closed_truth_escapes_diagnostics_and_matches_schema() {
    let status = WorktreeOperationStatus::Failed {
        reason: WorktreeOperationFailure::Git {
            stage: WorktreeGitStage::Mutate,
            error: WorktreeGitError::GitFailed,
            status: Some(1),
            diagnostics: b"failed\x1b[31m\n".to_vec(),
        },
        resumable: true,
        progress: WorktreeProgress {
            path: Some(WorkbenchDirectoryPath::parse("C:/trees/work").unwrap()),
            checkout: None,
            git_effect: WorktreeGitEffect::MayHaveChanged,
            restoration: None,
            workspace: WorktreeWorkspaceDisposition::Closed {
                workspace_id: WorkbenchWorkspaceId::parse(&id(4)).unwrap(),
                close_request_id: WorkbenchRequestId::parse(&id(15)).unwrap(),
            },
        },
    };
    let output = emit(
        Some(&SiteId::parse(&id(1)).unwrap()),
        answer(status.clone()),
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(output.status, 3);
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.worktree.operation.json")).unwrap();
    jsonschema::draft202012::new(&schema)
        .unwrap()
        .validate(&doc)
        .unwrap();
    assert_eq!(
        doc["result"]["observation"]["reply"]["outcome"]["status"]["progress"]["workspace"]["kind"],
        "closed"
    );
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let text = String::from_utf8(
            emit(None, answer(status.clone()), OutputFormat::Text, locale).stdout,
        )
        .unwrap();
        assert!(!text.contains('\u{1b}'));
        assert!(text.contains("--generation 2 --confirm"));
        assert!(text.contains("C:/trees/work"));
    }
    let mut retained = answer(status);
    retained.wait = Some(Wait::TransportLost);
    assert_eq!(retained.exit_code(), 5);
    assert_eq!(
        retained.document()["observation"]["reply"]["outcome"]["status"]["kind"],
        "failed"
    );
}

#[test]
fn opening_a_workspace_does_not_turn_a_failed_terminal_launch_into_success() {
    for (terminal_outcome, expected) in [
        (WorkbenchTerminalOutcome::OutcomeUnknown {}, 8),
        (
            WorkbenchTerminalOutcome::Started {
                geometry: WorkbenchTerminalGeometry::new(80, 24, 0, 0).unwrap(),
            },
            0,
        ),
    ] {
        let opened = answer(WorktreeOperationStatus::Opened {
            checkout: WorktreeCheckoutFacts {
                repository_root: WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
                common_directory: WorkbenchDirectoryPath::parse("C:/repo/.git").unwrap(),
                path: WorkbenchDirectoryPath::parse("C:/trees/work").unwrap(),
                branch: None,
                dirty: false,
            },
            workspace_id: WorkbenchWorkspaceId::parse(&id(4)).unwrap(),
            reused: false,
            terminal_outcome: Some(terminal_outcome),
        });
        assert_eq!(opened.exit_code(), expected);
        assert_eq!(
            post_open::opened(&opened.observation).is_some(),
            expected == 0
        );
    }
}

#[test]
fn restoration_receipt_has_localized_counts_without_losing_git_failure() {
    let status = WorktreeOperationStatus::Failed {
        reason: WorktreeOperationFailure::Git {
            stage: WorktreeGitStage::Mutate,
            error: WorktreeGitError::GitFailed,
            status: Some(128),
            diagnostics: b"locked".to_vec(),
        },
        resumable: false,
        progress: WorktreeProgress {
            path: Some(WorkbenchDirectoryPath::parse("C:/trees/work").unwrap()),
            checkout: None,
            git_effect: WorktreeGitEffect::MayHaveChanged,
            workspace: WorktreeWorkspaceDisposition::Open {
                workspace_id: WorkbenchWorkspaceId::parse(&id(4)).unwrap(),
            },
            restoration: Some(orchbee_workbench_protocol::WorktreeShellRestoration {
                total: 4,
                attempted: 3,
                started: 1,
                failed: 1,
                unknown: 1,
                failure: Some(WorktreeOperationFailure::WorkerUnavailable {}),
            }),
        },
    };
    let output = emit(
        None,
        answer(status.clone()),
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(output.status, 8);
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(doc["version"], 1);
    assert_eq!(
        doc["result"]["observation"]["reply"]["outcome"]["status"]["reason"]["status"],
        128
    );
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.worktree.operation.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    validator.validate(&doc).unwrap();
    let mut missing = doc.clone();
    missing["result"]["observation"]["reply"]["outcome"]["status"]["progress"]
        .as_object_mut()
        .unwrap()
        .remove("restoration");
    assert!(validator.is_valid(&missing));
    for (locale, expected) in [
        (
            Locale::EnUs,
            "Shell restoration: 1/4 started; 1 failed; 1 unknown; 0 pending; 1 unattempted",
        ),
        (
            Locale::ZhCn,
            "shell 恢复：已启动 1/4；失败 1；未知 1；进行中 0；未尝试 1",
        ),
    ] {
        let text = String::from_utf8(
            emit(None, answer(status.clone()), OutputFormat::Text, locale).stdout,
        )
        .unwrap();
        assert!(text.contains(expected), "{text}");
        assert!(text.contains("worker-unavailable"));
        assert!(
            !text.contains("--generation"),
            "unknown launch has no Resume recommendation"
        );
    }
}
