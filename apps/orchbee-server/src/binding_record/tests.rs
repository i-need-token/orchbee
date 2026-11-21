use super::{decode, encode};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// One change that makes a stored record unusable.
type Change = fn(&mut Value);

fn samples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/binding_record/tests/record-samples")
}

fn sample() -> Value {
    serde_json::from_slice(&fs::read(samples().join("bindings-v1.json")).unwrap()).unwrap()
}

/// Two installed versions read the record at once: every released format still decodes, and what
/// this build writes back from it decodes to the same record, a later release's members included.
#[test]
fn every_stored_sample_decodes_and_writes_back_unchanged() {
    let mut decoded = 0;
    for entry in fs::read_dir(samples()).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        assert!(
            name.starts_with("bindings-v") && name.ends_with(".json"),
            "unknown sample {name}"
        );
        let record = decode(&fs::read(samples().join(&name)).unwrap())
            .unwrap_or_else(|| panic!("{name} decodes"));
        let written = encode(&record).unwrap_or_else(|| panic!("{name} encodes"));
        assert_eq!(decode(&written), Some(record), "{name}");
        decoded += 1;
    }
    assert!(decoded > 0);
}

/// A reader that cannot use a record treats it as absent: another version, a member this build
/// does not know outside `extensions`, or contents that contradict each other.
#[test]
fn a_record_this_build_cannot_use_reads_as_absent() {
    let refusals: [(&str, Change); 6] = [
        ("another version", |record| record["version"] = 2.into()),
        ("an unknown member", |record| record["added"] = true.into()),
        ("a terminal on no recorded Host", |record| {
            record["hosts"][0]["epoch"] = "000000000000000000000000000000e2".into();
        }),
        ("one terminal recorded twice", |record| {
            record["terminals"][1]["terminal_id"] = record["terminals"][0]["terminal_id"].clone();
        }),
        ("a started terminal without its geometry", |record| {
            record["terminals"][0]["geometry"] = Value::Null;
        }),
        ("a terminal the next identity does not follow", |record| {
            record["next_identity"] = record["terminals"][1]["terminal_id"].clone();
        }),
    ];
    assert!(decode(&serde_json::to_vec(&sample()).unwrap()).is_some());
    for (case, change) in refusals {
        let mut record = sample();
        change(&mut record);
        assert_eq!(
            decode(&serde_json::to_vec(&record).unwrap()),
            None,
            "{case}"
        );
    }
}
