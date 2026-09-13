//! The single read-only Site query: existing-only rendezvous, one OBLC
//! admission, one snapshot read, then the session is dropped. Nothing here may
//! create directories, acquire locks, start processes, or mutate the Workbench.

#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};
#[cfg(target_os = "linux")]
use std::{env, path::PathBuf};

#[cfg(target_os = "linux")]
use orchbee_local_client_protocol::ConnectClientError;
#[cfg(target_os = "linux")]
use orchbee_local_site_runtime::{
    ModeCheckedLocalSiteRendezvous, NativeCohort, RendezvousError, SiteScope,
};
#[cfg(target_os = "linux")]
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, SnapshotReadIntent, WorkbenchRequestId, WorkbenchSnapshotOutcome,
};

#[cfg(target_os = "linux")]
use super::{Failure, FailureKind, stage_details, stage_reason_details};
#[cfg(target_os = "linux")]
use crate::local_client::LocalClient;
#[cfg(target_os = "linux")]
use crate::local_client::workbench::{SnapshotReadError, SnapshotReadFailure};

#[cfg(target_os = "linux")]
const COHORT_BUDGET: Duration = Duration::from_secs(5);
#[cfg(target_os = "linux")]
const NETWORK_BUDGET: Duration = Duration::from_secs(5);

#[cfg(target_os = "linux")]
pub(crate) fn read_snapshot() -> Result<Snapshot, Failure> {
    let cohort_deadline = Instant::now()
        .checked_add(COHORT_BUDGET)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    let cohort = NativeCohort::current(cohort_deadline)
        .map_err(|_| Failure::new(FailureKind::CheckFailed).details(stage_details("cohort")))?;
    let site = rendezvous()?;
    match site.client_socket_exists() {
        Ok(true) => {}
        Ok(false) => {
            return Err(
                Failure::new(FailureKind::ServiceUnavailable).details(stage_details("discovery"))
            );
        }
        Err(_) => {
            return Err(Failure::new(FailureKind::CheckFailed).details(stage_details("socket")));
        }
    }
    let deadline = Instant::now()
        .checked_add(NETWORK_BUDGET)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    let mut client =
        LocalClient::connect_before(site.client_socket_path(), cohort.identity(), deadline)
            .map_err(classify_connect)?;
    let intent = SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: request_id()?,
    };
    let result = client
        .submit_workbench_snapshot_read_before(intent, deadline)
        .map_err(classify_read);
    // The Client Session is fully dropped before the outcome is examined or
    // any output is prepared.
    drop(client);
    let result = result?;
    match result.outcome {
        WorkbenchSnapshotOutcome::Available { snapshot } => Ok(snapshot),
        WorkbenchSnapshotOutcome::Denied {} => Err(Failure::new(FailureKind::Denied)),
        WorkbenchSnapshotOutcome::Unavailable { .. } => {
            Err(Failure::new(FailureKind::ServiceUnavailable).details(stage_details("snapshot")))
        }
    }
}

#[cfg(target_os = "linux")]
fn rendezvous() -> Result<ModeCheckedLocalSiteRendezvous, Failure> {
    let base = env::var_os("ORCHBEE_RUNTIME_BASE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let scope_text = format!("orchbee-native-{}", nix::unistd::geteuid().as_raw());
    let scope =
        SiteScope::parse(&scope_text).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let scope_directory = base.join(&scope_text);
    ModeCheckedLocalSiteRendezvous::validate_mode_checked(&base, scope)
        .map_err(|error| classify_rendezvous(error, &scope_directory))
}

/// Split rendezvous failures by the typed path they carried: an absent or
/// unsafe Site scope belongs to `runtime-directory`, while anything rooted
/// above it belongs to `runtime-base`. A scope ENOENT is service-unavailable
/// discovery, never a configuration failure.
#[cfg(target_os = "linux")]
fn classify_rendezvous(error: RendezvousError, scope_directory: &std::path::Path) -> Failure {
    let path = rendezvous_path(&error);
    let stage = if path == Some(scope_directory) {
        "runtime-directory"
    } else {
        "runtime-base"
    };
    if let RendezvousError::System { source, .. } = &error
        && source.kind() == std::io::ErrorKind::NotFound
        && path == Some(scope_directory)
    {
        return Failure::new(FailureKind::ServiceUnavailable).details(stage_details("discovery"));
    }
    Failure::new(FailureKind::CheckFailed).details(stage_details(stage))
}

#[cfg(target_os = "linux")]
fn rendezvous_path(error: &RendezvousError) -> Option<&std::path::Path> {
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
fn classify_connect(error: crate::local_client::LocalClientConnectError) -> Failure {
    use orchbee_local_client_protocol::{ProtocolError, RejectionReason};
    match error {
        crate::local_client::LocalClientConnectError::Admission(connect) => match connect {
            ConnectClientError::ConnectFailed(_) => {
                Failure::new(FailureKind::ServiceUnavailable).details(stage_details("admission"))
            }
            ConnectClientError::Io(_) => {
                Failure::new(FailureKind::TransportLoss).details(stage_details("admission"))
            }
            ConnectClientError::Rejected(reason) => match reason {
                RejectionReason::CapacityExceeded => {
                    Failure::new(FailureKind::SessionCapacity).details(stage_details("admission"))
                }
                RejectionReason::HandshakeTimeout => Failure::new(FailureKind::ServiceUnavailable)
                    .details(stage_details("admission")),
                RejectionReason::IncompatibleProtocol | RejectionReason::IncompatibleCohort => {
                    Failure::new(FailureKind::Incompatible).details(stage_details("admission"))
                }
                RejectionReason::WrongMessage | RejectionReason::MalformedFrame => {
                    Failure::new(FailureKind::ProtocolInvalid).details(stage_details("admission"))
                }
            },
            ConnectClientError::Protocol(ProtocolError::IncompatibleVersion { .. }) => {
                Failure::new(FailureKind::Incompatible).details(stage_details("admission"))
            }
            ConnectClientError::Protocol(_) | ConnectClientError::UnexpectedResponse => {
                Failure::new(FailureKind::ProtocolInvalid).details(stage_details("admission"))
            }
        },
        crate::local_client::LocalClientConnectError::RequestIoConfiguration(_) => {
            Failure::new(FailureKind::InternalFailure)
        }
    }
}

#[cfg(target_os = "linux")]
fn classify_read(error: SnapshotReadError) -> Failure {
    use orchbee_workbench_protocol::WorkbenchWireError;
    match error {
        SnapshotReadError::SessionUnavailable | SnapshotReadError::RequestEncoding(_) => {
            Failure::new(FailureKind::InternalFailure)
        }
        SnapshotReadError::RequestNotSubmitted(_) => {
            Failure::new(FailureKind::TransportLoss).details(stage_details("snapshot"))
        }
        SnapshotReadError::Failed { reason, .. } => match reason {
            SnapshotReadFailure::Write(_) | SnapshotReadFailure::ResponseDeadline(_) => {
                Failure::new(FailureKind::TransportLoss).details(stage_details("snapshot"))
            }
            SnapshotReadFailure::Response(WorkbenchWireError::Io(_)) => {
                Failure::new(FailureKind::TransportLoss).details(stage_details("snapshot"))
            }
            SnapshotReadFailure::Response(_) | SnapshotReadFailure::UnexpectedResponse => {
                Failure::new(FailureKind::ProtocolInvalid).details(stage_details("snapshot"))
            }
            SnapshotReadFailure::CorrelationMismatch { .. } => {
                Failure::new(FailureKind::ProtocolInvalid)
                    .details(stage_reason_details("snapshot", "correlation-mismatch"))
            }
        },
    }
}

/// Sixteen fresh bytes become the canonical 32-lowercase-hex request identity.
#[cfg(target_os = "linux")]
fn request_id() -> Result<WorkbenchRequestId, Failure> {
    use std::io::Read;
    let mut bytes = [0_u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let mut text = String::with_capacity(32);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(text, "{byte:02x}");
    }
    WorkbenchRequestId::parse(&text).map_err(|_| Failure::new(FailureKind::InternalFailure))
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn read_snapshot() -> Result<orchbee_workbench_protocol::Snapshot, super::Failure> {
    Err(
        super::Failure::new(super::FailureKind::ServiceUnavailable).details(
            super::stage_reason_details("platform", "unsupported-platform"),
        ),
    )
}
