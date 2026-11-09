use std::{error::Error, io};

use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ConnectionControl, Envelope, Hello, HelloAccepted,
    MAXIMUM_CONTROL_JSON_BYTES, MAXIMUM_OPTIONAL_FEATURES, MAXIMUM_TERMINAL_FRAME_BYTES,
    MAXIMUM_TYPED_FAILURE_CODE_BYTES, MAXIMUM_TYPED_FAILURE_MESSAGE_BYTES, MessageRole,
    PayloadEncoding, PromptUnconfirmedReason, ProtocolError, ProtocolErrorCode, ProtocolRejected,
    ProtocolRejectionReason, ProtocolVersion, TypedFailure, decode_connection_control,
    decode_transport_envelope, encode_connection_control, encode_transport_envelope,
};
use serde_json::Value;

const CONTROL_FIXTURE: &str =
    include_str!("../../../schemas/terminal/v1/fixtures/control-conformance.json");

#[test]
fn every_canonical_connection_payload_decodes_and_reencodes_byte_for_byte()
-> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let cases = [
        (
            1,
            MessageRole::Request,
            fixture_string(&fixture, "/commands/hello")?,
        ),
        (
            2,
            MessageRole::Response,
            fixture_string(&fixture, "/outputs/hello_accepted")?,
        ),
        (
            3,
            MessageRole::Response,
            fixture_string(&fixture, "/outputs/protocol_rejected")?,
        ),
        (
            4,
            MessageRole::Request,
            fixture_string(&fixture, "/commands/empty")?,
        ),
        (
            4,
            MessageRole::Response,
            fixture_string(&fixture, "/outputs/empty")?,
        ),
    ];

    for (kind, role, payload) in cases {
        assert_canonical_round_trip(kind, role, payload)?;
    }
    for payload in fixture
        .pointer("/outputs/typed_failure")
        .and_then(Value::as_array)
        .ok_or_else(|| fixture_error("typed failure fixture is missing"))?
    {
        assert_canonical_round_trip(
            5,
            MessageRole::Response,
            payload
                .as_str()
                .ok_or_else(|| fixture_error("typed failure fixture is not text"))?,
        )?;
    }
    Ok(())
}

#[test]
fn canonical_values_retain_typed_connection_facts() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let hello = decode_connection_control(
        1,
        MessageRole::Request,
        fixture_string(&fixture, "/commands/hello")?.as_bytes(),
    )?;
    let ConnectionControl::Hello(hello) = hello else {
        return Err(fixture_error("Hello decoded to the wrong variant").into());
    };
    assert_eq!(hello.client_id.to_text(), "55".repeat(16));
    assert_eq!(hello.frame_bytes, 8 * 1024 * 1024);
    assert_eq!(hello.input_bytes, 1024 * 1024);
    assert_eq!(hello.protocol, CURRENT_PROTOCOL_VERSION);
    assert_eq!(hello.row_window_bytes, 44 * 1024 * 1024);
    assert_eq!(hello.supported_optional_features, ["future-x"]);

    let accepted = decode_connection_control(
        2,
        MessageRole::Response,
        fixture_string(&fixture, "/outputs/hello_accepted")?.as_bytes(),
    )?;
    let ConnectionControl::HelloAccepted(accepted) = accepted else {
        return Err(fixture_error("HelloAccepted decoded to the wrong variant").into());
    };
    assert_eq!(accepted.host_epoch.to_text(), "11".repeat(16));
    assert_eq!(accepted.host_version, "0.1.0");
    assert_eq!(accepted.protocol, CURRENT_PROTOCOL_VERSION);

    let rejected = decode_connection_control(
        3,
        MessageRole::Response,
        fixture_string(&fixture, "/outputs/protocol_rejected")?.as_bytes(),
    )?;
    assert_eq!(
        rejected,
        ConnectionControl::ProtocolRejected(ProtocolRejected {
            client: ProtocolVersion {
                major: 2,
                minor: 0,
                patch: 0,
            },
            host: CURRENT_PROTOCOL_VERSION,
            reason: ProtocolRejectionReason::VersionMismatch,
        })
    );
    Ok(())
}

#[test]
fn json_failures_keep_utf8_syntax_duplicate_and_field_classes_distinct()
-> Result<(), Box<dyn Error>> {
    assert_code(
        decode_connection_control(4, MessageRole::Request, &[0xff]),
        ProtocolErrorCode::InvalidUtf8,
    );
    assert_code(
        decode_connection_control(4, MessageRole::Request, b"{"),
        ProtocolErrorCode::InvalidJson,
    );
    assert_code(
        decode_connection_control(4, MessageRole::Request, b"\xef\xbb\xbf{}"),
        ProtocolErrorCode::InvalidJson,
    );
    assert_code(
        decode_connection_control(4, MessageRole::Request, br#"{"extra":true}"#),
        ProtocolErrorCode::InvalidField,
    );

    let fixture = read_fixture()?;
    let hello = fixture_string(&fixture, "/commands/hello")?;
    let duplicate = hello.replacen(
        "\"frameBytes\":8388608",
        "\"frameBytes\":8388608,\"frameBytes\":1",
        1,
    );
    assert_code(
        decode_connection_control(1, MessageRole::Request, duplicate.as_bytes()),
        ProtocolErrorCode::InvalidJson,
    );
    let escaped_duplicate = hello.replacen(
        "\"frameBytes\":8388608",
        "\"frameBytes\":8388608,\"frame\\u0042ytes\":1",
        1,
    );
    assert_code(
        decode_connection_control(1, MessageRole::Request, escaped_duplicate.as_bytes()),
        ProtocolErrorCode::InvalidJson,
    );
    let surrogate = hello.replacen("\"future-x\"", "\"\\ud800\"", 1);
    assert_code(
        decode_connection_control(1, MessageRole::Request, surrogate.as_bytes()),
        ProtocolErrorCode::InvalidJson,
    );

    let oversized = vec![b' '; MAXIMUM_CONTROL_JSON_BYTES + 1];
    assert_code(
        decode_connection_control(4, MessageRole::Request, &oversized),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn numeric_lexemes_follow_binary64_safe_integer_refinement() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let hello = fixture_string(&fixture, "/commands/hello")?;

    let rounded = hello_with_frame_bytes(hello, "1.0000000000000001")?;
    let decoded = decode_connection_control(1, MessageRole::Request, rounded.as_bytes())?;
    let ConnectionControl::Hello(decoded) = decoded else {
        return Err(fixture_error("rounded Hello decoded to the wrong variant").into());
    };
    assert_eq!(decoded.frame_bytes, 1);

    for invalid in [
        "0".to_owned(),
        "1e4000".to_owned(),
        "9007199254740992".to_owned(),
        (MAXIMUM_TERMINAL_FRAME_BYTES + 1).to_string(),
    ] {
        let payload = hello_with_frame_bytes(hello, &invalid)?;
        assert_code(
            decode_connection_control(1, MessageRole::Request, payload.as_bytes()),
            ProtocolErrorCode::InvalidField,
        );
    }

    let maximum = hello_with_frame_bytes(hello, &MAXIMUM_TERMINAL_FRAME_BYTES.to_string())?;
    let decoded = decode_connection_control(1, MessageRole::Request, maximum.as_bytes())?;
    let ConnectionControl::Hello(decoded) = decoded else {
        return Err(fixture_error("maximum Hello decoded to the wrong variant").into());
    };
    assert_eq!(
        decoded.frame_bytes,
        u32::try_from(MAXIMUM_TERMINAL_FRAME_BYTES)?
    );
    Ok(())
}

#[test]
fn identity_version_and_feature_boundaries_are_strict() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let hello_payload = fixture_string(&fixture, "/commands/hello")?;
    for identity in ["00".repeat(16), "AA".repeat(16), "1".repeat(31)] {
        let payload = hello_payload.replacen(&"55".repeat(16), &identity, 1);
        assert_code(
            decode_connection_control(1, MessageRole::Request, payload.as_bytes()),
            ProtocolErrorCode::InvalidIdentifier,
        );
    }

    let current = CURRENT_PROTOCOL_VERSION;
    let current_protocol = format!(
        "\"protocol\":{{\"major\":{},\"minor\":{},\"patch\":{}}}",
        current.major, current.minor, current.patch
    );
    assert!(hello_payload.contains(&current_protocol));
    let version_boundary = hello_payload.replacen(
        &current_protocol,
        "\"protocol\":{\"major\":65535,\"minor\":65535,\"patch\":65535}",
        1,
    );
    let decoded = decode_connection_control(1, MessageRole::Request, version_boundary.as_bytes())?;
    let ConnectionControl::Hello(mut hello) = decoded else {
        return Err(fixture_error("version-boundary Hello decoded to the wrong variant").into());
    };
    assert_eq!(
        hello.protocol,
        ProtocolVersion {
            major: u16::MAX,
            minor: u16::MAX,
            patch: u16::MAX,
        }
    );
    let version_overflow = version_boundary.replacen("65535", "65536", 1);
    assert_code(
        decode_connection_control(1, MessageRole::Request, version_overflow.as_bytes()),
        ProtocolErrorCode::InvalidField,
    );

    hello.supported_optional_features = vec!["x".to_owned(); MAXIMUM_OPTIONAL_FEATURES];
    assert!(encode_connection_control(&ConnectionControl::Hello(hello.clone())).is_ok());
    hello.supported_optional_features.push("x".to_owned());
    assert_code(
        encode_connection_control(&ConnectionControl::Hello(hello.clone())),
        ProtocolErrorCode::InvalidField,
    );
    hello.supported_optional_features = vec!["😀".repeat(16)];
    assert!(encode_connection_control(&ConnectionControl::Hello(hello.clone())).is_ok());
    hello.supported_optional_features = vec!["😀".repeat(17)];
    assert_code(
        encode_connection_control(&ConnectionControl::Hello(hello)),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn host_semantic_version_and_failure_text_boundaries_are_strict() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let accepted_payload = fixture_string(&fixture, "/outputs/hello_accepted")?;
    let decoded = decode_connection_control(2, MessageRole::Response, accepted_payload.as_bytes())?;
    let ConnectionControl::HelloAccepted(mut accepted) = decoded else {
        return Err(fixture_error("HelloAccepted decoded to the wrong variant").into());
    };
    for valid in ["0.0.0", "1.2.3-alpha", "1.2.3-alpha.1+build-5.7"] {
        accepted.host_version = valid.to_owned();
        assert!(
            encode_connection_control(&ConnectionControl::HelloAccepted(accepted.clone())).is_ok()
        );
    }
    for invalid in ["01.2.3", "1.2", "1.2.3-", "1.2.3+💥"] {
        accepted.host_version = invalid.to_owned();
        assert_code(
            encode_connection_control(&ConnectionControl::HelloAccepted(accepted.clone())),
            ProtocolErrorCode::InvalidField,
        );
    }

    let mut failure = TypedFailure {
        code: "x".repeat(MAXIMUM_TYPED_FAILURE_CODE_BYTES),
        message: "😀".repeat(MAXIMUM_TYPED_FAILURE_MESSAGE_BYTES / 4),
        retryable: false,
        reason: None,
    };
    assert!(encode_connection_control(&ConnectionControl::TypedFailure(failure.clone())).is_ok());
    failure.code.push('x');
    assert_code(
        encode_connection_control(&ConnectionControl::TypedFailure(failure.clone())),
        ProtocolErrorCode::InvalidField,
    );
    failure.code = "x".to_owned();
    failure.message.push('x');
    assert_code(
        encode_connection_control(&ConnectionControl::TypedFailure(failure)),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn kind_and_role_select_exactly_one_connection_schema() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let hello = fixture_string(&fixture, "/commands/hello")?;
    for (kind, role) in [
        (1, MessageRole::Response),
        (2, MessageRole::Request),
        (5, MessageRole::Event),
        (10, MessageRole::Request),
    ] {
        assert_code(
            decode_connection_control(kind, role, hello.as_bytes()),
            ProtocolErrorCode::InvalidField,
        );
    }
    Ok(())
}

#[test]
fn connection_control_composes_with_the_phase_5a_envelope() -> Result<(), Box<dyn Error>> {
    let value = ConnectionControl::Hello(Hello {
        client_id: orchbee_terminal_protocol::ClientId::parse(&"55".repeat(16))?,
        frame_bytes: 8 * 1024 * 1024,
        input_bytes: 1024 * 1024,
        protocol: CURRENT_PROTOCOL_VERSION,
        row_window_bytes: 44 * 1024 * 1024,
        supported_optional_features: vec!["future-x".to_owned()],
    });
    let control = encode_connection_control(&value)?;
    let record = encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: None,
        kind: control.kind,
        optional_message: false,
        payload: &control.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: control.role,
    })?;
    let envelope = decode_transport_envelope(&record)?;
    let decoded = decode_connection_control(envelope.kind, envelope.role, envelope.payload)?;
    assert_eq!(decoded, value);
    Ok(())
}

#[test]
fn public_values_are_revalidated_before_encoding() -> Result<(), Box<dyn Error>> {
    let invalid_hello = Hello {
        client_id: orchbee_terminal_protocol::ClientId::parse(&"55".repeat(16))?,
        frame_bytes: 0,
        input_bytes: 1,
        protocol: CURRENT_PROTOCOL_VERSION,
        row_window_bytes: 1,
        supported_optional_features: Vec::new(),
    };
    assert_code(
        encode_connection_control(&ConnectionControl::Hello(invalid_hello)),
        ProtocolErrorCode::InvalidField,
    );

    let invalid_accepted = HelloAccepted {
        frame_bytes: 1,
        host_epoch: orchbee_terminal_protocol::HostEpoch::parse(&"11".repeat(16))?,
        host_version: "not-semver".to_owned(),
        input_bytes: 1,
        protocol: CURRENT_PROTOCOL_VERSION,
        row_window_bytes: 1,
        supported_optional_features: Vec::new(),
    };
    assert_code(
        encode_connection_control(&ConnectionControl::HelloAccepted(invalid_accepted)),
        ProtocolErrorCode::InvalidField,
    );

    let invalid_failure = TypedFailure {
        code: String::new(),
        message: "message".to_owned(),
        retryable: true,
        reason: None,
    };
    assert_code(
        encode_connection_control(&ConnectionControl::TypedFailure(invalid_failure)),
        ProtocolErrorCode::InvalidField,
    );

    // A reason belongs to a prompt-unconfirmed refusal and to nothing else.
    let mut prompt = TypedFailure {
        code: "prompt-unconfirmed".to_owned(),
        message: "nothing was typed".to_owned(),
        retryable: false,
        reason: None,
    };
    assert_code(
        encode_connection_control(&ConnectionControl::TypedFailure(prompt.clone())),
        ProtocolErrorCode::InvalidField,
    );
    prompt.code = "invalid-state".to_owned();
    prompt.reason = Some(PromptUnconfirmedReason::Busy);
    assert_code(
        encode_connection_control(&ConnectionControl::TypedFailure(prompt)),
        ProtocolErrorCode::InvalidField,
    );
    assert!(
        decode_connection_control(
            5,
            MessageRole::Response,
            br#"{"code":"prompt-unconfirmed","message":"m","retryable":false,"reason":null}"#,
        )
        .is_err()
    );
    Ok(())
}

fn assert_canonical_round_trip(
    kind: u16,
    role: MessageRole,
    payload: &str,
) -> Result<(), Box<dyn Error>> {
    let decoded = decode_connection_control(kind, role, payload.as_bytes())?;
    let encoded = encode_connection_control(&decoded)?;
    assert_eq!(encoded.kind, kind);
    assert_eq!(encoded.role, role);
    assert_eq!(encoded.payload, payload.as_bytes());
    Ok(())
}

fn hello_with_frame_bytes(payload: &str, value: &str) -> Result<String, Box<dyn Error>> {
    let replaced = payload.replacen(
        "\"frameBytes\":8388608",
        &format!("\"frameBytes\":{value}"),
        1,
    );
    if replaced == payload {
        return Err(fixture_error("canonical frameBytes token is missing").into());
    }
    Ok(replaced)
}

fn read_fixture() -> Result<Value, serde_json::Error> {
    serde_json::from_str(CONTROL_FIXTURE)
}

fn fixture_string<'value>(value: &'value Value, pointer: &str) -> Result<&'value str, io::Error> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| fixture_error(&format!("fixture string {pointer} is missing")))
}

fn fixture_error(message: &str) -> io::Error {
    io::Error::other(message.to_owned())
}

fn assert_code<T>(result: Result<T, ProtocolError>, expected: ProtocolErrorCode) {
    match result {
        Ok(_value) => panic!("expected protocol error {expected}"),
        Err(error) => assert_eq!(error.code(), expected),
    }
}
