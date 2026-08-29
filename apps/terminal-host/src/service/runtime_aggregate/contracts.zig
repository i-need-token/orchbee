const std = @import("std");

const core_module = @import("terminal-host-core").state;
const terminal_engine = @import("terminal-host-engine");
const host_history_access = @import("../history_access.zig");
const host_input_admission = @import("../input_admission.zig");
const host_runtime_maintenance = @import("../runtime_maintenance/root.zig");
const host_selection_access = @import("../selection_access.zig");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const terminal_configuration = model.configuration;
const terminal_geometry = model.geometry;
const terminal_input = @import("terminal-input");
const terminal_launch = terminal_engine.launch;
const terminal_runtime = terminal_engine.runtime;

pub const TerminalIdentity = struct {
    creation_id: core_module.CreationId,
    terminal_id: core_module.TerminalId,
};

pub const TerminalExitNotice = host_runtime_maintenance.TerminalExitNotice;
pub const TerminalFailureNotice = host_runtime_maintenance.TerminalFailureNotice;
pub const TerminalOutcomeNotice = host_runtime_maintenance.TerminalOutcomeNotice;
pub const PendingOutcomeBatch = host_runtime_maintenance.PendingOutcomeBatch;

pub const TerminationMode = enum {
    graceful,
    force,
};

pub const TerminalConfigurationUpdate = struct {
    configuration: terminal_configuration.Configuration,
    configuration_revision: u64,
    generation: u64,
    history_rows_evicted: u64,
    revision: u64,
    terminal_id: core_module.TerminalId,
};

pub const TerminalResize = struct {
    attachment_id: core_module.AttachmentId,
    geometry: terminal_geometry.Geometry,
    revision: u64,
    terminal_id: core_module.TerminalId,
};

pub const TerminalControl = struct {
    attachment_id: core_module.AttachmentId,
    generation: u64,
    operation: terminal_runtime.TerminalControlOperation,
    revision: u64,
    terminal_id: core_module.TerminalId,
};

pub const TerminalThemeTransition = enum {
    applied,
    already_applied,
    stale,
};

pub const TerminalThemeResult = struct {
    current: model.theme.State,
    operation_id: identifiers.TerminalThemeOperationId,
    terminal_id: core_module.TerminalId,
    transition: TerminalThemeTransition,
};

/// A request-scoped, read-only view of the terminal inventory. The aggregate
/// retains every mutable Core owner; callers may serialize this view but cannot
/// use it to perform a Core transition.
pub const TerminalInventory = struct {
    generation: u64,
    host_epoch: core_module.HostEpoch,
    terminals: []const *const core_module.TerminalSession,
};

pub const InteractionAccessError = error{
    InteractionAccessUnavailable,
    TerminalRuntimeMissing,
};

pub const ReadRowsRequest = host_history_access.ReadRowsRequest;
pub const HistoryAccess = host_history_access.HistoryAccess;
pub const SelectionGestureRequest = host_selection_access.SelectionGestureRequest;
pub const SelectionCommandRequest = host_selection_access.SelectionCommandRequest;
pub const SelectionAccess = host_selection_access.SelectionAccess;
pub const SelectionExportHandoff = host_selection_access.SelectionExportHandoff;

pub const CreateOutcome = union(enum) {
    created: TerminalIdentity,
    replayed: TerminalIdentity,

    pub fn identity(self: CreateOutcome) TerminalIdentity {
        return switch (self) {
            .created => |value| value,
            .replayed => |value| value,
        };
    }

    pub fn isCreated(self: CreateOutcome) bool {
        return switch (self) {
            .created => true,
            .replayed => false,
        };
    }
};

pub const PendingCreate = struct {
    allocator: std.mem.Allocator,
    connection_id: core_module.ConnectionId,
    creation_id: core_module.CreationId,
    environment_overlay_digest: terminal_launch.Digest,
    initial_geometry: ?terminal_geometry.Geometry,
    launch_digest: terminal_launch.Digest,
    runtime_preparation: terminal_launch.RuntimePreparation,
    terminal_id: core_module.TerminalId,
};

pub const CreatePreparation = union(enum) {
    replayed: TerminalIdentity,
    pending: PendingCreate,
    consumed,

    pub fn deinit(self: *CreatePreparation) void {
        switch (self.*) {
            .pending => |*pending| pending.runtime_preparation.deinit(pending.allocator),
            .replayed, .consumed => {},
        }
        self.* = .consumed;
    }

    pub fn identity(self: *const CreatePreparation) TerminalIdentity {
        return switch (self.*) {
            .replayed => |value| value,
            .pending => |pending| .{
                .creation_id = pending.creation_id,
                .terminal_id = pending.terminal_id,
            },
            .consumed => unreachable,
        };
    }

    pub fn requiresEffect(self: *const CreatePreparation) bool {
        return switch (self.*) {
            .pending => true,
            .replayed => false,
            .consumed => unreachable,
        };
    }

    pub fn startsImmediately(self: *const CreatePreparation) bool {
        return switch (self.*) {
            .pending => |pending| pending.initial_geometry != null,
            .replayed => false,
            .consumed => unreachable,
        };
    }
};

pub const StartTransition = enum {
    started,
    already_started,
};

pub const StartOutcome = struct {
    geometry: terminal_geometry.Geometry,
    identity: TerminalIdentity,
    transition: StartTransition,
};

pub const PendingStart = struct {
    connection_id: core_module.ConnectionId,
    geometry: terminal_geometry.Geometry,
    identity: TerminalIdentity,
};

pub const StartPreparation = union(enum) {
    replayed: StartOutcome,
    pending: PendingStart,
    consumed,

    pub fn outcome(self: *const StartPreparation) StartOutcome {
        return switch (self.*) {
            .replayed => |value| value,
            .pending => |pending| .{
                .geometry = pending.geometry,
                .identity = pending.identity,
                .transition = .started,
            },
            .consumed => unreachable,
        };
    }

    pub fn requiresEffect(self: *const StartPreparation) bool {
        return switch (self.*) {
            .pending => true,
            .replayed => false,
            .consumed => unreachable,
        };
    }
};

pub const AttachmentBinding = struct {
    attachment_id: core_module.AttachmentId,
    capabilities: core_module.Capabilities,
    capability_revision: u64,
    resize_lease: bool,
    terminal_id: core_module.TerminalId,
};

pub const AttachOutcome = struct {
    binding: AttachmentBinding,
    transition: core_module.TransitionOutcome,
};

pub const ResizeLeaseClaimOutcome = struct {
    binding: AttachmentBinding,
    previous_owner: ?core_module.AttachmentId,
    transition: core_module.TransitionOutcome,
};

pub const AcceptedInput = host_input_admission.AcceptedInput;
pub const InputRejection = host_input_admission.Rejection;
pub const InputAdmission = host_input_admission.Admission;
pub const InputPreparation = host_input_admission.Preparation;

pub const CreatePrepareError = core_module.CoreError || std.mem.Allocator.Error || error{
    LaunchPreparationOutOfMemory,
};

pub const CreateCommitError = error{
    RuntimeAllocationOutOfMemory,
    RuntimeSpawnFailed,
};

pub const StartPrepareError = core_module.CoreError;

pub const StartCommitError = error{
    RuntimeAllocationOutOfMemory,
    RuntimeSpawnFailed,
    TerminalRuntimeMissing,
};

pub const AttachError = core_module.CoreError || std.mem.Allocator.Error || error{
    TerminalRuntimeMissing,
    TerminalRuntimeUnavailable,
};

pub const CapabilityUpdateError = AttachError;

pub const ResizeLeaseClaimError = core_module.CoreError;

pub const DisposeError = core_module.CoreError || error{TerminalRuntimeMissing};

pub const DisposeResult = union(enum) {
    transition: core_module.TransitionOutcome,
    cleanup_blocked: core_module.RuntimeCleanupFailure,
};

pub const TerminationError = core_module.CoreError || error{
    TerminalRuntimeMissing,
    TerminationDispatchFailed,
};

pub const ConfigurationUpdateError = error{
    ConfigurationRevisionOverflow,
    InvalidConfiguration,
    InvalidPhase,
    MissingConnection,
    MissingResource,
    RuntimeStateDiverged,
    TerminalRevisionOverflow,
    TerminalRuntimeMissing,
    TerminalRuntimeUnavailable,
};

pub const ResizeError = core_module.CoreError || error{
    InvalidGeometry,
    RuntimeResizeFailed,
    RuntimeStateDiverged,
    TerminalRevisionOverflow,
    TerminalRuntimeMissing,
    TerminalRuntimeUnavailable,
    UnsafeRuntimeAfterResizeFailure,
};

pub const TerminalControlError = core_module.CoreError || error{
    RuntimeStateDiverged,
    TerminalRuntimeMissing,
};

pub const TerminalThemeError = core_module.CoreError || error{
    RuntimeStateDiverged,
    TerminalRevisionOverflow,
    TerminalRuntimeMissing,
    TerminalRuntimeUnavailable,
};

/// THDD-001 retained-cleanup observability status. While a fatal-teardown pass
/// leaves an unconfirmed-contained process tree, the Host keeps the cleanup
/// ownership and retries containment on the bounded backoff schedule; this
/// status surfaces the retained-cleanup state (active), the bounded retry
/// progress (attempts), and the permanent-failure policy outcome (exhausted).
pub const RetainedCleanupStatus = host_runtime_maintenance.RetainedCleanupStatus;
pub const RetainedCleanupResult = host_runtime_maintenance.RetainedCleanupResult;
