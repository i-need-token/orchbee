//! Real Windows offline schema publication, including refusal and replacement boundaries.
#![cfg(windows)]
use serde_json::Value;
use std::{
    fs,
    os::windows::fs::OpenOptionsExt,
    path::PathBuf,
    process::{Command, Output, Stdio},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!(
            "orchbee-schema-export-{:032x}",
            u128::from_le_bytes(nonce)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("invalid-runtime"),
            b"must never be opened as a runtime",
        )
        .unwrap();
        Self(root)
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(crate::staged_client::client());
        command
            .args(args)
            .current_dir(&self.0)
            .env("ORCHBEE_RUNTIME_BASE", self.0.join("invalid-runtime"))
            .env("USERPROFILE", self.0.join("profile"))
            .env("LC_ALL", "en_US.UTF-8")
            .stdin(Stdio::null());
        command
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }
    fn no_temporary(&self) {
        assert!(!fs::read_dir(&self.0).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".orchbee-schema-")
        }));
        assert_eq!(
            fs::read(self.0.join("invalid-runtime")).unwrap(),
            b"must never be opened as a runtime"
        );
    }
    fn failure(&self, output: &Output, stage: &str, path: &str) {
        assert_eq!(
            output.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        let schema: Value = serde_json::from_str(include_str!(
            "../src/cli/schemas/orchbee.cli.failure.schema-export.json"
        ))
        .unwrap();
        jsonschema::validator_for(&schema)
            .unwrap()
            .validate(&document)
            .unwrap();
        assert_eq!(document["failure"]["details"]["stage"], stage);
        assert_eq!(document["failure"]["details"]["path"], path);
        assert!(document["failure"]["details"]["temporaryPath"].is_null());
        self.no_temporary();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let path = self.0.canonicalize().unwrap();
        assert!(path.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-schema-export-")
        );
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn exports_and_replaces_catalog_and_selected_schema_offline() {
    let fixture = Fixture::new();
    for id in [None, Some("orchbee.client.control")] {
        let mut args = vec!["api", "schema", "--format=json"];
        if let Some(id) = id {
            args.push(id);
        }
        let expected = fixture.run(&args);
        assert!(expected.status.success());
        let path = "用户 schema.json";
        fs::write(fixture.0.join(path), b"original destination").unwrap();
        args.extend(["--output", path]);
        let actual = fixture.run(&args);
        assert!(
            actual.status.success(),
            "{}",
            String::from_utf8_lossy(&actual.stderr)
        );
        assert_eq!(actual.stdout, expected.stdout);
        assert_eq!(fs::read(fixture.0.join(path)).unwrap(), actual.stdout);
        assert!(actual.stderr.is_empty());
        fixture.no_temporary();
    }
    let path = fixture.0.join("new schema.json");
    let output = fixture.run(&[
        "api",
        "schema",
        "orchbee.cli.failure.schema-export",
        "--output",
        path.to_str().unwrap(),
    ]);
    assert!(output.status.success());
    assert_eq!(fs::read(path).unwrap(), output.stdout);
    fixture.no_temporary();
}

#[test]
fn invalid_grammar_and_unknown_schema_have_no_file_effect() {
    let fixture = Fixture::new();
    let cases: &[&[&str]] = &[
        &["api", "schema", "--output"],
        &["api", "schema", "--output", ""],
        &["api", "schema", "--output", "NUL.json"],
        &["api", "schema", "--output", "C:relative.json"],
        &["api", "schema", "--output", "x:stream"],
        &["api", "schema", "--output", "folder/."],
        &["api", "schema", "--output", "a.json", "--output", "b.json"],
        &["api", "schema", "--output", "a.json", "--current"],
        &["api", "snapshot", "--output", "a.json"],
    ];
    for args in cases {
        let mut arguments = vec!["--format=json"];
        arguments.extend_from_slice(args);
        let args = arguments;
        let output = fixture.run(&args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(document["failure"]["kind"], "invalid-usage");
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }
    let output = fixture.run(&[
        "api",
        "schema",
        "no-such-schema",
        "--output",
        "not-created.json",
        "--format=json",
    ]);
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    fixture.no_temporary();
}

#[test]
fn missing_parent_directory_and_locked_target_preserve_existing_data() {
    let fixture = Fixture::new();
    let output = fixture.run(&[
        "api",
        "schema",
        "--output",
        "missing/schema.json",
        "--format=json",
    ]);
    fixture.failure(&output, "create-temporary", "missing/schema.json");
    assert!(!fixture.0.join("missing").exists());
    fs::create_dir(fixture.0.join("directory")).unwrap();
    let output = fixture.run(&["api", "schema", "--output", "directory", "--format=json"]);
    fixture.failure(&output, "destination", "directory");
    let chinese = fixture
        .command(&["api", "schema", "--output", "directory", "--format=json"])
        .env("LC_ALL", "zh_CN.UTF-8")
        .output()
        .unwrap();
    fixture.failure(&chinese, "destination", "directory");
    assert_eq!(chinese.stdout, output.stdout);
    assert!(String::from_utf8_lossy(&chinese.stderr).contains("导出 schema"));
    assert_ne!(chinese.stderr, output.stderr);

    assert!(fixture.0.join("directory").is_dir());
    let path = fixture.0.join("locked.json");
    fs::write(&path, b"original bytes").unwrap();
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&path)
        .unwrap();
    let output = fixture.run(&["api", "schema", "--output", "locked.json", "--format=json"]);
    fixture.failure(&output, "publish", "locked.json");
    drop(lock);
    assert_eq!(fs::read(path).unwrap(), b"original bytes");
    let output = fixture.run(&["api", "schema", "--output", "directory"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
    fixture.no_temporary();
}

#[test]
fn stdout_failure_keeps_the_already_published_schema() {
    let fixture = Fixture::new();
    let readonly = fixture.0.join("readonly-output");
    fs::write(&readonly, b"untouched").unwrap();
    let output = fixture
        .command(&[
            "api",
            "schema",
            "orchbee.client.view",
            "--output",
            "published.json",
        ])
        .stdout(fs::File::open(&readonly).unwrap())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!output.stderr.is_empty());
    let document: Value =
        serde_json::from_slice(&fs::read(fixture.0.join("published.json")).unwrap()).unwrap();
    assert_eq!(
        document["result"]["schema"]["schemaId"],
        "orchbee.client.view"
    );
    assert_eq!(fs::read(readonly).unwrap(), b"untouched");
    fixture.no_temporary();
}
