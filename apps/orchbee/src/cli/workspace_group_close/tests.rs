use super::*;
use crate::invocation::{Request, parse_request};
use orchbee_workbench_protocol::{WorkbenchResponse, parse_workbench_response};
use serde_json::Value;
use std::ffi::OsString;
const ID: &str = "00000000000000000000000000000003";
#[test]
fn group_grammar_requires_confirmation_and_keeps_retained_selectors_independent() {
    for words in [
        vec!["project", "close-all", "demo"],
        vec!["workspace", "close", "demo", "--group", "--confirm"],
        vec!["workspace", "close", "demo", "--confirm", "--detach"],
        vec!["project", "close-status"],
        vec!["project", "close-status", "--request", ID, "--project", ID],
        vec!["project", "close-status", "--request", ID, "--confirm"],
        vec!["project", "close-resume", "--request", ID, "--generation=1"],
        vec![
            "project",
            "close-resume",
            "--request",
            ID,
            "--generation=65",
            "--confirm",
        ],
        vec![
            "project",
            "close-resume",
            "--request",
            ID,
            "--generation=01",
            "--confirm",
        ],
    ] {
        assert!(
            parse_request(words.iter().map(OsString::from)).is_err(),
            "{words:?}"
        );
    }
    for words in [
        vec![
            "project",
            "close-all",
            "demo",
            "--confirm",
            "--request-id",
            ID,
            "--timeout=7",
            "--detach",
        ],
        vec!["project", "close-status", "--request", ID],
        vec![
            "project",
            "close-resume",
            "--request",
            ID,
            "--generation=64",
            "--confirm",
            "--detach",
        ],
    ] {
        assert!(
            matches!(
                parse_request(words.iter().map(OsString::from)),
                Ok(Request::WorkspaceGroupClose { .. })
            ),
            "{words:?}"
        );
    }

    assert!(matches!(
        parse_request(
            ["workspace", "close", "demo", "--confirm"]
                .into_iter()
                .map(OsString::from)
        ),
        Ok(Request::Mutation { .. })
    ));
}
fn reply(name: &str) -> WorkbenchWorkspaceGroupCloseResult {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../../../schemas/workbench/v1/workbench-workspace-group-close-conformance-v1.json"
    ))
    .unwrap();
    let case = corpus["valid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap();
    let WorkbenchResponse::WorkspaceGroupClose(reply) =
        parse_workbench_response(&serde_json::to_vec(&case["document"]).unwrap()).unwrap()
    else {
        panic!("group reply");
    };
    reply
}
fn answer(reply: WorkbenchWorkspaceGroupCloseResult) -> Answer {
    Answer {
        operation_id: reply.operation_id.clone(),
        observation: Observation::Server { reply },
        wait: None,
    }
}
#[test]
fn partial_group_truth_and_wait_disposition_survive_schema_and_localized_output() {
    let mut reply = reply("known-partial-deadline");
    let WorkspaceGroupCloseOutcome::Available { members, .. } = &mut reply.outcome else {
        panic!("available");
    };
    members[0].label =
        orchbee_workbench_protocol::WorkbenchLabel::parse("label\x1b[31m\nspoof").unwrap();
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.project.close-all.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    let output = emit(
        None,
        answer(reply.clone()),
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(output.status, 3);
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validator.validate(&document).unwrap();
    assert_eq!(
        document["result"]["observation"]["reply"]["outcome"]["members"][0]["close"]["observation"]
            ["status"]["kind"],
        "closed"
    );
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let output = emit(None, answer(reply.clone()), OutputFormat::Text, locale);
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(!text.contains('\x1b'));
        assert!(!text.contains("\nspoof"));
        assert!(text.contains("project close-status --request"));
        assert!(text.contains("--generation 1 --confirm"));
    }
    for (wait, code) in [
        (Wait::Cancelled, 130),
        (Wait::TimedOut, 7),
        (Wait::TransportLost, 5),
        (Wait::ObservationFailed, 1),
    ] {
        let mut retained = answer(reply.clone());
        retained.wait = Some(wait);
        assert_eq!(retained.exit_code(), code);
        assert_eq!(
            retained.document()["observation"]["reply"]["outcome"]["status"]["kind"],
            "failed"
        );
    }
}
#[test]
fn every_shared_response_embeds_in_the_offline_schema_and_unknown_is_not_success() {
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.project.close-all.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for (name, code) in [
        ("pending-frozen-members", 0),
        ("accepted-not-observed", 0),
        ("closed-project-members", 0),
        ("physical-unknown", 8),
        ("not-found", 3),
        ("unavailable-capacity", 4),
    ] {
        let output = emit(None, answer(reply(name)), OutputFormat::Json, Locale::EnUs);
        assert_eq!(output.status, code, "{name}");
        validator
            .validate(&serde_json::from_slice::<Value>(&output.stdout).unwrap())
            .unwrap();
    }
}

#[cfg(windows)]
#[test]
fn waiting_refuses_changed_scope_members_child_identity_and_regressed_observations() {
    use orchbee_workbench_protocol::{WorkbenchLabel, WorkbenchTabCloseAttemptGeneration};
    let previous = reply("known-partial-deadline");
    let observed = Observation::Server {
        reply: previous.clone(),
    };
    assert!(windows::same_attempt(&observed, &previous));
    for mutation in 0..7 {
        let mut next = previous.clone();
        let WorkspaceGroupCloseOutcome::Available {
            attempt, members, ..
        } = &mut next.outcome
        else {
            unreachable!()
        };
        match mutation {
            0 => *attempt += 1,
            1 => members.reverse(),
            2 => members[0].label = WorkbenchLabel::parse("different").unwrap(),
            3 => members[0].close = None,
            4 => {
                members[0].close.as_mut().unwrap().request_id =
                    WorkbenchRequestId::parse(&"ab".repeat(16)).unwrap()
            }
            5 => members[0].close.as_mut().unwrap().observation = None,
            _ => {
                members[0]
                    .close
                    .as_mut()
                    .unwrap()
                    .observation
                    .as_mut()
                    .unwrap()
                    .attempt_generation = WorkbenchTabCloseAttemptGeneration::new(2).unwrap()
            }
        }
        assert!(
            !windows::same_attempt(&observed, &next),
            "mutation {mutation}"
        );
    }
    let before = reply("pending-frozen-members");
    let after = reply("accepted-not-observed");
    assert!(windows::same_attempt(
        &Observation::Server { reply: before },
        &after
    ));
    assert!(windows::same_attempt(
        &Observation::Server { reply: after },
        &previous
    ));
}
