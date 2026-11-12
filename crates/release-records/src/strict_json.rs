//! Strict JSON reading for release records: one object, no repeated member at any depth, bounded
//! nesting, and closed member sets. `serde_json` supplies RFC 8259 syntax, UTF-8 and finite
//! numbers. Callers own every value rule; this module never defaults a member.
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;

/// The deepest nesting of arrays and objects a document may contain; the root object is depth 1.
const MAXIMUM_DEPTH: usize = 32;

/// Parses `bytes` as one strict JSON object, or `None` for any other document.
pub fn parse_object(bytes: &[u8]) -> Option<Map<String, Value>> {
    let text = std::str::from_utf8(bytes).ok()?;
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

/// The object when `value` is one whose members are exactly every `required` name plus any of the
/// `optional` names.
pub fn closed_object<'a>(
    value: &'a Value,
    required: &[&str],
    optional: &[&str],
) -> Option<&'a Map<String, Value>> {
    let object = value.as_object()?;
    let admitted = required.iter().all(|name| object.contains_key(*name))
        && object
            .keys()
            .all(|name| required.contains(&name.as_str()) || optional.contains(&name.as_str()));
    admitted.then_some(object)
}

/// An absent `extensions` member, or one holding any JSON object, which readers ignore.
pub fn check_extensions(object: &Map<String, Value>) -> Option<()> {
    match object.get("extensions") {
        None | Some(Value::Object(_)) => Some(()),
        Some(_) => None,
    }
}

/// Deserializes any JSON value whose arrays and objects start at `depth`, rejecting repeated
/// members and nesting beyond [`MAXIMUM_DEPTH`].
struct StrictValue {
    depth: usize,
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
                return Err(de::Error::custom("repeated JSON member"));
            }
            let value = members.next_value_seed(StrictValue {
                depth: self.depth + 1,
            })?;
            object.insert(name, value);
        }
        Ok(Value::Object(object))
    }
}
