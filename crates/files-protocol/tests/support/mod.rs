//! The File Bar corpora as the tests read them.

use serde::Deserialize;
use serde_json::Value;

/// Each capability's corpus, in the order the registry lists the capabilities.
const CORPORA: [(&str, &str); 2] = [
    (
        "files",
        include_str!("../../../../schemas/files/v1/files-conformance-v1.json"),
    ),
    (
        "files-edit",
        include_str!("../../../../schemas/files/v1/files-edit-conformance-v1.json"),
    ),
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Corpus {
    pub contract_version: u8,
    pub capability: String,
    pub bounds: Bounds,
    pub operations: Vec<OperationFacts>,
    #[serde(default)]
    pub records: RecordCases,
    pub requests: RequestCases,
    pub replies: DocumentCases,
    #[serde(default)]
    pub notices: DocumentCases,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Bounds {
    pub request_bytes: usize,
    pub reply_bytes: usize,
    pub data_bytes: usize,
    pub path_bytes: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OperationFacts {
    pub op: String,
    pub stream: String,
    pub grant: String,
    pub after_close: bool,
    pub result: Option<String>,
    #[serde(default)]
    pub chunk: bool,
    pub snapshot: Option<String>,
    pub update: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordCases {
    pub valid: Vec<ValidRecord>,
    pub invalid: Vec<InvalidRecord>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidRecord {
    pub name: String,
    pub place: String,
    pub hex: String,
    pub record: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvalidRecord {
    pub name: String,
    pub place: String,
    pub hex: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestCases {
    pub valid: Vec<Document>,
    pub invalid: Vec<Document>,
    #[serde(default)]
    pub unsupported: Vec<Document>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentCases {
    pub valid: Vec<Document>,
    pub invalid: Vec<Document>,
}

/// One JSON case: a JSON value, or JSON text a value cannot hold, such as a duplicate member.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub name: String,
    pub op: Option<String>,
    pub kind: Option<String>,
    pub json: Option<Value>,
    pub text: Option<String>,
    pub boundary: Option<String>,
}

impl Document {
    /// The bytes a record carries.
    pub fn bytes(&self) -> Vec<u8> {
        match (&self.json, &self.text) {
            (Some(json), None) => serde_json::to_vec(json).unwrap(),
            (None, Some(text)) => text.as_bytes().to_vec(),
            _ => panic!("{} needs exactly one of json and text", self.name),
        }
    }

    /// Whether only a binding refuses the case, because the schema bounds characters where the
    /// binding bounds UTF-8 bytes.
    pub fn binding_boundary(&self) -> bool {
        match self.boundary.as_deref() {
            None => false,
            Some("binding") => true,
            Some(other) => panic!("{}: unknown boundary {other}", self.name),
        }
    }
}

/// Every capability's corpus.
pub fn corpora() -> Vec<Corpus> {
    CORPORA
        .iter()
        .map(|(capability, text)| {
            let corpus: Corpus = serde_json::from_str(text).unwrap();
            assert_eq!(corpus.contract_version, 1);
            assert_eq!(corpus.capability, *capability);
            corpus
        })
        .collect()
}

pub fn bytes(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "{hex}");
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}
