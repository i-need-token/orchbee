//! Strict reading of File Bar JSON bodies: one object under the `OBHS` payload rules, read as a
//! typed value only when the value's own canonical spelling states the same members and items. A
//! binding therefore admits nothing `schemas/files/v1` refuses, such as an array where an object
//! belongs or `null` where a member may only be absent, which `serde` alone would accept.

use orchbee_connection_handshake::parse_strict_object;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::value::InvalidValue;

/// The strict object `json` spells.
///
/// # Errors
///
/// [`InvalidValue`] for bytes that break the `OBHS` payload rules or hold no object.
pub(crate) fn strict_object(json: &[u8]) -> Result<Value, InvalidValue> {
    parse_strict_object(json)
        .map(Value::Object)
        .ok_or(InvalidValue)
}

/// Reads `json` as `T`.
///
/// # Errors
///
/// [`InvalidValue`] for bytes [`strict_object`] refuses and for an object `T` does not spell
/// canonically.
pub(crate) fn read_object<T: DeserializeOwned + Serialize>(json: &[u8]) -> Result<T, InvalidValue> {
    read_value(&strict_object(json)?)
}

/// Reads the strict object `input` as `T`.
///
/// # Errors
///
/// [`InvalidValue`] for an object `T` does not spell canonically.
pub(crate) fn read_value<T: DeserializeOwned + Serialize>(
    input: &Value,
) -> Result<T, InvalidValue> {
    let value = T::deserialize(input).map_err(|_| InvalidValue)?;
    let canonical = serde_json::to_value(&value).map_err(|_| InvalidValue)?;
    if same_spelling(input, &canonical) {
        Ok(value)
    } else {
        Err(InvalidValue)
    }
}

/// Whether `input` states what `canonical`, a value's own serialization, states: the same members
/// and items, numbers of equal value, and `false` where `canonical` omits an optional boolean.
/// Every admitted number is a safe integer, so comparing them as `f64` is exact.
fn same_spelling(input: &Value, canonical: &Value) -> bool {
    match (input, canonical) {
        (Value::Object(input), Value::Object(canonical)) => {
            canonical.keys().all(|name| input.contains_key(name))
                && input.iter().all(|(name, value)| match canonical.get(name) {
                    Some(canonical) => same_spelling(value, canonical),
                    None => *value == Value::Bool(false),
                })
        }
        (Value::Array(input), Value::Array(canonical)) => {
            input.len() == canonical.len()
                && input
                    .iter()
                    .zip(canonical)
                    .all(|(input, canonical)| same_spelling(input, canonical))
        }
        (Value::Number(input), Value::Number(canonical)) => input.as_f64() == canonical.as_f64(),
        _ => input == canonical,
    }
}
