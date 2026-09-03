use std::fmt;

use orchbee_capability_node_protocol::{
    AbsoluteCwd, HostCreationId as CapabilityHostCreationId, HostEpoch as CapabilityHostEpoch,
    HostStartAccepted, HostTerminalId as CapabilityHostTerminalId, HostTerminalIdentity,
    ProtocolError as CapabilityProtocolError, StartTransition as CapabilityStartTransition,
    TerminalLaunchMode, TerminalPixelGeometry as CapabilityTerminalPixelGeometry,
};
use orchbee_terminal_protocol::{
    ConnectionControl, CreateTerminal, CreationId, HostEpoch as ProtocolHostEpoch, IdentityError,
    LaunchEnvironmentEntry, LaunchStartMode, LifecycleControl, ProtocolError as HostProtocolError,
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
    default_current_directory: AbsoluteCwd,
    environment: Vec<LaunchEnvironmentEntry>,
    executable: String,
    terminal_profile: TerminalProfile,
}

impl ResolvedTerminalLaunchFacts {
    pub(crate) const fn new(
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
pub(crate) enum HostKernelCompletion {
    Launch(LaunchHostCompletion),
    Start(StartHostCompletion),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalHostTransportOutcome {
    UnavailableBeforeHandoff,
    ResponseUnknownAfterHandoff,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostKernelTransportObservation {
    PreHandoffFailure,
    OutcomeUnknown,
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
                .write_str("Terminal Host creation identity does not match the product terminal"),
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

pub(crate) fn map_launch_dispatch(
    dispatch: &AuthorizedHostDispatch,
    facts: ResolvedTerminalLaunchFacts,
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
        creation_id: creation_id_for(launch.authorization().terminal_id())?,
        environment_overlay: Vec::new(),
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
    validate_start_target(start)?;
    let request = StartTerminal {
        geometry: protocol_geometry(start.geometry()),
        terminal_id: ProtocolTerminalId::from_bytes(start.identity().terminal_id().as_bytes())?,
    };
    encode_lifecycle_control(&LifecycleControl::StartTerminal(request))?;
    Ok(request)
}

pub(crate) fn map_host_response(
    dispatch: &AuthorizedHostDispatch,
    observed: ObservedTerminalHostResponse,
) -> Result<HostKernelCompletion, HostLaunchAdapterError> {
    validate_host_response(&observed.response)?;
    match (dispatch.operation(), observed.response) {
        (
            AuthorizedHostOperation::Launch(launch),
            TerminalHostResponse::TerminalCreated(identity),
        ) => map_terminal_created(
            launch.authorization().terminal_id(),
            observed.host_epoch,
            identity,
        )
        .map(HostKernelCompletion::Launch),
        (AuthorizedHostOperation::Launch(_), TerminalHostResponse::TypedFailure(failure)) => Ok(
            HostKernelCompletion::Launch(map_launch_failure(observed.host_epoch, failure)?),
        ),
        (
            AuthorizedHostOperation::Start(start),
            TerminalHostResponse::TerminalStartAccepted(accepted),
        ) => map_terminal_start_accepted(start, observed.host_epoch, accepted)
            .map(HostKernelCompletion::Start),
        (AuthorizedHostOperation::Start(start), TerminalHostResponse::TypedFailure(failure)) => {
            map_start_failure(start, observed.host_epoch, failure).map(HostKernelCompletion::Start)
        }
        (AuthorizedHostOperation::Launch(_), TerminalHostResponse::TerminalStartAccepted(_))
        | (AuthorizedHostOperation::Start(_), TerminalHostResponse::TerminalCreated(_)) => {
            Err(HostLaunchAdapterError::UnexpectedHostResponse)
        }
    }
}

#[must_use]
pub(crate) const fn map_host_transport_outcome(
    outcome: TerminalHostTransportOutcome,
) -> HostKernelTransportObservation {
    match outcome {
        TerminalHostTransportOutcome::UnavailableBeforeHandoff => {
            HostKernelTransportObservation::PreHandoffFailure
        }
        TerminalHostTransportOutcome::ResponseUnknownAfterHandoff => {
            HostKernelTransportObservation::OutcomeUnknown
        }
    }
}

fn map_terminal_created(
    product_terminal_id: orchbee_capability_node_protocol::TerminalId,
    response_epoch: ProtocolHostEpoch,
    identity: TerminalIdentity,
) -> Result<LaunchHostCompletion, HostLaunchAdapterError> {
    if identity.host_epoch != response_epoch {
        return Err(HostLaunchAdapterError::HostEpochMismatch);
    }
    if identity.creation_id != creation_id_for(product_terminal_id)? {
        return Err(HostLaunchAdapterError::CreationIdentityMismatch);
    }
    Ok(LaunchHostCompletion::Accepted {
        identity: capability_identity(identity)?,
    })
}

fn map_launch_failure(
    response_epoch: ProtocolHostEpoch,
    failure: TypedFailure,
) -> Result<LaunchHostCompletion, HostLaunchAdapterError> {
    let host_epoch = capability_host_epoch(response_epoch)?;
    if failure.retryable {
        Ok(LaunchHostCompletion::Failed { host_epoch })
    } else {
        Ok(LaunchHostCompletion::Rejected { host_epoch })
    }
}

fn map_terminal_start_accepted(
    start: &orchbee_capability_node_protocol::AuthorizedTerminalStart,
    response_epoch: ProtocolHostEpoch,
    accepted: TerminalStartAccepted,
) -> Result<StartHostCompletion, HostLaunchAdapterError> {
    validate_start_target(start)?;
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

fn map_start_failure(
    start: &orchbee_capability_node_protocol::AuthorizedTerminalStart,
    response_epoch: ProtocolHostEpoch,
    failure: TypedFailure,
) -> Result<StartHostCompletion, HostLaunchAdapterError> {
    validate_start_target(start)?;
    if response_epoch.as_bytes() != &start.identity().host_epoch().as_bytes() {
        return Err(HostLaunchAdapterError::HostEpochMismatch);
    }
    let host_epoch = capability_host_epoch(response_epoch)?;
    if failure.retryable {
        Ok(StartHostCompletion::Failed { host_epoch })
    } else {
        Ok(StartHostCompletion::Rejected { host_epoch })
    }
}

fn validate_start_target(
    start: &orchbee_capability_node_protocol::AuthorizedTerminalStart,
) -> Result<(), HostLaunchAdapterError> {
    if creation_id_for(start.authorization().terminal_id())?.as_bytes()
        != &start.identity().creation_id().as_bytes()
    {
        return Err(HostLaunchAdapterError::CreationIdentityMismatch);
    }
    Ok(())
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

fn creation_id_for(
    product_terminal_id: orchbee_capability_node_protocol::TerminalId,
) -> Result<CreationId, IdentityError> {
    CreationId::from_bytes(product_terminal_id.as_bytes())
}

fn protocol_identity(identity: HostTerminalIdentity) -> Result<TerminalIdentity, IdentityError> {
    Ok(TerminalIdentity {
        creation_id: CreationId::from_bytes(identity.creation_id().as_bytes())?,
        host_epoch: ProtocolHostEpoch::from_bytes(identity.host_epoch().as_bytes())?,
        terminal_id: ProtocolTerminalId::from_bytes(identity.terminal_id().as_bytes())?,
    })
}

fn capability_identity(
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
