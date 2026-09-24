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
        "navigation":{"standaloneTerminalContextId":id(2),"projects":[{"projectId":id(3),"label":"project","directories":[],
            "workspaces":[{"workspaceId":id(4),"label":"work","primaryDirectoryId":null,"terminalContextId":id(5)}]}]},
        "terminalContexts":[{"terminalContextId":id(2),"groups":[],"tabs":[]},{"terminalContextId":id(5),"groups":[],"tabs":[{
            "tabId":id(6),"label":"shell","groupId":null,"pinned":false,"root":{"kind":"panel","panelId":id(7),
                "primaryPresentation":{"presentationId":id(8),"label":"shell","binding":{"terminalId":id(9),"workerId":id(10)}},"childPresentations":[]}}]}]})).unwrap()).unwrap()
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
fn implicit_source_is_the_validated_caller_and_explicit_cwd_ignores_environment() {
    let snapshot = snapshot();
    let create = start(&["worktree", "create"]);
    let environment = |name: &str| match name {
        "ORCHBEE_TERMINAL_ID" => Some(id(9).into()),
        "ORCHBEE_WORKER_ID" => Some(id(10).into()),
        _ => None,
    };
    let (project, intent) = selection::intent(&create, &snapshot, environment, || {
        panic!("caller source must not read cwd")
    })
    .unwrap();
    assert_eq!(project.as_str(), id(3));
    assert!(
        matches!(intent,WorktreeIntent::Create {source:WorktreeSource::Workspace {workspace_id},..} if workspace_id.as_str()==id(4))
    );
    let explicit = start(&["worktree", "create", "--cwd=C:/explicit"]);
    let (_, intent) = selection::intent(
        &explicit,
        &snapshot,
        |_| panic!("explicit source must not read caller environment"),
        || panic!("explicit source must not read cwd"),
    )
    .unwrap();
    assert!(
        matches!(intent,WorktreeIntent::Create {source:WorktreeSource::Directory {path},..} if path.as_str()=="C:/explicit")
    );
    let failure = selection::intent(
        &create,
        &snapshot,
        |name| {
            if name == "ORCHBEE_TERMINAL_ID" {
                Some(id(9).into())
            } else {
                Some(id(11).into())
            }
        },
        || panic!("stale caller must not fall back to cwd"),
    )
    .unwrap_err();
    assert_eq!(failure.kind, FailureKind::NotFound);
    let (_, intent) =
        selection::intent(&create, &snapshot, |_| None, || Ok("C:/external".into())).unwrap();
    assert!(
        matches!(intent,WorktreeIntent::Create {source:WorktreeSource::Directory {path},..} if path.as_str()=="C:/external")
    );
    let mut multiple = snapshot.clone();
    let mut other = multiple.navigation.projects[0].clone();
    other.project_id = WorkbenchProjectId::parse(&id(11)).unwrap();
    other.workspaces.clear();
    multiple.navigation.projects.push(other);
    assert_eq!(
        selection::intent(
            &create,
            &multiple,
            |_| None,
            || panic!("ambiguous Project before cwd")
        )
        .unwrap_err()
        .kind,
        FailureKind::Ambiguous
    );
}

fn answer(status: WorktreeOperationStatus) -> Answer {
    Answer {
        operation_id: WorkbenchRequestId::parse(&id(12)).unwrap(),
        wait: None,
        observation: Observation::Server {
            reply: WorkbenchWorktreeResult {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&id(13)).unwrap(),
                operation_id: WorkbenchRequestId::parse(&id(12)).unwrap(),
                outcome: WorkbenchWorktreeOutcome::Available {
                    project_id: WorkbenchProjectId::parse(&id(3)).unwrap(),
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
    }
}
