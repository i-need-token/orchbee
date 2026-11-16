use super::{Rules, WindowFiles};
use serde_json::{Value, json};

const DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn list(links: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": 1, "version": "1.2.3",
        "files": [
            {"path": "A.app/Contents/MacOS/A", "size": 1, "sha256": DIGEST, "executable": true},
            {"path": "A.app/Contents/Frameworks/E F.framework/Versions/A/E F", "size": 1,
                "sha256": DIGEST},
        ],
        "links": links,
    }))
    .unwrap()
}

// The install makes every listed link, so a target that leaves the bundle, or names nothing the
// list holds, would let a signed list plant a path outside the window version; the reader is the
// only check before the install writes. Mirrors `readDesktopFiles` in scripts/desktop-files.mjs.
#[test]
fn a_macos_link_must_stay_inside_the_bundle_and_reach_a_listed_entry() {
    let framework = "A.app/Contents/Frameworks/E F.framework";
    for (links, admitted) in [
        (
            json!([{"path": format!("{framework}/Versions/Current"), "target": "A"}]),
            true,
        ),
        (
            json!([
                {"path": format!("{framework}/Versions/Current"), "target": "A"},
                {"path": format!("{framework}/E F"), "target": "Versions/Current/E F"},
            ]),
            true,
        ),
        (
            json!([{"path": "A.app/Contents/up", "target": "../../.."}]),
            false,
        ),
        (
            json!([{"path": "A.app/Contents/abs", "target": "/etc"}]),
            false,
        ),
        (
            json!([{"path": "A.app/Contents/none", "target": "Missing"}]),
            false,
        ),
        (
            json!([{"path": "A.app/Contents/self", "target": "self"}]),
            false,
        ),
        (json!([{"path": "B.app/x", "target": "y"}]), false),
    ] {
        assert_eq!(
            WindowFiles::parse(&list(links.clone()), Rules::Macos).is_some(),
            admitted,
            "{links}"
        );
    }
    assert!(WindowFiles::parse(&list(json!([])), Rules::Windows).is_none());
}
