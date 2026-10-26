use super::*;
use serde_json::Value;

fn selector(value: &str) -> LaunchProfileSelector {
    LaunchProfileSelector::parse(value).unwrap()
}

fn tab_create(launch: Option<LaunchSelection>, profile: Option<&str>) -> WorkbenchAction {
    WorkbenchAction::TabCreate {
        placement: None,
        automatic_label: false,
        terminal_context_id: WorkbenchTerminalContextId::parse("33333333333333333333333333333333")
            .unwrap(),
        label: WorkbenchLabel::parse("t").unwrap(),
        geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
        launch,
        profile: profile.map(selector),
        options: Default::default(),
    }
}

fn command_request(action: WorkbenchAction) -> WorkbenchRequest {
    WorkbenchRequest::Command(WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("11111111111111111111111111111111").unwrap(),
        session_id: SessionId::parse("22222222222222222222222222222222").unwrap(),
        expected_revision: WorkbenchRevision::new(0),
        action,
    })
}

fn command_json(action_json: &str) -> Vec<u8> {
    format!(
        r#"{{"kind":"workbench.command","protocolVersion":1,"requestId":"11111111111111111111111111111111","sessionId":"22222222222222222222222222222222","expectedRevision":"0","action":{action_json}}}"#
    )
    .into_bytes()
}

#[test]
fn explicit_launch_and_profile_are_mutually_exclusive_on_both_wire_directions() {
    let launch = LaunchSelection::new("/bin/sh".to_owned(), Vec::new()).unwrap();

    // Parse direction: a document carrying both members is rejected before any
    // action value exists.
    let both = command_json(
        r#"{"kind":"tab.create","terminalContextId":"33333333333333333333333333333333","label":"t","geometry":{"columns":80,"rows":24,"cellPixelWidth":0,"cellPixelHeight":0},"launch":{"exec":"/bin/sh","args":[]},"profile":"dev-shell"}"#,
    );
    assert_eq!(
        parse_workbench_request(&both),
        Err(ContractError::InvalidDocument)
    );

    // Typed direction: a typed action with both members refuses to serialize —
    // the wire can never carry the ambiguous shape.
    assert_eq!(
        serialize_workbench_request(&command_request(tab_create(
            Some(launch.clone()),
            Some("dev-shell"),
        ))),
        Err(ContractError::InvalidDocument)
    );

    // Each side alone round-trips unchanged.
    for action in [
        tab_create(Some(launch.clone()), None),
        tab_create(None, Some("dev-shell")),
        tab_create(None, None),
    ] {
        let request = command_request(action);
        let encoded = serialize_workbench_request(&request).unwrap();
        assert_eq!(parse_workbench_request(&encoded).unwrap(), request);
    }

    // Panel split enforces the same closed choice.
    let panel_both = command_json(
        r#"{"kind":"panel.split","terminalContextId":"33333333333333333333333333333333","tabId":"44444444444444444444444444444444","panelId":"55555555555555555555555555555555","direction":"left","label":"t","geometry":{"columns":80,"rows":24,"cellPixelWidth":0,"cellPixelHeight":0},"launch":{"exec":"/bin/sh","args":[]},"profile":"dev-shell"}"#,
    );
    assert_eq!(
        parse_workbench_request(&panel_both),
        Err(ContractError::InvalidDocument)
    );
}

#[test]
fn launch_profile_actions_serialize_their_documented_wire_kinds() {
    let create = command_request(WorkbenchAction::LaunchProfileCreate {
        name: LaunchProfileName::parse("dev-shell").unwrap(),
        launch: LaunchSelection::new(
            "/bin/zsh".to_owned(),
            vec!["-f".to_owned(), "--no-globalrcs".to_owned()],
        )
        .unwrap(),
    });
    let encoded = serialize_workbench_request(&create).unwrap();
    let json: Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(json["action"]["kind"], "launch-profile.create");
    assert_eq!(json["action"]["name"], "dev-shell");
    assert_eq!(json["action"]["launch"]["exec"], "/bin/zsh");
    assert_eq!(
        json["action"]["launch"]["args"],
        serde_json::json!(["-f", "--no-globalrcs"])
    );
    assert_eq!(parse_workbench_request(&encoded).unwrap(), create);

    let delete = command_request(WorkbenchAction::LaunchProfileDelete {
        selector: selector("dev-shell"),
    });
    let encoded = serialize_workbench_request(&delete).unwrap();
    let json: Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(json["action"]["kind"], "launch-profile.delete");
    assert_eq!(json["action"]["selector"], "dev-shell");
    assert_eq!(parse_workbench_request(&encoded).unwrap(), delete);
}

#[test]
fn profile_member_is_omitted_when_absent_and_carried_when_present() {
    let absent = serialize_workbench_request(&command_request(tab_create(None, None))).unwrap();
    let json: Value = serde_json::from_slice(&absent).unwrap();
    assert_eq!(json["action"]["profile"], Value::Null);

    let present =
        serialize_workbench_request(&command_request(tab_create(None, Some("dev-shell")))).unwrap();
    let json: Value = serde_json::from_slice(&present).unwrap();
    assert_eq!(json["action"]["profile"], "dev-shell");
}

#[test]
fn catalog_read_intent_and_result_round_trip_the_full_outcome_set() {
    for selector_value in [None, Some("dev-shell")] {
        let request =
            WorkbenchRequest::LaunchProfileCatalogRead(WorkbenchLaunchProfileCatalogRead {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse("11111111111111111111111111111111").unwrap(),
                selector: selector_value.map(selector),
            });
        let encoded = serialize_workbench_request(&request).unwrap();
        assert_eq!(parse_workbench_request(&encoded).unwrap(), request);
    }

    let session = SessionId::parse("22222222222222222222222222222222").unwrap();
    let entry = LaunchProfileEntry {
        profile_id: LaunchProfileId::parse(&format!("{}:a", session.as_str())).unwrap(),
        name: LaunchProfileName::parse("dev-shell").unwrap(),
        launch: LaunchSelection::new("/bin/zsh".to_owned(), Vec::new()).unwrap(),
    };
    let outcomes = [
        LaunchProfileCatalogOutcome::Catalog {
            session_id: session.clone(),
            catalog_revision: LaunchProfileCatalogRevision::new(7),
            entries: vec![entry.clone()],
        },
        LaunchProfileCatalogOutcome::Profile {
            session_id: session.clone(),
            catalog_revision: LaunchProfileCatalogRevision::new(7),
            profile: entry.clone(),
        },
        LaunchProfileCatalogOutcome::NotFound {},
        LaunchProfileCatalogOutcome::Ambiguous {
            candidate_ids: vec![entry.profile_id.clone()],
        },
        LaunchProfileCatalogOutcome::Denied {},
        LaunchProfileCatalogOutcome::Unavailable {
            reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
        },
    ];
    for outcome in outcomes {
        let response =
            WorkbenchResponse::LaunchProfileCatalog(WorkbenchLaunchProfileCatalogResult {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse("11111111111111111111111111111111").unwrap(),
                outcome,
            });
        let encoded = serialize_workbench_response(&response).unwrap();
        assert_eq!(parse_workbench_response(&encoded).unwrap(), response);
    }
}

#[test]
fn launch_profile_lexical_types_enforce_their_bounds() {
    // Name and selector share the 1..=512-byte UTF-8 envelope; the id carries
    // the fixed <sessionHex>:<counterHex> shape minted Server-side.
    assert!(LaunchProfileName::parse(&"n".repeat(512)).is_ok());
    assert!(LaunchProfileName::parse(&"n".repeat(513)).is_err());
    assert!(LaunchProfileName::parse("").is_err());
    assert!(LaunchProfileSelector::parse(&"s".repeat(512)).is_ok());
    assert!(LaunchProfileSelector::parse(&"s".repeat(513)).is_err());
    assert!(LaunchProfileSelector::parse("").is_err());
    assert!(LaunchProfileId::parse(&"i".repeat(128)).is_ok());
    assert!(LaunchProfileId::parse(&"i".repeat(129)).is_err());
    assert!(LaunchProfileId::parse("").is_err());
}
