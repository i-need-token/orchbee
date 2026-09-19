use std::collections::BTreeSet;

use orchbee_client_server_protocol::{
    ClientMessage, CreateAcceptedState, CreateFailureReason, CreateOutcome, CreateRejectedReason,
    CreateUnavailableReason, FRAME_HEADER_BYTES, FRAME_MAGIC, FRAME_VERSION,
    MAXIMUM_FRAME_PAYLOAD_BYTES, MAXIMUM_WORKING_DIRECTORY_BYTES, ProtocolError, ServerMessage,
    StartFailureReason, StartMode, StartOutcome, StartRejectedReason, StartTransition,
    StartUnavailableReason, WorkerConstraint, decode_client_frame, decode_server_frame,
    encode_client_frame, encode_server_frame,
};
use serde::Deserialize;
use serde_json::Value;

const SCHEMA: &str = include_str!("../../../schemas/client-server/v1/client-server.schema.json");
const CORPUS: &str =
    include_str!("../../../schemas/client-server/v1/client-server-conformance-v1.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Corpus {
    contract_version: u8,
    valid_messages: Vec<MessageFixture>,
    equivalent_messages: Vec<EquivalentFixture>,
    invalid_messages: Vec<InvalidFixture>,
    working_directory_cases: Vec<WorkingDirectoryFixture>,
    geometry_cases: Vec<GeometryFixture>,
    forbidden_client_fields: Vec<ForbiddenFieldFixture>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Direction {
    ClientToServer,
    ServerToClient,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Boundary {
    Schema,
    Semantic,
    Encoding,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MessageFixture {
    direction: Direction,
    name: String,
    payload: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EquivalentFixture {
    canonical: String,
    direction: Direction,
    name: String,
    payload: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InvalidFixture {
    boundary: Boundary,
    direction: Direction,
    name: String,
    payload: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ForbiddenFieldFixture {
    field: String,
    name: String,
    value: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkingDirectoryFixture {
    name: String,
    prefix: String,
    repeat: String,
    repeat_count: usize,
    suffix: String,
    valid: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GeometryFixture {
    name: String,
    geometry: Value,
    valid: bool,
}

#[derive(Debug, Eq, PartialEq)]
enum DecodedMessage {
    Client(ClientMessage),
    Server(ServerMessage),
}

#[test]
fn schema_and_corpus_are_well_formed_and_every_canonical_message_round_trips()
-> Result<(), Box<dyn std::error::Error>> {
    let schema = serde_json::from_str::<Value>(SCHEMA)?;
    assert_eq!(
        schema.pointer("/$schema").and_then(Value::as_str),
        Some("https://json-schema.org/draft/2020-12/schema")
    );
    assert_eq!(
        schema.pointer("/$id").and_then(Value::as_str),
        Some("urn:orchbee:schema:client-server:v1")
    );
    assert_eq!(
        schema
            .pointer("/oneOf")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(12)
    );
    assert!(jsonschema::draft202012::meta::is_valid(&schema));
    let schema_validator = jsonschema::draft202012::new(&schema)?;

    let corpus = read_corpus()?;
    assert_eq!(corpus.contract_version, 1);
    let mut names = BTreeSet::new();
    for fixture in &corpus.valid_messages {
        assert!(
            names.insert(fixture.name.as_str()),
            "duplicate fixture {}",
            fixture.name
        );
        let value = serde_json::from_str::<Value>(&fixture.payload)?;
        assert!(
            schema_validator.is_valid(&value),
            "schema rejected canonical fixture {}",
            fixture.name
        );
        let expected_frame = frame(fixture.payload.as_bytes())?;
        let decoded = decode(fixture.direction, fixture.payload.as_bytes())?;
        let encoded = encode(fixture.direction, &decoded)?;
        assert_eq!(
            encoded, expected_frame,
            "canonical fixture {}",
            fixture.name
        );
        assert_eq!(
            decode(fixture.direction, payload(&encoded)?)?,
            decoded,
            "semantic fixture {}",
            fixture.name
        );
    }
    for fixture in corpus
        .invalid_messages
        .iter()
        .filter(|fixture| fixture.boundary == Boundary::Schema)
    {
        let value = serde_json::from_str::<Value>(&fixture.payload)?;
        assert!(
            !schema_validator.is_valid(&value),
            "schema accepted invalid fixture {}",
            fixture.name
        );
    }
    Ok(())
}

#[test]
fn canonical_corpus_reaches_every_closed_message_and_result_variant()
-> Result<(), Box<dyn std::error::Error>> {
    let corpus = read_corpus()?;
    let mut reached = BTreeSet::new();
    for fixture in &corpus.valid_messages {
        let decoded = decode(fixture.direction, fixture.payload.as_bytes())?;
        record_coverage(&decoded, &mut reached);
    }

    let expected = BTreeSet::from([
        "control-acquire",
        "control-input",
        "control-resize",
        "control-acquired",
        "control-input-accepted",
        "control-resized",
        "control-pasted",
        "control-outcome-unknown",
        "control-rejected-viewer-unavailable",
        "control-rejected-controller-busy",
        "control-rejected-not-controller",
        "control-rejected-sequence-mismatch",
        "control-rejected-host-rejected",
        "control-rejected-host-input-invalid",
        "control-rejected-host-input-limit-exceeded",
        "control-rejected-host-input-backpressure",
        "control-rejected-host-input-unavailable",
        "control-unavailable-server-busy",
        "control-unavailable-terminal-runtime-unavailable",
        "viewer-attach",
        "viewer-attach-binding",
        "viewer-poll",
        "viewer-frame-applied",
        "viewer-need-snapshot",
        "viewer-read-rows",
        "viewer-detach",
        "viewer-attached",
        "viewer-empty",
        "viewer-feedback-accepted",
        "viewer-detached",
        "viewer-rejected-already-attached",
        "viewer-rejected-terminal-unavailable",
        "viewer-rejected-feedback-mismatch",
        "viewer-rows-rejected-stale-revision",
        "viewer-rows-rejected-stale-generation",
        "viewer-rows-rejected-history-evicted",
        "viewer-rows-rejected-missing-resource",
        "viewer-rows-rejected-invalid-field",
        "viewer-rows-rejected-invalid-acknowledgement",
        "viewer-rows-rejected-limit-exceeded",
        "viewer-rows-rejected-resource-exhausted",
        "viewer-rows-rejected-invalid-phase",
        "viewer-unavailable-server-busy",
        "viewer-unavailable-terminal-runtime",
        "viewer-unavailable-workbench",
        "create-accepted-awaiting-start",
        "create-accepted-started",
        "create-failed-terminal-create-failed",
        "create-outcome-unknown",
        "create-rejected-cwd-not-accepted",
        "create-rejected-not-authorized",
        "create-rejected-worker-constraint-unsatisfied",
        "create-request-cwd-null",
        "create-request-cwd-value",
        "create-request-deferred",
        "create-request-immediate",
        "create-request-worker-exact",
        "create-request-worker-null",
        "create-unavailable-no-compatible-worker",
        "create-unavailable-server-busy",
        "create-unavailable-terminal-runtime-unavailable",
        "message-terminal-open",
        "message-terminal-open-result",
        "open-opened",
        "open-unavailable-no-compatible-worker",
        "open-unavailable-terminal-runtime",
        "open-unavailable-server-busy",
        "open-failed",
        "open-outcome-unknown",
        "message-terminal-create",
        "message-terminal-create-result",
        "message-terminal-start",
        "message-terminal-start-result",
        "start-accepted-already-started",
        "start-accepted-started",
        "start-failed-terminal-start-failed",
        "start-outcome-unknown",
        "start-rejected-geometry-conflict",
        "start-rejected-not-authorized",
        "start-rejected-terminal-create-unavailable",
        "start-rejected-terminal-not-awaiting-start",
        "start-unavailable-server-busy",
        "start-unavailable-terminal-runtime-unavailable",
        "message-terminal-send",
        "send-payload-text-enter",
        "send-payload-text-plain",
        "send-payload-key",
        "message-terminal-send-result",
        "send-target-present",
        "send-target-absent",
        "send-accepted",
        "send-rejected",
        "send-unavailable",
        "send-outcome-unknown",
        "send-cleanup-not-needed",
        "send-cleanup-confirmed",
        "send-cleanup-unconfirmed",
    ]);
    assert_eq!(reached, expected);
    Ok(())
}

#[test]
fn consumes_every_invalid_and_forbidden_corpus_case_as_a_rejection()
-> Result<(), Box<dyn std::error::Error>> {
    let corpus = read_corpus()?;
    let schema = serde_json::from_str::<Value>(SCHEMA)?;
    let schema_validator = jsonschema::draft202012::new(&schema)?;
    for fixture in &corpus.invalid_messages {
        assert!(
            decode(fixture.direction, fixture.payload.as_bytes()).is_err(),
            "invalid fixture {} was accepted",
            fixture.name
        );
    }

    let base = corpus
        .valid_messages
        .iter()
        .find(|fixture| fixture.name == "create-immediate-default")
        .ok_or("missing canonical create fixture")?;
    let base_value = serde_json::from_str::<Value>(&base.payload)?;
    let mut forbidden_names = BTreeSet::new();
    for fixture in &corpus.forbidden_client_fields {
        assert!(
            forbidden_names.insert(fixture.name.as_str()),
            "duplicate forbidden-field fixture {}",
            fixture.name
        );
        let mut value = base_value.clone();
        value
            .as_object_mut()
            .ok_or("canonical create fixture is not an object")?
            .insert(fixture.field.clone(), fixture.value.clone());
        assert!(
            !schema_validator.is_valid(&value),
            "schema accepted forbidden Client field {}",
            fixture.name
        );
        let encoded = serde_json::to_vec(&value)?;
        assert_eq!(
            decode_client_frame(&frame(&encoded)?),
            Err(ProtocolError::InvalidPayload),
            "forbidden Client field {}",
            fixture.name
        );
    }
    assert_eq!(forbidden_names.len(), 20);
    Ok(())
}

#[test]
fn semantically_equivalent_noncanonical_json_decodes_to_the_same_value()
-> Result<(), Box<dyn std::error::Error>> {
    let corpus = read_corpus()?;
    let schema = serde_json::from_str::<Value>(SCHEMA)?;
    let schema_validator = jsonschema::draft202012::new(&schema)?;
    for fixture in &corpus.equivalent_messages {
        let canonical = corpus
            .valid_messages
            .iter()
            .find(|candidate| candidate.name == fixture.canonical)
            .ok_or("equivalent fixture references an unknown canonical message")?;
        assert!(matches!(
            (canonical.direction, fixture.direction),
            (Direction::ClientToServer, Direction::ClientToServer)
                | (Direction::ServerToClient, Direction::ServerToClient)
        ));
        let value = serde_json::from_str::<Value>(&fixture.payload)?;
        assert!(
            schema_validator.is_valid(&value),
            "schema rejected equivalent fixture {}",
            fixture.name
        );
        let decoded = decode(fixture.direction, fixture.payload.as_bytes())?;
        assert_eq!(
            decoded,
            decode(canonical.direction, canonical.payload.as_bytes())?,
            "equivalent fixture {}",
            fixture.name
        );
        assert_eq!(
            encode(fixture.direction, &decoded)?,
            frame(canonical.payload.as_bytes())?,
            "canonicalized fixture {}",
            fixture.name
        );
    }
    Ok(())
}

#[test]
fn numeric_refinement_corpus_keeps_server_result_regressions()
-> Result<(), Box<dyn std::error::Error>> {
    let corpus = read_corpus()?;
    for name in [
        "server-protocol-version-integral-decimal",
        "server-protocol-version-integral-exponent",
        "server-protocol-version-binary64-rounded",
        "server-create-geometry-integral-decimal",
        "server-start-geometry-integral-exponent",
    ] {
        let fixture = corpus
            .equivalent_messages
            .iter()
            .find(|fixture| fixture.name == name)
            .ok_or("missing Server numeric-equivalence fixture")?;
        assert!(matches!(fixture.direction, Direction::ServerToClient));
    }

    for (name, boundary) in [
        ("server-protocol-version-negative-zero", Boundary::Schema),
        ("server-protocol-version-fractional", Boundary::Schema),
        (
            "server-protocol-version-number-overflow",
            Boundary::Encoding,
        ),
        ("server-protocol-version-nonfinite", Boundary::Encoding),
        ("server-create-geometry-negative-zero", Boundary::Schema),
        ("server-create-geometry-fractional", Boundary::Schema),
        ("server-create-geometry-unsafe-integer", Boundary::Schema),
        ("server-start-geometry-number-overflow", Boundary::Encoding),
        ("server-start-geometry-nonfinite", Boundary::Encoding),
    ] {
        let fixture = corpus
            .invalid_messages
            .iter()
            .find(|fixture| fixture.name == name)
            .ok_or("missing Server numeric-rejection fixture")?;
        assert!(matches!(fixture.direction, Direction::ServerToClient));
        assert_eq!(fixture.boundary, boundary);
    }
    Ok(())
}

#[test]
fn every_canonical_message_is_rejected_by_the_opposite_direction_decoder()
-> Result<(), Box<dyn std::error::Error>> {
    let corpus = read_corpus()?;
    for fixture in &corpus.valid_messages {
        let encoded = frame(fixture.payload.as_bytes())?;
        let rejected = match fixture.direction {
            Direction::ClientToServer => decode_server_frame(&encoded).is_err(),
            Direction::ServerToClient => decode_client_frame(&encoded).is_err(),
        };
        assert!(rejected, "wrong-direction fixture {}", fixture.name);
    }
    Ok(())
}

#[test]
fn rejects_every_unknown_tag_enum_and_nested_object_field() -> Result<(), Box<dyn std::error::Error>>
{
    let client_payloads = [
        r#"{"kind":"terminal.create","protocolVersion":1,"requestId":"11111111111111111111111111111111","workerConstraint":{"kind":"any","workerId":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"cwd":null,"startMode":{"kind":"deferred"}}"#,
        r#"{"kind":"terminal.create","protocolVersion":1,"requestId":"11111111111111111111111111111111","workerConstraint":null,"cwd":null,"startMode":{"kind":"later"}}"#,
        r#"{"kind":"terminal.start","protocolVersion":1,"requestId":"22222222222222222222222222222222","terminalCreateRequestId":"11111111111111111111111111111111","geometry":{"columns":80,"rows":24,"cellPixelWidth":9,"cellPixelHeight":18,"pixels":1}}"#,
    ];
    for invalid in client_payloads {
        assert_eq!(
            decode_client_frame(&frame(invalid.as_bytes())?),
            Err(ProtocolError::InvalidPayload)
        );
    }

    let server_payloads = [
        r#"{"kind":"terminal.create.result","protocolVersion":1,"requestId":"11111111111111111111111111111111","outcome":{"kind":"degraded"}}"#,
        r#"{"kind":"terminal.create.result","protocolVersion":1,"requestId":"11111111111111111111111111111111","outcome":{"kind":"accepted","state":{"kind":"starting"}}}"#,
        r#"{"kind":"terminal.create.result","protocolVersion":1,"requestId":"11111111111111111111111111111111","outcome":{"kind":"unavailable","reason":"network"}}"#,
        r#"{"kind":"terminal.create.result","protocolVersion":1,"requestId":"11111111111111111111111111111111","outcome":{"kind":"failed","reason":"worker-failed"}}"#,
        r#"{"kind":"terminal.start.result","protocolVersion":1,"requestId":"11111111111111111111111111111111","outcome":{"kind":"rejected","reason":"missing"}}"#,
        r#"{"kind":"terminal.start.result","protocolVersion":1,"requestId":"11111111111111111111111111111111","outcome":{"kind":"unavailable","reason":"network"}}"#,
        r#"{"kind":"terminal.start.result","protocolVersion":1,"requestId":"11111111111111111111111111111111","outcome":{"kind":"failed","reason":"host-failed"}}"#,
        r#"{"kind":"terminal.start.result","protocolVersion":1,"requestId":"11111111111111111111111111111111","outcome":{"kind":"outcome-unknown","detail":"hidden"}}"#,
    ];
    for invalid in server_payloads {
        assert_eq!(
            decode_server_frame(&frame(invalid.as_bytes())?),
            Err(ProtocolError::InvalidPayload)
        );
    }
    Ok(())
}

#[test]
fn decoded_path_and_geometry_boundaries_match_the_language_neutral_contract()
-> Result<(), Box<dyn std::error::Error>> {
    let corpus = read_corpus()?;
    let mut path_names = BTreeSet::new();
    for fixture in &corpus.working_directory_cases {
        assert!(path_names.insert(fixture.name.as_str()));
        let cwd = format!(
            "{}{}{}",
            fixture.prefix,
            fixture.repeat.repeat(fixture.repeat_count),
            fixture.suffix
        );
        let encoded = create_with_cwd(&cwd)?;
        assert_eq!(
            decode_client_frame(&encoded).is_ok(),
            fixture.valid,
            "working-directory fixture {}",
            fixture.name
        );
        if fixture.name == "worst-case-json-escaping" {
            assert_eq!(cwd.len(), MAXIMUM_WORKING_DIRECTORY_BYTES);
            assert!(encoded.len() - FRAME_HEADER_BYTES > 24 * 1024);
        }
    }

    let mut geometry_names = BTreeSet::new();
    for fixture in &corpus.geometry_cases {
        assert!(geometry_names.insert(fixture.name.as_str()));
        assert_eq!(
            decode_client_frame(&start_with_geometry(&fixture.geometry)?).is_ok(),
            fixture.valid,
            "geometry fixture {}",
            fixture.name
        );
    }
    Ok(())
}

fn read_corpus() -> Result<Corpus, serde_json::Error> {
    serde_json::from_str(CORPUS)
}

fn decode(direction: Direction, payload: &[u8]) -> Result<DecodedMessage, ProtocolError> {
    match direction {
        Direction::ClientToServer => {
            decode_client_frame(&frame(payload).map_err(|_error| ProtocolError::InvalidPayload)?)
                .map(DecodedMessage::Client)
        }
        Direction::ServerToClient => {
            decode_server_frame(&frame(payload).map_err(|_error| ProtocolError::InvalidPayload)?)
                .map(DecodedMessage::Server)
        }
    }
}

fn encode(direction: Direction, message: &DecodedMessage) -> Result<Vec<u8>, ProtocolError> {
    match (direction, message) {
        (Direction::ClientToServer, DecodedMessage::Client(message)) => {
            encode_client_frame(message)
        }
        (Direction::ServerToClient, DecodedMessage::Server(message)) => {
            encode_server_frame(message)
        }
        (Direction::ClientToServer, DecodedMessage::Server(_))
        | (Direction::ServerToClient, DecodedMessage::Client(_)) => {
            Err(ProtocolError::InvalidPayload)
        }
    }
}

fn record_coverage(message: &DecodedMessage, reached: &mut BTreeSet<&'static str>) {
    match message {
        DecodedMessage::Client(ClientMessage::TerminalControl(request)) => {
            use orchbee_client_server_protocol::TerminalControlAction as Action;
            reached.insert(match request.action() {
                Action::Acquire { .. } => "control-acquire",
                Action::Input { .. } => "control-input",
                Action::Resize { .. } => "control-resize",
                Action::Paste { .. } => "control-paste",
            });
        }
        DecodedMessage::Server(ServerMessage::TerminalControlResult(result)) => {
            use orchbee_client_server_protocol::{
                ControlRejectedReason as Rejected, ControlUnavailableReason as Unavailable,
                TerminalControlOutcome as Outcome,
            };
            reached.insert(match result.outcome() {
                Outcome::Acquired { .. } => "control-acquired",
                Outcome::InputAccepted {} => "control-input-accepted",
                Outcome::Resized { .. } => "control-resized",
                Outcome::Pasted { .. } => "control-pasted",
                Outcome::OutcomeUnknown {} => "control-outcome-unknown",
                Outcome::Rejected { reason } => match reason {
                    Rejected::ViewerUnavailable => "control-rejected-viewer-unavailable",
                    Rejected::ControllerBusy => "control-rejected-controller-busy",
                    Rejected::NotController => "control-rejected-not-controller",
                    Rejected::SequenceMismatch => "control-rejected-sequence-mismatch",
                    Rejected::HostRejected => "control-rejected-host-rejected",
                    Rejected::HostInputInvalid => "control-rejected-host-input-invalid",
                    Rejected::HostInputLimitExceeded => {
                        "control-rejected-host-input-limit-exceeded"
                    }
                    Rejected::HostInputBackpressure => "control-rejected-host-input-backpressure",
                    Rejected::HostInputUnavailable => "control-rejected-host-input-unavailable",
                },
                Outcome::Unavailable { reason } => match reason {
                    Unavailable::ServerBusy => "control-unavailable-server-busy",
                    Unavailable::TerminalRuntimeUnavailable => {
                        "control-unavailable-terminal-runtime-unavailable"
                    }
                },
            });
        }
        DecodedMessage::Client(ClientMessage::TerminalViewer(request)) => {
            use orchbee_client_server_protocol::TerminalViewerAction;
            reached.insert(match request.action() {
                TerminalViewerAction::Attach { .. } => "viewer-attach",
                TerminalViewerAction::AttachBinding { .. } => "viewer-attach-binding",
                TerminalViewerAction::Poll { .. } => "viewer-poll",
                TerminalViewerAction::FrameApplied { .. } => "viewer-frame-applied",
                TerminalViewerAction::NeedSnapshot { .. } => "viewer-need-snapshot",
                TerminalViewerAction::ReadRows { .. } => "viewer-read-rows",
                TerminalViewerAction::Detach { .. } => "viewer-detach",
            });
        }
        DecodedMessage::Server(ServerMessage::TerminalViewerResult(result)) => {
            use orchbee_client_server_protocol::{
                TerminalViewerOutcome, ViewerRejectedReason, ViewerRowsRejectedReason,
                ViewerUnavailableReason,
            };
            reached.insert(match result.outcome() {
                TerminalViewerOutcome::Attached { .. } => "viewer-attached",
                TerminalViewerOutcome::Empty {} => "viewer-empty",
                TerminalViewerOutcome::FeedbackAccepted {} => "viewer-feedback-accepted",
                TerminalViewerOutcome::Detached {} => "viewer-detached",
                TerminalViewerOutcome::Rejected { reason } => match reason {
                    ViewerRejectedReason::AlreadyAttached => "viewer-rejected-already-attached",
                    ViewerRejectedReason::TerminalUnavailable => {
                        "viewer-rejected-terminal-unavailable"
                    }
                    ViewerRejectedReason::FeedbackMismatch => "viewer-rejected-feedback-mismatch",
                },
                TerminalViewerOutcome::RowsRejected { reason } => match reason {
                    ViewerRowsRejectedReason::StaleRevision => {
                        "viewer-rows-rejected-stale-revision"
                    }
                    ViewerRowsRejectedReason::StaleGeneration => {
                        "viewer-rows-rejected-stale-generation"
                    }
                    ViewerRowsRejectedReason::HistoryEvicted => {
                        "viewer-rows-rejected-history-evicted"
                    }
                    ViewerRowsRejectedReason::MissingResource => {
                        "viewer-rows-rejected-missing-resource"
                    }
                    ViewerRowsRejectedReason::InvalidField => "viewer-rows-rejected-invalid-field",
                    ViewerRowsRejectedReason::InvalidAcknowledgement => {
                        "viewer-rows-rejected-invalid-acknowledgement"
                    }
                    ViewerRowsRejectedReason::LimitExceeded => {
                        "viewer-rows-rejected-limit-exceeded"
                    }
                    ViewerRowsRejectedReason::ResourceExhausted => {
                        "viewer-rows-rejected-resource-exhausted"
                    }
                    ViewerRowsRejectedReason::InvalidPhase => "viewer-rows-rejected-invalid-phase",
                },
                TerminalViewerOutcome::Unavailable { reason } => match reason {
                    ViewerUnavailableReason::ServerBusy => "viewer-unavailable-server-busy",
                    ViewerUnavailableReason::TerminalRuntimeUnavailable => {
                        "viewer-unavailable-terminal-runtime"
                    }
                    ViewerUnavailableReason::WorkbenchUnavailable => "viewer-unavailable-workbench",
                },
                TerminalViewerOutcome::Record(_) => "viewer-binary-record",
            });
        }
        DecodedMessage::Client(ClientMessage::TerminalOpen(_)) => {
            reached.insert("message-terminal-open");
        }
        DecodedMessage::Server(ServerMessage::TerminalOpenResult(result)) => {
            reached.insert("message-terminal-open-result");
            reached.insert(match result.outcome() {
                orchbee_client_server_protocol::OpenOutcome::Opened { .. } => "open-opened",
                orchbee_client_server_protocol::OpenOutcome::Unavailable {
                    reason: CreateUnavailableReason::NoCompatibleWorker,
                } => "open-unavailable-no-compatible-worker",
                orchbee_client_server_protocol::OpenOutcome::Unavailable {
                    reason: CreateUnavailableReason::TerminalRuntimeUnavailable,
                } => "open-unavailable-terminal-runtime",
                orchbee_client_server_protocol::OpenOutcome::Unavailable {
                    reason: CreateUnavailableReason::ServerBusy,
                } => "open-unavailable-server-busy",
                orchbee_client_server_protocol::OpenOutcome::Failed { .. } => "open-failed",
                orchbee_client_server_protocol::OpenOutcome::OutcomeUnknown {} => {
                    "open-outcome-unknown"
                }
            });
        }
        DecodedMessage::Client(ClientMessage::TerminalCreate(request)) => {
            reached.insert("message-terminal-create");
            match request.worker_constraint() {
                Some(WorkerConstraint::Exact { worker_id: _ }) => {
                    reached.insert("create-request-worker-exact");
                }
                None => {
                    reached.insert("create-request-worker-null");
                }
            }
            reached.insert(if request.cwd().is_some() {
                "create-request-cwd-value"
            } else {
                "create-request-cwd-null"
            });
            reached.insert(match request.start_mode() {
                StartMode::Immediate { geometry: _ } => "create-request-immediate",
                StartMode::Deferred => "create-request-deferred",
            });
        }
        DecodedMessage::Client(ClientMessage::TerminalStart(_request)) => {
            reached.insert("message-terminal-start");
        }
        DecodedMessage::Server(ServerMessage::TerminalCreateResult(result)) => {
            reached.insert("message-terminal-create-result");
            reached.insert(match result.outcome() {
                CreateOutcome::Accepted {
                    state: CreateAcceptedState::AwaitingStart,
                } => "create-accepted-awaiting-start",
                CreateOutcome::Accepted {
                    state: CreateAcceptedState::Started { geometry: _ },
                } => "create-accepted-started",
                CreateOutcome::Rejected {
                    reason: CreateRejectedReason::NotAuthorized,
                } => "create-rejected-not-authorized",
                CreateOutcome::Rejected {
                    reason: CreateRejectedReason::WorkerConstraintUnsatisfied,
                } => "create-rejected-worker-constraint-unsatisfied",
                CreateOutcome::Rejected {
                    reason: CreateRejectedReason::CwdNotAccepted,
                } => "create-rejected-cwd-not-accepted",
                CreateOutcome::Unavailable {
                    reason: CreateUnavailableReason::NoCompatibleWorker,
                } => "create-unavailable-no-compatible-worker",
                CreateOutcome::Unavailable {
                    reason: CreateUnavailableReason::TerminalRuntimeUnavailable,
                } => "create-unavailable-terminal-runtime-unavailable",
                CreateOutcome::Unavailable {
                    reason: CreateUnavailableReason::ServerBusy,
                } => "create-unavailable-server-busy",
                CreateOutcome::Failed {
                    reason: CreateFailureReason::TerminalCreateFailed,
                } => "create-failed-terminal-create-failed",
                CreateOutcome::OutcomeUnknown => "create-outcome-unknown",
            });
        }
        DecodedMessage::Server(ServerMessage::TerminalStartResult(result)) => {
            reached.insert("message-terminal-start-result");
            reached.insert(match result.outcome() {
                StartOutcome::Accepted {
                    geometry: _,
                    transition: StartTransition::Started,
                } => "start-accepted-started",
                StartOutcome::Accepted {
                    geometry: _,
                    transition: StartTransition::AlreadyStarted,
                } => "start-accepted-already-started",
                StartOutcome::Rejected {
                    reason: StartRejectedReason::NotAuthorized,
                } => "start-rejected-not-authorized",
                StartOutcome::Rejected {
                    reason: StartRejectedReason::TerminalCreateUnavailable,
                } => "start-rejected-terminal-create-unavailable",
                StartOutcome::Rejected {
                    reason: StartRejectedReason::TerminalNotAwaitingStart,
                } => "start-rejected-terminal-not-awaiting-start",
                StartOutcome::Rejected {
                    reason: StartRejectedReason::GeometryConflict,
                } => "start-rejected-geometry-conflict",
                StartOutcome::Unavailable {
                    reason: StartUnavailableReason::TerminalRuntimeUnavailable,
                } => "start-unavailable-terminal-runtime-unavailable",
                StartOutcome::Unavailable {
                    reason: StartUnavailableReason::ServerBusy,
                } => "start-unavailable-server-busy",
                StartOutcome::Failed {
                    reason: StartFailureReason::TerminalStartFailed,
                } => "start-failed-terminal-start-failed",
                StartOutcome::OutcomeUnknown => "start-outcome-unknown",
            });
        }
        DecodedMessage::Client(ClientMessage::TerminalSend(request)) => {
            use orchbee_client_server_protocol::TerminalSendPayload;
            reached.insert("message-terminal-send");
            reached.insert(match request.payload() {
                TerminalSendPayload::Text(input) => {
                    if input.enter() {
                        "send-payload-text-enter"
                    } else {
                        "send-payload-text-plain"
                    }
                }
                TerminalSendPayload::Key { .. } => "send-payload-key",
            });
        }
        DecodedMessage::Server(ServerMessage::TerminalSendResult(result)) => {
            use orchbee_client_server_protocol::{
                SendCleanupStatus, TerminalSendOutcome,
            };
            reached.insert("message-terminal-send-result");
            reached.insert(if result.target().is_some() {
                "send-target-present"
            } else {
                "send-target-absent"
            });
            let outcome = result.outcome();
            reached.insert(match outcome {
                TerminalSendOutcome::Accepted { .. } => "send-accepted",
                TerminalSendOutcome::Rejected { .. } => "send-rejected",
                TerminalSendOutcome::Unavailable { .. } => "send-unavailable",
                TerminalSendOutcome::OutcomeUnknown { .. } => "send-outcome-unknown",
            });
            let cleanup = match outcome {
                TerminalSendOutcome::Accepted { cleanup, .. }
                | TerminalSendOutcome::Rejected { cleanup, .. }
                | TerminalSendOutcome::Unavailable { cleanup, .. }
                | TerminalSendOutcome::OutcomeUnknown { cleanup } => *cleanup,
            };
            reached.insert(match cleanup {
                SendCleanupStatus::NotNeeded => "send-cleanup-not-needed",
                SendCleanupStatus::Confirmed => "send-cleanup-confirmed",
                SendCleanupStatus::Unconfirmed => "send-cleanup-unconfirmed",
            });
        }
    }
}

fn frame(payload: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if payload.len() > MAXIMUM_FRAME_PAYLOAD_BYTES {
        return Err("payload exceeds OBCS frame limit".into());
    }
    let payload_length = u16::try_from(payload.len())?;
    let mut encoded = Vec::with_capacity(FRAME_HEADER_BYTES + payload.len());
    encoded.extend_from_slice(&FRAME_MAGIC);
    encoded.extend_from_slice(&FRAME_VERSION.to_be_bytes());
    encoded.extend_from_slice(&payload_length.to_be_bytes());
    encoded.extend_from_slice(payload);
    Ok(encoded)
}

fn payload(frame: &[u8]) -> Result<&[u8], Box<dyn std::error::Error>> {
    frame
        .get(FRAME_HEADER_BYTES..)
        .ok_or_else(|| "frame has no payload boundary".into())
}

fn create_with_cwd(cwd: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let value = serde_json::json!({
        "kind": "terminal.create",
        "protocolVersion": 1,
        "requestId": "11111111111111111111111111111111",
        "workerConstraint": null,
        "cwd": cwd,
        "startMode": { "kind": "deferred" }
    });
    frame(&serde_json::to_vec(&value)?)
}

fn start_with_geometry(geometry: &Value) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let message = serde_json::json!({
        "kind": "terminal.start",
        "protocolVersion": 1,
        "requestId": "22222222222222222222222222222222",
        "terminalCreateRequestId": "11111111111111111111111111111111",
        "geometry": geometry.clone()
    });
    frame(&serde_json::to_vec(&message)?)
}
