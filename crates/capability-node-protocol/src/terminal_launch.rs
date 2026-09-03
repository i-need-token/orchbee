use std::num::{NonZeroU16, NonZeroU64};

use crate::{
    error::ProtocolError,
    identity::{
        HostCreationId, HostEpoch, HostTerminalId, LeaseId, OperationId, TerminalId, WorkerId,
    },
};

pub const MAXIMUM_AUTHORIZED_CWD_BYTES: usize = 4_096;
pub const TERMINAL_LAUNCH_CAPABILITY_VERSION: TerminalLaunchCapabilityVersion =
    TerminalLaunchCapabilityVersion::new(NonZeroU16::MIN.saturating_add(1));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalLaunchCapabilityVersion(NonZeroU16);

impl TerminalLaunchCapabilityVersion {
    #[must_use]
    pub const fn new(value: NonZeroU16) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalFence(NonZeroU64);

impl TerminalFence {
    #[must_use]
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerGeneration(NonZeroU64);

impl WorkerGeneration {
    #[must_use]
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LeaseExpiresAtUnixMilliseconds(NonZeroU64);

impl LeaseExpiresAtUnixMilliseconds {
    #[must_use]
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalOperationLease {
    id: LeaseId,
    expires_at: LeaseExpiresAtUnixMilliseconds,
}

impl TerminalOperationLease {
    #[must_use]
    pub const fn new(id: LeaseId, expires_at: LeaseExpiresAtUnixMilliseconds) -> Self {
        Self { id, expires_at }
    }

    #[must_use]
    pub const fn id(self) -> LeaseId {
        self.id
    }

    #[must_use]
    pub const fn expires_at(self) -> LeaseExpiresAtUnixMilliseconds {
        self.expires_at
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalOperationAuthorization {
    capability_version: TerminalLaunchCapabilityVersion,
    fence: TerminalFence,
    lease: TerminalOperationLease,
    operation_id: OperationId,
    terminal_id: TerminalId,
    worker_generation: WorkerGeneration,
    worker_id: WorkerId,
}

impl TerminalOperationAuthorization {
    #[must_use]
    pub const fn new(
        terminal_id: TerminalId,
        operation_id: OperationId,
        worker_id: WorkerId,
        worker_generation: WorkerGeneration,
        fence: TerminalFence,
        lease: TerminalOperationLease,
        capability_version: TerminalLaunchCapabilityVersion,
    ) -> Self {
        Self {
            capability_version,
            fence,
            lease,
            operation_id,
            terminal_id,
            worker_generation,
            worker_id,
        }
    }

    #[must_use]
    pub const fn terminal_id(self) -> TerminalId {
        self.terminal_id
    }

    #[must_use]
    pub const fn operation_id(self) -> OperationId {
        self.operation_id
    }

    #[must_use]
    pub const fn worker_id(self) -> WorkerId {
        self.worker_id
    }

    #[must_use]
    pub const fn worker_generation(self) -> WorkerGeneration {
        self.worker_generation
    }

    #[must_use]
    pub const fn fence(self) -> TerminalFence {
        self.fence
    }

    #[must_use]
    pub const fn lease(self) -> TerminalOperationLease {
        self.lease
    }

    #[must_use]
    pub const fn capability_version(self) -> TerminalLaunchCapabilityVersion {
        self.capability_version
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbsoluteCwd {
    encoded_length: u16,
    value: String,
}

impl AbsoluteCwd {
    pub fn new(value: &str) -> Result<Self, ProtocolError> {
        if value.is_empty()
            || value.len() > MAXIMUM_AUTHORIZED_CWD_BYTES
            || !value.starts_with('/')
            || value.as_bytes().contains(&0)
        {
            return Err(ProtocolError::InvalidCwd);
        }
        let encoded_length =
            u16::try_from(value.len()).map_err(|_error| ProtocolError::InvalidCwd)?;
        Ok(Self {
            encoded_length,
            value: value.to_owned(),
        })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub(crate) const fn encoded_length(&self) -> u16 {
        self.encoded_length
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalPixelGeometry {
    cell_pixel_height: u16,
    cell_pixel_width: u16,
    columns: u16,
    rows: u16,
}

impl TerminalPixelGeometry {
    pub fn new(
        columns: u16,
        rows: u16,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
    ) -> Result<Self, ProtocolError> {
        let geometry = Self {
            cell_pixel_height,
            cell_pixel_width,
            columns,
            rows,
        };
        validate_geometry(geometry)?;
        Ok(geometry)
    }

    #[must_use]
    pub const fn columns(self) -> u16 {
        self.columns
    }

    #[must_use]
    pub const fn rows(self) -> u16 {
        self.rows
    }

    #[must_use]
    pub const fn cell_pixel_width(self) -> u16 {
        self.cell_pixel_width
    }

    #[must_use]
    pub const fn cell_pixel_height(self) -> u16 {
        self.cell_pixel_height
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalLaunchMode {
    Immediate(TerminalPixelGeometry),
    Deferred,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedTerminalLaunch {
    authorization: TerminalOperationAuthorization,
    cwd: Option<AbsoluteCwd>,
    mode: TerminalLaunchMode,
}

impl AuthorizedTerminalLaunch {
    #[must_use]
    pub const fn new(
        authorization: TerminalOperationAuthorization,
        cwd: Option<AbsoluteCwd>,
        mode: TerminalLaunchMode,
    ) -> Self {
        Self {
            authorization,
            cwd,
            mode,
        }
    }

    #[must_use]
    pub const fn authorization(&self) -> TerminalOperationAuthorization {
        self.authorization
    }

    #[must_use]
    pub fn cwd(&self) -> Option<&AbsoluteCwd> {
        self.cwd.as_ref()
    }

    #[must_use]
    pub const fn mode(&self) -> TerminalLaunchMode {
        self.mode
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostTerminalIdentity {
    creation_id: HostCreationId,
    host_epoch: HostEpoch,
    terminal_id: HostTerminalId,
}

impl HostTerminalIdentity {
    #[must_use]
    pub const fn new(
        creation_id: HostCreationId,
        host_epoch: HostEpoch,
        terminal_id: HostTerminalId,
    ) -> Self {
        Self {
            creation_id,
            host_epoch,
            terminal_id,
        }
    }

    #[must_use]
    pub const fn creation_id(self) -> HostCreationId {
        self.creation_id
    }

    #[must_use]
    pub const fn host_epoch(self) -> HostEpoch {
        self.host_epoch
    }

    #[must_use]
    pub const fn terminal_id(self) -> HostTerminalId {
        self.terminal_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostLaunchState {
    AwaitingStart,
    Started(TerminalPixelGeometry),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostLaunchAccepted {
    identity: HostTerminalIdentity,
    state: HostLaunchState,
}

impl HostLaunchAccepted {
    #[must_use]
    pub const fn new(identity: HostTerminalIdentity, state: HostLaunchState) -> Self {
        Self { identity, state }
    }

    #[must_use]
    pub const fn identity(self) -> HostTerminalIdentity {
        self.identity
    }

    #[must_use]
    pub const fn state(self) -> HostLaunchState {
        self.state
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalLaunchRejection {
    WrongWorker,
    StaleWorkerGeneration,
    LeaseExpired,
    StaleFence,
    ConflictingReplay,
    UnsupportedCapability,
    CapacityExceeded,
    HostRejected { host_epoch: HostEpoch },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalLaunchFailure {
    HostUnavailableBeforeHandoff,
    HostReportedFailure { host_epoch: HostEpoch },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalLaunchOutcome {
    HostAccepted(HostLaunchAccepted),
    Rejected(TerminalLaunchRejection),
    Failed(TerminalLaunchFailure),
    OutcomeUnknown { host_epoch: HostEpoch },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalLaunchResult {
    authorization: TerminalOperationAuthorization,
    outcome: TerminalLaunchOutcome,
}

impl TerminalLaunchResult {
    #[must_use]
    pub const fn new(
        authorization: TerminalOperationAuthorization,
        outcome: TerminalLaunchOutcome,
    ) -> Self {
        Self {
            authorization,
            outcome,
        }
    }

    #[must_use]
    pub const fn authorization(self) -> TerminalOperationAuthorization {
        self.authorization
    }

    #[must_use]
    pub const fn outcome(self) -> TerminalLaunchOutcome {
        self.outcome
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthorizedTerminalStart {
    authorization: TerminalOperationAuthorization,
    geometry: TerminalPixelGeometry,
    identity: HostTerminalIdentity,
}

impl AuthorizedTerminalStart {
    #[must_use]
    pub const fn new(
        authorization: TerminalOperationAuthorization,
        identity: HostTerminalIdentity,
        geometry: TerminalPixelGeometry,
    ) -> Self {
        Self {
            authorization,
            geometry,
            identity,
        }
    }

    #[must_use]
    pub const fn authorization(self) -> TerminalOperationAuthorization {
        self.authorization
    }

    #[must_use]
    pub const fn identity(self) -> HostTerminalIdentity {
        self.identity
    }

    #[must_use]
    pub const fn geometry(self) -> TerminalPixelGeometry {
        self.geometry
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartTransition {
    Started,
    AlreadyStarted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostStartAccepted {
    geometry: TerminalPixelGeometry,
    identity: HostTerminalIdentity,
    transition: StartTransition,
}

impl HostStartAccepted {
    #[must_use]
    pub const fn new(
        identity: HostTerminalIdentity,
        geometry: TerminalPixelGeometry,
        transition: StartTransition,
    ) -> Self {
        Self {
            geometry,
            identity,
            transition,
        }
    }

    #[must_use]
    pub const fn identity(self) -> HostTerminalIdentity {
        self.identity
    }

    #[must_use]
    pub const fn geometry(self) -> TerminalPixelGeometry {
        self.geometry
    }

    #[must_use]
    pub const fn transition(self) -> StartTransition {
        self.transition
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalStartOutcome {
    HostAccepted(HostStartAccepted),
    Rejected(TerminalStartRejection),
    Failed(TerminalStartFailure),
    OutcomeUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalStartRejection {
    WrongWorker,
    StaleWorkerGeneration,
    LeaseExpired,
    StaleFence,
    ConflictingReplay,
    UnsupportedCapability,
    CapacityExceeded,
    HostRejected { host_epoch: HostEpoch },
    HostIdentityChanged { observed_host_epoch: HostEpoch },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalStartFailure {
    HostUnavailableBeforeHandoff,
    HostReportedFailure { host_epoch: HostEpoch },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalStartResult {
    authorization: TerminalOperationAuthorization,
    outcome: TerminalStartOutcome,
    target: HostTerminalIdentity,
}

impl TerminalStartResult {
    pub fn new(
        authorization: TerminalOperationAuthorization,
        target: HostTerminalIdentity,
        outcome: TerminalStartOutcome,
    ) -> Result<Self, ProtocolError> {
        validate_start_outcome(target, outcome)?;
        Ok(Self {
            authorization,
            outcome,
            target,
        })
    }

    #[must_use]
    pub const fn authorization(self) -> TerminalOperationAuthorization {
        self.authorization
    }

    #[must_use]
    pub const fn outcome(self) -> TerminalStartOutcome {
        self.outcome
    }

    #[must_use]
    pub const fn target(self) -> HostTerminalIdentity {
        self.target
    }
}

fn validate_start_outcome(
    target: HostTerminalIdentity,
    outcome: TerminalStartOutcome,
) -> Result<(), ProtocolError> {
    let target_epoch = target.host_epoch();
    let consistent = match outcome {
        TerminalStartOutcome::HostAccepted(accepted) => accepted.identity() == target,
        TerminalStartOutcome::Rejected(TerminalStartRejection::HostRejected { host_epoch })
        | TerminalStartOutcome::Failed(TerminalStartFailure::HostReportedFailure { host_epoch }) => {
            host_epoch == target_epoch
        }
        TerminalStartOutcome::Rejected(TerminalStartRejection::HostIdentityChanged {
            observed_host_epoch,
        }) => observed_host_epoch != target_epoch,
        TerminalStartOutcome::Rejected(
            TerminalStartRejection::WrongWorker
            | TerminalStartRejection::StaleWorkerGeneration
            | TerminalStartRejection::LeaseExpired
            | TerminalStartRejection::StaleFence
            | TerminalStartRejection::ConflictingReplay
            | TerminalStartRejection::UnsupportedCapability
            | TerminalStartRejection::CapacityExceeded,
        )
        | TerminalStartOutcome::Failed(TerminalStartFailure::HostUnavailableBeforeHandoff)
        | TerminalStartOutcome::OutcomeUnknown => true,
    };
    if consistent {
        Ok(())
    } else {
        Err(ProtocolError::ContradictoryHostObservation)
    }
}

fn validate_geometry(geometry: TerminalPixelGeometry) -> Result<(), ProtocolError> {
    const MAXIMUM_DIMENSION: u16 = 4096;
    const MAXIMUM_CELLS: u32 = 256 * 1024;
    const MAXIMUM_PIXEL_EXTENT: u32 = u16::MAX as u32;

    if geometry.columns == 0
        || geometry.columns > MAXIMUM_DIMENSION
        || geometry.rows == 0
        || geometry.rows > MAXIMUM_DIMENSION
        || geometry.cell_pixel_width == 0
        || geometry.cell_pixel_width > MAXIMUM_DIMENSION
        || geometry.cell_pixel_height == 0
        || geometry.cell_pixel_height > MAXIMUM_DIMENSION
        || u32::from(geometry.columns) * u32::from(geometry.rows) > MAXIMUM_CELLS
        || u32::from(geometry.columns) * u32::from(geometry.cell_pixel_width) > MAXIMUM_PIXEL_EXTENT
        || u32::from(geometry.rows) * u32::from(geometry.cell_pixel_height) > MAXIMUM_PIXEL_EXTENT
    {
        return Err(ProtocolError::InvalidGeometry);
    }
    Ok(())
}
