use super::super::metadata_peer;
use super::*;
pub(super) use metadata_peer::unavailable;

#[test]
fn panel_runtime_uses_one_attachment_and_preserves_unavailable_metadata_without_partial_errors() {
    for current in [false, true] {
        for mode in [
            "observed",
            "process-unavailable",
            "terminal-unavailable",
            "wrong-epoch",
            "changed-workbench",
            "deadline",
        ] {
            let fixture = if mode == "deadline" {
                FocusFixture::deadline_sensitive()
            } else {
                FocusFixture::new()
            };
            let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
            let server = peer(&fixture, listener, move |mut stream| {
                snapshot_reply(&mut stream);
                page(&mut stream, 3);
                if mode == "terminal-unavailable" {
                    unavailable(&mut stream);
                } else {
                    metadata_peer::sample(&mut stream, mode, "cmd.exe");
                }
                if !matches!(mode, "wrong-epoch" | "deadline") {
                    snapshot_at(
                        &mut stream,
                        if mode == "changed-workbench" { 13 } else { 12 },
                    );
                }
                assert_closed(&mut stream);
            });
            let started = Instant::now();
            let output = if current {
                fixture.run(&["panel", "current", &id(6), "--format=json"])
            } else {
                fixture.run(&["panel", "get", &id(6), "--format=json"])
            };
            let exited_after = started.elapsed();
            server.join().unwrap();
            let doc = document(&output);
            if mode == "deadline" {
                assert_eq!(output.status.code(), Some(7));
                assert_eq!(doc["failure"]["kind"], "timeout");
                assert!(doc.get("result").is_none());
                assert!(exited_after >= Duration::from_secs(4));
                assert!(
                    exited_after < Duration::from_secs(8),
                    "mode={mode}; current={current}; elapsed={exited_after:?}; output={output:?}"
                );
                continue;
            }
            if mode == "changed-workbench" {
                assert_eq!(output.status.code(), Some(3));
                assert_eq!(doc["failure"]["details"]["reason"], "workbench-changed");
                assert!(doc.get("result").is_none());
                continue;
            }
            if mode == "wrong-epoch" {
                assert_eq!(output.status.code(), Some(5));
                assert_eq!(doc["failure"]["kind"], "protocol-invalid");
                assert!(doc.get("result").is_none());
                continue;
            }
            assert!(output.status.success(), "{output:?}");
            validate(
                &doc,
                if current {
                    include_str!("../../../src/cli/schemas/orchbee.panel.current.json")
                } else {
                    include_str!("../../../src/cli/schemas/orchbee.panel.get.json")
                },
            );
            let panel = &doc["result"]["panel"];
            if mode == "terminal-unavailable" {
                assert_eq!(panel["terminalObservation"]["state"], "unavailable");
                assert!(panel["cwd"].is_null());
                continue;
            }
            assert_eq!(panel["terminalTitle"], "Orchbee 🐝");
            assert_eq!(panel["cwd"], "C:\\frame 项目");
            assert_eq!(panel["cwdSource"], "terminal-frame");
            assert_eq!(
                panel["foregroundCwd"],
                if mode == "observed" {
                    json!("C:\\root")
                } else {
                    Value::Null
                }
            );
            for field in ["terminalObservation", "cwd", "terminalTitle"] {
                let mut broken = doc.clone();
                broken["result"]["panel"]
                    .as_object_mut()
                    .unwrap()
                    .remove(field);
                let schema: Value = serde_json::from_str(if current {
                    include_str!("../../../src/cli/schemas/orchbee.panel.current.json")
                } else {
                    include_str!("../../../src/cli/schemas/orchbee.panel.get.json")
                })
                .unwrap();
                assert!(
                    !jsonschema::validator_for(&schema)
                        .unwrap()
                        .is_valid(&broken)
                );
            }
        }
    }
}
