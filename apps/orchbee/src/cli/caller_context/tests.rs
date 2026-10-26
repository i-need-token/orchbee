use super::*;
use crate::cli_text::CliText;
#[cfg(not(windows))]
use crate::invocation::QueryCommand;
use crate::{cli::OutputFormat, localization::Locale};
use orchbee_workbench_protocol::{PanelNode, parse_snapshot};
use serde_json::json;

const TERMINAL: &str = "0000000000000000000000000000000a";
const WORKER: &str = "0000000000000000000000000000000b";

fn hex(value: u8) -> String {
    format!("{value:032x}")
}

fn environment(name: &str) -> Option<OsString> {
    Some(match name {
        "ORCHBEE_TERMINAL_ID" => TERMINAL.into(),
        "ORCHBEE_WORKER_ID" => WORKER.into(),
        _ => panic!("undeclared environment read"),
    })
}

fn snapshot(child: bool, workspace: bool) -> Snapshot {
    let bound = json!({"customLabel":null,"presentationId": hex(5), "label": "caller", "binding": {"terminalId": TERMINAL, "workerId": WORKER}});
    let unbound =
        json!({"customLabel":null,"presentationId": hex(6), "label": "empty", "binding": null});
    let tab = json!({"tabId": hex(3), "label": "shell", "pinned": false, "groupId": null,
        "root": {"kind": "panel", "panelId": hex(4), "primaryPresentation": if child { &unbound } else { &bound },
        "childPresentations": if child { vec![bound.clone()] } else { vec![] }}});
    let mut document = json!({"kind": "workbench.snapshot", "protocolVersion": 1, "sessionId": hex(1), "revision": "7",
        "navigation": {"projects":[], "directories":[], "workspaces":[{"workspaceId":hex(2),"terminalContextId":hex(2),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"}]},
        "terminalContexts": [{"terminalContextId": hex(2), "groups": [], "tabs": [tab.clone()]}]});
    if workspace {
        document["navigation"]["projects"] = json!([{"projectId": hex(7), "label": "project"}]);
        document["navigation"]["workspaces"] = json!([{"workspaceId": hex(8), "projectId": hex(7), "label": "work", "primaryDirectoryId": null, "terminalContextId": hex(9)}]);
        document["terminalContexts"] = json!([
            {"terminalContextId": hex(9), "groups": [], "tabs": [tab]}
        ]);
    }
    parse_snapshot(&serde_json::to_vec(&document).expect("encode fixture")).expect("valid fixture")
}

#[test]
fn current_binding_follows_primary_child_and_moved_workspace_with_one_read() {
    for child in [false, true] {
        for workspace in [false, true] {
            let (source, projection) =
                read(None, environment, || Ok(snapshot(child, workspace))).expect("caller");
            let result = projection.result();
            assert_eq!(result["panel"]["panelId"], hex(4));
            assert_eq!(result["panel"]["tabId"], hex(3));
            assert_eq!(result["panel"]["context"]["kind"], "workspace");
            assert_eq!(
                result["panel"]["context"].get("workspaceId"),
                Some(&json!(hex(if workspace { 8 } else { 2 })))
            );
            assert_eq!(source["revision"], "7");
            // Windows composes this target into WorkbenchQuery before emitting the current Windows query version.
            #[cfg(not(windows))]
            {
                let output = super::super::success_output(
                    projection,
                    source,
                    &QueryCommand::PanelCurrent(crate::invocation::PanelQueryTarget::Current),
                    OutputFormat::Json,
                    Locale::EnUs,
                );
                let document: Value = serde_json::from_slice(&output.stdout).expect("envelope");
                let schema: Value =
                    serde_json::from_str(include_str!("../schemas/orchbee.panel.current.json"))
                        .expect("schema");
                jsonschema::draft202012::new(&schema)
                    .expect("validator")
                    .validate(&document)
                    .expect("valid result");
                assert_eq!(document["schema"], "orchbee.panel.current");
                assert_eq!(document["version"], 1);
                assert_eq!(output.status, 0);
            }
        }
    }
}

#[test]
fn explicit_selector_never_reads_environment() {
    let (_, result) = read(
        Some(&Target::Selector("@panel:0x4".into())),
        |_| panic!("explicit target cannot read environment"),
        || Ok(snapshot(false, false)),
    )
    .expect("explicit target");
    assert_eq!(result.result()["panel"]["panelId"], hex(4));
}

#[test]
fn invalid_or_absent_context_fails_before_discovery_without_echoing_values() {
    for variable in ["ORCHBEE_TERMINAL_ID", "ORCHBEE_WORKER_ID"] {
        for invalid in [
            None,
            Some(""),
            Some("@terminal:0xabc"),
            Some("secret-untrusted-value"),
            Some("00000000000000000000000000000000"),
            Some("000000000000000000000000000000AB"),
        ] {
            let failure = read(
                None,
                |name| {
                    if name == variable {
                        invalid.map(OsString::from)
                    } else {
                        environment(name)
                    }
                },
                || panic!("invalid context cannot discover runtime"),
            )
            .expect_err("failure");
            assert_eq!(failure.kind, FailureKind::InvalidUsage);
            assert!(failure.source.is_none());
            assert!(failure.details.is_none());
            for locale in [Locale::EnUs, Locale::ZhCn] {
                let diagnostic = CliText::new(locale).cli_failure(&failure);
                assert!(diagnostic.contains("panel current --panel"));
                assert!(!diagnostic.contains("secret-untrusted-value"));
            }
        }
    }
}

#[cfg(windows)]
#[test]
fn non_unicode_context_is_local_usage_failure() {
    use std::os::windows::ffi::OsStringExt;
    let failure = read(
        None,
        |_| Some(OsString::from_wide(&[0xd800])),
        || panic!("no discovery"),
    )
    .expect_err("failure");
    assert_eq!(failure.kind, FailureKind::InvalidUsage);
}

#[test]
fn stale_or_wrong_worker_does_not_fall_back_to_any_panel() {
    for variable in ["ORCHBEE_TERMINAL_ID", "ORCHBEE_WORKER_ID"] {
        let failure = read(
            None,
            |name| {
                if name == variable {
                    Some(hex(99).into())
                } else {
                    environment(name)
                }
            },
            || Ok(snapshot(false, false)),
        )
        .expect_err("failure");
        assert_eq!(failure.kind, FailureKind::NotFound);
        assert_eq!(failure.source.as_ref().expect("source")["revision"], "7");
        validate_failure(failure);
    }
}

#[test]
fn duplicate_terminal_identity_fails_closed_even_with_one_matching_worker() {
    let mut snapshot = snapshot(false, false);
    let PanelNode::Panel {
        primary_presentation,
        child_presentations,
        ..
    } = &mut snapshot.terminal_contexts[0].tabs[0].root
    else {
        panic!("panel fixture")
    };
    let mut duplicate = primary_presentation.clone();
    duplicate.binding.as_mut().expect("binding").worker_id =
        WorkerId::parse(&hex(99)).expect("worker");
    child_presentations.push(duplicate);
    let failure = read(None, environment, || Ok(snapshot)).expect_err("duplicate failure");
    assert_eq!(failure.kind, FailureKind::ProtocolInvalid);
    validate_failure(failure);
}

fn validate_failure(failure: Failure) {
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.cli.failure.json")).expect("schema");
    let output = crate::cli::failure_output(failure, Some(OutputFormat::Json), Locale::EnUs);
    let document: Value = serde_json::from_slice(&output.stdout).expect("failure document");
    jsonschema::draft202012::new(&schema)
        .expect("validator")
        .validate(&document)
        .expect("valid failure envelope");
}

#[test]
fn optional_client_caller_requires_both_ids_or_neither_and_reads_each_once() {
    for mask in 0..4 {
        let mut reads = Vec::new();
        let result = PanelTarget::prepare_optional(|name| {
            reads.push(name.to_owned());
            if (name == "ORCHBEE_TERMINAL_ID" && mask & 1 != 0)
                || (name == "ORCHBEE_WORKER_ID" && mask & 2 != 0)
            {
                environment(name)
            } else {
                None
            }
        });
        assert_eq!(reads, ["ORCHBEE_TERMINAL_ID", "ORCHBEE_WORKER_ID"]);
        match mask {
            0 => assert!(result.unwrap().is_none()),
            3 => assert_eq!(
                result
                    .unwrap()
                    .unwrap()
                    .resolve(&snapshot(false, false))
                    .unwrap(),
                Target::Id(hex(4))
            ),
            _ => assert!(result.is_err()),
        }
    }
}

#[test]
fn exact_caller_resolution_keeps_terminal_identity_for_primary_and_child() {
    for child in [false, true] {
        for workspace in [false, true] {
            let (panel, terminal) = PanelTarget::prepare(None, environment)
                .unwrap()
                .resolve_exact(&snapshot(child, workspace))
                .unwrap();
            assert_eq!(panel, Target::Id(hex(4)));
            assert_eq!(terminal.unwrap().as_str(), TERMINAL);
        }
    }
    let (_, terminal) = PanelTarget::prepare(Some(&Target::Id(hex(4))), |_| {
        panic!("explicit does not read env")
    })
    .unwrap()
    .resolve_exact(&snapshot(true, false))
    .unwrap();
    assert!(terminal.is_none());
}
