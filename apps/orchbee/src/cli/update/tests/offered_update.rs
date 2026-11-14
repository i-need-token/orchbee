//! `update` through the engine on a local signed feed: the decision from the feed and the store
//! before any archive request, then one archive checked against the feed's signed size and SHA-256,
//! unpacked by a stand-in step that copies a prepared release directory (the system `tar.exe` has
//! its own cases below), and installed by the verified install. After every failure the versions,
//! the pointer and the download folder are as they were, save the folder's lock.
use super::super::{
    Context, Error, Report,
    archive::{self, Unpack},
    document, engine,
    install::Completion,
    signature::TEST_TABLE,
};
use super::install_fixture::{
    Fixture, context, copy_into, entry_for, in_process_completion, manifest, never_unpack, pointer,
    print_probe, publish_manifest, sha, stage_cohort,
};
use super::{sign, sign_feed};
use orchbee_local_runtime::{PrivateCache, UpdateChannel};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    fs, io,
    path::{Path, PathBuf},
};

/// The archive's name beside the feed.
const ARCHIVE: &str = "release.zip";

struct Run {
    result: Result<(), Error>,
    report: Report,
    rendered: Value,
}

impl Run {
    fn code(&self) -> &'static str {
        self.result.err().map_or("complete", Error::code)
    }
}

/// A signed release directory `name` of `version` under the fixture, and its manifest bytes.
fn release(fixture: &Fixture, name: &str, version: &str, build: &str) -> (PathBuf, Vec<u8>) {
    let source = fixture.0.join(name);
    stage_cohort(&source, version);
    let bytes = manifest(&source, |value| {
        value["version"] = json!(version);
        value["buildId"] = json!(build);
    });
    publish_manifest(&source, &bytes, Some(&sign(&bytes)));
    (source, bytes)
}

/// Signs and writes the fixture's feed offering `version` with `manifest` in an archive of the
/// bytes `archive`, with `change` applied; the archive file itself is the caller's to write.
fn offer(
    fixture: &Fixture,
    version: &str,
    manifest: &[u8],
    archive: &[u8],
    change: impl FnOnce(&mut Value),
) {
    let folder = fixture.0.join("feed");
    fs::create_dir_all(&folder).unwrap();
    let release: Value = serde_json::from_slice(manifest).unwrap();
    let mut feed = json!({
        "schemaVersion": 1, "product": "orchbee", "channel": "stable",
        "platform": "windows-x86_64", "version": version, "buildId": release["buildId"],
        "manifest": {"sha256": sha(manifest)},
        "archive": {"format": "zip", "url": ARCHIVE, "size": archive.len(), "sha256": sha(archive)},
    });
    change(&mut feed);
    let bytes = serde_json::to_vec(&feed).unwrap();
    fs::write(folder.join("feed.json"), &bytes).unwrap();
    fs::write(folder.join("feed.json.sig"), sign_feed(&bytes)).unwrap();
}

/// Places `bytes` as the archive beside the feed.
fn serve(fixture: &Fixture, bytes: &[u8]) {
    fs::write(fixture.0.join("feed").join(ARCHIVE), bytes).unwrap();
}

/// One update from the fixture's feed, unpacking with `unpack`.
fn update(fixture: &Fixture, channel: Option<UpdateChannel>, unpack: Unpack<'_>) -> Run {
    let data = fixture.0.join("data");
    let state = move |name: &str| (name == "ORCHBEE_HOME").then(|| data.clone().into_os_string());
    let inputs = engine::Inputs {
        channel,
        state: &state,
        feed_source: Some(
            fixture
                .0
                .join("feed")
                .join("feed.json")
                .to_string_lossy()
                .into_owned(),
        ),
        keys: TEST_TABLE,
        install_root: Ok(fixture.0.join("install")),
        loopback: false,
        probe: print_probe,
        activation: in_process_completion,
        unpack,
    };
    let mut context = context();
    let mut report = Report::new();
    let result = engine::run(&inputs, &mut context, &mut report);
    let rendered = document(channel, &report, result.err(), false);
    let schema: Value =
        serde_json::from_str(include_str!("../../schemas/orchbee.update.json")).unwrap();
    assert!(
        jsonschema::validator_for(&schema)
            .unwrap()
            .is_valid(&rendered),
        "{rendered}"
    );
    Run {
        result,
        report,
        rendered,
    }
}

/// One `install` of the signed release directory `source` into the fixture's store.
fn install(fixture: &Fixture, source: &Path, allow_downgrade: bool) {
    let inputs = engine::InstallInputs {
        directory: Ok(source.to_path_buf()),
        keys: TEST_TABLE,
        install_root: Ok(fixture.0.join("install")),
        probe: print_probe,
        activation: in_process_completion,
        completion: Completion {
            path: false,
            allow_downgrade,
        },
    };
    let mut report = Report::new();
    let result = engine::install(&inputs, &mut context(), &mut report);
    assert_eq!(result.err().map(Error::code), None);
}

/// The names in `folder`, sorted; none when it does not exist.
fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .map(|entries| {
            entries
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// An unpack step that copies `tree` into the unpacked folder and counts its calls.
fn copying<'a>(
    tree: &'a Path,
    calls: &'a Cell<usize>,
) -> impl Fn(&Path, &mut Context) -> Result<(), Error> + 'a {
    move |unpacked, _context| {
        calls.set(calls.get() + 1);
        copy_into(tree, unpacked);
        Ok(())
    }
}

#[test]
fn update_decides_from_the_signed_feed_before_any_archive_request() {
    let fixture = Fixture::new();
    let root = fixture.0.join("install");
    let (_, older_manifest) = release(&fixture, "older", "0.1.0", "test");
    let (newer, newer_manifest) = release(&fixture, "newer", "0.2.0", "test");
    install(&fixture, &newer, false);
    let active = entry_for("0.2.0", &newer_manifest);

    // No archive file exists beside the feed, and the unpack step fails the test when called: each
    // of these ends from the feed and the store alone.
    offer(&fixture, "0.2.0", &newer_manifest, b"never fetched", |_| {});
    let equal = update(&fixture, None, &never_unpack);
    assert_eq!(equal.code(), "complete");
    let result = &equal.rendered["result"];
    assert_eq!(
        (&result["upToDate"], &result["activated"], &result["reused"]),
        (&json!(true), &json!(true), &json!(true)),
        "{result}"
    );
    assert_eq!(
        equal.report.published.as_deref(),
        Some(root.join("versions/0.2.0").as_path())
    );

    offer(&fixture, "0.1.0", &older_manifest, b"never fetched", |_| {});
    let lower = update(&fixture, None, &never_unpack);
    assert_eq!(lower.code(), "complete");
    let result = &lower.rendered["result"];
    assert_eq!(
        (
            &result["upToDate"],
            &result["activated"],
            &result["releasePath"]
        ),
        (&json!(true), &json!(false), &Value::Null),
        "{result}"
    );

    let other = manifest(&newer, |value| {
        value["version"] = json!("0.2.0");
        value["buildId"] = json!("other");
    });
    offer(&fixture, "0.2.0", &other, b"never fetched", |_| {});
    assert_eq!(
        update(&fixture, None, &never_unpack).code(),
        "version-conflict"
    );

    // A feed of another channel than the selected one is refused after its signature verified.
    offer(
        &fixture,
        "0.3.0",
        &newer_manifest,
        b"never fetched",
        |feed| {
            feed["channel"] = json!("preview");
        },
    );
    assert_eq!(
        update(&fixture, None, &never_unpack).code(),
        "manifest-invalid"
    );

    assert_eq!(names(&root.join("versions")), ["0.2.0"]);
    assert_eq!(pointer(&root).unwrap().active, active);
    assert!(!root.join(archive::FOLDER).exists());
}

#[test]
fn update_installs_one_verified_archive_and_leaves_only_the_lock() {
    let fixture = Fixture::new();
    let root = fixture.0.join("install");
    let (older, _) = release(&fixture, "older", "0.1.0", "test");
    let (newer, newer_manifest) = release(&fixture, "newer", "0.2.0", "test");
    install(&fixture, &older, false);

    // What a crashed run left, and a file the signed manifest does not list (rule U4).
    let folder = root.join(archive::FOLDER);
    fs::create_dir_all(folder.join("unpacked/orchbee-0.2.0")).unwrap();
    fs::write(folder.join("archive.zip"), b"partial").unwrap();
    let tree = fixture.0.join("unpacked-tree");
    copy_into(&newer, &tree);
    fs::write(tree.join("unlisted.exe"), b"never published").unwrap();

    let archive = b"the archive bytes the feed signs";
    offer(&fixture, "0.2.0", &newer_manifest, archive, |_| {});
    serve(&fixture, archive);
    let calls = Cell::new(0);
    let updated = update(&fixture, None, &copying(&tree, &calls));
    assert_eq!(updated.code(), "complete", "{}", updated.rendered);
    assert_eq!(calls.get(), 1);
    assert!(updated.report.activated && !updated.report.reused);
    assert!(updated.report.cleanup_pending.is_none());
    assert_eq!(updated.rendered["result"]["upToDate"], false);
    assert_eq!(
        pointer(&root).unwrap().active,
        entry_for("0.2.0", &newer_manifest)
    );
    assert!(!root.join("versions/0.2.0/unlisted.exe").exists());
    assert_eq!(names(&folder), ["download.lock"]);

    // A stored version that is not active is activated from the store, without the archive.
    install(&fixture, &older, true);
    fs::remove_file(fixture.0.join("feed").join(ARCHIVE)).unwrap();
    let stored = update(&fixture, None, &never_unpack);
    assert_eq!(stored.code(), "complete", "{}", stored.rendered);
    assert!(stored.report.reused && stored.report.activated);
    assert_eq!(
        pointer(&root).unwrap().active,
        entry_for("0.2.0", &newer_manifest)
    );
    assert_eq!(names(&folder), ["download.lock"]);
}

#[test]
fn a_refused_archive_changes_no_version_and_leaves_only_the_lock() {
    let fixture = Fixture::new();
    let root = fixture.0.join("install");
    let (older, older_manifest) = release(&fixture, "older", "0.1.0", "test");
    let (newer, newer_manifest) = release(&fixture, "newer", "0.2.0", "test");
    install(&fixture, &older, false);
    let active = entry_for("0.1.0", &older_manifest);
    let folder = root.join(archive::FOLDER);
    // The inner release disagrees with the feed: same version, another build.
    let (rebuilt, _) = release(&fixture, "rebuilt", "0.2.0", "rebuilt");
    let empty = fixture.0.join("empty");
    fs::create_dir(&empty).unwrap();

    let archive = b"the archive bytes the feed signs".to_vec();
    let mut changed = archive.clone();
    changed[3] ^= 1;
    let mut longer = archive.clone();
    longer.push(b'!');
    let failing = |unpacked: &Path, context: &mut Context| {
        Err(context.storage(unpacked, &io::Error::other("unpack failed")))
    };
    // The case, the bytes served as the archive, the tree the unpack step copies (none: it fails),
    // whether the unpack step runs, and the error code.
    type Case<'a> = (&'a str, &'a [u8], Option<&'a Path>, bool, &'a str);
    let cases: [Case<'_>; 6] = [
        (
            "changed bytes (U1)",
            &changed,
            Some(&newer),
            false,
            "checksum",
        ),
        (
            "cut short (U1)",
            &archive[..10],
            Some(&newer),
            false,
            "checksum",
        ),
        (
            "longer than signed (U1)",
            &longer,
            Some(&newer),
            false,
            "checksum",
        ),
        ("failed unpack (U5)", &archive, None, true, "storage"),
        (
            "inner manifest disagrees",
            &archive,
            Some(&rebuilt),
            true,
            "manifest-invalid",
        ),
        (
            "not a release directory",
            &archive,
            Some(&empty),
            true,
            "manifest-invalid",
        ),
    ];
    offer(&fixture, "0.2.0", &newer_manifest, &archive, |_| {});
    for (name, served, tree, unpacked, expected) in cases {
        serve(&fixture, served);
        let calls = Cell::new(0);
        let run = match tree {
            Some(tree) => update(&fixture, None, &copying(tree, &calls)),
            None => update(&fixture, None, &|unpacked: &Path, context: &mut Context| {
                calls.set(calls.get() + 1);
                failing(unpacked, context)
            }),
        };
        assert_eq!(run.code(), expected, "{name}: {}", run.rendered);
        assert_eq!(calls.get(), usize::from(unpacked), "{name}");
        assert!(
            run.report.published.is_none() && !run.report.activated,
            "{name}"
        );
        assert!(run.report.cleanup_pending.is_none(), "{name}");
        assert_eq!(names(&root.join("versions")), ["0.1.0"], "{name}");
        assert_eq!(pointer(&root).unwrap().active, active, "{name}");
        assert_eq!(names(&folder), ["download.lock"], "{name}");
    }

    // Another update holds the download folder: busy before the archive is requested.
    serve(&fixture, &archive);
    let held = PrivateCache::acquire(&folder, "download").unwrap();
    let busy = update(&fixture, None, &never_unpack);
    assert_eq!(busy.code(), "busy");
    drop(held);
    assert_eq!(names(&root.join("versions")), ["0.1.0"]);
    assert_eq!(pointer(&root).unwrap().active, active);
}

#[test]
fn the_system_tar_unpacks_the_one_top_folder_from_fixed_arguments_and_refuses_a_bad_archive() {
    let fixture = Fixture::new();
    // A store root with a space, a Chinese character and a character outside the system code page
    // (rule U3): no part of it is ever an argument.
    let folder = fixture.0.join("store 中 \u{1200}").join(archive::FOLDER);
    let unpacked = folder.join("unpacked");
    fs::create_dir_all(&unpacked).unwrap();

    let command = archive::unpack_command(&unpacked).unwrap();
    let program = Path::new(command.get_program());
    assert!(program.is_absolute(), "{program:?}");
    assert!(
        program.ends_with(r"System32\tar.exe") || program.ends_with(r"system32\tar.exe"),
        "{program:?}"
    );
    let arguments: Vec<_> = command.get_args().collect();
    assert_eq!(
        arguments,
        ["-x", "-f", r"..\archive.zip", "--strip-components", "1"]
    );
    let environment: Vec<_> = command.get_envs().map(|(name, _)| name).collect();
    assert_eq!(environment, ["SystemRoot"]);
    assert_eq!(command.get_current_dir(), Some(unpacked.as_path()));

    // A zip whose one top folder is the release directory unpacks into `unpacked` directly.
    let packed = fixture.0.join("pack");
    fs::create_dir_all(packed.join("orchbee-0.2.0/sub")).unwrap();
    fs::write(packed.join("orchbee-0.2.0/release.json"), b"{}").unwrap();
    fs::write(packed.join("orchbee-0.2.0/sub/file.txt"), b"inside").unwrap();
    let zip = std::process::Command::new(command.get_program())
        .current_dir(&packed)
        .args(["-a", "-c", "-f", "archive.zip", "orchbee-0.2.0"])
        .output()
        .unwrap();
    assert!(zip.status.success(), "{zip:?}");
    fs::copy(packed.join("archive.zip"), folder.join("archive.zip")).unwrap();
    let mut context = context();
    let unpack = archive::system_unpack(&unpacked, &mut context);
    assert_eq!(unpack.err().map(Error::code), None);
    assert_eq!(names(&unpacked), ["release.json", "sub"]);
    assert_eq!(fs::read(unpacked.join("sub/file.txt")).unwrap(), b"inside");

    // Bytes that are not an archive fail the unpack as storage (rule U5).
    fs::remove_dir_all(&unpacked).unwrap();
    fs::create_dir(&unpacked).unwrap();
    fs::write(folder.join("archive.zip"), b"not an archive").unwrap();
    assert_eq!(
        archive::system_unpack(&unpacked, &mut context)
            .err()
            .map(Error::code),
        Some("storage")
    );
}
