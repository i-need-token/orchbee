//! Raw nested members keep duplicate detection at every admitted template object.
use serde_core::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};
use serde_json::value::RawValue;
use std::{collections::BTreeMap, fmt};

pub(super) struct Object(BTreeMap<String, Box<RawValue>>);
impl<'de> Deserialize<'de> for Object {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Fields;
        impl<'de> Visitor<'de> for Fields {
            type Value = Object;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a bounded JSON object with unique members")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Object, M::Error> {
                let mut fields = BTreeMap::new();
                while let Some((name, value)) = map.next_entry::<String, Box<RawValue>>()? {
                    if fields.len() >= 64 || fields.insert(name, value).is_some() {
                        return Err(de::Error::custom("invalid layout members"));
                    }
                }
                Ok(Object(fields))
            }
        }
        deserializer.deserialize_map(Fields)
    }
}
impl Object {
    pub(super) fn parse(raw: &RawValue) -> Result<Self, &'static str> {
        serde_json::from_str(raw.get()).map_err(|_| "invalid-layout")
    }
    pub(super) fn require(&self, names: &[&str]) -> Result<(), &'static str> {
        if names.iter().all(|name| self.contains(name)) {
            Ok(())
        } else {
            Err("invalid-layout")
        }
    }
    pub(super) fn contains(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }
    pub(super) fn names(&self) -> Vec<String> {
        self.0.keys().cloned().collect()
    }
    pub(super) fn remove(&mut self, name: &str) -> Option<Box<RawValue>> {
        self.0.remove(name)
    }
    pub(super) fn take(&mut self, name: &str) -> Result<Box<RawValue>, &'static str> {
        self.remove(name).ok_or("invalid-layout")
    }
    pub(super) fn decode<T: serde_core::de::DeserializeOwned>(
        &mut self,
        name: &str,
    ) -> Result<T, &'static str> {
        serde_json::from_str(self.take(name)?.get()).map_err(|_| "invalid-layout")
    }
    pub(super) fn string(&mut self, name: &str) -> Result<String, &'static str> {
        let value: String = self.decode(name)?;
        if value.contains('\0') {
            return Err("invalid-layout");
        }
        Ok(value)
    }
    pub(super) fn optional_string(&mut self, name: &str) -> Result<Option<String>, &'static str> {
        let Some(raw) = self.remove(name) else {
            return Ok(None);
        };
        let value: Option<String> =
            serde_json::from_str(raw.get()).map_err(|_| "invalid-layout")?;
        if value.as_ref().is_some_and(|text| text.contains('\0')) {
            return Err("invalid-layout");
        }
        Ok(value)
    }
    pub(super) fn finish(self) -> Result<(), &'static str> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err("invalid-layout")
        }
    }
}
