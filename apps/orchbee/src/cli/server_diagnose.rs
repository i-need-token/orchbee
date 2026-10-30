//! `server diagnose`: bounded read-only metadata and private-runtime reports.
//! Native ports preserve the host scope and the finite pathname/descriptor observations.
//! The shared workflows only interpret those observed facts — no endpoint is
//! connected, no lock file is created, no marker or socket is touched, and
//! no repair is attempted or implied. Every row is a pathname or descriptor
//! fact at its own observation instant: `held` means some owner held the
//! lifetime lock then, never that a Session or any process is alive; `absent`
//! is a pathname fact, never proof that an execution ended.

use std::path::Path;
#[cfg(all(test, target_os = "linux"))]
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use super::results::local_report::DiagnoseDocument;
use super::{Failure, FailureKind, Output, failure_output, output};
use crate::invocation::OutputFormat;
use crate::localization::Locale;

mod native;
mod private;

use orchbee_local_runtime::{
    EntryKind, EntryObservation, ModeCheckedLocalSessionRendezvous, RendezvousError,
    SessionLifetimeObservation, SessionScope,
};
#[cfg(all(test, target_os = "linux"))]
use orchbee_local_runtime::{LockAttempt, LockKind};

/// A cooperative budget checked between bounded filesystem operations —
/// not a hard wall promise on synchronous filesystems.
const DIAGNOSE_BUDGET: Duration = Duration::from_secs(5);

/// The advisory emitted when a valid activation marker was observed.
const ADVISORY_ACTIVATION_RECORDED: &str = "activation-recorded";
/// The advisory emitted when a valid client or worker socket residue was
/// observed — the other half of the current activation predicate.
const ADVISORY_SOCKET_RESIDUE: &str = "socket-residue";

/// Platform-specific observations share one machine shape. Text keeps its concise rows.
fn complete_rows(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    if object.contains_key("state") {
        let absence = object.get("state") == Some(&json!("absent"));
        for key in [
            "reason",
            "path",
            "pathBytesHex",
            "pathUtf16",
            "kind",
            "ownerMatch",
            "mode",
            "size",
            "linkCount",
            "private",
            "activation",
            "stopRequestId",
        ] {
            let unsupported = native::unsupported_field(key);
            object.entry(key.to_owned()).or_insert_with(|| {
                if unsupported {
                    json!({"kind":"unsupported"})
                } else if absence
                    || matches!(
                        key,
                        "reason" | "path" | "pathBytesHex" | "pathUtf16" | "stopRequestId"
                    )
                {
                    Value::Null
                } else {
                    json!("unknown")
                }
            });
        }
    } else {
        for member in object.values_mut() {
            complete_rows(member);
        }
    }
}

pub(crate) fn run(format: OutputFormat, locale: Locale) -> Output {
    match collect() {
        // Like `server start`, every `server diagnose` answer — failure
        // included — comes from the local runtime authority.
        Err(failure) => failure_output(
            failure.source(crate::cli_outcome::local_runtime_source()),
            Some(format),
            locale,
        ),
        Ok(document) => {
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
                OutputFormat::Json => match output::encode_document(&document.envelope) {
                    Ok(stdout) => Output {
                        status: 0,
                        stdout,
                        stderr: String::new(),
                        finalization: None,
                    },
                    Err(output::TooLarge) => too_large(),
                },
                OutputFormat::Text => {
                    match output::render_text(|out| {
                        super::present::server_diagnose(out, &document, locale)
                    }) {
                        Ok(stdout) => Output {
                            status: 0,
                            stdout,
                            stderr: String::new(),
                            finalization: None,
                        },
                        Err(output::TooLarge) => too_large(),
                    }
                }
            }
        }
    }
}

/// The expected shape of one finite entry — the caller's verdict table over
/// the raw `EntryObservation` facts, mirroring the checks the activation
/// predicate itself performs on the same names.
#[derive(Clone, Copy, PartialEq)]
enum Expect {
    Socket,
    RegularFile,
    /// The activation marker: a regular file the predicate additionally
    /// requires to have exactly one link and zero bytes.
    MarkerFile,
    Directory,
}

impl Expect {
    fn matches(self, kind: EntryKind) -> bool {
        match self {
            Self::Socket => kind == EntryKind::Socket,
            Self::RegularFile | Self::MarkerFile => kind == EntryKind::RegularFile,
            Self::Directory => kind == EntryKind::Directory,
        }
    }

    fn mode(self) -> u32 {
        match self {
            Self::Socket | Self::RegularFile | Self::MarkerFile => 0o600,
            Self::Directory => 0o700,
        }
    }
}

/// The `state` fact one built row carries — for token comparisons only;
/// callers that need the `&'static` literal itself reuse the same statics
/// they built the row from.
fn row_state(row: &Map<String, Value>) -> &str {
    row.get("state").and_then(Value::as_str).unwrap_or("")
}

fn state_row(state: &'static str, reason: Option<&'static str>) -> Map<String, Value> {
    let mut row = Map::from_iter([("state".to_owned(), json!(state))]);
    if let Some(reason) = reason {
        row.insert("reason".to_owned(), json!(reason));
    }
    row
}

/// Map one raw entry observation onto the closed row vocabulary against
/// this row's expectation: facts are always reported; the verdict only
/// arbitrates `known`/`unsafe`/`absent`/`uninspectable`.
fn entry_row(observation: std::io::Result<EntryObservation>, expect: Expect) -> Map<String, Value> {
    match observation {
        Ok(EntryObservation::Absent) => state_row("absent", None),
        Ok(EntryObservation::Present {
            kind,
            owner_matches,
            mode,
            size,
            link_count,
        }) => {
            let reason = if kind == EntryKind::Symlink {
                Some("symlink")
            } else if !expect.matches(kind) {
                Some("invalid-kind")
            } else if !owner_matches {
                Some("foreign-owner")
            } else if mode != expect.mode() {
                Some("invalid-mode")
            } else if matches!(expect, Expect::RegularFile | Expect::MarkerFile) && link_count != 1
            {
                Some("invalid-link-count")
            } else if expect == Expect::MarkerFile && size != 0 {
                Some("invalid-size")
            } else {
                None
            };
            let mut row = state_row(if reason.is_some() { "unsafe" } else { "known" }, reason);
            row.insert(
                "kind".to_owned(),
                json!(match kind {
                    EntryKind::Socket => "socket",
                    EntryKind::RegularFile => "regular",
                    EntryKind::Directory => "directory",
                    EntryKind::Symlink => "symlink",
                    EntryKind::Other => "other",
                }),
            );
            row.insert("ownerMatch".to_owned(), json!(owner_matches));
            row.insert("mode".to_owned(), json!(format!("{mode:04o}")));
            row.insert("size".to_owned(), json!(size));
            row.insert("linkCount".to_owned(), json!(link_count));
            row
        }
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            state_row("uninspectable", Some("denied"))
        }
        Err(_) => state_row("uninspectable", Some("io-error")),
    }
}

/// The host endpoint row: a `PermissionDenied` from the rendezvous means
/// the parent Host directory itself was unadmitted, so the row is honestly
/// `uninspectable` with `unsafe-parent` — never traversed, never guessed.
fn host_endpoint_row(observation: std::io::Result<EntryObservation>) -> Map<String, Value> {
    match observation {
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            state_row("uninspectable", Some("unsafe-parent"))
        }
        other => entry_row(other, Expect::Socket),
    }
}

fn lifetime_row(observation: std::io::Result<SessionLifetimeObservation>) -> Map<String, Value> {
    match observation {
        Ok(SessionLifetimeObservation::Absent) => state_row("absent", None),
        Ok(SessionLifetimeObservation::Free) => state_row("free", None),
        Ok(SessionLifetimeObservation::Held) => state_row("held", None),
        Ok(SessionLifetimeObservation::Unsafe) => state_row("unsafe", Some("invalid-lock")),
        Err(_) => state_row("uninspectable", Some("io-error")),
    }
}

/// The row for a descendant under an unadmitted runtime directory: the
/// parent verdict selects `unsafe-parent` (unsafe to traverse) versus
/// `parent-unavailable` (absent, invalid, or itself uninspectable).
fn descendant_row(parent_unsafe: bool) -> Map<String, Value> {
    state_row(
        "uninspectable",
        Some(if parent_unsafe {
            "unsafe-parent"
        } else {
            "parent-unavailable"
        }),
    )
}

/// Classify a rendezvous validation failure onto the runtime-directory row:
/// typed states and reasons only — an inspection failure is a fact row,
/// never an engine failure merely because validation refused the path.
fn classify_directory(
    error: &RendezvousError,
) -> (Map<String, Value>, &'static str, Option<&'static str>, bool) {
    let (state, reason) = match error {
        RendezvousError::System { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
            ("absent", None)
        }
        RendezvousError::System { source, .. }
            if source.kind() == std::io::ErrorKind::PermissionDenied =>
        {
            ("uninspectable", Some("denied"))
        }
        RendezvousError::System { .. } => ("uninspectable", Some("io-error")),
        RendezvousError::RuntimeBaseNotAbsolute { .. }
        | RendezvousError::RuntimeBaseNotNormalized { .. }
        | RendezvousError::InvalidSessionScope => ("uninspectable", Some("invalid-path")),
        RendezvousError::DirectorySymlink { .. } | RendezvousError::LockFileSymlink { .. } => {
            ("unsafe", Some("symlink"))
        }
        RendezvousError::NotDirectory { .. } | RendezvousError::NotRegularLockFile { .. } => {
            ("unsafe", Some("invalid-kind"))
        }
        RendezvousError::UnexpectedOwner { .. } => ("unsafe", Some("foreign-owner")),
        RendezvousError::UnexpectedMode { .. } => ("unsafe", Some("invalid-mode")),
        RendezvousError::RuntimeDirectoryIdentityChanged { .. } => {
            ("unsafe", Some("identity-changed"))
        }
        RendezvousError::UnexpectedLockLinkCount { .. } => ("unsafe", Some("invalid-link-count")),
    };
    (state_row(state, reason), state, reason, state == "unsafe")
}

/// One bounded observation step: while budget remains the observation runs;
/// once it is spent every later row closes `uninspectable`/`deadline` and
/// the document reports partial coverage.
fn step(
    deadline: Instant,
    exhausted: &mut bool,
    observe: impl FnOnce() -> Map<String, Value>,
) -> Map<String, Value> {
    if *exhausted || Instant::now() >= deadline {
        *exhausted = true;
        state_row("uninspectable", Some("deadline"))
    } else {
        observe()
    }
}

/// The conditional advisory vocabulary: each token fires only on the same
/// facts the current activation predicate reads — a fully valid marker, or
/// a fully valid client/worker socket leaf.
fn advisories(marker: &str, client: &str, worker: &str) -> Vec<&'static str> {
    let mut advisories = Vec::new();
    if marker == "known" {
        advisories.push(ADVISORY_ACTIVATION_RECORDED);
    }
    if client == "known" || worker == "known" {
        advisories.push(ADVISORY_SOCKET_RESIDUE);
    }
    advisories
}

fn collect() -> Result<DiagnoseDocument, Failure> {
    super::execution::admit(super::execution::Capability::RuntimeInspection)?;
    if native::PRIVATE_RUNTIME {
        private::collect()
    } else {
        collect_metadata()
    }
}

fn collect_metadata() -> Result<DiagnoseDocument, Failure> {
    let base = native::metadata_base().map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let deadline = Instant::now()
        .checked_add(DIAGNOSE_BUDGET)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    collect_at(&base, deadline)
}

/// The bounded collection itself, split from environment reads so tests can
/// drive every row class against a scratch runtime base and a past deadline.
fn collect_at(base: &Path, deadline: Instant) -> Result<DiagnoseDocument, Failure> {
    let scope_text =
        native::metadata_scope().map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let scope =
        SessionScope::parse(&scope_text).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let scope_directory = base.join(&scope_text);
    let session = ModeCheckedLocalSessionRendezvous::validate_mode_checked(base, scope);

    let mut rows: Vec<(String, Map<String, Value>)> = Vec::with_capacity(6);
    let (base_key, base_value) = crate::native_path::detail(base);

    match session {
        Err(error) => {
            let (mut dir_row, dir_state, dir_reason, parent_unsafe) = classify_directory(&error);
            let (dir_key, dir_value) = crate::native_path::detail(&scope_directory);
            dir_row.insert(dir_key, dir_value);
            // The base row inherits the verdict only when the failure names
            // the base path itself; otherwise the derived base stays a
            // pathname fact.
            let base_row = if matches!(
                super::config::rendezvous_error_path(&error),
                Some(path) if path == base
            ) {
                let mut row = state_row(dir_state, dir_reason);
                row.insert(base_key, base_value);
                row
            } else {
                let mut row = state_row("known", None);
                row.insert(base_key, base_value);
                row
            };
            rows.push(("runtimeBase".to_owned(), base_row));
            rows.push(("runtimeDirectory".to_owned(), dir_row));
            let descendant = || descendant_row(parent_unsafe);
            rows.push(("marker".to_owned(), descendant()));
            rows.push((
                "locks".to_owned(),
                Map::from_iter([
                    ("activation".to_owned(), Value::Object(descendant())),
                    ("sessionLifetime".to_owned(), Value::Object(descendant())),
                ]),
            ));
            rows.push((
                "endpoints".to_owned(),
                Map::from_iter([
                    ("client".to_owned(), Value::Object(descendant())),
                    ("worker".to_owned(), Value::Object(descendant())),
                    ("clientPending".to_owned(), Value::Object(descendant())),
                    ("agentObservation".to_owned(), Value::Object(descendant())),
                    ("hostEndpoint".to_owned(), Value::Object(descendant())),
                ]),
            ));
            rows.push(("hostScope".to_owned(), descendant()));
            // Deterministically classified from the parent's admission —
            // `complete` counts attempted OR parent-derived rows alike.
            Ok(envelope(true, rows, Vec::new(), &scope_text))
        }
        Ok(session) => {
            let mut base_row = state_row("known", None);
            base_row.insert(base_key, base_value);
            rows.push(("runtimeBase".to_owned(), base_row));
            let mut dir_row = state_row("known", None);
            let (dir_key, dir_value) = crate::native_path::detail(session.runtime_directory());
            dir_row.insert(dir_key, dir_value);
            rows.push(("runtimeDirectory".to_owned(), dir_row));

            let mut exhausted = false;
            let marker_row = step(deadline, &mut exhausted, || {
                entry_row(
                    native::observe_entry(&session, native::Entry::Marker),
                    Expect::MarkerFile,
                )
            });
            let activation_row = step(deadline, &mut exhausted, || {
                entry_row(
                    native::observe_entry(&session, native::Entry::ActivationLock),
                    Expect::RegularFile,
                )
            });
            let lifetime = step(deadline, &mut exhausted, || {
                lifetime_row(session.observe_session_lifetime_lock())
            });
            let client_row = step(deadline, &mut exhausted, || {
                entry_row(
                    native::observe_entry(&session, native::Entry::ClientSocket),
                    Expect::Socket,
                )
            });
            let worker_row = step(deadline, &mut exhausted, || {
                entry_row(
                    native::observe_entry(&session, native::Entry::WorkerSocket),
                    Expect::Socket,
                )
            });
            let pending_row = step(deadline, &mut exhausted, || {
                entry_row(
                    native::observe_entry(&session, native::Entry::ClientPendingSocket),
                    Expect::Socket,
                )
            });
            let observation_row = step(deadline, &mut exhausted, || {
                entry_row(
                    native::observe_entry(&session, native::Entry::AgentObservationSocket),
                    Expect::Socket,
                )
            });
            // The host endpoint is observed before the host directory row so
            // its parent admission re-check precedes the traversal.
            let host_endpoint = step(deadline, &mut exhausted, || {
                host_endpoint_row(native::observe_entry(&session, native::Entry::HostEndpoint))
            });
            let host_scope = step(deadline, &mut exhausted, || {
                entry_row(
                    native::observe_entry(&session, native::Entry::HostDirectory),
                    Expect::Directory,
                )
            });

            let emitted = advisories(
                row_state(&marker_row),
                row_state(&client_row),
                row_state(&worker_row),
            );

            rows.push(("marker".to_owned(), marker_row));
            rows.push((
                "locks".to_owned(),
                Map::from_iter([
                    ("activation".to_owned(), Value::Object(activation_row)),
                    ("sessionLifetime".to_owned(), Value::Object(lifetime)),
                ]),
            ));
            rows.push((
                "endpoints".to_owned(),
                Map::from_iter([
                    ("client".to_owned(), Value::Object(client_row)),
                    ("worker".to_owned(), Value::Object(worker_row)),
                    ("clientPending".to_owned(), Value::Object(pending_row)),
                    (
                        "agentObservation".to_owned(),
                        Value::Object(observation_row),
                    ),
                    ("hostEndpoint".to_owned(), Value::Object(host_endpoint)),
                ]),
            ));
            rows.push(("hostScope".to_owned(), host_scope));
            Ok(envelope(!exhausted, rows, emitted, &scope_text))
        }
    }
}

/// Assemble the `orchbee.server.diagnose` v1 envelope: `complete` is the
/// attempted-or-classified coverage fact, never a health verdict.
fn envelope(
    complete: bool,
    rows: Vec<(String, Map<String, Value>)>,
    advisories: Vec<&'static str>,
    scope: &str,
) -> DiagnoseDocument {
    let mut result = Map::from_iter([
        ("complete".to_owned(), json!(complete)),
        ("scope".to_owned(), json!(scope)),
        // Retained-record attribution belongs to the Windows private runtime catalog.
        (
            "unrecognizedRecords".to_owned(),
            json!({"kind":"unsupported"}),
        ),
    ]);
    for (name, row) in &rows {
        result.insert(name.clone(), Value::Object(row.clone()));
    }
    result.insert(
        "advisories".to_owned(),
        json!(advisories.iter().collect::<Vec<_>>()),
    );
    let mut result = Value::Object(result);
    complete_rows(&mut result);
    DiagnoseDocument {
        envelope: json!({
            "schema": "orchbee.server.diagnose",
            "version": 1,
            "source": crate::cli_outcome::local_runtime_source(),
            "result": result,
        }),
        rows,
        advisories,
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests;
