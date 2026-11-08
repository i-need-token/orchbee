//! Strict JSON reading for unnegotiated messages: one object, no duplicate member at any depth,
//! and at most [`MAXIMUM_DEPTH`] nested arrays and objects. `serde_json` supplies RFC 8259 syntax,
//! UTF-8, finite numbers, and the rejection of lone surrogate escapes.

use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

/// The deepest nesting of arrays and objects a payload may contain; the message object is
/// depth 1.
pub(crate) const MAXIMUM_DEPTH: usize = 32;

/// Parses `payload` as one strict JSON object.
pub(crate) fn parse_object(payload: &[u8]) -> Option<Map<String, Value>> {
    let text = std::str::from_utf8(payload).ok()?;
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value = StrictValue { depth: 1 }
        .deserialize(&mut deserializer)
        .ok()?;
    deserializer.end().ok()?;
    match value {
        Value::Object(object) => Some(object),
        _ => None,
    }
}

/// The nesting depth of `value`, counting each array and object; a scalar has depth 0.
pub(crate) fn depth(value: &Value) -> usize {
    match value {
        Value::Array(items) => 1 + items.iter().map(depth).max().unwrap_or(0),
        Value::Object(members) => 1 + members.values().map(depth).max().unwrap_or(0),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 0,
    }
}

/// Deserializes any JSON value whose arrays and objects start at `depth`, rejecting duplicate
/// members and nesting beyond [`MAXIMUM_DEPTH`].
pub(crate) struct StrictValue {
    pub(crate) depth: usize,
}

impl<'de> DeserializeSeed<'de> for StrictValue {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for StrictValue {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a strict JSON value")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite number"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        if self.depth > MAXIMUM_DEPTH {
            return Err(de::Error::custom("JSON nesting is too deep"));
        }
        let mut items = Vec::new();
        while let Some(item) = sequence.next_element_seed(StrictValue {
            depth: self.depth + 1,
        })? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut members: A) -> Result<Value, A::Error> {
        if self.depth > MAXIMUM_DEPTH {
            return Err(de::Error::custom("JSON nesting is too deep"));
        }
        let mut object = Map::new();
        while let Some(name) = members.next_key::<String>()? {
            if object.contains_key(&name) {
                return Err(de::Error::custom("duplicate JSON member"));
            }
            let value = members.next_value_seed(StrictValue {
                depth: self.depth + 1,
            })?;
            object.insert(name, value);
        }
        Ok(Value::Object(object))
    }
}
