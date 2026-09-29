//! `config check`: pure local inspection. No socket is connected, no process
//! is started, no directory is created, and no lock is taken.

#[cfg(any(unix, windows))]
use std::path::Path;
#[cfg(all(test, target_os = "linux"))]
use std::path::PathBuf;
#[cfg(any(target_os = "linux", windows))]
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use super::{Failure, FailureKind, Output, failure_output, output};
use crate::invocation::OutputFormat;
use crate::localization::Locale;

#[cfg(any(target_os = "linux", windows))]
use orchbee_local_client_protocol::{UNIX_SOCKET_PATH_CAPACITY, unix_socket_path_fits};
#[cfg(any(target_os = "linux", windows))]
use orchbee_local_site_runtime::{
    ModeCheckedLocalSiteRendezvous, NativeCohort, RendezvousError, SiteScope,
};

#[cfg(any(target_os = "linux", windows))]
const CHECK_BUDGET: Duration = Duration::from_secs(5);

pub(crate) struct CheckReport {
    pub(crate) name: &'static str,
    pub(crate) status: &'static str,
    pub(crate) details: Map<String, Value>,
}

#[cfg(any(target_os = "linux", windows))]
fn pass(name: &'static str, details: Map<String, Value>) -> CheckReport {
    CheckReport {
        name,
        status: "pass",
        details,
    }
}

#[cfg(any(target_os = "linux", windows))]
fn fail(name: &'static str, details: Map<String, Value>) -> CheckReport {
    CheckReport {
        name,
        status: "fail",
        details,
    }
}

#[cfg(any(target_os = "linux", windows))]
fn not_applicable(name: &'static str, details: Map<String, Value>) -> CheckReport {
    CheckReport {
        name,
        status: "not-applicable",
        details,
    }
}

#[cfg(any(target_os = "linux", windows))]
fn detail(pairs: impl IntoIterator<Item = (String, Value)>) -> Map<String, Value> {
    Map::from_iter(pairs)
}

/// A filesystem path as a machine field: UTF-8 paths are `path`, others are
/// `pathBytesHex`; never a lossy display string.
#[cfg(unix)]
pub(crate) fn path_detail(path: &Path) -> (String, Value) {
    match path.to_str() {
        Some(text) => ("path".to_owned(), json!(text)),
        None => {
            use std::os::unix::ffi::OsStrExt;
            let hex: String =
                path.as_os_str()
                    .as_bytes()
                    .iter()
                    .fold(String::new(), |mut output, byte| {
                        use std::fmt::Write;
                        let _ = write!(output, "{byte:02x}");
                        output
                    });
            ("pathBytesHex".to_owned(), json!(hex))
        }
    }
}

pub(crate) fn local_runtime_source() -> Value {
    json!({ "kind": "local-runtime" })
}

pub(crate) fn check(format: OutputFormat, locale: Locale) -> Output {
    match run_checks() {
        Err(failure) => failure_output(failure, Some(format), locale),
        Ok(checks) => {
            let failed = checks.iter().any(|check| check.status == "fail");
            let result = json!({
                "ok": !failed,
                "checks": checks
                    .iter()
                    .map(|check| {
                        json!({
                            "name": check.name,
                            "status": check.status,
                            "details": check.details,
                        })
                    })
                    .collect::<Vec<_>>(),
            });
            let stderr = if failed {
                locale.cli_config_failed().to_owned()
            } else {
                String::new()
            };
            // A preparation overflow answers in the caller's format, never
            // with a second stdout document.
            let too_large = || {
                failure_output(
                    Failure::new(FailureKind::OutputTooLarge).details(Map::from_iter([(
                        "limitBytes".to_owned(),
                        json!(output::LIMIT),
                    )])),
                    Some(format),
                    locale,
                )
            };
            match format {
                OutputFormat::Json => {
                    let document = json!({
                        "schema": "orchbee.config.check",
                        "version": if cfg!(windows) { 9 } else { 1 },
                        "source": local_runtime_source(),
                        "result": result,
                    });
                    match output::encode_document(&document) {
                        Ok(stdout) => Output {
                            status: if failed { 1 } else { 0 },
                            stdout,
                            stderr,
                            finalization: None,
                        },
                        Err(output::TooLarge) => too_large(),
                    }
                }
                OutputFormat::Text => {
                    match output::render_text(|out| {
                        super::present::config_checks(out, &checks, locale)
                    }) {
                        Ok(stdout) => Output {
                            status: if failed { 1 } else { 0 },
                            stdout,
                            stderr,
                            finalization: None,
                        },
                        Err(output::TooLarge) => too_large(),
                    }
                }
            }
        }
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
fn run_checks() -> Result<Vec<CheckReport>, Failure> {
    Err(
        Failure::new(FailureKind::CheckFailed).details(super::stage_reason_details(
            "platform",
            "unsupported-platform",
        )),
    )
}

#[cfg(any(target_os = "linux", windows))]
fn run_checks() -> Result<Vec<CheckReport>, Failure> {
    let (base, scope_text) = crate::local_activation::runtime_location()
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let scope =
        SiteScope::parse(&scope_text).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let scope_directory = base.join(&scope_text);
    let site = ModeCheckedLocalSiteRendezvous::validate_mode_checked(&base, scope);

    let mut checks = Vec::with_capacity(5);
    let (base_check, directory_check) = classify_paths(&site, &base, &scope_directory);
    checks.push(base_check);
    checks.push(directory_check);

    // The cohort check reuses the current-cohort verifier with its own
    // independent five-second budget.
    let deadline = Instant::now()
        .checked_add(CHECK_BUDGET)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    checks.push(cohort_report(NativeCohort::current(deadline)));

    checks.push(locale_check());
    checks.push(socket_checks(&site, &scope_directory));
    #[cfg(windows)]
    checks.push(match super::configuration::load() {
        Ok(settings) => pass("user-config", settings.check_details()),
        Err(error) => fail(
            "user-config",
            Map::from_iter([
                ("field".into(), json!(error.field)),
                ("reason".into(), json!(error.reason)),
            ]),
        ),
    });
    Ok(checks)
}

#[cfg(any(target_os = "linux", windows))]
fn classify_paths(
    site: &Result<ModeCheckedLocalSiteRendezvous, RendezvousError>,
    base: &Path,
    scope_directory: &Path,
) -> (CheckReport, CheckReport) {
    match site {
        Ok(_) => (
            pass(
                "runtime-base",
                detail([path_detail(base), ("exists".to_owned(), json!(true))]),
            ),
            pass(
                "runtime-directory",
                detail([
                    path_detail(scope_directory),
                    ("exists".to_owned(), json!(true)),
                ]),
            ),
        ),
        Err(error) => {
            let path = rendezvous_error_path(error);
            let scoped = path == Some(scope_directory);
            let absent = matches!(
                error,
                RendezvousError::System { source, .. }
                    if source.kind() == std::io::ErrorKind::NotFound
            );
            // `exists` appears only when inspection established the path
            // state: `false` for an established absence, `true` for an
            // observed entry. A permission or I/O failure — or a path that is
            // not even valid — omits the field instead of guessing.
            let established = match error {
                RendezvousError::System { source, .. }
                    if source.kind() == std::io::ErrorKind::NotFound =>
                {
                    Some(false)
                }
                RendezvousError::System { .. }
                | RendezvousError::RuntimeBaseNotAbsolute { .. }
                | RendezvousError::RuntimeBaseNotNormalized { .. }
                | RendezvousError::InvalidSiteScope => None,
                _ => Some(true),
            };
            let invalid_path = matches!(
                error,
                RendezvousError::RuntimeBaseNotAbsolute { .. }
                    | RendezvousError::RuntimeBaseNotNormalized { .. }
                    | RendezvousError::InvalidSiteScope
            );
            let reason = if invalid_path {
                "invalid-path"
            } else if absent {
                "absent"
            } else if matches!(error, RendezvousError::System { .. }) {
                "io-error"
            } else {
                "unsafe"
            };
            let base_pass = || {
                pass(
                    "runtime-base",
                    detail([path_detail(base), ("exists".to_owned(), json!(true))]),
                )
            };
            let path_failure = |name: &'static str, target: &Path| {
                let mut details = detail([("reason".to_owned(), json!(reason))]);
                if let Some(exists) = established {
                    details.insert("exists".to_owned(), json!(exists));
                }
                let (key, value) = path_detail(target);
                details.insert(key, value);
                fail(name, details)
            };
            match (scoped, absent) {
                // A missing Site scope under a healthy base is not-applicable:
                // no running Site is a legal state, not a configuration fault.
                (true, true) => (
                    base_pass(),
                    not_applicable(
                        "runtime-directory",
                        detail([
                            path_detail(scope_directory),
                            ("exists".to_owned(), json!(false)),
                            ("reason".to_owned(), json!("absent")),
                        ]),
                    ),
                ),
                (true, false) => (
                    base_pass(),
                    path_failure("runtime-directory", scope_directory),
                ),
                (false, _) => (
                    path_failure("runtime-base", base),
                    not_applicable("runtime-directory", Map::new()),
                ),
            }
        }
    }
}

/// The path one `RendezvousError` names, when it names one — shared with
/// `site diagnose`, which classifies the same error family onto rows.
#[cfg(any(target_os = "linux", windows))]
pub(crate) fn rendezvous_error_path(error: &RendezvousError) -> Option<&Path> {
    match error {
        RendezvousError::InvalidSiteScope => None,
        RendezvousError::RuntimeBaseNotAbsolute { path }
        | RendezvousError::RuntimeBaseNotNormalized { path }
        | RendezvousError::DirectorySymlink { path }
        | RendezvousError::NotDirectory { path }
        | RendezvousError::NotRegularLockFile { path, .. }
        | RendezvousError::UnexpectedOwner { path, .. }
        | RendezvousError::UnexpectedMode { path, .. }
        | RendezvousError::UnexpectedLockLinkCount { path, .. }
        | RendezvousError::RuntimeDirectoryIdentityChanged { path }
        | RendezvousError::LockFileSymlink { path, .. }
        | RendezvousError::System { path, .. } => Some(path.as_path()),
    }
}

/// The cohort check reports only what the verifier established: `verified`
/// plus `reason` `deadline`/`invalid-cohort` on failure.
#[cfg(any(target_os = "linux", windows))]
fn cohort_report(result: std::io::Result<NativeCohort>) -> CheckReport {
    match result {
        Ok(_) => pass("cohort", detail([("verified".to_owned(), json!(true))])),
        Err(error) => {
            let reason = if error.kind() == std::io::ErrorKind::TimedOut {
                "deadline"
            } else {
                "invalid-cohort"
            };
            fail(
                "cohort",
                detail([
                    ("verified".to_owned(), json!(false)),
                    ("reason".to_owned(), json!(reason)),
                ]),
            )
        }
    }
}

#[cfg(any(target_os = "linux", windows))]
fn locale_check() -> CheckReport {
    let (locale, source) = crate::localization::locale_source();
    pass(
        "locale",
        detail([
            ("locale".to_owned(), json!(locale)),
            ("source".to_owned(), json!(source)),
        ]),
    )
}

/// `sockets` reports presence and the derived address budget; presence never
/// means a live service.
#[cfg(any(target_os = "linux", windows))]
fn socket_checks(
    site: &Result<ModeCheckedLocalSiteRendezvous, RendezvousError>,
    scope_directory: &Path,
) -> CheckReport {
    let runtime = site
        .as_ref()
        .map_or(scope_directory, |site| site.runtime_directory());
    let [client_path, worker_path, client_pending_path, host_path] =
        ModeCheckedLocalSiteRendezvous::socket_budget_paths(runtime);
    let mut details = Map::new();
    let mut any_fail = false;
    let mut any_present = false;
    for (name, path, exists) in [
        (
            "client",
            client_path,
            site.as_ref().ok().map(|site| site.client_socket_exists()),
        ),
        (
            "worker",
            worker_path,
            site.as_ref().ok().map(|site| site.worker_socket_exists()),
        ),
    ] {
        let (entry, present, fail) = socket_entry(exists.as_ref(), &path);
        any_present |= present;
        any_fail |= fail;
        details.insert(name.to_owned(), Value::Object(entry));
    }
    // `clientPending`/`host` are derived-address budget entries only: presence
    // is never probed, so a fitting address honestly reports
    // `presence-not-inspected` instead of an invented state — while an
    // over-budget address still fails on its own.
    for (name, path) in [("clientPending", client_pending_path), ("host", host_path)] {
        let (entry, fail) = socket_budget_entry(&path);
        any_fail |= fail;
        details.insert(name.to_owned(), Value::Object(entry));
    }
    let status = if any_fail {
        "fail"
    } else if any_present {
        "pass"
    } else {
        "not-applicable"
    };
    CheckReport {
        name: "sockets",
        status,
        details,
    }
}

/// One derived-address budget entry: `presence` is always `unchecked`, the
/// path fact and both byte counts are always reported, and only the ABI fit
/// decides between `presence-not-inspected` and `address-too-long`.
#[cfg(any(target_os = "linux", windows))]
fn socket_budget_entry(path: &Path) -> (Map<String, Value>, bool) {
    let fits = unix_socket_path_fits(path);
    let mut details = detail([
        ("presence".to_owned(), json!("unchecked")),
        (
            "status".to_owned(),
            json!(if fits { "not-applicable" } else { "fail" }),
        ),
        (
            "reason".to_owned(),
            json!(if fits {
                "presence-not-inspected"
            } else {
                "address-too-long"
            }),
        ),
        (
            "addressBytes".to_owned(),
            json!(path.as_os_str().as_encoded_bytes().len()),
        ),
        (
            "addressCapacityBytes".to_owned(),
            json!(UNIX_SOCKET_PATH_CAPACITY),
        ),
    ]);
    let (key, value) = path_detail(path);
    details.insert(key, value);
    (details, !fits)
}

/// One socket's inspection result → its typed entry plus the aggregate
/// (`compliant present`, `failed`) bits. `PermissionDenied` is the inspector's
/// synthesized verdict for an entry that exists but is not a compliant
/// socket — presence was established, so the entry is `present`+`fail`. Any
/// other `fstatat` error means presence itself was never established, so the
/// entry is honestly `unchecked`; no field invents a definitive state.
///
/// The derived endpoint is pure path math, known even when the runtime
/// directory cannot be inspected: an address that cannot fit this platform's
/// socket ABI can never be bound or connected, so it fails `address-too-long`
/// regardless of presence.
#[cfg(any(target_os = "linux", windows))]
fn socket_entry(
    exists: Option<&std::io::Result<bool>>,
    path: &Path,
) -> (Map<String, Value>, bool, bool) {
    if !unix_socket_path_fits(path) {
        let presence = match exists {
            Some(Ok(true)) => "present",
            Some(Ok(false)) => "absent",
            Some(Err(error)) if error.kind() == std::io::ErrorKind::PermissionDenied => "present",
            Some(Err(_)) | None => "unchecked",
        };
        let mut details = detail([
            ("presence".to_owned(), json!(presence)),
            ("status".to_owned(), json!("fail")),
            ("reason".to_owned(), json!("address-too-long")),
            (
                "addressBytes".to_owned(),
                json!(path.as_os_str().as_encoded_bytes().len()),
            ),
            (
                "addressCapacityBytes".to_owned(),
                json!(UNIX_SOCKET_PATH_CAPACITY),
            ),
        ]);
        let (key, value) = path_detail(path);
        details.insert(key, value);
        return (details, presence == "present", true);
    }
    match exists {
        Some(Ok(true)) => (
            detail([
                ("presence".to_owned(), json!("present")),
                ("status".to_owned(), json!("pass")),
            ]),
            true,
            false,
        ),
        Some(Ok(false)) => (
            detail([
                ("presence".to_owned(), json!("absent")),
                ("status".to_owned(), json!("not-applicable")),
            ]),
            false,
            false,
        ),
        Some(Err(error)) if error.kind() == std::io::ErrorKind::PermissionDenied => (
            detail([
                ("presence".to_owned(), json!("present")),
                ("status".to_owned(), json!("fail")),
                ("reason".to_owned(), json!("unsafe")),
            ]),
            false,
            true,
        ),
        Some(Err(_)) => (
            detail([
                ("presence".to_owned(), json!("unchecked")),
                ("status".to_owned(), json!("not-applicable")),
                ("reason".to_owned(), json!("io-error")),
            ]),
            false,
            false,
        ),
        None => (
            detail([
                ("presence".to_owned(), json!("unchecked")),
                ("status".to_owned(), json!("not-applicable")),
                ("reason".to_owned(), json!("runtime-unavailable")),
            ]),
            false,
            false,
        ),
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    //! `config check` coverage at the function boundary: every RendezvousError
    //! class, every socket inspection outcome, the cohort report, and the
    //! locale row — real filesystem only inside each test's scratch directory.

    use super::*;
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    fn scratch(tag: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "obcli-config-check-{}-{tag}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir(&directory).expect("create scratch base");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("chmod scratch base");
        directory
    }

    const BASE: &str = "/ob-config-test-base";
    const SCOPE_DIR: &str = "/ob-config-test-base/orchbee-native-1000";

    fn classify(error: RendezvousError) -> (CheckReport, CheckReport) {
        classify_paths(&Err(error), Path::new(BASE), Path::new(SCOPE_DIR))
    }

    #[test]
    fn unsafe_owner_mode_and_type_fail_with_established_presence() {
        // Inspected invalid entries prove presence: exists:true + unsafe.
        for error in [
            RendezvousError::UnexpectedOwner {
                path: BASE.into(),
                expected: 1000,
                actual: 0,
            },
            RendezvousError::UnexpectedMode {
                path: BASE.into(),
                expected: 0o700,
                actual: 0o777,
            },
            RendezvousError::NotDirectory { path: BASE.into() },
            RendezvousError::DirectorySymlink { path: BASE.into() },
        ] {
            let repr = format!("{error:?}");
            let (base, directory) = classify(error);
            assert_eq!(base.status, "fail", "{repr}");
            assert_eq!(base.details["reason"], json!("unsafe"), "{repr}");
            assert_eq!(base.details["exists"], json!(true), "{repr}");
            assert_eq!(base.details["path"], json!(BASE), "{repr}");
            assert_eq!(directory.status, "not-applicable", "{repr}");
            assert!(directory.details.is_empty(), "{repr}");
        }

        // The same verdict at the scope directory flips which check fails.
        let (base, directory) = classify(RendezvousError::UnexpectedMode {
            path: SCOPE_DIR.into(),
            expected: 0o700,
            actual: 0o750,
        });
        assert_eq!(base.status, "pass");
        assert_eq!(base.details["exists"], json!(true));
        assert_eq!(directory.status, "fail");
        assert_eq!(directory.details["reason"], json!("unsafe"));
        assert_eq!(directory.details["exists"], json!(true));
        assert_eq!(directory.details["path"], json!(SCOPE_DIR));
    }

    #[test]
    fn invalid_paths_fail_without_an_established_exists() {
        for error in [
            RendezvousError::RuntimeBaseNotAbsolute {
                path: "relative".into(),
            },
            RendezvousError::RuntimeBaseNotNormalized {
                path: "/tmp/../tmp".into(),
            },
            RendezvousError::InvalidSiteScope,
        ] {
            let repr = format!("{error:?}");
            let (base, directory) = classify(error);
            assert_eq!(base.status, "fail", "{repr}");
            assert_eq!(base.details["reason"], json!("invalid-path"), "{repr}");
            // `exists` appears only when the path state was established.
            assert!(base.details.get("exists").is_none(), "{repr}");
            assert_eq!(directory.status, "not-applicable", "{repr}");
            assert!(directory.details.is_empty(), "{repr}");
        }
    }

    #[test]
    fn absent_paths_report_exists_false_io_errors_omit_it() {
        // An established absence is a fact: exists:false + reason absent.
        let (base, _) = classify(RendezvousError::System {
            operation: "test",
            path: BASE.into(),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        });
        assert_eq!(base.status, "fail");
        assert_eq!(base.details["reason"], json!("absent"));
        assert_eq!(base.details["exists"], json!(false));

        // The same absence at the scope directory is not-applicable, not a
        // fault — but the established fact is still reported.
        let (base, directory) = classify(RendezvousError::System {
            operation: "test",
            path: SCOPE_DIR.into(),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        });
        assert_eq!(base.status, "pass");
        assert_eq!(directory.status, "not-applicable");
        assert_eq!(directory.details["reason"], json!("absent"));
        assert_eq!(directory.details["exists"], json!(false));

        // A genuine I/O failure establishes nothing: `exists` is omitted.
        for source in [
            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
            std::io::Error::from_raw_os_error(5), // EIO
        ] {
            let (base, directory) = classify(RendezvousError::System {
                operation: "test",
                path: BASE.into(),
                source,
            });
            assert_eq!(base.status, "fail");
            assert_eq!(base.details["reason"], json!("io-error"));
            assert!(base.details.get("exists").is_none());
            assert_eq!(directory.status, "not-applicable");
            assert!(directory.details.is_empty());
        }
    }

    #[test]
    fn non_utf8_paths_encode_as_path_bytes_hex() {
        // The machine field is exact bytes, never a lossy display string.
        let (key, value) = path_detail(Path::new("/tmp/utf8"));
        assert_eq!(key, "path");
        assert_eq!(value, json!("/tmp/utf8"));
        let raw = std::ffi::OsStr::from_bytes(b"/tmp/\xff\xfe");
        let (key, value) = path_detail(Path::new(raw));
        assert_eq!(key, "pathBytesHex");
        let hex: String = raw
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(value, json!(hex));

        // The same field lands in check details when the base itself is
        // non-UTF-8.
        let weird = PathBuf::from(raw);
        let (base, _) = classify_paths(
            &Err(RendezvousError::UnexpectedOwner {
                path: weird.clone(),
                expected: 1000,
                actual: 0,
            }),
            &weird,
            Path::new(SCOPE_DIR),
        );
        assert_eq!(base.status, "fail");
        assert_eq!(base.details["pathBytesHex"], json!(hex));
        assert!(base.details.get("path").is_none());
    }

    #[test]
    fn cohort_report_maps_verifier_outcomes() {
        // Pass requires a real validated cohort: stage the four artifacts
        // plus a hash-true manifest, all owner-private.
        let cohort = scratch("cohort-pass");
        let mut artifacts = serde_json::Map::new();
        for name in [
            "orchbee",
            "orchbee-server",
            "orchbee-worker",
            "orchbee-terminal-host",
        ] {
            let path = cohort.join(name);
            fs::write(&path, format!("stand-in for {name}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            let digest = Sha256::digest(fs::read(&path).unwrap());
            artifacts.insert(name.into(), format!("{digest:x}").into());
        }
        // A controlled dummy digest stands in for the adapters payload — the
        // fixture stages no sidecar; cohort admission stays lazy.
        artifacts.insert(
            "agent-observation-adapters".into(),
            format!("{:x}", Sha256::digest(b"staged stand-in adapters\n")).into(),
        );
        let manifest = cohort.join("cohort.json");
        fs::write(
            &manifest,
            serde_json::json!({"schemaVersion": 2, "artifacts": artifacts}).to_string(),
        )
        .unwrap();
        fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600)).unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        let report = cohort_report(NativeCohort::load(&cohort, deadline));
        assert_eq!(report.status, "pass");
        assert_eq!(report.details["verified"], json!(true));
        fs::remove_dir_all(&cohort).ok();

        // A missing manifest is an invalid-cohort failure, not a crash.
        let empty = scratch("cohort-empty");
        let report = cohort_report(NativeCohort::load(&empty, deadline));
        assert_eq!(report.status, "fail");
        assert_eq!(report.details["verified"], json!(false));
        assert_eq!(report.details["reason"], json!("invalid-cohort"));
        fs::remove_dir_all(&empty).ok();

        // An already-spent deadline reports `deadline` before any I/O.
        let report = cohort_report(NativeCohort::load(
            Path::new("/nonexistent"),
            Instant::now(),
        ));
        assert_eq!(report.status, "fail");
        assert_eq!(report.details["reason"], json!("deadline"));
    }

    #[test]
    fn locale_check_reports_the_selected_locale() {
        let report = locale_check();
        assert_eq!(report.status, "pass");
        assert!(
            ["en-US", "zh-CN"].contains(&report.details["locale"].as_str().unwrap()),
            "locale: {}",
            report.details["locale"]
        );
        assert!(
            ["LC_ALL", "LC_MESSAGES", "LANG", "default"]
                .contains(&report.details["source"].as_str().unwrap()),
            "source: {}",
            report.details["source"]
        );
    }

    #[test]
    fn socket_entry_maps_every_inspection_outcome() {
        let path = Path::new("/tmp/oc-test/orchbee-native-1000/client.sock");
        // Compliant socket: present + pass, aggregate pass bit.
        let (entry, present, fail) = socket_entry(Some(&Ok(true)), path);
        assert_eq!(entry["presence"], json!("present"));
        assert_eq!(entry["status"], json!("pass"));
        assert!(!entry.contains_key("reason"));
        assert!(present && !fail);

        // Established absence: absent + not-applicable.
        let (entry, present, fail) = socket_entry(Some(&Ok(false)), path);
        assert_eq!(entry["presence"], json!("absent"));
        assert_eq!(entry["status"], json!("not-applicable"));
        assert!(!present && !fail);

        // The synthesized invalid-entry verdict stays present + fail + unsafe.
        let (entry, present, fail) = socket_entry(
            Some(&Err(std::io::Error::from(
                std::io::ErrorKind::PermissionDenied,
            ))),
            path,
        );
        assert_eq!(entry["presence"], json!("present"));
        assert_eq!(entry["status"], json!("fail"));
        assert_eq!(entry["reason"], json!("unsafe"));
        assert!(!present && fail);

        // A real fstatat failure establishes nothing: presence is honestly
        // unchecked, never a fabricated definitive field.
        let (entry, present, fail) =
            socket_entry(Some(&Err(std::io::Error::from_raw_os_error(5))), path);
        assert_eq!(entry["presence"], json!("unchecked"));
        assert_eq!(entry["status"], json!("not-applicable"));
        assert_eq!(entry["reason"], json!("io-error"));
        assert!(!present && !fail);

        // No runtime directory at all: unchecked + runtime-unavailable.
        let (entry, present, fail) = socket_entry(None, path);
        assert_eq!(entry["presence"], json!("unchecked"));
        assert_eq!(entry["status"], json!("not-applicable"));
        assert_eq!(entry["reason"], json!("runtime-unavailable"));
        assert!(!present && !fail);
    }

    #[test]
    fn socket_entry_fails_when_the_derived_address_cannot_fit() {
        // The budget is path bytes, not Unicode scalars: `capacity + 1`
        // ASCII bytes fails while a scalar-shorter but byte-longer non-ASCII
        // path fails identically.
        let too_long_ascii = PathBuf::from("/".to_owned() + &"a".repeat(UNIX_SOCKET_PATH_CAPACITY));
        assert_eq!(
            too_long_ascii.as_os_str().as_encoded_bytes().len(),
            UNIX_SOCKET_PATH_CAPACITY + 1
        );
        for exists in [
            Some(Ok(true)),
            Some(Ok(false)),
            Some(Err(std::io::Error::from(
                std::io::ErrorKind::PermissionDenied,
            ))),
            Some(Err(std::io::Error::from_raw_os_error(5))),
            None,
        ] {
            let (entry, _present, fail) = socket_entry(exists.as_ref(), &too_long_ascii);
            assert_eq!(entry["status"], json!("fail"), "{exists:?}");
            assert_eq!(entry["reason"], json!("address-too-long"), "{exists:?}");
            assert_eq!(
                entry["addressBytes"],
                json!(UNIX_SOCKET_PATH_CAPACITY + 1),
                "{exists:?}"
            );
            assert_eq!(
                entry["addressCapacityBytes"],
                json!(UNIX_SOCKET_PATH_CAPACITY),
                "{exists:?}"
            );
            assert_eq!(entry["path"], json!(too_long_ascii), "{exists:?}");
            assert!(fail, "{exists:?}");
        }
        // Presence is still reported honestly under the fail verdict.
        let (entry, present, _) = socket_entry(Some(&Ok(true)), &too_long_ascii);
        assert_eq!(entry["presence"], json!("present"));
        assert!(present);
        let (entry, _, _) = socket_entry(Some(&Ok(false)), &too_long_ascii);
        assert_eq!(entry["presence"], json!("absent"));
        let (entry, _, _) = socket_entry(None, &too_long_ascii);
        assert_eq!(entry["presence"], json!("unchecked"));

        // 40 scalars but 121 bytes: byte length, not char count, is measured.
        let non_ascii = PathBuf::from("/".to_owned() + &"日".repeat(40));
        assert_eq!(non_ascii.as_os_str().as_encoded_bytes().len(), 1 + 40 * 3);
        let (entry, _, fail) = socket_entry(Some(&Ok(false)), &non_ascii);
        assert_eq!(entry["reason"], json!("address-too-long"));
        assert_eq!(entry["addressBytes"], json!(121));
        assert!(fail);

        // The exact boundary still fits — bytes equal to capacity pass.
        let boundary = PathBuf::from("/".to_owned() + &"a".repeat(UNIX_SOCKET_PATH_CAPACITY - 1));
        assert_eq!(
            boundary.as_os_str().as_encoded_bytes().len(),
            UNIX_SOCKET_PATH_CAPACITY
        );
        let (entry, _, fail) = socket_entry(Some(&Ok(false)), &boundary);
        assert_eq!(entry["status"], json!("not-applicable"));
        assert!(!fail);

        // A non-UTF-8 overlong path reports its exact bytes, never lossy text.
        let raw = std::ffi::OsStr::from_bytes(b"/\xff\xfe");
        let mut weird = PathBuf::from(raw);
        weird.push("a".repeat(UNIX_SOCKET_PATH_CAPACITY));
        let (entry, _, fail) = socket_entry(None, &weird);
        assert_eq!(entry["reason"], json!("address-too-long"));
        assert!(entry.get("path").is_none());
        assert_eq!(
            entry["pathBytesHex"].as_str().unwrap().len(),
            weird.as_os_str().as_encoded_bytes().len() * 2
        );
        assert!(fail);
    }

    #[test]
    fn socket_checks_report_derived_budget_entries() {
        // `clientPending` and `host` are derived-address budget entries only:
        // presence is never probed, so every fitting path reports
        // `presence-not-inspected` — never a claim that the endpoint exists.
        let report = socket_checks(
            &Err(RendezvousError::System {
                operation: "test",
                path: BASE.into(),
                source: std::io::Error::from(std::io::ErrorKind::NotFound),
            }),
            Path::new(SCOPE_DIR),
        );
        for name in ["clientPending", "host"] {
            let entry = &report.details[name];
            assert_eq!(entry["presence"], json!("unchecked"), "{name}: {entry}");
            assert_eq!(entry["status"], json!("not-applicable"), "{name}: {entry}");
            assert_eq!(
                entry["reason"],
                json!("presence-not-inspected"),
                "{name}: {entry}"
            );
            assert_eq!(
                entry["addressCapacityBytes"],
                json!(UNIX_SOCKET_PATH_CAPACITY),
                "{name}: {entry}"
            );
            assert!(
                entry["addressBytes"].as_u64().unwrap() > 0,
                "{name}: {entry}"
            );
            assert!(
                entry.get("path").is_some() || entry.get("pathBytesHex").is_some(),
                "{name}: {entry}"
            );
        }
        // A fitting unchecked budget never promotes the aggregate to pass.
        assert_eq!(report.status, "not-applicable");

        // An over-budget derived address fails on its own: only the budget
        // verdict changes, `presence` stays honestly unchecked.
        let long_dir = PathBuf::from("/".to_owned() + &"a".repeat(UNIX_SOCKET_PATH_CAPACITY));
        let report = socket_checks(&Err(RendezvousError::InvalidSiteScope), &long_dir);
        assert_eq!(report.status, "fail");
        for name in ["clientPending", "host"] {
            let entry = &report.details[name];
            assert_eq!(entry["presence"], json!("unchecked"), "{name}: {entry}");
            assert_eq!(entry["status"], json!("fail"), "{name}: {entry}");
            assert_eq!(
                entry["reason"],
                json!("address-too-long"),
                "{name}: {entry}"
            );
            assert_eq!(
                entry["addressCapacityBytes"],
                json!(UNIX_SOCKET_PATH_CAPACITY),
                "{name}: {entry}"
            );
            let bytes = entry["addressBytes"].as_u64().unwrap() as usize;
            assert!(bytes > UNIX_SOCKET_PATH_CAPACITY, "{name}: {entry}");
        }
    }

    #[test]
    fn socket_checks_aggregate_real_states() {
        // No valid runtime directory: both entries unchecked.
        let report = socket_checks(
            &Err(RendezvousError::System {
                operation: "test",
                path: BASE.into(),
                source: std::io::Error::from(std::io::ErrorKind::NotFound),
            }),
            Path::new(SCOPE_DIR),
        );
        assert_eq!(report.status, "not-applicable");
        for name in ["client", "worker"] {
            assert_eq!(report.details[name]["presence"], json!("unchecked"));
            assert_eq!(report.details[name]["reason"], json!("runtime-unavailable"));
        }

        // A real prepared scope without sockets: absent + not-applicable. The
        // base must stay short — the derived Host endpoint is the deepest
        // budget fact and a deep scratch would itself trip `address-too-long`.
        let base = {
            let directory = std::env::temp_dir().join(format!(
                "obsc-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            fs::create_dir(&directory).expect("create short socket base");
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                .expect("chmod short socket base");
            directory
        };
        let site = || {
            ModeCheckedLocalSiteRendezvous::validate_mode_checked(
                &base,
                SiteScope::parse("ob-test-sockets").unwrap(),
            )
            .expect("validated scope")
        };
        let scope_directory = ModeCheckedLocalSiteRendezvous::prepare_mode_checked(
            &base,
            SiteScope::parse("ob-test-sockets").unwrap(),
        )
        .expect("prepared scope")
        .runtime_directory()
        .to_owned();
        let report = socket_checks(&Ok(site()), &scope_directory);
        assert_eq!(report.status, "not-applicable");
        assert_eq!(report.details["client"]["presence"], json!("absent"));
        assert_eq!(report.details["worker"]["presence"], json!("absent"));

        // One compliant socket passes the check.
        let listener = UnixListener::bind(scope_directory.join("client.sock")).unwrap();
        fs::set_permissions(
            scope_directory.join("client.sock"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let report = socket_checks(&Ok(site()), &scope_directory);
        assert_eq!(report.status, "pass");
        assert_eq!(report.details["client"]["presence"], json!("present"));
        assert_eq!(report.details["client"]["status"], json!("pass"));
        assert_eq!(report.details["worker"]["presence"], json!("absent"));
        drop(listener);

        // An inspected invalid entry fails: a regular file is not a socket.
        fs::remove_file(scope_directory.join("client.sock")).unwrap();
        fs::write(scope_directory.join("client.sock"), b"not a socket").unwrap();
        let report = socket_checks(&Ok(site()), &scope_directory);
        assert_eq!(report.status, "fail");
        assert_eq!(report.details["client"]["presence"], json!("present"));
        assert_eq!(report.details["client"]["status"], json!("fail"));
        assert_eq!(report.details["client"]["reason"], json!("unsafe"));
        assert_eq!(report.details["worker"]["presence"], json!("absent"));
        fs::remove_dir_all(&base).ok();
    }
}

#[cfg(windows)]
pub(crate) fn path_detail(path: &std::path::Path) -> (String, Value) {
    use std::os::windows::ffi::OsStrExt;
    match path.to_str() {
        Some(text) => ("path".to_owned(), json!(text)),
        None => (
            "pathUtf16".to_owned(),
            json!(path.as_os_str().encode_wide().collect::<Vec<_>>()),
        ),
    }
}
