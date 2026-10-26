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
use orchbee_local_runtime::{
    Cohort, ModeCheckedLocalSessionRendezvous, RendezvousError, SessionScope,
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
fn warning(name: &'static str, details: Map<String, Value>) -> CheckReport {
    CheckReport {
        name,
        status: "warning",
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
            let check_documents = checks
                .iter()
                .map(|check| {
                    json!({
                        "name":check.name,"status":check.status,"details":check.details
                    })
                })
                .collect::<Vec<_>>();
            #[cfg(not(windows))]
            let check_documents = {
                let mut documents = check_documents;
                documents.push(json!({"kind":"unsupported"}));
                documents
            };
            let result = json!({
                "ok": !failed,
                "checks": check_documents,
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
                        "version": 1,
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
        Failure::new(FailureKind::ServiceUnavailable).details(super::stage_reason_details(
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
        SessionScope::parse(&scope_text).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let scope_directory = base.join(&scope_text);
    let session = ModeCheckedLocalSessionRendezvous::validate_mode_checked(&base, scope);

    let mut checks = Vec::with_capacity(5);
    let (base_check, directory_check) = classify_paths(&session, &base, &scope_directory);
    checks.push(base_check);
    checks.push(directory_check);

    // The cohort check reuses the current-cohort verifier with its own
    // independent five-second budget.
    let deadline = Instant::now()
        .checked_add(CHECK_BUDGET)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    checks.push(cohort_report(Cohort::current(deadline)));

    checks.push(locale_check());
    checks.push(socket_checks(&session, &scope_directory));
    if crate::tui_capabilities::Capability::ConfigurationFile
        .require()
        .is_ok()
    {
        checks.push(match super::configuration::load() {
            Ok(settings) => {
                let status = if settings.diagnostics.is_empty() {
                    pass
                } else {
                    warning
                };
                status("user-config", settings.check_details())
            }
            Err(error) => fail(
                "user-config",
                Map::from_iter([
                    ("field".into(), json!(error.field)),
                    ("reason".into(), json!(error.reason)),
                ]),
            ),
        });
    }
    Ok(checks)
}

#[cfg(any(target_os = "linux", windows))]
fn classify_paths(
    session: &Result<ModeCheckedLocalSessionRendezvous, RendezvousError>,
    base: &Path,
    scope_directory: &Path,
) -> (CheckReport, CheckReport) {
    match session {
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
                | RendezvousError::InvalidSessionScope => None,
                _ => Some(true),
            };
            let invalid_path = matches!(
                error,
                RendezvousError::RuntimeBaseNotAbsolute { .. }
                    | RendezvousError::RuntimeBaseNotNormalized { .. }
                    | RendezvousError::InvalidSessionScope
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
                // A missing Session scope under a healthy base is not-applicable:
                // no running Session is a legal state, not a configuration fault.
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
/// `server diagnose`, which classifies the same error family onto rows.
#[cfg(any(target_os = "linux", windows))]
pub(crate) fn rendezvous_error_path(error: &RendezvousError) -> Option<&Path> {
    match error {
        RendezvousError::InvalidSessionScope => None,
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
fn cohort_report(result: std::io::Result<Cohort>) -> CheckReport {
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
    session: &Result<ModeCheckedLocalSessionRendezvous, RendezvousError>,
    scope_directory: &Path,
) -> CheckReport {
    let runtime = session
        .as_ref()
        .map_or(scope_directory, |session| session.runtime_directory());
    let [client_path, worker_path, client_pending_path, host_path] =
        ModeCheckedLocalSessionRendezvous::socket_budget_paths(runtime);
    let mut details = Map::new();
    let mut any_fail = false;
    let mut any_present = false;
    for (name, path, exists) in [
        (
            "client",
            client_path,
            session
                .as_ref()
                .ok()
                .map(|session| session.client_socket_exists()),
        ),
        (
            "worker",
            worker_path,
            session
                .as_ref()
                .ok()
                .map(|session| session.worker_socket_exists()),
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
mod tests;

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
