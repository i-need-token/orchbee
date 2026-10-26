//! Windows-only projection of admitted file and unauthenticated pipe-name facts.
use super::{DiagnoseDocument, Failure, FailureKind};
use crate::cli::config;
use orchbee_local_runtime::{
    ActivationState, ModeCheckedLocalSessionRendezvous as Rendezvous, RendezvousError,
    SessionLifetimeObservation, SessionScope,
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
    let (key, value) = config::path_detail(path);
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

pub(super) fn collect() -> Result<DiagnoseDocument, Failure> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let (base, scope) = crate::local_activation::runtime_location()
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    collect_at(&base, &scope, deadline)
}

fn collect_at(base: &Path, scope: &str, deadline: Instant) -> Result<DiagnoseDocument, Failure> {
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
        ("runtimeBase".to_owned(), path_row(base, row("known", None))),
        (
            "runtimeDirectory".to_owned(),
            path_row(&base.join(scope), directory),
        ),
    ];
    let mut advisories = Vec::new();
    let descendant = || row("uninspectable", Some("parent-unavailable"));
    let (marker, activation, lifetime, endpoints) = if let Some(session) = session {
        let marker = step(deadline, &mut complete, || {
            match session.activation_state() {
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
            }
        });
        let activation = step(deadline, &mut complete, || {
            presence(session.activation_lock_exists())
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
                Rendezvous::observation_endpoint_path(session.runtime_directory()),
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
                    || match orchbee_local_ipc::endpoint_exists_before(&path, deadline) {
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
    let mut result = Map::from_iter([
        ("complete".to_owned(), json!(complete)),
        ("scope".to_owned(), json!(scope)),
        ("advisories".to_owned(), json!(advisories)),
        ("hostScope".to_owned(), json!({"kind":"unsupported"})),
    ]);
    result.extend(
        rows.iter()
            .map(|(key, value)| (key.clone(), Value::Object(value.clone()))),
    );
    let mut result = Value::Object(result);
    super::complete_rows(&mut result);
    Ok(DiagnoseDocument {
        envelope: json!({"schema":"orchbee.server.diagnose", "version":1,
            "source":config::local_runtime_source(), "result":result}),
        rows,
        advisories,
    })
}

#[cfg(test)]
mod tests;
