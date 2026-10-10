use super::*;
use crate::invocation::{Request, parse_request};
use orchbee_workbench_protocol::{WorkbenchResponse, WorktreeSource, parse_workbench_response};
use serde_json::{Value, json};
use std::{ffi::OsString, path::PathBuf};

#[test]
fn source_grammar_is_disjoint_from_registered_directory_and_mutation_flags() {
    for words in [
        vec!["worktree", "list"],
        vec!["--trust-repository", "worktree", "list", "--cwd=C:/repo"],
        vec![
            "worktree",
            "list",
            "--workspace=@workspace:abc",
            "--format=json",
        ],
    ] {
        assert!(matches!(
            parse_request(words.iter().map(OsString::from)),
            Ok(Request::WorktreeQuery { .. })
        ));
    }
    for tail in [
        vec!["--workspace=w", "--cwd=C:/repo"],
        vec!["--project=p", "--cwd=C:/repo"],
        vec!["--project=p", "--trust-repository"],
        vec![
            "--directory=00000000000000000000000000000001",
            "--workspace=w",
        ],
        vec!["--cwd=C:relative"],
        vec!["--confirm"],
        vec!["--detach"],
        vec!["--path=C:/repo"],
        vec!["--force"],
    ] {
        let words = [vec!["worktree", "list"], tail].concat();
        assert!(
            parse_request(words.iter().map(OsString::from)).is_err(),
            "{words:?}"
        );
    }
}

#[test]
fn directory_source_needs_no_project() {
    let snapshot = orchbee_workbench_protocol::parse_snapshot(&serde_json::to_vec(&json!({
        "kind":"workbench.snapshot", "protocolVersion":1, "siteId":"00000000000000000000000000000001", "revision":"1",
        "navigation":{"standaloneTerminalContextId":"00000000000000000000000000000002", "projects":[], "directories":[], "workspaces":[]},
        "terminalContexts":[{"terminalContextId":"00000000000000000000000000000002", "groups":[], "tabs":[]}]
    })).unwrap()).unwrap();
    let (project, source) =
        crate::cli::worktree_source::resolve(&None, &None, &None, &snapshot, || {
            Ok(PathBuf::from("C:/repo/sub"))
        })
        .unwrap();
    assert!(project.is_none());
    assert!(matches!(source, WorktreeSource::Directory {path} if path.as_str() == "C:/repo/sub"));
}

#[test]
fn emitted_query_envelopes_validate_shared_corpus_and_escape_text() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../../../schemas/workbench/v1/workbench-worktree-query-conformance-v1.json"
    ))
    .unwrap();
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.worktree.query.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for case in corpus["valid"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["direction"] == "response")
    {
        let WorkbenchResponse::WorktreeQuery(result) =
            parse_workbench_response(&serde_json::to_vec(&case["document"]).unwrap()).unwrap()
        else {
            panic!("query result")
        };
        let output = emit(result.clone(), OutputFormat::Json, Locale::EnUs);
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            validator.is_valid(&document),
            "{}: {document}",
            case["name"]
        );
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.status == 0,
            case["document"]["outcome"]["kind"] == "found"
        );
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let output = emit(result.clone(), OutputFormat::Text, locale);
            assert!(!output.stdout.is_empty());
            assert!(!output.stdout.contains(&0x1b));
        }
    }
}
