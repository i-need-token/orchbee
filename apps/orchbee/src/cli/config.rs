//! `config check`: pure local inspection. No socket is connected, no process
//! is started, no directory is created, and no lock is taken.

#[cfg(target_os = "linux")]
use std::env;
#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use super::{Failure, FailureKind, Output, failure_output, output};
use crate::invocation::OutputFormat;
use crate::localization::Locale;

#[cfg(target_os = "linux")]
use orchbee_local_site_runtime::{
    ModeCheckedLocalSiteRendezvous, NativeCohort, RendezvousError, SiteScope,
};

#[cfg(target_os = "linux")]
const CHECK_BUDGET: Duration = Duration::from_secs(5);

pub(crate) struct CheckReport {
    pub(crate) name: &'static str,
    pub(crate) status: &'static str,
    pub(crate) details: Map<String, Value>,
}

#[cfg(target_os = "linux")]
fn pass(name: &'static str, details: Map<String, Value>) -> CheckReport {
    CheckReport {
        name,
        status: "pass",
        details,
    }
}

#[cfg(target_os = "linux")]
fn fail(name: &'static str, details: Map<String, Value>) -> CheckReport {
    CheckReport {
        name,
        status: "fail",
        details,
    }
}

#[cfg(target_os = "linux")]
fn not_applicable(name: &'static str, details: Map<String, Value>) -> CheckReport {
    CheckReport {
        name,
        status: "not-applicable",
        details,
    }
}

#[cfg(target_os = "linux")]
fn detail(pairs: impl IntoIterator<Item = (String, Value)>) -> Map<String, Value> {
    Map::from_iter(pairs)
}

/// A filesystem path as a machine field: UTF-8 paths are `path`, others are
/// `pathBytesHex`; never a lossy display string.
#[cfg(target_os = "linux")]
fn path_detail(path: &Path) -> (String, Value) {
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

fn local_runtime_source() -> Value {
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
                        },
                        Err(output::TooLarge) => failure_output(
                            Failure::new(FailureKind::OutputTooLarge).details(Map::from_iter([(
                                "limitBytes".to_owned(),
                                json!(output::LIMIT),
                            )])),
                            Some(OutputFormat::Json),
                            locale,
                        ),
                    }
                }
                OutputFormat::Text => Output {
                    status: if failed { 1 } else { 0 },
                    stdout: super::present::config_checks(&checks, locale).into(),
                    stderr,
                },
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn run_checks() -> Result<Vec<CheckReport>, Failure> {
    Err(
        Failure::new(FailureKind::CheckFailed).details(super::stage_reason_details(
            "platform",
            "unsupported-platform",
        )),
    )
}

#[cfg(target_os = "linux")]
fn run_checks() -> Result<Vec<CheckReport>, Failure> {
    let base = env::var_os("ORCHBEE_RUNTIME_BASE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let scope_text = format!("orchbee-native-{}", nix::unistd::geteuid().as_raw());
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
    checks.push(match NativeCohort::current(deadline) {
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
    });

    checks.push(locale_check());
    checks.push(socket_checks(&site));
    Ok(checks)
}

#[cfg(target_os = "linux")]
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
            let path = rendezvous_path(error);
            let scoped = path == Some(scope_directory);
            let absent = matches!(
                error,
                RendezvousError::System { source, .. }
                    if source.kind() == std::io::ErrorKind::NotFound
            );
            match (scoped, absent) {
                // A missing Site scope under a healthy base is not-applicable:
                // no running Site is a legal state, not a configuration fault.
                (true, true) => (
                    pass(
                        "runtime-base",
                        detail([path_detail(base), ("exists".to_owned(), json!(true))]),
                    ),
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
                    pass(
                        "runtime-base",
                        detail([path_detail(base), ("exists".to_owned(), json!(true))]),
                    ),
                    fail(
                        "runtime-directory",
                        detail([
                            path_detail(scope_directory),
                            ("exists".to_owned(), json!(true)),
                            (
                                "reason".to_owned(),
                                json!(if matches!(error, RendezvousError::System { .. }) {
                                    "io-error"
                                } else {
                                    "unsafe"
                                }),
                            ),
                        ]),
                    ),
                ),
                (false, _) => {
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
                    let mut details = detail([
                        ("exists".to_owned(), json!(!invalid_path)),
                        ("reason".to_owned(), json!(reason)),
                    ]);
                    let (key, value) = path_detail(base);
                    details.insert(key, value);
                    (
                        fail("runtime-base", details),
                        not_applicable("runtime-directory", Map::new()),
                    )
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn rendezvous_path(error: &RendezvousError) -> Option<&Path> {
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

#[cfg(target_os = "linux")]
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

/// `sockets` reports presence only; presence never means a live service.
#[cfg(target_os = "linux")]
fn socket_checks(site: &Result<ModeCheckedLocalSiteRendezvous, RendezvousError>) -> CheckReport {
    let mut details = Map::new();
    let mut any_fail = false;
    let mut any_present = false;
    for (name, exists) in [
        (
            "client",
            site.as_ref().ok().map(|site| site.client_socket_exists()),
        ),
        (
            "worker",
            site.as_ref().ok().map(|site| site.worker_socket_exists()),
        ),
    ] {
        let entry = match exists {
            Some(Ok(true)) => {
                any_present = true;
                detail([
                    ("presence".to_owned(), json!("present")),
                    ("status".to_owned(), json!("pass")),
                ])
            }
            Some(Ok(false)) => detail([
                ("presence".to_owned(), json!("absent")),
                ("status".to_owned(), json!("not-applicable")),
            ]),
            Some(Err(error)) => {
                any_fail = true;
                let reason = if error.kind() == std::io::ErrorKind::PermissionDenied {
                    "unsafe"
                } else {
                    "io-error"
                };
                detail([
                    ("presence".to_owned(), json!("present")),
                    ("status".to_owned(), json!("fail")),
                    ("reason".to_owned(), json!(reason)),
                ])
            }
            None => detail([
                ("presence".to_owned(), json!("unchecked")),
                ("status".to_owned(), json!("not-applicable")),
                ("reason".to_owned(), json!("runtime-unavailable")),
            ]),
        };
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
