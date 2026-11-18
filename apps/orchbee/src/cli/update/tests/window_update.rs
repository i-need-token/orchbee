//! The window part of `update` on a local signed feed: once the runtime part has installed the
//! offered runtime, a feed with `extensions.desktop` installs the offered window version into the
//! store's `desktop` folder from its own verified archive, under the store's rules for this
//! platform, moves the window pointer and reports the part in the document; a second run finds it
//! up to date without fetching anything.
use super::super::{
    Context, Error, archive,
    window::FOLDER,
    window_files::{NAME, Rules},
};
use super::install_fixture::{Fixture, copy_into, entry_for, never_unpack, pointer, sha};
use super::offered_update::{install, offer, release, serve, update};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// The window archive's name beside the feed.
const WINDOW_ARCHIVE: &str = "window.zip";

/// One file of a window directory: its list path, its bytes and whether the list marks it
/// executable.
type WindowSource = (&'static str, &'static [u8], bool);

/// A window directory of `version` under the fixture and its list: two files, and on macOS a bundle
/// path with spaces and parentheses, an executable file and an in-bundle link.
fn window(fixture: &Fixture, version: &str) -> (PathBuf, Vec<u8>) {
    let directory = fixture.0.join(format!("window-{version}"));
    let (files, links): (&[WindowSource], Value) = match Rules::own() {
        Rules::Windows => (
            &[
                ("orchbee-desktop.exe", b"window".as_slice(), false),
                ("resources/app.asar", b"page".as_slice(), false),
            ],
            Value::Null,
        ),
        Rules::Macos => (
            &[
                (
                    "Orchbee Test.app/Contents/MacOS/Orchbee Test",
                    b"window".as_slice(),
                    true,
                ),
                (
                    "Orchbee Test.app/Contents/Frameworks/A (B).framework/Versions/A/A (B)",
                    b"page".as_slice(),
                    false,
                ),
            ],
            json!([{
                "path": "Orchbee Test.app/Contents/Frameworks/A (B).framework/Versions/Current",
                "target": "A",
            }]),
        ),
    };
    let mut listed = Vec::new();
    for (path, bytes, executable) in files {
        let file = directory.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, bytes).unwrap();
        let mut entry = json!({"path": path, "size": bytes.len(), "sha256": sha(bytes)});
        if *executable {
            entry["executable"] = json!(true);
        }
        listed.push(entry);
    }
    let mut document = json!({"schemaVersion": 1, "version": version, "files": listed});
    if !links.is_null() {
        document["links"] = links;
    }
    let list = serde_json::to_vec(&document).unwrap();
    fs::write(directory.join(NAME), &list).unwrap();
    (directory, list)
}

#[test]
fn update_installs_the_offered_window_after_the_runtime() {
    let fixture = Fixture::new();
    let root = fixture.0.join("install");
    let (older, _) = release(&fixture, "older", "0.1.0", "test");
    let (newer, newer_manifest) = release(&fixture, "newer", "0.2.0", "test");
    install(&fixture, &older, false);
    let desktop = root.join(FOLDER);
    fs::create_dir(&desktop).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&desktop, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let (window_tree, list) = window(&fixture, "0.2.0");

    let runtime_archive = b"the runtime archive bytes".as_slice();
    let window_archive = b"the window archive bytes".as_slice();
    offer(
        &fixture,
        "0.2.0",
        &newer_manifest,
        runtime_archive,
        |feed| {
            feed["extensions"] = json!({"desktop": {
                "version": "0.2.0", "manifest": {"sha256": sha(&list)},
                "archive": {"format": "zip", "url": WINDOW_ARCHIVE, "size": window_archive.len(),
                    "sha256": sha(window_archive)},
            }});
        },
    );
    serve(&fixture, runtime_archive);
    fs::write(fixture.0.join("feed").join(WINDOW_ARCHIVE), window_archive).unwrap();
    // The stand-in unpack step tells the two verified archives apart by their bytes.
    let unpack = |unpacked: &Path, _context: &mut Context| -> Result<(), Error> {
        let fetched = fs::read(unpacked.parent().unwrap().join(archive::ARCHIVE)).unwrap();
        let tree = if fetched == window_archive {
            &window_tree
        } else {
            &newer
        };
        copy_into(tree, unpacked);
        Ok(())
    };

    let updated = update(&fixture, None, &unpack);
    assert_eq!(updated.code(), "complete", "{}", updated.rendered);
    // The runtime's transfer, then the window's.
    let sizes: Vec<u64> = updated.announced.iter().map(|(_, size)| *size).collect();
    assert_eq!(
        sizes,
        [runtime_archive.len() as u64, window_archive.len() as u64]
    );
    assert_eq!(
        pointer(&root).unwrap().active,
        entry_for("0.2.0", &newer_manifest)
    );
    let part = &updated.rendered["result"]["desktop"];
    assert_eq!(
        (
            &part["activated"],
            &part["upToDate"],
            &part["phase"],
            &part["error"]
        ),
        (
            &json!(true),
            &json!(false),
            &json!("complete"),
            &Value::Null
        ),
        "{part}"
    );
    assert_eq!(pointer(&desktop).unwrap().active, entry_for("0.2.0", &list));
    let stored = desktop.join("versions").join("0.2.0");
    assert_eq!(fs::read(stored.join(NAME)).unwrap(), list);
    assert_eq!(part["releasePath"], json!(stored));
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &str| {
            fs::metadata(stored.join(path))
                .unwrap()
                .permissions()
                .mode()
                & 0o777
        };
        assert_eq!(mode("Orchbee Test.app/Contents/MacOS/Orchbee Test"), 0o700);
        let framework = "Orchbee Test.app/Contents/Frameworks/A (B).framework";
        assert_eq!(mode(&format!("{framework}/Versions/A/A (B)")), 0o600);
        assert_eq!(
            fs::read_link(stored.join(format!("{framework}/Versions/Current"))).unwrap(),
            Path::new("A")
        );
    }

    // Both parts are now current: nothing is fetched, and the window part says so.
    fs::remove_file(fixture.0.join("feed").join(WINDOW_ARCHIVE)).unwrap();
    let again = update(&fixture, None, &never_unpack);
    assert_eq!(again.code(), "complete", "{}", again.rendered);
    assert!(again.announced.is_empty());
    assert_eq!(again.rendered["result"]["desktop"]["upToDate"], true);

    // A later release advances `desktop/versions/` while bounding disk: Windows keeps active +
    // previous (`0.2.0`, `0.3.0`), and macOS keeps only active (`0.3.0`).
    let (newest, newest_manifest) = release(&fixture, "newest", "0.3.0", "test");
    let (newest_window_tree, newest_list) = window(&fixture, "0.3.0");
    offer(
        &fixture,
        "0.3.0",
        &newest_manifest,
        runtime_archive,
        |feed| {
            feed["extensions"] = json!({"desktop": {
                "version": "0.3.0", "manifest": {"sha256": sha(&newest_list)},
                "archive": {"format": "zip", "url": WINDOW_ARCHIVE, "size": window_archive.len(),
                    "sha256": sha(window_archive)},
            }});
        },
    );
    serve(&fixture, runtime_archive);
    fs::write(fixture.0.join("feed").join(WINDOW_ARCHIVE), window_archive).unwrap();
    let unpack_newest = |unpacked: &Path, _context: &mut Context| -> Result<(), Error> {
        let fetched = fs::read(unpacked.parent().unwrap().join(archive::ARCHIVE)).unwrap();
        let tree = if fetched == window_archive {
            &newest_window_tree
        } else {
            &newest
        };
        copy_into(tree, unpacked);
        Ok(())
    };
    let upgraded = update(&fixture, None, &unpack_newest);
    assert_eq!(upgraded.code(), "complete", "{}", upgraded.rendered);
    let mut stored_versions: Vec<String> = fs::read_dir(desktop.join("versions"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    stored_versions.sort();
    #[cfg(target_os = "macos")]
    assert_eq!(stored_versions, ["0.3.0"]);
    #[cfg(not(target_os = "macos"))]
    assert_eq!(stored_versions, ["0.2.0", "0.3.0"]);
}
