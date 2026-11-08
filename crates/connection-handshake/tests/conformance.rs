//! Every payload and frame fixture against the schema, the decoders, and the stream reader, plus
//! the bounds too large for a fixture file.

use std::{
    collections::VecDeque,
    error::Error,
    io::{self, Cursor, Read},
    num::NonZeroU32,
};

use orchbee_connection_handshake::{
    AcceptorMessage, Build, CapabilityName, CapabilitySet, ConnectionEntry, ConnectionKind,
    ContractName, DecodeError, EncodeError, EnvironmentName, ExitResult, Extensions,
    FRAME_HEADER_BYTES, FRAME_MAGIC, InitiatorMessage, MAXIMUM_PAYLOAD_BYTES,
    MAXIMUM_QUERY_PROCESSES, Offer, ProcessEntry, ProcessRole, ProcessState, ProductVersion,
    QueryResult, ReadError, decode_acceptor_frame, decode_initiator_frame, encode_acceptor_frame,
    encode_initiator_frame, read_acceptor_message, read_initiator_message,
};
use serde::Deserialize;
use serde_json::Value;

use crate::support::{self, Direction, TestResult};

const VALID: &str = include_str!("../../../schemas/connection/v1/fixtures/valid.json");
const BOUNDARY: &str = include_str!("../../../schemas/connection/v1/fixtures/boundary.json");
const INVALID: &str = include_str!("../../../schemas/connection/v1/fixtures/invalid.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cases<Case> {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptedCase {
    name: String,
    direction: Direction,
    payload: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RefusedCase {
    name: String,
    direction: Direction,
    layer: Layer,
    payload: Option<String>,
    frame_hex: Option<String>,
    reader: Option<ReaderOutcome>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Layer {
    Frame,
    Encoding,
    Schema,
    Decoder,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum ReaderOutcome {
    Closed,
    Foreign,
    Malformed,
    Message,
}

#[test]
fn valid_and_boundary_payloads_satisfy_the_schema_and_round_trip_in_their_direction() -> TestResult
{
    let schema = support::schema()?;
    for source in [VALID, BOUNDARY] {
        let fixtures: Cases<AcceptedCase> = serde_json::from_str(source)?;
        assert!(!fixtures.cases.is_empty());
        for case in fixtures.cases {
            let name = &case.name;
            let value: Value = serde_json::from_str(&case.payload)?;
            assert!(schema.is_valid(&value), "{name} must satisfy the schema");
            let frame = support::frame(case.payload.as_bytes())?;
            let decoded = support::decode(case.direction, &frame)
                .map_err(|error| format!("{name} must decode: {error}"))?;
            assert!(
                support::decode(case.direction.opposite(), &frame).is_err(),
                "{name} must be refused from the other direction"
            );
            let encoded = support::encode(&decoded)?;
            let reencoded: Value = serde_json::from_slice(support::payload(&encoded)?)?;
            assert!(
                schema.is_valid(&reencoded),
                "{name} must re-encode inside the schema"
            );
            assert_eq!(
                support::decode(case.direction, &encoded)?,
                decoded,
                "{name} must round-trip"
            );
        }
    }
    Ok(())
}

#[test]
fn every_invalid_case_is_refused_by_its_first_layer() -> TestResult {
    let schema = support::schema()?;
    let fixtures: Cases<RefusedCase> = serde_json::from_str(INVALID)?;
    assert!(!fixtures.cases.is_empty());
    for case in fixtures.cases {
        let name = &case.name;
        match (case.layer, case.payload, case.frame_hex, case.reader) {
            (Layer::Frame, None, Some(frame_hex), Some(expected)) => {
                let frame = support::from_hex(&frame_hex)?;
                assert!(
                    support::decode(case.direction, &frame).is_err(),
                    "{name} must be refused as a complete frame"
                );
                let mut stream = Cursor::new(frame.as_slice());
                let outcome = match support::read(case.direction, &mut stream) {
                    Ok(_) => {
                        let read = usize::try_from(stream.position())?;
                        assert!(
                            read < frame.len(),
                            "{name} must leave bytes after the frame"
                        );
                        ReaderOutcome::Message
                    }
                    Err(ReadError::Closed) => ReaderOutcome::Closed,
                    Err(ReadError::ForeignProtocol) => ReaderOutcome::Foreign,
                    Err(ReadError::Malformed(_)) => ReaderOutcome::Malformed,
                    Err(ReadError::Io(error)) => return Err(error.into()),
                };
                assert_eq!(outcome, expected, "{name} must end the stream reader so");
            }
            (layer, Some(payload), None, None) if layer != Layer::Frame => {
                let expected = if layer == Layer::Encoding {
                    DecodeError::Encoding
                } else {
                    DecodeError::Message
                };
                assert_eq!(
                    support::decode(case.direction, &support::frame(payload.as_bytes())?).err(),
                    Some(expected),
                    "{name} must be refused by the {layer:?} layer"
                );
                if layer != Layer::Encoding {
                    let value: Value = serde_json::from_str(&payload)?;
                    assert_eq!(
                        schema.is_valid(&value),
                        layer == Layer::Decoder,
                        "{name}: only schema-layer cases fail the schema"
                    );
                }
            }
            _ => {
                return Err(format!(
                    "{name}: a frame case holds frameHex and reader, a payload case holds payload"
                )
                .into());
            }
        }
    }
    Ok(())
}

#[test]
fn payload_bound_is_exact() -> TestResult {
    let padded = |padding: usize| -> Result<InitiatorMessage, Box<dyn Error>> {
        let mut offer = Offer::local(
            ConnectionKind::ClientServer,
            ProductVersion::new("1.6.0")?,
            Build::default(),
            EnvironmentName::release(),
        );
        offer.extensions.insert(
            CapabilityName::new("padding")?,
            Value::String("x".repeat(padding)),
        )?;
        Ok(InitiatorMessage::Offer(offer))
    };
    let unpadded = encode_initiator_frame(&padded(0)?)?.len() - FRAME_HEADER_BYTES;
    let largest = padded(MAXIMUM_PAYLOAD_BYTES - unpadded)?;
    let frame = encode_initiator_frame(&largest)?;
    assert_eq!(frame.len(), FRAME_HEADER_BYTES + MAXIMUM_PAYLOAD_BYTES);
    assert_eq!(decode_initiator_frame(&frame)?, largest);
    assert_eq!(read_initiator_message(&mut frame.as_slice())?, largest);
    assert_eq!(
        encode_initiator_frame(&padded(MAXIMUM_PAYLOAD_BYTES - unpadded + 1)?),
        Err(EncodeError::PayloadTooLarge)
    );
    Ok(())
}

#[test]
fn query_result_lists_at_most_258_processes() -> TestResult {
    let schema = support::schema()?;
    let process = |role, capabilities: Option<CapabilitySet>| -> Result<_, Box<dyn Error>> {
        Ok(ProcessEntry {
            role,
            product_version: ProductVersion::new("1.6.0")?,
            build: Build::default(),
            environment: EnvironmentName::release(),
            pid: NonZeroU32::MIN,
            state: ProcessState::Ready,
            terminals: 0,
            connections: vec![ConnectionEntry {
                kind: ContractName::new("worker-host")?,
                generations: ConnectionKind::WorkerHost.supported_generations(),
                capabilities,
            }],
        })
    };
    let host = process(ProcessRole::TerminalHost, None)?;
    let mut processes = vec![
        process(ProcessRole::Server, Some(CapabilitySet::default()))?,
        process(ProcessRole::Worker, None)?,
    ];
    processes.resize(MAXIMUM_QUERY_PROCESSES, host.clone());
    let largest = AcceptorMessage::QueryResult(QueryResult::new(
        processes.clone(),
        Some(true),
        Extensions::new(),
    )?);
    let frame = encode_acceptor_frame(&largest)?;
    assert_eq!(decode_acceptor_frame(&frame)?, largest);
    let mut value: Value = serde_json::from_slice(support::payload(&frame)?)?;
    assert!(schema.is_valid(&value));

    processes.push(host);
    assert!(QueryResult::new(processes, Some(true), Extensions::new()).is_err());
    let entries = value
        .get_mut("processes")
        .and_then(Value::as_array_mut)
        .ok_or("a query result lists processes")?;
    let peer = entries
        .last()
        .cloned()
        .ok_or("a query result lists a peer")?;
    entries.push(peer);
    assert!(!schema.is_valid(&value));
    assert_eq!(
        decode_acceptor_frame(&support::frame(&serde_json::to_vec(&value)?)?).err(),
        Some(DecodeError::Message)
    );
    Ok(())
}

/// Yields one byte per read and fails once its bytes run out, so a reader that waits for bytes
/// it does not need surfaces as an I/O error.
struct Trickle(VecDeque<u8>);

impl Read for Trickle {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match (buffer.first_mut(), self.0.pop_front()) {
            (Some(slot), Some(byte)) => {
                *slot = byte;
                Ok(1)
            }
            _ => Err(io::Error::other("read past the bytes the frame needs")),
        }
    }
}

#[test]
fn reader_decides_without_waiting_for_bytes_it_does_not_need() -> TestResult {
    for prefix in [&b"X"[..], b"OX", b"OBX", b"OBHX"] {
        assert!(matches!(
            read_acceptor_message(&mut Trickle(prefix.iter().copied().collect())),
            Err(ReadError::ForeignProtocol)
        ));
    }
    let oversized = u32::try_from(MAXIMUM_PAYLOAD_BYTES + 1)?;
    for length in [0, oversized, u32::MAX] {
        let header = FRAME_MAGIC.iter().copied().chain(length.to_be_bytes());
        assert!(matches!(
            read_acceptor_message(&mut Trickle(header.collect())),
            Err(ReadError::Malformed(DecodeError::Length))
        ));
    }
    let exit = AcceptorMessage::ExitResult(ExitResult::default());
    let frame = encode_acceptor_frame(&exit)?;
    assert_eq!(
        read_acceptor_message(&mut Trickle(frame.into_iter().collect()))?,
        exit
    );
    Ok(())
}
