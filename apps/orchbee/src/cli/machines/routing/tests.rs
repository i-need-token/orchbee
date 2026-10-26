use super::*;
use crate::machines::{Draft, ProfileId};
#[test]
fn routed_path_packets_retain_the_exact_unresolved_operand() {
    for arguments in [
        ["plugin", "link", r"~\remote", "--format=json"].as_slice(),
        ["workspace", "create", "--cwd", r"~\remote", "--format=json"].as_slice(),
        ["worktree", "list", "--cwd", r"~\remote", "--format=json"].as_slice(),
        ["worktree", "create", "--cwd", r"~\remote", "--format=json"].as_slice(),
        ["worktree", "create", "--path", r"~\remote", "--format=json"].as_slice(),
        ["worktree", "open", "--path", r"~\remote", "--format=json"].as_slice(),
    ] {
        let packet = serde_json::json!({"version":1,"cohort":"a".repeat(64),"site":"work","arguments":arguments,"input":null,"locale":"en_US.UTF-8"});
        let (admitted, input) = admit_packet(&serde_json::to_vec(&packet).unwrap()).unwrap();
        assert_eq!(admitted.arguments, arguments);
        assert!(input.is_empty());
    }
}

#[test]
fn selection_uses_id_before_exact_label_and_rejects_ambiguity_and_disabled() {
    let id = "11111111111111111111111111111111";
    let a = Profile {
        id: ProfileId::parse(id).unwrap(),
        draft: Draft::parse("same", "host", "work").unwrap(),
        enabled: true,
    };
    let mut b = a.clone();
    b.id = ProfileId::parse("22222222222222222222222222222222").unwrap();
    let mut catalog = Catalog {
        revision: 1,
        profiles: vec![a, b],
    };
    assert_eq!(select(&catalog, "same").unwrap_err(), "ambiguous-machine");
    catalog.profiles[1].draft.label = id.into();
    assert_eq!(select(&catalog, id).unwrap().id.as_str(), id);
    assert_eq!(select(&catalog, "Same").unwrap_err(), "not-found");
    catalog.profiles[0].enabled = false;
    assert_eq!(select(&catalog, id).unwrap_err(), "disabled-machine");
}
#[test]
fn wire_rejects_extra_authority_and_preserves_binary_input_and_failed_exit() {
    let packet = Packet {
        version: 1,
        cohort: "a".repeat(64),
        site: "work".into(),
        arguments: vec![
            "terminal".into(),
            "send".into(),
            "@terminal:0x22".into(),
            "--stdin".into(),
        ],
        input: Some(STANDARD.encode([0, 255, 13, 10])),
        locale: "en_US.UTF-8".into(),
    };
    let bytes = serde_json::to_vec(&packet).unwrap();
    assert_eq!(admit_packet(&bytes).unwrap().1, [0, 255, 13, 10]);
    for args in [
        vec!["--machine=x", "workspace", "list"],
        vec!["--machine-relay-v1"],
        vec!["--session=x", "workspace", "list"],
        vec!["machine", "list"],
        vec!["terminal", "send", "@terminal:0x22", "--file=local-secret"],
    ] {
        let mut value = serde_json::to_value(&packet).unwrap();
        value["arguments"] = serde_json::json!(args);
        assert!(admit_packet(&serde_json::to_vec(&value).unwrap()).is_none());
    }
    let mut value = serde_json::to_value(&packet).unwrap();
    value["executable"] = serde_json::json!("cmd.exe");
    assert!(admit_packet(&serde_json::to_vec(&value).unwrap()).is_none());
    let reply = Reply {
        version: 1,
        outcome: Outcome::Completed,
        status: Some(255),
        stdout: STANDARD.encode([0, 255]),
        stderr: STANDARD.encode("中文 failure\n"),
    };
    let output = reply.output().ok().unwrap();
    assert_eq!(output.status, 255);
    assert_eq!(output.stdout, [0, 255]);
    assert_eq!(output.stderr, "中文 failure\n");
    for outcome in [Outcome::NotSubmitted, Outcome::Unknown] {
        let output = failed(
            "remote-protocol",
            5,
            outcome,
            OutputFormat::Json,
            Locale::EnUs,
        );
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../schemas/orchbee.cli.failure.machine-route.json"
        ))
        .unwrap();
        jsonschema::validator_for(&schema)
            .unwrap()
            .validate(&serde_json::from_slice(&output.stdout).unwrap())
            .unwrap();
    }
}
