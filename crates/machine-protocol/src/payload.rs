//! The encoding layer of JSON payloads: one strict object under the `OBHS` payload rules, parsed
//! by `crates/connection-handshake`, with every value outside the top-level `extensions` member
//! free of `null` and written as plain integer tokens, since no record of either connection has a
//! nullable or fractional member. Inside `extensions` the `OBHS` rule admits any JSON. Which
//! members a record holds is the record layer's check.

use orchbee_connection_handshake::parse_strict_object;
use serde_json::{Map, Value};

use crate::frame::MAXIMUM_JSON_PAYLOAD_BYTES;

/// The member name of the one open slot of a payload object.
pub(crate) const EXTENSIONS: &str = "extensions";

const MINIMUM_PAYLOAD_BYTES: usize = 2;

/// The payload's object, or `None` when the bytes break an encoding rule.
pub(crate) fn parse(payload: &[u8]) -> Option<Map<String, Value>> {
    let object = parse_strict_object(payload)?;
    object
        .iter()
        .all(|(name, value)| name == EXTENSIONS || is_plain(value))
        .then_some(object)
}

/// Whether `value` holds no `null` and only integer tokens without a fraction, exponent, or
/// minus sign; `serde_json` reads every other number as signed or floating.
fn is_plain(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Number(number) => number.is_u64(),
        Value::Array(items) => items.iter().all(is_plain),
        Value::Object(members) => members.values().all(is_plain),
        Value::Bool(_) | Value::String(_) => true,
    }
}

/// The compact JSON text of `object`, or `None` when it does not fit one frame.
pub(crate) fn encode(object: Map<String, Value>) -> Option<Vec<u8>> {
    let payload = serde_json::to_vec(&Value::Object(object))
        .expect("a JSON value with string keys and finite numbers serializes");
    (MINIMUM_PAYLOAD_BYTES..=MAXIMUM_JSON_PAYLOAD_BYTES)
        .contains(&payload.len())
        .then_some(payload)
}
