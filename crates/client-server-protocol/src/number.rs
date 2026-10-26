//! Refines JSON numeric tokens to finite binary64 safe integers and counters to canonical
//! decimal strings, preserving required nullable counter semantics. These adapters validate
//! representation; they do not allocate sequences or judge whether a counter is current.

use std::fmt;

use serde::{Deserialize, Deserializer, de};

const MAXIMUM_BINARY64_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Binary64SafeInteger(u64);

impl Binary64SafeInteger {
    pub(crate) fn try_into_u16(self) -> Result<u16, &'static str> {
        u16::try_from(self.0).map_err(|_error| "number exceeds the unsigned 16-bit range")
    }

    pub(crate) const fn value(self) -> u64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for Binary64SafeInteger {
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        deserializer.deserialize_any(Binary64SafeIntegerVisitor)
    }
}

struct Binary64SafeIntegerVisitor;

impl de::Visitor<'_> for Binary64SafeIntegerVisitor {
    type Value = Binary64SafeInteger;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a finite nonnegative binary64 safe integer")
    }

    fn visit_u64<ErrorType>(self, value: u64) -> Result<Self::Value, ErrorType>
    where
        ErrorType: de::Error,
    {
        if value > MAXIMUM_BINARY64_SAFE_INTEGER {
            return Err(ErrorType::custom(
                "number exceeds the binary64 safe-integer range",
            ));
        }
        Ok(Binary64SafeInteger(value))
    }

    fn visit_i64<ErrorType>(self, value: i64) -> Result<Self::Value, ErrorType>
    where
        ErrorType: de::Error,
    {
        let value = u64::try_from(value).map_err(|_error| {
            ErrorType::custom("number must normalize to a nonnegative safe integer")
        })?;
        self.visit_u64(value)
    }

    fn visit_f64<ErrorType>(self, value: f64) -> Result<Self::Value, ErrorType>
    where
        ErrorType: de::Error,
    {
        if !value.is_finite()
            || value < 0.0
            || value.trunc() != value
            || value > MAXIMUM_BINARY64_SAFE_INTEGER as f64
        {
            return Err(ErrorType::custom(
                "number must normalize to a nonnegative safe integer",
            ));
        }
        self.visit_u64(value as u64)
    }
}

#[cfg(test)]
mod tests;

pub(crate) mod decimal {
    use serde::{Deserialize, Deserializer, Serializer, de};
    pub(crate) fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let text = String::deserialize(deserializer)?;
        let value = text.parse::<u64>().map_err(de::Error::custom)?;
        if value.to_string() != text {
            return Err(de::Error::custom("expected canonical decimal u64"));
        }
        Ok(value)
    }
}
/// `Option<NonZeroU64>` as a required-but-nullable positive decimal string:
/// the key is always present, `null` marks absence, and a present value uses
/// the same canonical positive-decimal rule as `positive_decimal`.
pub(crate) mod positive_decimal_nullable {
    use serde::{Deserialize, Deserializer, Serializer, de};
    use std::num::NonZeroU64;
    pub(crate) fn serialize<S: Serializer>(
        value: &Option<NonZeroU64>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => super::positive_decimal::serialize(value, serializer),
            None => serializer.serialize_none(),
        }
    }
    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<NonZeroU64>, D::Error> {
        let Some(text) = Option::<String>::deserialize(deserializer)? else {
            return Ok(None);
        };
        let value = text.parse::<u64>().map_err(de::Error::custom)?;
        if value.to_string() != text {
            return Err(de::Error::custom("expected canonical decimal u64"));
        }
        NonZeroU64::new(value)
            .map(Some)
            .ok_or_else(|| de::Error::custom("expected positive decimal u64"))
    }
}
pub(crate) mod positive_decimal {
    use serde::{Deserializer, Serializer, de};
    use std::num::NonZeroU64;
    pub(crate) fn serialize<S: Serializer>(
        value: &NonZeroU64,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        super::decimal::serialize(&value.get(), serializer)
    }
    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<NonZeroU64, D::Error> {
        NonZeroU64::new(super::decimal::deserialize(deserializer)?)
            .ok_or_else(|| de::Error::custom("expected positive decimal u64"))
    }
}
