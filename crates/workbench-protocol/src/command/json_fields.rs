//! Shared JSON number semantics and required-but-nullable fields.
use serde::{Deserialize, Deserializer, de};

pub(super) fn deserialize_nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}

pub(super) fn deserialize_i32<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<i32>, D::Error> {
    Option::<f64>::deserialize(deserializer)?
        .map(|value| {
            if value.is_finite()
                && value.fract() == 0.0
                && value >= f64::from(i32::MIN)
                && value <= f64::from(i32::MAX)
            {
                Ok(value as i32)
            } else {
                Err(de::Error::custom("invalid process status"))
            }
        })
        .transpose()
}
