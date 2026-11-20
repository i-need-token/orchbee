//! The active-version pointer `active.json`: closed JSON naming the active version directory and,
//! optionally, the rollback target, each bound to the lowercase SHA-256 of its `release.json`.
//! `version` is the record's format and `extensions` the only open slot, which every rewrite keeps;
//! its `fallback` member is the [fallback record](crate::Fallback), and `noPath` records an install
//! made with `--no-path`.
//! Bytes that are not such a record mean no active version. The window pointer
//! `desktop\active.json` has the same grammar: there `product` names a window version directory and
//! `manifest` binds the SHA-256 of its `desktop-files.json`.
use crate::{
    strict_json::{check_extensions, closed_object, parse_object},
    tokens::{lowercase_sha256, valid_version},
};
use serde_json::{Map, Value};

/// The `extensions` member that records an install made with `--no-path`.
const NO_PATH: &str = "noPath";

/// One version directory and the digest of the manifest verified when it was installed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub product: String,
    pub manifest: String,
}

impl Entry {
    pub(crate) fn parse(value: &Value) -> Option<Self> {
        let object = closed_object(value, &["product", "manifest"], &[])?;
        let product = object.get("product")?.as_str()?;
        let manifest = object.get("manifest")?.as_str()?;
        (valid_version(product) && lowercase_sha256(manifest)).then(|| Self {
            product: product.to_owned(),
            manifest: manifest.to_owned(),
        })
    }

    pub(crate) fn to_value(&self) -> Value {
        let mut object = Map::new();
        object.insert("product".into(), Value::String(self.product.clone()));
        object.insert("manifest".into(), Value::String(self.manifest.clone()));
        Value::Object(object)
    }

    /// Both members are validated tokens, so neither needs escaping.
    fn encode(&self) -> String {
        format!(
            "{{ \"product\": \"{}\", \"manifest\": \"{}\" }}",
            self.product, self.manifest
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pointer {
    pub active: Entry,
    pub previous: Option<Entry>,
    extensions: Option<Map<String, Value>>,
}

impl Pointer {
    /// The format-1 pointer in `bytes`, or `None` when they hold anything else.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let root = Value::Object(parse_object(bytes)?);
        let object = closed_object(&root, &["version", "active"], &["previous", "extensions"])?;
        check_extensions(object)?;
        if object.get("version")?.as_u64()? != 1 {
            return None;
        }
        let previous = match object.get("previous") {
            Some(value) => Some(Entry::parse(value)?),
            None => None,
        };
        Some(Self {
            active: Entry::parse(object.get("active")?)?,
            previous,
            extensions: object.get("extensions").and_then(Value::as_object).cloned(),
        })
    }

    /// The pointer after `entry` becomes active. The version it replaces becomes the rollback
    /// target; activating the active version again keeps the target it had.
    pub fn activate(current: Option<Self>, entry: Entry) -> Self {
        match current {
            None => Self {
                active: entry,
                previous: None,
                extensions: None,
            },
            Some(current) => Self {
                previous: if current.active == entry {
                    current.previous
                } else {
                    Some(current.active)
                },
                active: entry,
                extensions: current.extensions,
            },
        }
    }

    /// The `extensions` member `name`, when present.
    pub(crate) fn extension(&self, name: &str) -> Option<&Value> {
        self.extensions.as_ref()?.get(name)
    }

    /// Sets the `extensions` member `name`, keeping every other member.
    pub(crate) fn set_extension(&mut self, name: &str, value: Value) {
        self.extensions
            .get_or_insert_with(Map::new)
            .insert(name.to_owned(), value);
    }

    /// Whether the latest install ran without the PATH step, so a later update keeps it off. Only
    /// the value `true` counts; any other content reads as absent.
    pub fn no_path(&self) -> bool {
        self.extension(NO_PATH) == Some(&Value::Bool(true))
    }

    /// Records, or clears, that the latest install ran without the PATH step. Clearing removes the
    /// member, and `extensions` as well when nothing else is left in it.
    pub fn set_no_path(&mut self, no_path: bool) {
        if no_path {
            self.set_extension(NO_PATH, Value::Bool(true));
            return;
        }
        if let Some(extensions) = self.extensions.as_mut() {
            extensions.remove(NO_PATH);
        }
        if self.extensions.as_ref().is_some_and(Map::is_empty) {
            self.extensions = None;
        }
    }

    /// Whether `entry` is the active version or the rollback target.
    pub fn names(&self, entry: &Entry) -> bool {
        self.active == *entry || self.previous.as_ref() == Some(entry)
    }

    /// The record in its written member order, ending in a newline.
    pub fn encode(&self) -> Vec<u8> {
        let mut text = format!(
            "{{\n  \"version\": 1,\n  \"active\": {}",
            self.active.encode()
        );
        if let Some(previous) = &self.previous {
            text.push_str(&format!(",\n  \"previous\": {}", previous.encode()));
        }
        if let Some(extensions) = &self.extensions {
            text.push_str(",\n  \"extensions\": ");
            text.push_str(&Value::Object(extensions.clone()).to_string());
        }
        text.push_str("\n}\n");
        text.into_bytes()
    }
}

#[cfg(test)]
mod tests;
