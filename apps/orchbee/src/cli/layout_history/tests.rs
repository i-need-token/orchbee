//! The hand-written machine schemas accept what the command emits, in both locales' text paths.
use super::*;
use orchbee_workbench_protocol::{
    LayoutHistoryEntry, LayoutHistoryFocus, ProtocolVersion, WorkbenchRevision, WorkbenchTabId,
    WorkbenchTerminalContextId,
};

fn id(value: u128) -> String {
    format!("{value:032x}")
}

#[test]
fn receipts_validate_their_public_schemas() {
    let history: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/orchbee.layout.history.json")).unwrap();
    let reopen: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/orchbee.layout.reopen.json")).unwrap();
    let (history, reopen) = (
        jsonschema::draft202012::new(&history).unwrap(),
        jsonschema::draft202012::new(&reopen).unwrap(),
    );
    let request_id = WorkbenchRequestId::parse(&id(1)).unwrap();
    let reply = |outcome| {
        Observation::Server(WorkbenchLayoutHistoryResult {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
            outcome,
        })
    };
    let mut answer = Answer {
        schema: "orchbee.layout.history",
        request_id: request_id.clone(),
        session_id: SessionId::parse(&id(2)).unwrap(),
        observation: reply(LayoutHistoryOutcome::Listed {
            entries: vec![LayoutHistoryEntry {
                saved_at: 1_762_480_800_000,
                workspaces: 2,
                tabs: 5,
                workspaces_not_open: 1,
                tabs_not_open: 3,
            }],
        }),
        interrupted: false,
        cleanup_failed: false,
    };
    assert!(history.is_valid(&answer.document()));
    assert!(!reopen.is_valid(&answer.document()));
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let emitted = answer.emit(OutputFormat::Text, locale);
        assert_eq!(emitted.status, 0);
        assert!(
            String::from_utf8(emitted.stdout)
                .unwrap()
                .contains("1762480800000")
        );
    }
    answer.schema = "orchbee.layout.reopen";
    answer.observation = reply(LayoutHistoryOutcome::Reopened {
        revision: WorkbenchRevision::new(9),
        workspaces: 1,
        tabs: 2,
        focus: Some(LayoutHistoryFocus {
            terminal_context_id: WorkbenchTerminalContextId::parse(&id(3)).unwrap(),
            tab_id: WorkbenchTabId::parse(&id(4)).unwrap(),
        }),
    });
    assert_eq!(answer.exit_code(), 0);
    assert!(reopen.is_valid(&answer.document()));
    answer.observation = Observation::Unknown("response-deadline");
    assert_eq!(answer.exit_code(), 8);
    assert!(reopen.is_valid(&answer.document()));
}
