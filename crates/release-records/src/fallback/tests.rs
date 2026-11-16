use super::client_failure_helper;
use std::path::Path;

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

// A failed Client is recorded through the previous version only while the pointer still makes the
// failed version active through a counting move that may fall back.
#[test]
fn a_failed_client_is_recorded_through_the_previous_version_only_while_its_move_counts() {
    let entry = |product: &str| format!(r#"{{"product":"{product}","manifest":"{DIGEST}"}}"#);
    let pointer = |active: &str, previous: Option<&str>, moved: &str, back: bool| {
        let previous = previous.map_or_else(String::new, |previous| {
            format!(r#","previous":{}"#, entry(previous))
        });
        format!(
            r#"{{"version":1,"active":{}{previous},"extensions":{{"fallback":{{"held":[],"move":{{"entry":{},"back":{back}}}}}}}}}"#,
            entry(active),
            entry(moved)
        )
    };
    let versions = Path::new("store").join("versions");
    let client = versions.join("0.2.0").join("orchbee");
    let counting = pointer("0.2.0", Some("0.1.0"), "0.2.0", true);
    assert_eq!(
        client_failure_helper(&client, Some(counting.as_bytes())),
        Some((versions.join("0.1.0").join("orchbee"), "0.2.0".to_owned()))
    );
    for (case, bytes) in [
        (
            "may not fall back",
            pointer("0.2.0", Some("0.1.0"), "0.2.0", false),
        ),
        ("no previous", pointer("0.2.0", None, "0.2.0", true)),
        (
            "a move for another entry",
            pointer("0.2.0", Some("0.1.0"), "0.1.0", true),
        ),
        (
            "another version active",
            pointer("0.3.0", Some("0.2.0"), "0.3.0", true),
        ),
        (
            "no fallback record",
            pointer("0.2.0", Some("0.1.0"), "0.2.0", true).replace("fallback", "later"),
        ),
        ("invalid", counting[..40].to_owned()),
    ] {
        assert_eq!(
            client_failure_helper(&client, Some(bytes.as_bytes())),
            None,
            "{case}"
        );
    }
    assert_eq!(client_failure_helper(&client, None), None);
}
