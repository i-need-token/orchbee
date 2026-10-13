//! `server diagnose`: bounded read-only observation of the implicit per-euid
//! local Site runtime scope. It inspects a finite set of known paths through
//! the rendezvous' own validated directory descriptor — no endpoint is
//! connected, no lock file is created, no marker or socket is touched, and
//! no repair is attempted or implied. Every row is a pathname or descriptor
//! fact at its own observation instant: `held` means some owner held the
//! lifetime lock then, never that a Site or any process is alive; `absent`
//! is a pathname fact, never proof that an execution ended.

#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use super::{Failure, FailureKind, Output, failure_output, output};
use crate::invocation::OutputFormat;
use crate::localization::Locale;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows::collect;

#[cfg(target_os = "linux")]
use orchbee_local_site_runtime::{
    EntryKind, EntryObservation, ModeCheckedLocalSiteRendezvous, RendezvousError,
    SiteLifetimeObservation, SiteScope,
};
#[cfg(all(test, target_os = "linux"))]
use orchbee_local_site_runtime::{LockAttempt, LockKind};

/// A cooperative budget checked between bounded filesystem operations —
/// not a hard wall promise on synchronous filesystems.
#[cfg(target_os = "linux")]
const DIAGNOSE_BUDGET: Duration = Duration::from_secs(5);

/// The advisory emitted when a valid activation marker was observed.
#[cfg(target_os = "linux")]
const ADVISORY_ACTIVATION_RECORDED: &str = "activation-recorded";
/// The advisory emitted when a valid client or worker socket residue was
/// observed — the other half of the current activation predicate.
#[cfg(target_os = "linux")]
const ADVISORY_SOCKET_RESIDUE: &str = "socket-residue";

/// The collected diagnose product: the machine envelope plus the row table
/// and advisory tokens the text renderer shares with it.
pub(crate) struct DiagnoseDocument {
    #[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
    pub(crate) envelope: Value,
    #[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
    pub(crate) rows: Vec<(String, Map<String, Value>)>,
    #[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
    pub(crate) advisories: Vec<&'static str>,
}

pub(crate) fn run(format: OutputFormat, locale: Locale) -> Output {
    match collect() {
        // Like `server start`, every `server diagnose` answer — failure
        // included — comes from the local runtime authority.
        Err(failure) => failure_output(
            failure.source(super::config::local_runtime_source()),
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
                        super::present::site_diagnose(out, &document, locale)
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
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, PartialEq)]
enum Expect {
    Socket,
    RegularFile,
    /// The activation marker: a regular file the predicate additionally
    /// requires to have exactly one link and zero bytes.
    MarkerFile,
    Directory,
}

#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
fn row_state(row: &Map<String, Value>) -> &str {
    row.get("state").and_then(Value::as_str).unwrap_or("")
}

#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
fn host_endpoint_row(observation: std::io::Result<EntryObservation>) -> Map<String, Value> {
    match observation {
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            state_row("uninspectable", Some("unsafe-parent"))
        }
        other => entry_row(other, Expect::Socket),
    }
}

#[cfg(target_os = "linux")]
fn lifetime_row(observation: std::io::Result<SiteLifetimeObservation>) -> Map<String, Value> {
    match observation {
        Ok(SiteLifetimeObservation::Absent) => state_row("absent", None),
        Ok(SiteLifetimeObservation::Free) => state_row("free", None),
        Ok(SiteLifetimeObservation::Held) => state_row("held", None),
        Ok(SiteLifetimeObservation::Unsafe) => state_row("unsafe", Some("invalid-lock")),
        Err(_) => state_row("uninspectable", Some("io-error")),
    }
}

/// The row for a descendant under an unadmitted runtime directory: the
/// parent verdict selects `unsafe-parent` (unsafe to traverse) versus
/// `parent-unavailable` (absent, invalid, or itself uninspectable).
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
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
        | RendezvousError::InvalidSiteScope => ("uninspectable", Some("invalid-path")),
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
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
fn collect() -> Result<DiagnoseDocument, Failure> {
    let base = std::env::var_os("ORCHBEE_RUNTIME_BASE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let deadline = Instant::now()
        .checked_add(DIAGNOSE_BUDGET)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    collect_at(&base, deadline)
}

/// The bounded collection itself, split from environment reads so tests can
/// drive every row class against a scratch runtime base and a past deadline.
#[cfg(target_os = "linux")]
fn collect_at(base: &Path, deadline: Instant) -> Result<DiagnoseDocument, Failure> {
    let scope_text = format!("orchbee-native-{}", nix::unistd::geteuid().as_raw());
    let scope =
        SiteScope::parse(&scope_text).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let scope_directory = base.join(&scope_text);
    let site = ModeCheckedLocalSiteRendezvous::validate_mode_checked(base, scope);

    let mut rows: Vec<(String, Map<String, Value>)> = Vec::with_capacity(6);
    let (base_key, base_value) = super::config::path_detail(base);

    match site {
        Err(error) => {
            let (mut dir_row, dir_state, dir_reason, parent_unsafe) = classify_directory(&error);
            let (dir_key, dir_value) = super::config::path_detail(&scope_directory);
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
                    ("siteLifetime".to_owned(), Value::Object(descendant())),
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
        Ok(site) => {
            let mut base_row = state_row("known", None);
            base_row.insert(base_key, base_value);
            rows.push(("runtimeBase".to_owned(), base_row));
            let mut dir_row = state_row("known", None);
            let (dir_key, dir_value) = super::config::path_detail(site.runtime_directory());
            dir_row.insert(dir_key, dir_value);
            rows.push(("runtimeDirectory".to_owned(), dir_row));

            let mut exhausted = false;
            let marker_row = step(deadline, &mut exhausted, || {
                entry_row(site.observe_activation_marker(), Expect::MarkerFile)
            });
            let activation_row = step(deadline, &mut exhausted, || {
                entry_row(site.observe_activation_lock(), Expect::RegularFile)
            });
            let lifetime = step(deadline, &mut exhausted, || {
                lifetime_row(site.observe_site_lifetime_lock())
            });
            let client_row = step(deadline, &mut exhausted, || {
                entry_row(site.observe_client_socket(), Expect::Socket)
            });
            let worker_row = step(deadline, &mut exhausted, || {
                entry_row(site.observe_worker_socket(), Expect::Socket)
            });
            let pending_row = step(deadline, &mut exhausted, || {
                entry_row(site.observe_client_pending_socket(), Expect::Socket)
            });
            let observation_row = step(deadline, &mut exhausted, || {
                entry_row(site.observe_agent_observation_socket(), Expect::Socket)
            });
            // The host endpoint is observed before the host directory row so
            // its parent admission re-check precedes the traversal.
            let host_endpoint = step(deadline, &mut exhausted, || {
                host_endpoint_row(site.observe_host_endpoint())
            });
            let host_scope = step(deadline, &mut exhausted, || {
                entry_row(site.observe_host_directory(), Expect::Directory)
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
                    ("siteLifetime".to_owned(), Value::Object(lifetime)),
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

/// Assemble the `orchbee.site.diagnose` v1 envelope: `complete` is the
/// attempted-or-classified coverage fact, never a health verdict.
#[cfg(target_os = "linux")]
fn envelope(
    complete: bool,
    rows: Vec<(String, Map<String, Value>)>,
    advisories: Vec<&'static str>,
    scope: &str,
) -> DiagnoseDocument {
    let mut result = Map::from_iter([
        ("complete".to_owned(), json!(complete)),
        ("scope".to_owned(), json!(scope)),
    ]);
    for (name, row) in &rows {
        result.insert(name.clone(), Value::Object(row.clone()));
    }
    result.insert(
        "advisories".to_owned(),
        json!(advisories.iter().collect::<Vec<_>>()),
    );
    DiagnoseDocument {
        envelope: json!({
            "schema": "orchbee.site.diagnose",
            "version": 1,
            "source": super::config::local_runtime_source(),
            "result": result,
        }),
        rows,
        advisories,
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
fn collect() -> Result<DiagnoseDocument, Failure> {
    Err(
        Failure::new(FailureKind::ServiceUnavailable).details(super::stage_reason_details(
            "platform",
            "unsupported-platform",
        )),
    )
}

#[cfg(all(test, target_os = "linux"))]
mod tests;
