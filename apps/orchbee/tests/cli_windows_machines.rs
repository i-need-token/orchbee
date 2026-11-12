//! Actual CLI catalog operations without changing the user's settings or contacting SSH.
#![cfg(windows)]
use crate::peer_support as support;
use orchbee_local_runtime::PrivateCache;
use serde_json::{Value, json};
use support::{Fixture, validate};
const ID: &str = "11111111111111111111111111111111";
fn document(output: std::process::Output, status: i32) -> Value {
    assert_eq!(output.status.code(), Some(status), "{output:?}");
    let value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &value,
        include_str!("../src/cli/schemas/orchbee.machine.json"),
    );
    value
}
#[test]
fn saved_machine_commands_round_trip_without_local_activation_or_remote_stop() {
    let fixture = Fixture::new();
    let root = fixture.data_root().state();
    let list = document(fixture.run(&["machine", "list", "--json"]), 0);
    assert_eq!(list["result"]["profiles"], json!([]));
    assert!(!root.exists());
    let mut cache = PrivateCache::acquire(&root, "machines").unwrap();
    cache
        .replace(
            "machines.json",
            &serde_json::to_vec(&json!({"version":1,"revision":1,"profiles":[{
                "id":ID,"label":"Work","connection":{"kind":"ssh","target":"example.invalid"},"session":"work","enabled":true,"remoteInstall":"manual"
            }]}))
            .unwrap(),
        )
        .unwrap();
    drop(cache);
    for (args, revision, changed) in [
        (
            vec!["machine", "rename", ID, "--label=中文工作", "--format=json"],
            2,
            true,
        ),
        (vec!["machine", "disable", ID, "--format=json"], 3, true),
        (vec!["machine", "disable", ID, "--format=json"], 3, false),
        (vec!["machine", "enable", ID, "--format=json"], 4, true),
        (vec!["machine", "remove", ID, "--format=json"], 5, true),
    ] {
        let value = document(fixture.run(&args), 0);
        assert_eq!(value["result"]["revision"], revision);
        assert_eq!(value["result"]["changed"], changed);
        assert_eq!(value["result"]["remote"], "not-contacted");
    }
    let missing = document(fixture.run(&["machine", "remove", ID, "--format=json"]), 3);
    assert_eq!(missing["result"]["error"], "not-found");
    assert_eq!(
        document(fixture.run(&["machine", "list", "--json"]), 0)["result"]["profiles"],
        json!([])
    );
    assert!(!fixture.0.join("runtime").exists());
}
#[test]
fn invalid_catalog_and_invalid_grammar_refuse_add_before_ssh_and_preserve_bytes() {
    let fixture = Fixture::new();
    let root = fixture.data_root().state();
    let mut cache = PrivateCache::acquire(&root, "machines").unwrap();
    cache.replace("machines.json", b"not json").unwrap();
    drop(cache);
    let value = document(
        fixture.run(&[
            "machine",
            "add",
            "example.invalid",
            "--label=x",
            "--format=json",
        ]),
        1,
    );
    assert_eq!(value["result"]["error"], "invalid-catalog");
    assert_eq!(value["result"]["remote"], "not-contacted");
    assert_eq!(
        std::fs::read(root.join("machines.json")).unwrap(),
        b"not json"
    );
    for args in [
        vec!["machine", "add", "host;calc", "--label=x"],
        vec![
            "machine",
            "add",
            "host",
            "--label=x",
            "--remote-session=../work",
        ],
        vec!["machine", "list", "--json", "--format=json"],
    ] {
        assert_eq!(fixture.run(&args).status.code(), Some(2));
    }
    assert!(!fixture.0.join("runtime").exists());
}
