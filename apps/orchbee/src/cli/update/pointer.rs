//! The active-version pointer `active.json`: closed JSON naming the active version directory and,
//! optionally, the rollback target, each bound to the lowercase SHA-256 of its `release.json`.
//! `version` is the record's format and `extensions` the only open slot, which every rewrite keeps.
//! Bytes that are not such a record mean no active version.
use super::{manifest, strict_json};
use serde_json::{Map, Value};

/// One version directory and the digest of the manifest verified when it was installed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Entry {
    pub(super) product: String,
    pub(super) manifest: String,
}

impl Entry {
    fn parse(value: &Value) -> Option<Self> {
        let object = strict_json::closed(value, &["product", "manifest"], &[])?;
        let product = object.get("product")?.as_str()?;
        let manifest = object.get("manifest")?.as_str()?;
        (manifest::valid_version(product) && manifest::lowercase_sha256(manifest)).then(|| Self {
            product: product.to_owned(),
            manifest: manifest.to_owned(),
        })
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
pub(super) struct Pointer {
    pub(super) active: Entry,
    pub(super) previous: Option<Entry>,
    extensions: Option<Map<String, Value>>,
}

impl Pointer {
    /// The format-1 pointer in `bytes`, or `None` when they hold anything else.
    pub(super) fn parse(bytes: &[u8]) -> Option<Self> {
        let root = Value::Object(strict_json::parse_object(bytes)?);
        let object =
            strict_json::closed(&root, &["version", "active"], &["previous", "extensions"])?;
        strict_json::extensions(object)?;
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
    pub(super) fn activate(current: Option<Self>, entry: Entry) -> Self {
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

    /// Whether `entry` is the active version or the rollback target.
    pub(super) fn names(&self, entry: &Entry) -> bool {
        self.active == *entry || self.previous.as_ref() == Some(entry)
    }

    /// The record in its written member order, ending in a newline.
    pub(super) fn encode(&self) -> Vec<u8> {
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
