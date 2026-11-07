use super::*;
use orchbee_workbench_protocol::{PresentationBinding, TerminalId};

const CLAUDE: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";
const CODEX: &str = "7C9E6679-7425-40DE-944B-E07FC1F90AE7";
const OTHER: &str = "a3bb189e-8bf9-3888-9912-ace4e6543002";

fn directories() -> Value {
    json!([
        {"presentationId": identity(11), "path": "C:\\primary"},
        {"presentationId": identity(15), "path": "D:\\child"}
    ])
}

fn sessions() -> Value {
    json!([
        {"presentationId": identity(11), "provider": "claude-code", "sessionId": CLAUDE},
        {"presentationId": identity(15), "provider": "codex", "sessionId": CODEX}
    ])
}

fn saved() -> Vec<u8> {
    changed(|v| {
        v["terminalDirectories"] = directories();
        v["agentSessions"] = sessions();
    })
}

fn presentation_id(id: u128) -> TerminalPresentationId {
    TerminalPresentationId::parse(&identity(id)).unwrap()
}

fn session(provider: AgentResumeProvider, id: &str) -> RetainedAgentSession {
    RetainedAgentSession::new(provider, id).unwrap()
}

fn rows(bytes: &[u8]) -> Value {
    serde_json::from_slice::<Value>(bytes).unwrap()["agentSessions"].clone()
}

#[test]
fn checkpoint_requires_strict_agent_session_rows() {
    assert!(WorkbenchCheckpoint::parse(&saved()).is_ok());
    // One id under two providers is two distinct references.
    let shared_id = changed(|v| {
        v["terminalDirectories"] = directories();
        v["agentSessions"] = sessions();
        v["agentSessions"][1]["sessionId"] = json!(CLAUDE);
    });
    assert!(WorkbenchCheckpoint::parse(&shared_id).is_ok());
    for fault in 0..12 {
        let bad = changed(|v| {
            v["terminalDirectories"] = directories();
            v["agentSessions"] = sessions();
            let row = &mut v["agentSessions"][0];
            match fault {
                0 => v["agentSessions"] = Value::Null,
                1 => row["presentationId"] = json!(identity(999)),
                2 => {
                    let mut copy = row.clone();
                    copy["sessionId"] = json!(OTHER);
                    v["agentSessions"].as_array_mut().unwrap().push(copy);
                }
                3 => row["provider"] = json!("claude"),
                4 => row["sessionId"] = json!("-f8fad5b-d9cb-469f-a165-70867728950e"),
                5 => row["sessionId"] = json!("0f8fad5bd9cb-469f-a165-70867728950e-"),
                6 => row["sessionId"] = json!(format!("{CLAUDE} ")),
                7 => row["sessionId"] = json!("0f8fad5b-d9cb-469f-a165-70867728950g"),
                8 => {
                    v["agentSessions"][1]["provider"] = json!("claude-code");
                    v["agentSessions"][1]["sessionId"] = json!(CLAUDE);
                }
                9 => row["presentationId"] = json!(identity(14)),
                10 => row["extra"] = json!(true),
                _ => {
                    row.as_object_mut().unwrap().remove("sessionId");
                }
            }
        });
        assert!(WorkbenchCheckpoint::parse(&bad).is_err(), "fault {fault}");
    }
}

/// Each provider admits only its own id grammar, and every admitted id is inert in cmd and
/// PowerShell: ASCII letters, digits, `_` and `-` (plus Letta's stored `default:`), never a leading
/// `-`.
#[test]
fn each_provider_admits_only_its_own_inert_id_grammar() {
    use AgentResumeProvider as P;
    let hex64 = "0123456789abcdef".repeat(4);
    let uuid_rows = [CLAUDE.to_owned(), CODEX.to_owned(), OTHER.to_owned()];
    let uuid_faults = vec![
        format!("{CLAUDE}0"),
        CLAUDE.replace('-', ""),
        format!("session_{CLAUDE}"),
        "0f8fad5bd9cb-469f-a165-70867728950e-".to_owned(),
        "0f8fad5b-d9cb-469f-a165-70867728950g".to_owned(),
    ];
    let mut table: Vec<(P, Vec<String>, Vec<String>)> = [
        P::ClaudeCode,
        P::Codex,
        P::Grok,
        P::Cursor,
        P::QoderCli,
        P::Antigravity,
        P::Droid,
        P::Pi,
        P::Omp,
        P::Gemini,
        P::GitHubCopilot,
        P::QwenCode,
    ]
    .into_iter()
    .map(|provider| (provider, uuid_rows.to_vec(), uuid_faults.clone()))
    .collect();
    let opencode = "ses_0123456789abCdEfGhIjKlMnOp";
    for provider in [P::OpenCode, P::Kilo] {
        table.push((
            provider,
            vec![opencode.to_owned()],
            vec![
                opencode.replace("ses_", "SES_"),
                opencode.replace("ses_", "sess_"),
                opencode.replace("0123456789ab", "0123456789AB"),
                format!("{opencode}x"),
                opencode[..29].to_owned(),
                CLAUDE.to_owned(),
            ],
        ));
    }
    table.push((
        P::Kimi,
        vec![format!("session_{CLAUDE}"), format!("session_{CODEX}")],
        vec![
            CLAUDE.to_owned(),
            format!("Session_{CLAUDE}"),
            format!("session-{CLAUDE}"),
            format!("session_{OTHER}"),
            format!("session_{CLAUDE}0"),
        ],
    ));
    table.push((
        P::Letta,
        vec![
            format!("conv-{CLAUDE}"),
            format!("local-conv-{CLAUDE}"),
            format!("default:agent-{CLAUDE}"),
            format!("default:agent-local-{CLAUDE}"),
        ],
        vec![
            CLAUDE.to_owned(),
            format!("Conv-{CLAUDE}"),
            format!("conversation-{CLAUDE}"),
            format!("agent-{CLAUDE}"),
            format!("default:{CLAUDE}"),
            format!("default:conv-{CLAUDE}"),
            format!("local-default:agent-{CLAUDE}"),
            format!("local-local-conv-{CLAUDE}"),
            format!("default:agent-local-local-{CLAUDE}"),
            format!("conv-{CLAUDE}0"),
        ],
    ));
    table.push((
        P::Devin,
        vec![
            "abc".to_owned(),
            "A1-b2-C3".to_owned(),
            "a-b-c-d-e-f-g-h".to_owned(),
            "a".repeat(64),
            CLAUDE.to_owned(),
        ],
        vec![
            String::new(),
            "a-b-c-d-e-f-g-h-i".to_owned(),
            "a".repeat(65),
            "a--b".to_owned(),
            "a-".to_owned(),
            "a_b".to_owned(),
            "a.b".to_owned(),
            "a:b".to_owned(),
        ],
    ));
    table.push((
        P::Hermes,
        vec![
            "20250101_120000_abcdef".to_owned(),
            "20250101_120000_0123456789ab".to_owned(),
            format!("local-{hex64}"),
        ],
        vec![
            "20250101_120000_abcde".to_owned(),
            "20250101_120000_0123456789abc".to_owned(),
            "20250101_120000_ABCDEF".to_owned(),
            "2025010_120000_abcdef".to_owned(),
            "20250101-120000-abcdef".to_owned(),
            "20250101_120000_abcdef_0".to_owned(),
            format!("local-{}", &hex64[..63]),
            format!("local-{}", hex64.to_uppercase()),
            format!("local-{hex64}0"),
            format!("Local-{hex64}"),
        ],
    ));
    assert_eq!(table.len(), P::ALL.len());
    for (provider, accepted, rejected) in table {
        for id in &accepted {
            assert!(
                RetainedAgentSession::new(provider, id).is_some(),
                "{provider:?} rejects {id}"
            );
            assert!(
                !id.starts_with('-')
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
                    || provider == P::Letta && id.starts_with("default:agent-"),
                "{provider:?} admits a token that is not inert: {id}"
            );
            let middle = id.len() / 2;
            for fault in [
                format!("-{id}"),
                format!(" {id}"),
                format!("{id} "),
                format!("{}'{}", &id[..middle], &id[middle..]),
                format!("{}\u{e9}{}", &id[..middle], &id[middle..]),
            ] {
                assert!(
                    RetainedAgentSession::new(provider, &fault).is_none(),
                    "{provider:?} admits {fault:?}"
                );
            }
        }
        for id in &rejected {
            assert!(
                RetainedAgentSession::new(provider, id).is_none(),
                "{provider:?} admits {id}"
            );
        }
    }
}

#[test]
fn an_omitted_list_reads_as_no_references() {
    let omitted = changed(|v| {
        v["terminalDirectories"] = directories();
        v.as_object_mut().unwrap().remove("agentSessions");
    });
    let a = restore(&omitted).unwrap();
    assert_eq!(a.pending_agent_session(&presentation_id(11)), None);
    let bytes = a
        .checkpoint_with_observations(&worker(400), &[], Some(&[]))
        .unwrap();
    assert_eq!(rows(&bytes), json!([]));
}

#[test]
fn restored_references_stay_pending_until_taken_and_a_capture_carries_them() {
    let mut a = restore(&saved()).unwrap();
    assert_eq!(
        a.pending_agent_session(&presentation_id(11)),
        Some(&session(AgentResumeProvider::ClaudeCode, CLAUDE))
    );
    assert_eq!(
        a.pending_agent_session(&presentation_id(15)),
        Some(&session(AgentResumeProvider::Codex, CODEX))
    );
    let carried = a
        .checkpoint_with_observations(&worker(400), &[], Some(&[]))
        .unwrap();
    assert_eq!(rows(&carried), sessions());
    assert_eq!(rows(&a.checkpoint(&worker(400)).unwrap()), json!([]));
    assert_eq!(
        rows(
            &a.checkpoint_with_observations(&worker(400), &[], None)
                .unwrap()
        ),
        json!([])
    );

    let revision = a.snapshot().revision;
    assert_eq!(
        a.take_pending_agent_session(&presentation_id(15)),
        Some(session(AgentResumeProvider::Codex, CODEX))
    );
    assert_eq!(a.take_pending_agent_session(&presentation_id(15)), None);
    assert_eq!(a.snapshot().revision, revision);
    let taken = a
        .checkpoint_with_observations(&worker(400), &[], Some(&[]))
        .unwrap();
    assert_eq!(rows(&taken), json!([sessions()[0].clone()]));
    let fresh = restore(&taken).unwrap();
    assert!(fresh.pending_agent_session(&presentation_id(11)).is_some());
    assert_eq!(fresh.pending_agent_session(&presentation_id(15)), None);

    // A pending reference without a directory row, or for a removed member, is not written.
    a.restart_agent_sessions.insert(
        presentation_id(14),
        session(AgentResumeProvider::Codex, OTHER),
    );
    a.restart_agent_sessions.insert(
        presentation_id(1000),
        session(AgentResumeProvider::Codex, CODEX),
    );
    let bytes = a
        .checkpoint_with_observations(&worker(400), &[], Some(&[]))
        .unwrap();
    assert_eq!(rows(&bytes), json!([sessions()[0].clone()]));
}

#[test]
fn a_live_reference_overrides_pending_and_the_first_member_keeps_a_shared_session() {
    let mut a = restore(&saved()).unwrap();
    let targets = a.restoration_targets(&worker(400)).unwrap();
    assert_eq!(targets[0].presentation_id(), &presentation_id(11));
    let binding = PresentationBinding {
        worker_id: worker(400),
        terminal_id: TerminalId::parse(&identity(999)).unwrap(),
    };
    a.prepare_restored_terminal(&targets[0], binding.clone(), None)
        .unwrap()
        .commit();
    let before = a.snapshot().clone();

    let live = [(binding.clone(), session(AgentResumeProvider::Codex, OTHER))];
    let bytes = a
        .checkpoint_with_observations(&worker(400), &[], Some(&live))
        .unwrap();
    assert_eq!(
        rows(&bytes),
        json!([
            {"presentationId": identity(11), "provider": "codex", "sessionId": OTHER},
            sessions()[1].clone()
        ])
    );
    assert_eq!(a.snapshot(), &before);

    // The live row on the earlier member keeps the session; the later pending duplicate drops.
    let shared = [(binding.clone(), session(AgentResumeProvider::Codex, CODEX))];
    let bytes = a
        .checkpoint_with_observations(&worker(400), &[], Some(&shared))
        .unwrap();
    assert_eq!(
        rows(&bytes),
        json!([{"presentationId": identity(11), "provider": "codex", "sessionId": CODEX}])
    );
    assert!(restore(&bytes).is_ok());

    let mut foreign = live[0].clone();
    foreign.0.worker_id = worker(500);
    let mut stranger = live[0].clone();
    stranger.0.terminal_id = TerminalId::parse(&identity(998)).unwrap();
    for observed in [
        vec![foreign],
        vec![stranger],
        vec![live[0].clone(), live[0].clone()],
    ] {
        assert_eq!(
            a.checkpoint_with_observations(&worker(400), &[], Some(&observed))
                .err(),
            Some(WorkbenchCheckpointError::InvalidAgentSession)
        );
    }
}
