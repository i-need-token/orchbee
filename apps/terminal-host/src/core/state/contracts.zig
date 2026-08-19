const attachment_projection = @import("../attachment_projection.zig");
const broker_session = @import("../broker_session.zig");
const host_settings = @import("../host_settings.zig");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const terminal_lifecycle = model.lifecycle;
const terminal_registry = @import("../terminal_registry.zig");
const terminal_session = @import("../terminal_session.zig");

pub const AttachmentId = identifiers.AttachmentId;
pub const ClientId = identifiers.ClientId;
pub const CreationId = identifiers.CreationId;
pub const HostEpoch = identifiers.HostEpoch;
pub const TerminalId = identifiers.TerminalId;
pub const LaunchDigest = terminal_session.LaunchDigest;
pub const ConnectionId = broker_session.ConnectionId;

pub const TerminalPhase = terminal_lifecycle.Phase;
pub const TransitionOutcome = terminal_lifecycle.TransitionOutcome;
pub const TerminationAdmission = terminal_lifecycle.TerminationAdmission;

pub const StartAdmission = enum {
    replay,
    start,
};

/// Disposal admission across the Core boundary: an absent terminal is an
/// idempotent no-op; a settled published terminal is disposable; stopping
/// containment has no retained failing phase; blocked containment carries the
/// exact cleanup phase that the service must report as the typed failure.
pub const TerminalDisposalAdmission = union(enum) {
    idempotent,
    dispose,
    containment_stopping,
    cleanup_blocked: terminal_lifecycle.RuntimeCleanupFailure,
};
pub const ExitResult = terminal_lifecycle.ExitResult;
pub const TerminationRequest = terminal_lifecycle.TerminationRequest;
pub const RuntimeFailureKind = terminal_lifecycle.RuntimeFailureKind;
pub const RuntimeFailureStage = terminal_lifecycle.RuntimeFailureStage;
pub const RuntimeFailure = terminal_lifecycle.RuntimeFailure;
pub const RuntimeCleanupFailure = terminal_lifecycle.RuntimeCleanupFailure;
pub const RetainedCleanup = terminal_lifecycle.RetainedCleanup;
pub const ProcessObservation = terminal_lifecycle.ProcessObservation;
pub const FailureContainment = terminal_lifecycle.FailureContainment;
pub const OutcomeKind = terminal_lifecycle.OutcomeKind;
pub const OutcomePublicationToken = terminal_session.OutcomePublicationToken;
pub const PendingOutcome = terminal_session.PendingOutcome;
pub const TerminalConfigurationState = terminal_session.ConfigurationState;
pub const EngineObservation = terminal_session.EngineObservation;
pub const TerminalLifecycle = terminal_lifecycle.Lifecycle;
pub const TerminalSession = terminal_session.Session;
pub const TerminalRegistry = terminal_registry.Registry;
pub const PreparedTerminalDefaultsUpdate = host_settings.PreparedDefaultsUpdate;

pub const AttachmentAdmission = broker_session.Admission;

pub const ResizeLeaseChange = struct {
    attachment_id: AttachmentId,
    resize_lease: bool,
    terminal_id: TerminalId,
};

pub const AttachmentDetach = struct {
    outcome: TransitionOutcome,
    resize_lease_change: ?ResizeLeaseChange,
};

pub const ProjectionDemand = attachment_projection.Demand;
pub const ProjectionBlock = attachment_projection.Block;
pub const ProjectionFrameKind = attachment_projection.FrameKind;
pub const InFlightFrame = attachment_projection.InFlightFrame;
pub const ProjectionReady = attachment_projection.Ready;
pub const ProjectionFlight = attachment_projection.Flight;
pub const ProjectionBlocked = attachment_projection.Blocked;
pub const ProjectionState = attachment_projection.State;
pub const ProjectionFrameCommit = attachment_projection.FrameCommit;
pub const ProjectionBlockCommit = attachment_projection.BlockCommit;

pub const Capabilities = broker_session.Capabilities;
pub const Attachment = broker_session.Attachment;

pub const BellDeliveryToken = struct {
    attachment_id: AttachmentId,
    sequence: u64,
};

pub const CoreError = error{
    AttachmentConflict,
    AttachmentSequenceOverflow,
    ConnectionConflict,
    CreationConflict,
    FailureContainmentIncomplete,
    InvalidCapabilities,
    InvalidConfiguration,
    InvalidFrameAcknowledgement,
    InvalidPhase,
    InvalidProcessObservation,
    InventoryGenerationOverflow,
    MissingConnection,
    MissingResource,
    OutcomePublicationPending,
    RevisionOverflow,
    ResizeLeaseDenied,
    SequenceOverflow,
    StartConflict,
    TerminalConflict,
};

pub const RequiredCapability = enum {
    none,
    input,
    read,
    resize,
};
