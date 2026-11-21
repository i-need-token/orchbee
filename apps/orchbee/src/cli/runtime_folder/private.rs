//! Private-runtime report workflow over admitted file and unauthenticated endpoint-name facts.
use super::{Failure, FailureKind, RuntimeFolder};
use orchbee_local_runtime::{
    ActivationState, DataRoot, ModeCheckedLocalSessionRendezvous as Rendezvous, NamedSessionEntry,
    RecoveryBlocker, RendezvousError, SessionCatalog, SessionLifetimeObservation, SessionScope,
    UnrecognizedRecord, UnrecognizedRecordKind, UnrecognizedRecordReason,
};
use serde_json::{Map, Value, json};
use std::{
    io,
    path::Path,
    time::{Duration, Instant},
};

type Row = Map<String, Value>;

fn row(state: &str, reason: Option<&str>) -> Row {
    let mut result = Map::from_iter([("state".to_owned(), json!(state))]);
    if let Some(reason) = reason {
        result.insert("reason".to_owned(), json!(reason));
    }
    result
}

fn error_row(error: &io::Error) -> Row {
    match error.kind() {
        io::ErrorKind::PermissionDenied => row("uninspectable", Some("denied")),
        io::ErrorKind::InvalidInput => row("uninspectable", Some("invalid-path")),
        _ => row("uninspectable", Some("io-error")),
    }
}

fn path_row(path: &Path, mut result: Row) -> Row {
    let (key, value) = crate::native_path::detail(path);
    result.insert(key, value);
    result
}

fn private_row(state: &str) -> Row {
    let mut result = row(state, None);
    result.insert("private".to_owned(), json!(true));
    result
}

fn presence(observation: io::Result<bool>) -> Row {
    match observation {
        Ok(true) => private_row("known"),
        Ok(false) => row("absent", None),
        Err(error) => error_row(&error),
    }
}

fn step(deadline: Instant, complete: &mut bool, observe: impl FnOnce() -> Row) -> Row {
    if Instant::now() >= deadline {
        *complete = false;
        return row("uninspectable", Some("deadline"));
    }
    let result = observe();
    if result.get("reason").and_then(Value::as_str) == Some("observation-limit") {
        *complete = false;
    }
    result
}

pub(super) fn collect() -> Result<RuntimeFolder, Failure> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let (base, scope) = crate::local_activation::runtime_location()
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let unstarted = crate::local_activation::runtime_base_unstarted()
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let root = orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name)).ok();
    collect_at(&base, unstarted, root.as_ref(), &scope, deadline)
}

/// One retained record as found: what holds it, why this Session cannot admit it, and where.
fn record_row(record: &UnrecognizedRecord) -> Row {
    let mut result = Map::from_iter([
        (
            "record".to_owned(),
            json!(match record.kind {
                UnrecognizedRecordKind::CatalogRecord => "catalog-record",
            }),
        ),
        (
            "reason".to_owned(),
            json!(match record.reason {
                UnrecognizedRecordReason::Unreadable => "unreadable",
                UnrecognizedRecordReason::Version => "version",
                UnrecognizedRecordReason::LetterCase => "letter-case",
            }),
        ),
    ]);
    let (key, value) = crate::native_path::detail(&record.path);
    result.insert(key, value);
    result
}

/// Records that refuse this Session, read without admitting, migrating, or moving any of them.
fn unrecognized(root: Option<&DataRoot>, scope: &str, deadline: Instant) -> (Row, Vec<Row>) {
    let observed = root
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        .and_then(|root| SessionCatalog::unrecognized_records(root, scope, deadline));
    match observed {
        Ok(records) => (row("known", None), records.iter().map(record_row).collect()),
        Err(error) => (
            match error.kind() {
                io::ErrorKind::NotFound => row("uninspectable", Some("parent-unavailable")),
                io::ErrorKind::TimedOut => row("uninspectable", Some("deadline")),
                io::ErrorKind::FileTooLarge => row("uninspectable", Some("observation-limit")),
                _ => error_row(&error),
            },
            Vec::new(),
        ),
    }
}

/// `unstarted` marks an environment whose default runtime base was never created: nothing has
/// started there, so the base is absent and no record can name it.
fn collect_at(
    base: &Path,
    unstarted: bool,
    root: Option<&DataRoot>,
    scope: &str,
    deadline: Instant,
) -> Result<RuntimeFolder, Failure> {
    let scope_id =
        SessionScope::parse(scope).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let mut complete = true;
    let mut session = None;
    let directory = step(
        deadline,
        &mut complete,
        || match Rendezvous::validate_mode_checked(base, scope_id) {
            Ok(admitted) => {
                session = Some(admitted);
                private_row("known")
            }
            Err(RendezvousError::System { source, .. })
                if source.kind() == io::ErrorKind::NotFound =>
            {
                row("absent", None)
            }
            Err(RendezvousError::System { source, .. }) => error_row(&source),
            Err(_) => row("uninspectable", Some("invalid-path")),
        },
    );
    let mut rows = vec![
        (
            "runtimeBase".to_owned(),
            path_row(base, row(if unstarted { "absent" } else { "known" }, None)),
        ),
        (
            "runtimeDirectory".to_owned(),
            path_row(&base.join(scope), directory),
        ),
    ];
    let mut advisories = Vec::new();
    let descendant = || row("uninspectable", Some("parent-unavailable"));
    let (marker, activation, lifetime, endpoints) = if let Some(session) = session {
        let marker = step(
            deadline,
            &mut complete,
            || match super::native::activation_state(&session) {
                Ok(ActivationState::Absent) => row("absent", None),
                Ok(state) => {
                    let mut result = private_row("known");
                    let stopped = matches!(state, ActivationState::Stopped { .. });
                    result.insert(
                        "activation".to_owned(),
                        json!(if stopped { "stopped" } else { "active" }),
                    );
                    if let ActivationState::Stopped { request_id } = state {
                        let identity: String = request_id
                            .iter()
                            .map(|byte| format!("{byte:02x}"))
                            .collect();
                        result.insert("stopRequestId".to_owned(), json!(identity));
                    }
                    result
                }
                Err(error) => error_row(&error),
            },
        );
        let activation = step(deadline, &mut complete, || {
            presence(super::native::activation_lock_exists(&session))
        });
        let lifetime = step(deadline, &mut complete, || {
            match session.observe_session_lifetime_lock() {
                Ok(SessionLifetimeObservation::Absent) => row("absent", None),
                Ok(SessionLifetimeObservation::Free) => private_row("free"),
                Ok(SessionLifetimeObservation::Held) => private_row("held"),
                Ok(SessionLifetimeObservation::Unsafe) => row("unsafe", Some("denied")),
                Err(error) => error_row(&error),
            }
        });
        let mut endpoints = Map::new();
        for (name, path) in [
            ("client", session.client_socket_path().to_owned()),
            ("worker", session.worker_socket_path().to_owned()),
            (
                "clientPending",
                Rendezvous::client_pending_socket_path(session.runtime_directory()),
            ),
            (
                "agentObservation",
                super::native::observation_endpoint_path(session.runtime_directory()).map_err(
                    |super::native::Unsupported| {
                        Failure::new(FailureKind::ServiceUnavailable).details(
                            crate::cli::stage_reason_details("platform", "unsupported-platform"),
                        )
                    },
                )?,
            ),
            (
                "hostEndpoint",
                Rendezvous::host_endpoint_path(session.runtime_directory()),
            ),
        ] {
            let observed =
                step(
                    deadline,
                    &mut complete,
                    || match super::native::endpoint_exists_before(&path, deadline) {
                        Ok(Some(true)) => row("known", None),
                        Ok(Some(false)) => row("absent", None),
                        Ok(None) => row("uninspectable", Some("observation-limit")),
                        Err(error) => error_row(&error),
                    },
                );
            if matches!(name, "client" | "worker")
                && observed.get("state") == Some(&json!("known"))
                && !advisories.contains(&"pipe-name-present")
            {
                advisories.push("pipe-name-present");
            }
            endpoints.insert(name.to_owned(), Value::Object(path_row(&path, observed)));
        }
        if marker.get("activation") == Some(&json!("active")) {
            advisories.insert(0, "activation-recorded");
        }
        (marker, activation, lifetime, endpoints)
    } else {
        let endpoints = [
            "client",
            "worker",
            "clientPending",
            "agentObservation",
            "hostEndpoint",
        ]
        .into_iter()
        .map(|key| (key.to_owned(), Value::Object(descendant())))
        .collect();
        (descendant(), descendant(), descendant(), endpoints)
    };
    rows.extend([
        ("marker".to_owned(), marker),
        (
            "locks".to_owned(),
            Map::from_iter([
                ("activation".to_owned(), Value::Object(activation)),
                ("sessionLifetime".to_owned(), Value::Object(lifetime)),
            ]),
        ),
        ("endpoints".to_owned(), endpoints),
    ]);
    let mut records = Vec::new();
    let summary = step(deadline, &mut complete, || {
        let (summary, observed) = if unstarted {
            (row("known", None), Vec::new())
        } else {
            unrecognized(root, scope, deadline)
        };
        records = observed;
        summary
    });
    if summary.get("reason") == Some(&json!("deadline")) {
        complete = false;
    }
    if !records.is_empty() {
        advisories.push("unrecognized-records");
    }
    let blocker = if unstarted {
        None
    } else if Instant::now() >= deadline {
        complete = false;
        None
    } else {
        match recovery_blocker(base, root, scope, deadline) {
            Ok(blocker) => blocker,
            // Only a spent budget leaves the report incomplete; an unreadable record names none.
            Err(error) => {
                complete &= error.kind() != io::ErrorKind::TimedOut;
                None
            }
        }
    };
    // What blocks a start leads, so the check's warning names it.
    if blocker.is_some() {
        advisories.insert(0, "recovery-blocked");
    }
    let mut result = Map::from_iter([
        ("complete".to_owned(), json!(complete)),
        ("scope".to_owned(), json!(scope)),
        ("advisories".to_owned(), json!(advisories)),
        ("hostScope".to_owned(), json!({"kind":"unsupported"})),
    ]);
    result.extend(
        rows.into_iter()
            .map(|(key, value)| (key, Value::Object(value))),
    );
    let mut result = Value::Object(result);
    super::complete_rows(&mut result);
    // A list member rather than a row: it is completed here.
    let mut member = summary;
    member.entry("reason").or_insert(Value::Null);
    member.insert(
        "records".to_owned(),
        records
            .into_iter()
            .map(|mut record| {
                for key in ["path", "pathUtf16"] {
                    record.entry(key).or_insert(Value::Null);
                }
                Value::Object(record)
            })
            .collect(),
    );
    result["unrecognizedRecords"] = Value::Object(member);
    if let Some(blocker) = blocker {
        result["blockingProcess"] = crate::cli::blocking_process::member(blocker);
    }
    Ok(RuntimeFolder { result, advisories })
}

/// The registered process that keeps a start from recovering this Session, read from its record by
/// the observation `session list` uses for `recoverable`. No Server is contacted and nothing ends.
fn recovery_blocker(
    base: &Path,
    root: Option<&DataRoot>,
    scope: &str,
    deadline: Instant,
) -> io::Result<Option<RecoveryBlocker>> {
    let Some(root) = root else {
        return Ok(None);
    };
    SessionCatalog::list(base, root, deadline)?
        .iter()
        .find(|entry| entry.name.as_str().eq_ignore_ascii_case(scope))
        .map_or(Ok(None), NamedSessionEntry::recovery_blocker)
}

#[cfg(all(test, windows))]
mod tests;
