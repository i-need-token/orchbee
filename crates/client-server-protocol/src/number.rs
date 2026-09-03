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
