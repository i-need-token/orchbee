//! The `extensions` object, the only open slot of an unnegotiated message. Member names follow
//! the capability-name grammar and a receiver ignores names it does not know; values are opaque
//! JSON within the payload's nesting bound.

use std::{collections::BTreeMap, fmt};

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, MapAccess, Visitor},
};
use serde_json::Value;

use crate::{
    CapabilityName, InvalidValue,
    name::is_name,
    strict_json::{MAXIMUM_DEPTH, StrictValue, depth},
};

/// The depth of the `extensions` object inside a message.
const EXTENSIONS_DEPTH: usize = 2;

/// Named extension values. Decoding admits an `extensions` member of a top-level message object:
/// an object whose member names follow the capability-name grammar, without duplicates at any
/// depth, and whose values keep the payload within its nesting bound.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extensions(BTreeMap<String, Value>);

impl Extensions {
    /// An empty set, which encoders omit.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether no member is present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The value of member `name`, if present.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.0.get(name)
    }

    /// The members in bytewise name order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.0.iter().map(|(name, value)| (name.as_str(), value))
    }

    /// Sets member `name`, returning its previous value.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidValue`] when `value` nests deeper than a message admits.
    pub fn insert(
        &mut self,
        name: CapabilityName,
        value: Value,
    ) -> Result<Option<Value>, InvalidValue> {
        if EXTENSIONS_DEPTH + depth(&value) > MAXIMUM_DEPTH {
            return Err(InvalidValue);
        }
        Ok(self.0.insert(name.into(), value))
    }
}

impl Serialize for Extensions {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Extensions {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(ExtensionsVisitor)
    }
}

struct ExtensionsVisitor;

impl<'de> Visitor<'de> for ExtensionsVisitor {
    type Value = Extensions;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an extensions object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut members: A) -> Result<Extensions, A::Error> {
        let mut extensions = BTreeMap::new();
        while let Some(name) = members.next_key::<String>()? {
            if !is_name(&name) {
                return Err(de::Error::custom(
                    "extension name violates the name grammar",
                ));
            }
            if extensions.contains_key(&name) {
                return Err(de::Error::custom("duplicate extension member"));
            }
            let value = members.next_value_seed(StrictValue {
                depth: EXTENSIONS_DEPTH + 1,
            })?;
            extensions.insert(name, value);
        }
        Ok(Extensions(extensions))
    }
}
