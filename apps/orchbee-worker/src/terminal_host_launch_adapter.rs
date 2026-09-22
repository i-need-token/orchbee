#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::{fmt, path::Path};

use orchbee_capability_node_protocol::{
    AbsoluteCwd, HostCreationId as CapabilityHostCreationId, HostEpoch as CapabilityHostEpoch,
    HostStartAccepted, HostTerminalId as CapabilityHostTerminalId, HostTerminalIdentity,
    ProtocolError as CapabilityProtocolError, StartTransition as CapabilityStartTransition,
    TerminalLaunchMode, TerminalPixelGeometry as CapabilityTerminalPixelGeometry,
};
use orchbee_terminal_protocol::{
    ConnectionControl, CreateTerminal, CreationId, EnvironmentOverlayEntry,
    HostEpoch as ProtocolHostEpoch, IdentityError, LaunchEnvironmentEntry, LaunchStartMode,
    LifecycleControl, MAXIMUM_ENVIRONMENT_OVERLAY_VALUE_BYTES, ProtocolError as HostProtocolError,
    StartTerminal, StartTransition as ProtocolStartTransition, TerminalConfiguration,
    TerminalCreationControl, TerminalId as ProtocolTerminalId, TerminalIdentity, TerminalLaunch,
    TerminalPixelGeometry as ProtocolTerminalPixelGeometry, TerminalProfile, TerminalStartAccepted,
    TypedFailure, encode_connection_control, encode_lifecycle_control,
    encode_terminal_creation_control,
};

use crate::terminal_authorization::{
    AuthorizedHostDispatch, AuthorizedHostOperation, LaunchHostCompletion, StartHostCompletion,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedTerminalLaunchFacts {
    arguments: Vec<String>,
    configuration: Option<TerminalConfiguration>,
    creation_id: CreationId,
    default_current_directory: AbsoluteCwd,
    environment: Vec<LaunchEnvironmentEntry>,
    executable: String,
    terminal_profile: TerminalProfile,
}

impl ResolvedTerminalLaunchFacts {
    pub(crate) const fn new(
        creation_id: CreationId,
        executable: String,
        arguments: Vec<String>,
        environment: Vec<LaunchEnvironmentEntry>,
        default_current_directory: AbsoluteCwd,
        terminal_profile: TerminalProfile,
        configuration: Option<TerminalConfiguration>,
    ) -> Self {
        Self {
            arguments,
            configuration,
            creation_id,
            default_current_directory,
            environment,
            executable,
            terminal_profile,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TerminalHostResponse {
    TerminalCreated(TerminalIdentity),
    TerminalStartAccepted(TerminalStartAccepted),
    TypedFailure(TypedFailure),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ObservedTerminalHostResponse {
    host_epoch: ProtocolHostEpoch,
    response: TerminalHostResponse,
}

impl ObservedTerminalHostResponse {
    pub(crate) const fn new(host_epoch: ProtocolHostEpoch, response: TerminalHostResponse) -> Self {
        Self {
            host_epoch,
            response,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostLaunchAdapterError {
    OperationKindMismatch,
    UnexpectedHostResponse,
    CreationIdentityMismatch,
    HostEpochMismatch,
    TerminalIdentityMismatch,
    GeometryMismatch,
    CapabilityProtocol(CapabilityProtocolError),
    HostIdentity(IdentityError),
    HostProtocol(HostProtocolError),
}

impl fmt::Display for HostLaunchAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OperationKindMismatch => {
                formatter.write_str("authorized Host dispatch has the wrong operation kind")
            }
            Self::UnexpectedHostResponse => {
                formatter.write_str("Terminal Host response does not answer the authorized request")
            }
            Self::CreationIdentityMismatch => formatter
                .write_str("Terminal Host creation identity does not match the resolved request"),
            Self::HostEpochMismatch => {
                formatter.write_str("Terminal Host response epoch is internally inconsistent")
            }
            Self::TerminalIdentityMismatch => formatter
                .write_str("Terminal Host response identity does not match the authorized target"),
            Self::GeometryMismatch => formatter.write_str(
                "Terminal Host response geometry does not match the authorized geometry",
            ),
            Self::CapabilityProtocol(error) => error.fmt(formatter),
            Self::HostIdentity(error) => error.fmt(formatter),
            Self::HostProtocol(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for HostLaunchAdapterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CapabilityProtocol(error) => Some(error),
            Self::HostIdentity(error) => Some(error),
            Self::HostProtocol(error) => Some(error),
            Self::OperationKindMismatch
            | Self::UnexpectedHostResponse
            | Self::CreationIdentityMismatch
            | Self::HostEpochMismatch
            | Self::TerminalIdentityMismatch
            | Self::GeometryMismatch => None,
        }
    }
}

impl From<CapabilityProtocolError> for HostLaunchAdapterError {
    fn from(error: CapabilityProtocolError) -> Self {
        Self::CapabilityProtocol(error)
    }
}

impl From<IdentityError> for HostLaunchAdapterError {
    fn from(error: IdentityError) -> Self {
        Self::HostIdentity(error)
    }
}

impl From<HostProtocolError> for HostLaunchAdapterError {
    fn from(error: HostProtocolError) -> Self {
        Self::HostProtocol(error)
    }
}

// Cross-owner convention with Server's private endpoint: the managed Site
// runtime directory holds the Agent observation socket under this exact file
// name. The Worker keeps its own literal copy; no Server code is imported.
#[cfg(unix)]
const OBSERVATION_SOCKET_NAME: &str = "agent-observation-v3.sock";
const OBSERVATION_SOCKET_ENV: &str = "ORCHBEE_AGENT_OBSERVATION_SOCKET_V3";

pub(crate) fn map_launch_dispatch(
    dispatch: &AuthorizedHostDispatch,
    facts: ResolvedTerminalLaunchFacts,
    site_runtime: Option<&Path>,
) -> Result<CreateTerminal, HostLaunchAdapterError> {
    let AuthorizedHostOperation::Launch(launch) = dispatch.operation() else {
        return Err(HostLaunchAdapterError::OperationKindMismatch);
    };
    let current_directory = launch
        .cwd()
        .unwrap_or(&facts.default_current_directory)
        .as_str()
        .to_owned();
    let start_mode = match launch.mode() {
        TerminalLaunchMode::Immediate(geometry) => {
            LaunchStartMode::Immediate(protocol_geometry(geometry))
        }
        TerminalLaunchMode::Deferred => LaunchStartMode::Deferred,
    };
    let control = TerminalCreationControl::CreateTerminal(Box::new(CreateTerminal {
        creation_id: facts.creation_id,
        environment_overlay: observation_overlay(site_runtime),
        launch: TerminalLaunch {
            arguments: facts.arguments,
            configuration: facts.configuration,
            current_directory: Some(current_directory),
            environment: facts.environment,
            executable: facts.executable,
            start_mode,
            terminal_profile: facts.terminal_profile,
        },
    }));
    encode_terminal_creation_control(&control)?;
    match control {
        TerminalCreationControl::CreateTerminal(request) => Ok(*request),
        TerminalCreationControl::TerminalCreated(_) => {
            Err(HostLaunchAdapterError::UnexpectedHostResponse)
        }
    }
}

pub(crate) fn map_start_dispatch(
    dispatch: &AuthorizedHostDispatch,
) -> Result<StartTerminal, HostLaunchAdapterError> {
    let AuthorizedHostOperation::Start(start) = dispatch.operation() else {
        return Err(HostLaunchAdapterError::OperationKindMismatch);
    };
    let request = StartTerminal {
        geometry: protocol_geometry(start.geometry()),
        terminal_id: ProtocolTerminalId::from_bytes(start.identity().terminal_id().as_bytes())?,
    };
    encode_lifecycle_control(&LifecycleControl::StartTerminal(request))?;
    Ok(request)
}

pub(crate) fn map_launch_host_response(
    dispatch: &AuthorizedHostDispatch,
    expected_creation_id: CreationId,
    observed: ObservedTerminalHostResponse,
) -> Result<LaunchHostCompletion, HostLaunchAdapterError> {
    let AuthorizedHostOperation::Launch(_) = dispatch.operation() else {
        return Err(HostLaunchAdapterError::OperationKindMismatch);
    };
    validate_host_response(&observed.response)?;
    match observed.response {
        TerminalHostResponse::TerminalCreated(identity) => {
            map_terminal_created(expected_creation_id, observed.host_epoch, identity)
        }
        TerminalHostResponse::TypedFailure(_) => map_reported_launch_failure(observed.host_epoch),
        TerminalHostResponse::TerminalStartAccepted(_) => {
            Err(HostLaunchAdapterError::UnexpectedHostResponse)
        }
    }
}

pub(crate) fn map_start_host_response(
    dispatch: &AuthorizedHostDispatch,
    observed: ObservedTerminalHostResponse,
) -> Result<StartHostCompletion, HostLaunchAdapterError> {
    let AuthorizedHostOperation::Start(start) = dispatch.operation() else {
        return Err(HostLaunchAdapterError::OperationKindMismatch);
    };
    validate_host_response(&observed.response)?;
    match observed.response {
        TerminalHostResponse::TerminalStartAccepted(accepted) => {
            map_terminal_start_accepted(start, observed.host_epoch, accepted)
        }
        TerminalHostResponse::TypedFailure(_) => {
            map_reported_start_failure(start, observed.host_epoch)
        }
        TerminalHostResponse::TerminalCreated(_) => {
            Err(HostLaunchAdapterError::UnexpectedHostResponse)
        }
    }
}

fn map_terminal_created(
    expected_creation_id: CreationId,
    response_epoch: ProtocolHostEpoch,
    identity: TerminalIdentity,
) -> Result<LaunchHostCompletion, HostLaunchAdapterError> {
    if identity.host_epoch != response_epoch {
        return Err(HostLaunchAdapterError::HostEpochMismatch);
    }
    if identity.creation_id != expected_creation_id {
        return Err(HostLaunchAdapterError::CreationIdentityMismatch);
    }
    Ok(LaunchHostCompletion::Accepted {
        identity: capability_identity(identity)?,
    })
}

// The overlay admits only a live socket owned by this Worker's effective user.
// A managed Site binds it before spawning the Worker, so a missing or
// unqualified candidate simply produces no variable — the launch keeps its
// resolved facts and the producer runs without an observation channel.
#[cfg(unix)]
fn observation_overlay(site_runtime: Option<&Path>) -> Vec<EnvironmentOverlayEntry> {
    let Some(runtime) = site_runtime else {
        return Vec::new();
    };
    let candidate = runtime.join(OBSERVATION_SOCKET_NAME);
    let admitted = std::fs::symlink_metadata(&candidate)
        .map(|metadata| {
            metadata.file_type().is_socket() && metadata.uid() == nix::unistd::geteuid().as_raw()
        })
        .unwrap_or(false);
    if !admitted {
        return Vec::new();
    }
    let Some(path) = candidate.to_str() else {
        return Vec::new();
    };
    if path.len() > MAXIMUM_ENVIRONMENT_OVERLAY_VALUE_BYTES {
        return Vec::new();
    }
    vec![EnvironmentOverlayEntry {
        name: OBSERVATION_SOCKET_ENV.to_owned(),
        value: path.to_owned(),
    }]
}

// Terminal Protocol retryability is retry guidance, not a rejection/failure discriminator.
fn map_reported_launch_failure(
    response_epoch: ProtocolHostEpoch,
) -> Result<LaunchHostCompletion, HostLaunchAdapterError> {
    let host_epoch = capability_host_epoch(response_epoch)?;
    Ok(LaunchHostCompletion::Failed { host_epoch })
}

fn map_terminal_start_accepted(
    start: &orchbee_capability_node_protocol::AuthorizedTerminalStart,
    response_epoch: ProtocolHostEpoch,
    accepted: TerminalStartAccepted,
) -> Result<StartHostCompletion, HostLaunchAdapterError> {
    if accepted.identity.host_epoch != response_epoch
        || response_epoch.as_bytes() != &start.identity().host_epoch().as_bytes()
    {
        return Err(HostLaunchAdapterError::HostEpochMismatch);
    }
    let expected_identity = protocol_identity(start.identity())?;
    if accepted.identity != expected_identity {
        return Err(HostLaunchAdapterError::TerminalIdentityMismatch);
    }
    if accepted.geometry != protocol_geometry(start.geometry()) {
        return Err(HostLaunchAdapterError::GeometryMismatch);
    }
    Ok(StartHostCompletion::Accepted(HostStartAccepted::new(
        capability_identity(accepted.identity)?,
        start.geometry(),
        match accepted.transition {
            ProtocolStartTransition::Started => CapabilityStartTransition::Started,
            ProtocolStartTransition::AlreadyStarted => CapabilityStartTransition::AlreadyStarted,
        },
    )))
}

fn map_reported_start_failure(
    start: &orchbee_capability_node_protocol::AuthorizedTerminalStart,
    response_epoch: ProtocolHostEpoch,
) -> Result<StartHostCompletion, HostLaunchAdapterError> {
    if response_epoch.as_bytes() != &start.identity().host_epoch().as_bytes() {
        return Err(HostLaunchAdapterError::HostEpochMismatch);
    }
    let host_epoch = capability_host_epoch(response_epoch)?;
    Ok(StartHostCompletion::Failed { host_epoch })
}

fn validate_host_response(response: &TerminalHostResponse) -> Result<(), HostProtocolError> {
    match response {
        TerminalHostResponse::TerminalCreated(identity) => {
            encode_terminal_creation_control(&TerminalCreationControl::TerminalCreated(*identity))?;
        }
        TerminalHostResponse::TerminalStartAccepted(accepted) => {
            encode_lifecycle_control(&LifecycleControl::TerminalStartAccepted(*accepted))?;
        }
        TerminalHostResponse::TypedFailure(failure) => {
            encode_connection_control(&ConnectionControl::TypedFailure(failure.clone()))?;
        }
    }
    Ok(())
}

fn protocol_identity(identity: HostTerminalIdentity) -> Result<TerminalIdentity, IdentityError> {
    Ok(TerminalIdentity {
        creation_id: CreationId::from_bytes(identity.creation_id().as_bytes())?,
        host_epoch: ProtocolHostEpoch::from_bytes(identity.host_epoch().as_bytes())?,
        terminal_id: ProtocolTerminalId::from_bytes(identity.terminal_id().as_bytes())?,
    })
}

pub(crate) fn capability_identity(
    identity: TerminalIdentity,
) -> Result<HostTerminalIdentity, CapabilityProtocolError> {
    Ok(HostTerminalIdentity::new(
        CapabilityHostCreationId::from_bytes(*identity.creation_id.as_bytes())?,
        capability_host_epoch(identity.host_epoch)?,
        CapabilityHostTerminalId::from_bytes(*identity.terminal_id.as_bytes())?,
    ))
}

fn capability_host_epoch(
    host_epoch: ProtocolHostEpoch,
) -> Result<CapabilityHostEpoch, CapabilityProtocolError> {
    CapabilityHostEpoch::from_bytes(*host_epoch.as_bytes())
}

const fn protocol_geometry(
    geometry: CapabilityTerminalPixelGeometry,
) -> ProtocolTerminalPixelGeometry {
    ProtocolTerminalPixelGeometry {
        cell_pixel_height: geometry.cell_pixel_height(),
        cell_pixel_width: geometry.cell_pixel_width(),
        columns: geometry.columns(),
        rows: geometry.rows(),
    }
}

#[cfg(test)]
mod tests;

#[cfg(windows)]
fn observation_overlay(site_runtime: Option<&Path>) -> Vec<EnvironmentOverlayEntry> {
    let Some(runtime) = site_runtime else {
        return Vec::new();
    };
    let candidate =
        orchbee_local_site_runtime::ModeCheckedLocalSiteRendezvous::observation_endpoint_path(
            runtime,
        );
    if !orchbee_local_ipc::endpoint_exists(&candidate).unwrap_or(false) {
        return Vec::new();
    }
    let Some(path) = candidate.to_str() else {
        return Vec::new();
    };
    if path.len() > MAXIMUM_ENVIRONMENT_OVERLAY_VALUE_BYTES {
        return Vec::new();
    }
    vec![EnvironmentOverlayEntry {
        name: OBSERVATION_SOCKET_ENV.to_owned(),
        value: path.to_owned(),
    }]
}
