//! `server diagnose` coverage at the function boundary: every row
//! mapping, the conditional advisories, the directory-failure
//! classification, and real scratch-runtime collections — including
//! the no-creation invariant and a spent budget.
use super::*;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;

fn scratch(tag: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "obcli-site-diagnose-{}-{tag}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir(&directory).expect("create scratch base");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("chmod scratch base");
    directory
}

fn present(
    kind: EntryKind,
    owner: bool,
    mode: u32,
    size: u64,
    links: u64,
) -> io::Result<EntryObservation> {
    Ok(EntryObservation::Present {
        kind,
        owner_matches: owner,
        mode,
        size,
        link_count: links,
    })
}

fn state(row: &Map<String, Value>) -> &str {
    row["state"].as_str().unwrap_or("")
}

fn reason(row: &Map<String, Value>) -> Option<&str> {
    row.get("reason").and_then(Value::as_str)
}

#[test]
fn entry_row_maps_the_closed_state_and_reason_vocabulary() {
    assert_eq!(
        state(&entry_row(Ok(EntryObservation::Absent), Expect::Socket)),
        "absent"
    );
    assert_eq!(
        state(&entry_row(
            present(EntryKind::Socket, true, 0o600, 0, 1),
            Expect::Socket
        )),
        "known"
    );
    // Every admissible deviation reports its own reason — never guessed.
    for (observation, expected) in [
        (
            present(EntryKind::Symlink, true, 0o777, 0, 1),
            Some("symlink"),
        ),
        (
            present(EntryKind::RegularFile, true, 0o600, 0, 1),
            Some("invalid-kind"),
        ),
        (
            present(EntryKind::Socket, false, 0o600, 0, 1),
            Some("foreign-owner"),
        ),
        (
            present(EntryKind::Socket, true, 0o644, 0, 1),
            Some("invalid-mode"),
        ),
    ] {
        let row = entry_row(observation, Expect::Socket);
        assert_eq!(state(&row), "unsafe");
        assert_eq!(reason(&row), expected);
    }
    assert_eq!(
        reason(&entry_row(
            Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            Expect::Socket
        )),
        Some("denied")
    );
    assert_eq!(
        reason(&entry_row(
            Err(io::Error::from(io::ErrorKind::InvalidData)),
            Expect::Socket
        )),
        Some("io-error")
    );
}

#[test]
fn entry_row_applies_marker_and_lock_strictness() {
    // A hard-linked lock file or a non-empty marker is a fully
    // reported unsafe fact — the predicate treats it the same way.
    let hard_linked = entry_row(
        present(EntryKind::RegularFile, true, 0o600, 0, 2),
        Expect::RegularFile,
    );
    assert_eq!(state(&hard_linked), "unsafe");
    assert_eq!(reason(&hard_linked), Some("invalid-link-count"));

    let non_empty_marker = entry_row(
        present(EntryKind::RegularFile, true, 0o600, 4, 1),
        Expect::MarkerFile,
    );
    assert_eq!(state(&non_empty_marker), "unsafe");
    assert_eq!(reason(&non_empty_marker), Some("invalid-size"));

    let valid_marker = entry_row(
        present(EntryKind::RegularFile, true, 0o600, 0, 1),
        Expect::MarkerFile,
    );
    assert_eq!(state(&valid_marker), "known");
}

#[test]
fn lifetime_row_reports_lock_facts_not_liveness() {
    assert_eq!(
        state(&lifetime_row(Ok(SiteLifetimeObservation::Absent))),
        "absent"
    );
    assert_eq!(
        state(&lifetime_row(Ok(SiteLifetimeObservation::Free))),
        "free"
    );
    assert_eq!(
        state(&lifetime_row(Ok(SiteLifetimeObservation::Held))),
        "held"
    );
    let unsafe_row = lifetime_row(Ok(SiteLifetimeObservation::Unsafe));
    assert_eq!(state(&unsafe_row), "unsafe");
    assert_eq!(reason(&unsafe_row), Some("invalid-lock"));
    assert_eq!(
        reason(&lifetime_row(Err(io::Error::other("io")))),
        Some("io-error")
    );
}

#[test]
fn directory_failures_classify_without_becoming_engine_failures() {
    let path = PathBuf::from("/ob-diagnose-test");
    let cases: &[(RendezvousError, &str, Option<&str>, bool)] = &[
        (
            RendezvousError::System {
                operation: "open",
                path: path.clone(),
                source: io::Error::from(io::ErrorKind::NotFound),
            },
            "absent",
            None,
            false,
        ),
        (
            RendezvousError::DirectorySymlink { path: path.clone() },
            "unsafe",
            Some("symlink"),
            true,
        ),
        (
            RendezvousError::UnexpectedOwner {
                path: path.clone(),
                expected: 1000,
                actual: 0,
            },
            "unsafe",
            Some("foreign-owner"),
            true,
        ),
        (
            RendezvousError::UnexpectedMode {
                path: path.clone(),
                expected: 0o700,
                actual: 0o777,
            },
            "unsafe",
            Some("invalid-mode"),
            true,
        ),
        (
            RendezvousError::NotDirectory { path: path.clone() },
            "unsafe",
            Some("invalid-kind"),
            true,
        ),
        (
            RendezvousError::RuntimeBaseNotAbsolute { path: path.clone() },
            "uninspectable",
            Some("invalid-path"),
            false,
        ),
        (
            RendezvousError::RuntimeDirectoryIdentityChanged { path: path.clone() },
            "unsafe",
            Some("identity-changed"),
            true,
        ),
        (
            RendezvousError::UnexpectedLockLinkCount {
                kind: LockKind::Activation,
                path: path.clone(),
                actual: 2,
            },
            "unsafe",
            Some("invalid-link-count"),
            true,
        ),
        (
            RendezvousError::LockFileSymlink {
                kind: LockKind::Activation,
                path: path.clone(),
            },
            "unsafe",
            Some("symlink"),
            true,
        ),
    ];
    for (error, expected_state, expected_reason, expected_unsafe) in cases {
        let (row, row_state_value, row_reason_value, parent_unsafe) = classify_directory(error);
        assert_eq!(state(&row), *expected_state, "{error:?}");
        assert_eq!(reason(&row), *expected_reason, "{error:?}");
        assert_eq!(row_state_value, *expected_state, "{error:?}");
        assert_eq!(row_reason_value, *expected_reason, "{error:?}");
        assert_eq!(parent_unsafe, *expected_unsafe, "{error:?}");
    }
}

#[test]
fn advisories_fire_only_on_the_predicates_own_facts() {
    assert!(advisories("absent", "absent", "absent").is_empty());
    assert_eq!(
        advisories("known", "absent", "absent"),
        vec![ADVISORY_ACTIVATION_RECORDED]
    );
    assert_eq!(
        advisories("absent", "known", "absent"),
        vec![ADVISORY_SOCKET_RESIDUE]
    );
    assert_eq!(
        advisories("absent", "absent", "known"),
        vec![ADVISORY_SOCKET_RESIDUE]
    );
    assert_eq!(
        advisories("known", "known", "absent"),
        vec![ADVISORY_ACTIVATION_RECORDED, ADVISORY_SOCKET_RESIDUE]
    );
    // Unsafe or uninspectable residue stays a fact row — no advisory.
    assert!(advisories("unsafe", "unsafe", "uninspectable").is_empty());
    assert!(advisories("uninspectable", "absent", "absent").is_empty());
}

fn names(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .expect("list scratch")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

fn scope_name() -> String {
    format!("orchbee-native-{}", nix::unistd::geteuid().as_raw())
}

#[test]
fn collect_at_reports_every_row_and_creates_nothing() {
    let base = scratch("populated");
    let scope_text = scope_name();
    let site = ModeCheckedLocalSiteRendezvous::prepare_mode_checked(
        &base,
        SiteScope::parse(&scope_text).expect("scope"),
    )
    .expect("prepare scratch runtime");
    site.commit_activation().expect("commit marker");
    let _listener =
        UnixListener::bind(site.runtime_directory().join("client.sock")).expect("bind client leaf");
    fs::set_permissions(
        site.runtime_directory().join("client.sock"),
        fs::Permissions::from_mode(0o600),
    )
    .expect("chmod client leaf");
    let before = names(&base)
        .into_iter()
        .chain(names(site.runtime_directory()))
        .collect::<Vec<_>>();

    let document =
        collect_at(&base, Instant::now() + Duration::from_secs(5)).expect("collect populated");
    let result = &document.envelope["result"];
    assert_eq!(
        document.envelope["schema"],
        json!("orchbee.server.diagnose")
    );
    assert_eq!(document.envelope["version"], json!(1));
    assert_eq!(document.envelope["source"]["kind"], json!("local-runtime"));
    assert_eq!(result["complete"], json!(true));
    assert_eq!(result["scope"], json!(scope_text));
    assert_eq!(result["runtimeBase"]["state"], json!("known"));
    assert_eq!(result["runtimeDirectory"]["state"], json!("known"));
    assert_eq!(result["marker"]["state"], json!("known"));
    assert_eq!(result["marker"]["kind"], json!("regular"));
    assert_eq!(result["locks"]["activation"]["state"], json!("absent"));
    assert_eq!(result["locks"]["sessionLifetime"]["state"], json!("absent"));
    assert_eq!(result["endpoints"]["client"]["state"], json!("known"));
    assert_eq!(result["endpoints"]["worker"]["state"], json!("absent"));
    assert_eq!(
        result["endpoints"]["agentObservation"]["state"],
        json!("absent")
    );
    assert_eq!(
        result["endpoints"]["hostEndpoint"]["state"],
        json!("absent")
    );
    assert_eq!(result["hostScope"]["state"], json!("absent"));
    assert_eq!(
        result["advisories"],
        json!([ADVISORY_ACTIVATION_RECORDED, ADVISORY_SOCKET_RESIDUE])
    );
    // The observation is path-only: the runtime holds exactly the
    // leaves the setup itself created.
    let mut after = names(&base);
    after.extend(names(site.runtime_directory()));
    assert_eq!(after, before);
    fs::remove_dir_all(&base).ok();
}

#[test]
fn collect_at_closes_missing_and_unsafe_directories_without_creating() {
    let base = scratch("missing");
    let document =
        collect_at(&base, Instant::now() + Duration::from_secs(5)).expect("collect missing");
    let result = &document.envelope["result"];
    assert_eq!(result["complete"], json!(true));
    assert_eq!(result["runtimeDirectory"]["state"], json!("absent"));
    assert_eq!(result["marker"]["state"], json!("uninspectable"));
    assert_eq!(result["marker"]["reason"], json!("parent-unavailable"));
    assert_eq!(
        result["locks"]["sessionLifetime"]["state"],
        json!("uninspectable")
    );
    assert_eq!(
        result["locks"]["sessionLifetime"]["reason"],
        json!("parent-unavailable")
    );
    assert_eq!(
        result["endpoints"]["client"]["state"],
        json!("uninspectable")
    );
    assert_eq!(result["advisories"], json!([]));

    // An unsafe scope directory propagates unsafe-parent to descendants.
    let linked_base = scratch("unsafe");
    let scope_text = scope_name();
    let real_dir = linked_base.join("real");
    fs::create_dir(&real_dir).expect("real dir");
    std::os::unix::fs::symlink(&real_dir, linked_base.join(&scope_text)).expect("link scope");
    let document =
        collect_at(&linked_base, Instant::now() + Duration::from_secs(5)).expect("collect unsafe");
    let result = &document.envelope["result"];
    assert_eq!(result["runtimeDirectory"]["state"], json!("unsafe"));
    assert_eq!(result["runtimeDirectory"]["reason"], json!("symlink"));
    assert_eq!(result["marker"]["state"], json!("uninspectable"));
    assert_eq!(result["marker"]["reason"], json!("unsafe-parent"));
    assert_eq!(
        result["endpoints"]["hostEndpoint"]["reason"],
        json!("unsafe-parent")
    );
    fs::remove_dir_all(&base).ok();
    fs::remove_dir_all(&linked_base).ok();
}

#[test]
fn collect_at_marks_a_held_lifetime_lock_without_acquiring_it() {
    let base = scratch("held");
    let scope_text = scope_name();
    let site = ModeCheckedLocalSiteRendezvous::prepare_mode_checked(
        &base,
        SiteScope::parse(&scope_text).expect("scope"),
    )
    .expect("prepare scratch runtime");
    let held = match site.try_acquire_site_lifetime_lock().expect("acquire") {
        LockAttempt::Acquired(lock) => lock,
        LockAttempt::Contended => panic!("fresh lock contended"),
    };

    let document =
        collect_at(&base, Instant::now() + Duration::from_secs(5)).expect("collect held");
    assert_eq!(
        document.envelope["result"]["locks"]["sessionLifetime"]["state"],
        json!("held")
    );
    drop(held);
    let document =
        collect_at(&base, Instant::now() + Duration::from_secs(5)).expect("collect free");
    assert_eq!(
        document.envelope["result"]["locks"]["sessionLifetime"]["state"],
        json!("free")
    );
    fs::remove_dir_all(&base).ok();
}

#[test]
fn collect_at_with_a_spent_budget_closes_every_later_row() {
    let base = scratch("deadline");
    let scope_text = scope_name();
    ModeCheckedLocalSiteRendezvous::prepare_mode_checked(
        &base,
        SiteScope::parse(&scope_text).expect("scope"),
    )
    .expect("prepare scratch runtime");
    let past = Instant::now() - Duration::from_secs(1);
    let document = collect_at(&base, past).expect("collect deadline");
    let result = &document.envelope["result"];
    assert_eq!(result["complete"], json!(false));
    for (name, row) in &document.rows {
        if name == "runtimeBase" || name == "runtimeDirectory" {
            assert_eq!(row.get("state"), Some(&json!("known")), "{name}");
            continue;
        }
        if name == "locks" || name == "endpoints" {
            for (_inner, nested) in row {
                assert_eq!(nested["state"], json!("uninspectable"), "{name}");
                assert_eq!(nested["reason"], json!("deadline"), "{name}");
            }
            continue;
        }
        assert_eq!(row.get("state"), Some(&json!("uninspectable")), "{name}");
        assert_eq!(row.get("reason"), Some(&json!("deadline")), "{name}");
    }
    fs::remove_dir_all(&base).ok();
}

#[test]
fn emitted_documents_validate_against_the_embedded_schema() {
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.server.diagnose.json"))
            .expect("schema parses");
    let validator = jsonschema::draft202012::new(&schema).expect("schema compiles");

    let populated = scratch("schema-populated");
    let scope_text = scope_name();
    let site = ModeCheckedLocalSiteRendezvous::prepare_mode_checked(
        &populated,
        SiteScope::parse(&scope_text).expect("scope"),
    )
    .expect("prepare");
    site.commit_activation().expect("marker");
    let document =
        collect_at(&populated, Instant::now() + Duration::from_secs(5)).expect("collect");
    assert!(
        validator.validate(&document.envelope).is_ok(),
        "populated document conforms"
    );

    let missing = scratch("schema-missing");
    let document = collect_at(&missing, Instant::now() + Duration::from_secs(5)).expect("collect");
    assert!(
        validator.validate(&document.envelope).is_ok(),
        "absent-runtime document conforms"
    );
    fs::remove_dir_all(&populated).ok();
    fs::remove_dir_all(&missing).ok();
}
