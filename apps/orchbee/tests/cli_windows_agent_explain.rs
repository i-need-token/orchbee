//! Actual CLI transport: physical explanations, binding refusal and localized safe rendering.
#![cfg(windows)]
use crate::peer_support as support;
use orchbee_local_ipc::LocalListener;
use orchbee_workbench_protocol::*;
use serde_json::{Value, json};
use support::{Fixture, assert_closed, bound_snapshot_reply, peer, resolve_reply, validate};

#[test]
fn physical_explanation_pins_binding_preserves_evidence_and_escapes_both_locales() {
    for case in [
        "rules",
        "fallback",
        "manifest",
        "wrong-binding",
        "changed-workbench",
        "en",
        "zh",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let corpus: Value = serde_json::from_str(include_str!(
                "../../../schemas/workbench/v1/workbench-panel-agent-conformance-v1.json"
            ))
            .unwrap();
            let name = match case {
                "fallback" => "explain-detected-provider-behind-custom-label",
                "manifest" | "en" | "zh" => "explain-manifest-local-override",
                _ => "explain-first-priority-tie",
            };
            let mut terminal = corpus["valid"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["name"] == name)
                .unwrap()["document"]["outcome"]["explanation"]
                .clone();
            terminal["binding"]["presentationId"] = json!("11111111111111111111111111111111");
            let binding = terminal["binding"].clone();
            resolve_reply(
                &mut stream,
                json!({"kind":"resolved-target","resolvedBy":"name","target":{"kind":"terminal","binding":binding}}),
            );
            bound_snapshot_reply(&mut stream);
            let WorkbenchRequest::PanelAgent(request) =
                read_workbench_request(&mut stream).unwrap()
            else {
                panic!("one Explain")
            };
            assert_eq!(
                json!(request.action),
                json!({"kind":"explain","binding":binding})
            );
            if case == "wrong-binding" {
                terminal["binding"]["generation"] = json!("9".repeat(32));
            }
            if matches!(case, "en" | "zh") {
                terminal["explanation"]["manifest"]["cachedRemoteVersion"] = json!("0");
                terminal["explanation"]["manifest"]["warnings"] =
                    json!([{"source":"remote-cache","kind":"older-than-bundled"}]);
                terminal["explanation"]["evaluatedRules"][0]["evidence"]["regionPreview"] =
                    json!("\u{1b}[31mThinking");
            }
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::PanelAgent(PanelAgentResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    session_id: request.session_id,
                    outcome: PanelAgentOutcome::Explained {
                        workbench_revision: WorkbenchRevision::new(
                            if case == "changed-workbench" { 10 } else { 9 },
                        ),
                        report_revision: WorkbenchRevision::new(2),
                        explanation: Box::new(serde_json::from_value(terminal).unwrap()),
                    },
                }),
            )
            .unwrap();
            assert_closed(&mut stream);
        });
        let text = matches!(case, "en" | "zh");
        let locale = if case == "zh" {
            "zh_CN.UTF-8"
        } else {
            "en_US.UTF-8"
        };
        let output = fixture.run_with_env(
            &[
                "agent",
                "explain",
                "--name",
                "alpha",
                "--format",
                if text { "text" } else { "json" },
            ],
            None,
            None,
            &[("LC_ALL", std::ffi::OsStr::new(locale))],
        );
        handler.join().unwrap();
        if matches!(case, "wrong-binding" | "changed-workbench") {
            assert!(!output.status.success(), "{output:?}");
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_ne!(document["schema"], "orchbee.agent.explain");
            continue;
        }
        assert!(output.status.success(), "{output:?}");
        if text {
            let output = String::from_utf8(output.stdout).unwrap();
            assert!(
                output.contains(if case == "zh" {
                    "当前有效状态"
                } else {
                    "Effective state"
                }),
                "{output}"
            );
            assert!(output.contains("Thinking"));
            assert!(
                output.contains(if case == "zh" {
                    "claude-code 智能体，已识别"
                } else {
                    "claude-code Agent, recognized"
                }),
                "{output}"
            );
            assert!(
                output.contains(if case == "zh" {
                    "缓存版本低于内置规则"
                } else {
                    "cached version is older than bundled rules"
                }),
                "{output}"
            );
            assert!(
                !output.contains('\u{1b}'),
                "terminal control bytes must be escaped"
            );
        } else {
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.agent.explain.json"),
            );
            assert_eq!(document["version"], 1);
            assert_eq!(document["source"]["kind"], "panel-agent-explain");
            assert_eq!(document["result"]["kind"], "terminal-found");
            assert!(document["result"]["terminal"]["sample"].is_object());
            assert_eq!(
                document["result"]["terminal"]["identity"],
                json!({"result":"agent","providerId":"claude-code","grade":"recognized"})
            );
        }
    }
}
