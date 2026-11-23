use super::{Entry, Pointer};
use serde_json::Value;

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn entry(product: &str) -> Entry {
    Entry {
        product: product.to_owned(),
        manifest: DIGEST.to_owned(),
    }
}

// A record written before the member existed reads as an install with the PATH step, and only the
// value `true` records a `--no-path` install.
#[test]
fn a_record_without_the_member_reads_with_the_path_step_and_only_true_counts() {
    let plain = format!(r#"{{"version":1,"active":{{"product":"0.2.0","manifest":"{DIGEST}"}}}}"#);
    assert!(!Pointer::parse(plain.as_bytes()).unwrap().no_path());
    for value in ["false", r#""yes""#, "1"] {
        let text = format!(
            r#"{{"version":1,"active":{{"product":"0.2.0","manifest":"{DIGEST}"}},"extensions":{{"noPath":{value}}}}}"#
        );
        assert!(
            !Pointer::parse(text.as_bytes()).unwrap().no_path(),
            "{value}"
        );
    }
}

// The member survives a rewrite and a later move beside the fallback record, and clearing it
// leaves that record alone.
#[test]
fn the_member_round_trips_survives_a_move_and_leaves_the_fallback_record_alone() {
    let text = format!(
        r#"{{"version":1,"active":{{"product":"0.2.0","manifest":"{DIGEST}"}},"extensions":{{"fallback":{{"held":[],"move":{{"entry":{{"product":"0.2.0","manifest":"{DIGEST}"}},"back":false}}}}}}}}"#
    );
    let mut pointer = Pointer::parse(text.as_bytes()).unwrap();
    pointer.set_no_path(true);
    let mut reread = Pointer::parse(&pointer.encode()).unwrap();
    assert!(reread.no_path() && reread.extension("fallback").is_some());
    let moved = Pointer::activate(Some(reread.clone()), entry("0.3.0"));
    assert!(moved.no_path() && moved.extension("fallback").is_some());
    reread.set_no_path(false);
    assert!(!reread.no_path() && reread.extension("fallback").is_some());
}

// Clearing the member from a record that held nothing else gives back the bytes it had.
#[test]
fn clearing_the_member_leaves_no_extensions_object_behind() {
    let mut pointer = Pointer::activate(None, entry("0.2.0"));
    let before = pointer.encode();
    pointer.set_no_path(true);
    assert!(String::from_utf8_lossy(&pointer.encode()).contains(r#""noPath":true"#));
    pointer.set_no_path(false);
    assert_eq!(pointer.encode(), before);
}

// The shared samples, which the npm launcher and Desktop's window pointer read as well: each names
// the active version this reader finds, or none, and the versions a well-formed fallback holds.
#[test]
fn the_shared_samples_read_as_their_expectations_say() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/pointers.json");
    let samples: Value =
        serde_json::from_slice(&std::fs::read(path).expect("the samples read")).expect("JSON");
    for case in samples["cases"].as_array().expect("cases") {
        let bytes = match case.get("text") {
            Some(text) => text.as_str().expect("text").as_bytes().to_vec(),
            None => serde_json::to_vec(&case["pointer"]).expect("pointer"),
        };
        let pointer = Pointer::parse(&bytes);
        let active = pointer
            .as_ref()
            .map(|pointer| pointer.active.product.as_str());
        assert_eq!(active, case["active"].as_str(), "{}", case["name"]);
        let held = pointer
            .as_ref()
            .and_then(Pointer::fallback)
            .map(|fallback| fallback.held)
            .unwrap_or_default();
        let expected: Vec<&str> = case["held"]
            .as_array()
            .expect("held")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(held, expected, "{}", case["name"]);
    }
}
