use super::*;
use crate::machines::{Draft, ProfileId};
/// The packet and input of an admission that runs the routed command.
fn admitted(bytes: &[u8]) -> (Packet, Vec<u8>) {
    match admit_packet(bytes) {
        Some(Admission::Run(packet, input)) => (packet, input),
        _ => panic!("packet was not admitted to run"),
    }
}
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
        let packet = serde_json::json!({"version":1,"session":"work","arguments":arguments,"input":null,"locale":"en_US.UTF-8"});
        let (admitted, input) = admitted(&serde_json::to_vec(&packet).unwrap());
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
        ..Default::default()
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
        session: "work".into(),
        arguments: vec![
            "terminal".into(),
            "send".into(),
            "@terminal:0x22".into(),
            "--stdin".into(),
        ],
        input: Some(STANDARD.encode([0, 255, 13, 10])),
        locale: "en_US.UTF-8".into(),
        extensions: Extensions::new(),
    };
    let bytes = serde_json::to_vec(&packet).unwrap();
    assert_eq!(admitted(&bytes).1, [0, 255, 13, 10]);
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
    for (member, value) in [
        ("executable", serde_json::json!("cmd.exe")),
        ("cohort", serde_json::json!("a".repeat(64))),
    ] {
        let mut packet = serde_json::to_value(&packet).unwrap();
        packet[member] = value;
        assert!(admit_packet(&serde_json::to_vec(&packet).unwrap()).is_none());
    }
    let reply = Reply {
        version: 1,
        outcome: Outcome::Completed,
        status: Some(255),
        stdout: STANDARD.encode([0, 255]),
        stderr: STANDARD.encode("中文 failure\n"),
        extensions: Extensions::new(),
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

#[test]
fn packets_and_replies_ignore_extension_content_and_read_only_their_generations() {
    let packet = serde_json::json!({"version":1,"session":"work","arguments":["workspace","list"],
        "input":null,"locale":"zh_CN.UTF-8","extensions":{"future-member":{"nested":[1,"x"]}}});
    let (admitted, _) = admitted(&serde_json::to_vec(&packet).unwrap());
    assert_eq!(admitted.arguments, ["workspace", "list"]);
    for (member, value) in [
        ("extensions", serde_json::json!({"Bad Name":1})),
        ("extensions", serde_json::json!([])),
        ("version", serde_json::json!(0)),
        ("version", serde_json::json!(2)),
    ] {
        let mut packet = packet.clone();
        packet[member] = value;
        assert!(admit_packet(&serde_json::to_vec(&packet).unwrap()).is_none());
    }
    let reply = serde_json::json!({"version":1,"outcome":"completed","status":0,"stdout":"",
        "stderr":"","extensions":{"future-member":true}});
    let reply: Reply = serde_json::from_value(reply).unwrap();
    assert_eq!(reply.output().ok().unwrap().status, 0);
    let reply = serde_json::json!({"version":2,"outcome":"completed","status":0,"stdout":"",
        "stderr":""});
    let reply: Reply = serde_json::from_value(reply).unwrap();
    assert!(matches!(reply.output(), Err(Outcome::Unknown)));
}

#[test]
fn routed_arguments_this_grammar_refuses_answer_its_usage_failure_without_running() {
    for (locale, expected) in [("en_US.UTF-8", Locale::EnUs), ("zh_CN.UTF-8", Locale::ZhCn)] {
        let arguments = ["workspace", "list", "--no-such-option"];
        let packet = serde_json::json!({"version":1,"session":"work","arguments":arguments,
            "input":null,"locale":locale});
        let Some(Admission::Usage(usage)) = admit_packet(&serde_json::to_vec(&packet).unwrap())
        else {
            panic!("an unparseable routed argument must answer the usage failure");
        };
        let Err(failure) = invocation::parse_selected_request(arguments.iter().map(Into::into))
        else {
            panic!("the routed argument must be unparseable");
        };
        let local = cli::parse_failure(failure, expected);
        assert_eq!(usage.status, 2);
        assert_eq!(
            (usage.status, &usage.stdout, &usage.stderr),
            (local.status, &local.stdout, &local.stderr)
        );
        assert!(!usage.stderr.is_empty());
    }
}
