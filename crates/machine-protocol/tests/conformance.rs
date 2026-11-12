//! The payload and frame fixtures against the frame decoders and each connection's records.

use std::error::Error;

use orchbee_machine_protocol::{
    Frame, FrameDecoder, FrameError, FrameKind, Protocol, Record, RecordError, Role, decode_frame,
    hub::Hub, link::Link,
};
use serde::Deserialize;

use crate::support::{self, TestResult};

const VALID: &str = include_str!("../../../schemas/machine/v1/fixtures/valid.json");
const BOUNDARY: &str = include_str!("../../../schemas/machine/v1/fixtures/boundary.json");
const INVALID: &str = include_str!("../../../schemas/machine/v1/fixtures/invalid.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cases {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Case {
    name: String,
    connection: String,
    direction: String,
    kind: Option<String>,
    payload: Option<String>,
    frame_hex: Option<String>,
    layer: Option<String>,
}

impl Case {
    fn sender(&self) -> Result<Role, Box<dyn Error>> {
        Role::from_name(&self.direction)
            .ok_or_else(|| format!("{}: unknown direction {}", self.name, self.direction).into())
    }

    /// The fixture's bytes: `frameHex` as written, or `payload` framed on channel 0 for a
    /// `message` and channel 1 for every channel kind.
    fn bytes(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        match (&self.frame_hex, &self.kind, &self.payload) {
            (Some(hex), None, None) => support::from_hex(hex),
            (None, Some(kind), Some(payload)) => {
                let kind = FrameKind::from_name(kind)
                    .ok_or_else(|| format!("{}: unknown frame kind {kind}", self.name))?;
                let channel = u32::from(!kind.on_channel_zero());
                Ok(Frame::new(kind, channel, payload.as_bytes().to_vec())
                    .map_err(|error| format!("{}: {error}", self.name))?
                    .encode())
            }
            _ => Err(format!("{}: a case holds frameHex or kind and payload", self.name).into()),
        }
    }
}

fn round_trip<P: Protocol>(case: &Case) -> TestResult {
    let name = &case.name;
    let bytes = case.bytes()?;
    let frame = decode_frame(&bytes).map_err(|error| format!("{name} must frame: {error}"))?;
    assert_eq!(frame.encode(), bytes, "{name} must re-frame to its bytes");
    let mut decoder = FrameDecoder::new();
    let mut streamed = None;
    for byte in &bytes {
        let (consumed, frame) = decoder.decode(std::slice::from_ref(byte))?;
        assert_eq!(consumed, 1, "{name} must be read one byte at a time");
        streamed = streamed.or(frame);
    }
    assert_eq!(streamed.as_ref(), Some(&frame), "{name} must stream");
    let sender = case.sender()?;
    let record = Record::<P>::decode(sender, &frame)
        .map_err(|error| format!("{name} must decode: {error}"))?;
    let encoded = record
        .encode(frame.channel())
        .map_err(|error| format!("{name} must re-encode: {error}"))?;
    assert_eq!(
        Record::<P>::decode(sender, &encoded),
        Ok(record),
        "{name} must re-encode to an equal value"
    );
    Ok(())
}

#[test]
fn valid_and_boundary_cases_decode_and_re_encode_to_an_equal_value() -> TestResult {
    for source in [VALID, BOUNDARY] {
        let fixtures: Cases = serde_json::from_str(source)?;
        assert!(!fixtures.cases.is_empty());
        for case in &fixtures.cases {
            assert_eq!(case.layer, None, "{} carries no layer", case.name);
            match case.connection.as_str() {
                "machine-hub" => round_trip::<Hub>(case)?,
                "machine-link" => round_trip::<Link>(case)?,
                other => return Err(format!("{}: unknown connection {other}", case.name).into()),
            }
        }
    }
    Ok(())
}

fn refuse<P: Protocol>(case: &Case, layer: &str) -> TestResult {
    let name = &case.name;
    let bytes = case.bytes()?;
    let framed = decode_frame(&bytes);
    if layer == "frame" {
        let Err(error) = framed else {
            return Err(format!("{name} must be refused as a frame").into());
        };
        let mut decoder = FrameDecoder::new();
        let streamed = decoder.decode(&bytes);
        match error {
            FrameError::Truncated => assert!(
                matches!(streamed, Ok((consumed, None)) if consumed == bytes.len()),
                "{name} must leave a stream reader waiting"
            ),
            FrameError::TrailingBytes => assert!(
                matches!(streamed, Ok((consumed, Some(_))) if consumed < bytes.len()),
                "{name} must leave the trailing bytes unread"
            ),
            header => assert_eq!(
                streamed.map(|_| ()),
                Err(header),
                "{name} must be refused by a stream reader at its header"
            ),
        }
        return Ok(());
    }
    let frame = framed.map_err(|error| format!("{name} must frame: {error}"))?;
    let expected = match layer {
        "encoding" => RecordError::Encoding,
        "record" => RecordError::Record,
        other => return Err(format!("{name}: unknown layer {other}").into()),
    };
    assert_eq!(
        Record::<P>::decode(case.sender()?, &frame).err(),
        Some(expected),
        "{name} must be refused by the {layer} layer"
    );
    Ok(())
}

#[test]
fn every_invalid_case_is_refused_by_its_first_layer() -> TestResult {
    let fixtures: Cases = serde_json::from_str(INVALID)?;
    assert!(!fixtures.cases.is_empty());
    for case in &fixtures.cases {
        let layer = case
            .layer
            .as_deref()
            .ok_or_else(|| format!("{} names its layer", case.name))?;
        match case.connection.as_str() {
            "machine-hub" => refuse::<Hub>(case, layer)?,
            "machine-link" => refuse::<Link>(case, layer)?,
            other => return Err(format!("{}: unknown connection {other}", case.name).into()),
        }
    }
    Ok(())
}
