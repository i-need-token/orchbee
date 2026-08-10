const std = @import("std");
const target_builtin = @import("builtin");
const attachment_projection = @import("attachment_projection.zig");
const broker_session = @import("broker_session.zig");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const terminal_configuration = model.configuration;
const terminal_lifecycle = model.lifecycle;
const terminal_registry = @import("terminal_registry.zig");
const terminal_runtime = @import("terminal-host-engine").runtime;
const terminal_session = @import("terminal_session.zig");

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
pub const ProcessObservation = terminal_lifecycle.ProcessObservation;
pub const FailureContainment = terminal_lifecycle.FailureContainment;
pub const OutcomeKind = terminal_lifecycle.OutcomeKind;
pub const OutcomePublicationToken = terminal_session.OutcomePublicationToken;
pub const PendingOutcome = terminal_session.PendingOutcome;
pub const TerminalEngine = terminal_session.Engine;
pub const TerminalConfigurationState = terminal_session.ConfigurationState;
pub const EngineObservation = terminal_session.EngineObservation;
pub const TerminalLifecycle = terminal_lifecycle.Lifecycle;
pub const TerminalSession = terminal_session.Session;
pub const TerminalRegistry = terminal_registry.Registry;

pub const AttachmentAdmission = broker_session.Admission;

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

pub const CoreError = error{
    AttachmentConflict,
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
    TerminalConflict,
};

const BrokerSession = broker_session.Session;

pub const RequiredCapability = enum {
    none,
    input,
    read,
    resize,
};

pub const HostCore = struct {
    allocator: std.mem.Allocator,
    broker: ?BrokerSession = null,
    epoch: HostEpoch,
    terminal_registry: TerminalRegistry,
    /// Test-only: the active-projection-demand scan count captured from the
    /// broker session when it disconnects. The session owns the live counter,
    /// which is destroyed with the session; this field preserves it so a
    /// disconnect teardown can assert its demand-query work afterwards.
    last_active_read_demand_scans: usize = 0,

    pub fn init(allocator: std.mem.Allocator, epoch: HostEpoch) HostCore {
        return .{
            .allocator = allocator,
            .epoch = epoch,
            .terminal_registry = .init(allocator),
        };
    }

    pub fn deinit(self: *HostCore) void {
        if (target_builtin.is_test) {
            for (self.terminal_registry.values()) |terminal| {
                std.debug.assert(terminal.runtime() == null);
            }
        } else {
            // HostRuntimeAggregate must transfer and destroy every engine
            // before releasing the semantic registry.
            std.debug.assert(self.terminal_registry.count() == 0);
        }
        if (self.broker) |*broker| broker.deinit();
        self.terminal_registry.deinit();
        self.* = undefined;
    }

    pub fn connect(
        self: *HostCore,
        connection_id: ConnectionId,
        client_id: ClientId,
    ) CoreError!TransitionOutcome {
        if (self.broker) |broker| {
            return if (broker.matches(connection_id, client_id))
                .idempotent
            else
                error.ConnectionConflict;
        }
        self.broker = BrokerSession.init(self.allocator, connection_id, client_id);
        return .applied;
    }

    pub fn disconnect(self: *HostCore, connection_id: ConnectionId) TransitionOutcome {
        const broker = if (self.broker) |*active| active else return .idempotent;
        if (!broker.owns(connection_id)) return .idempotent;
        if (target_builtin.is_test) {
            self.last_active_read_demand_scans = broker_session.testing.activeReadDemandScans(broker);
        }
        broker.deinit();
        self.broker = null;
        return .applied;
    }

    pub fn attachmentsForConnection(self: *const HostCore, connection_id: ConnectionId) []const AttachmentId {
        const broker = if (self.broker) |*active| active else return &.{};
        return if (broker.owns(connection_id))
            broker.keys()
        else
            &.{};
    }

    pub fn resolveCreation(
        self: *HostCore,
        connection_id: ConnectionId,
        creation_id: CreationId,
        launch_digest: LaunchDigest,
    ) CoreError!?TerminalId {
        if (!self.connected(connection_id)) return error.MissingConnection;
        return self.terminal_registry.resolveCreation(creation_id, launch_digest);
    }

    pub fn reserveTerminal(self: *HostCore) (CoreError || std.mem.Allocator.Error)!void {
        try self.terminal_registry.reserveAdmission();
    }

    fn commitTerminal(
        self: *HostCore,
        connection_id: ConnectionId,
        creation_id: CreationId,
        terminal_id: TerminalId,
        launch_digest: LaunchDigest,
        columns: u16,
        rows: u16,
    ) CoreError!void {
        return self.commitTerminalWithConfiguration(
            connection_id,
            creation_id,
            terminal_id,
            launch_digest,
            columns,
            rows,
            .{},
        );
    }

    fn commitTerminalWithConfiguration(
        self: *HostCore,
        connection_id: ConnectionId,
        creation_id: CreationId,
        terminal_id: TerminalId,
        launch_digest: LaunchDigest,
        columns: u16,
        rows: u16,
        configuration: terminal_configuration.Configuration,
    ) CoreError!void {
        std.debug.assert(target_builtin.is_test);
        if (!terminal_configuration.valid(configuration)) return error.InvalidConfiguration;
        if (!self.connected(connection_id)) return error.MissingConnection;
        try self.terminal_registry.validateAdmission(creation_id, terminal_id);
        self.terminal_registry.commitAssumeCapacity(.{
            .configuration_state = .{ .effective = configuration },
            .creation_id = creation_id,
            .engine = .model,
            .engine_observation = .{ .geometry = .{
                .cell_pixel_height = 1,
                .cell_pixel_width = 1,
                .columns = columns,
                .rows = rows,
            } },
            .launch_digest = launch_digest,
            .lifecycle = .{ .running = .{} },
            .terminal_id = terminal_id,
        });
    }

    // The aggregate calls this only after reserving both registry capacities
    // and constructing the complete native engine. Host commands are
    // linearized on one thread, so the validated admission facts cannot
    // change between prepare and this infallible commit.
    pub fn commitRunningTerminal(
        self: *HostCore,
        connection_id: ConnectionId,
        creation_id: CreationId,
        terminal_id: TerminalId,
        launch_digest: LaunchDigest,
        columns: u16,
        rows: u16,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
        configuration: terminal_configuration.Configuration,
        engine: *terminal_runtime.TerminalRuntime,
    ) void {
        std.debug.assert(terminal_configuration.valid(configuration));
        std.debug.assert(self.connected(connection_id));
        self.terminal_registry.validateAdmission(creation_id, terminal_id) catch unreachable;
        self.terminal_registry.commitAssumeCapacity(.{
            .configuration_state = .{ .effective = configuration },
            .creation_id = creation_id,
            .engine = .{ .native = engine },
            .engine_observation = .{ .geometry = .{
                .cell_pixel_height = cell_pixel_height,
                .cell_pixel_width = cell_pixel_width,
                .columns = columns,
                .rows = rows,
            } },
            .launch_digest = launch_digest,
            .lifecycle = .{ .running = .{} },
            .terminal_id = terminal_id,
        });
    }

    /// Test-only constructor for the deterministic lifecycle model. Production
    /// creation must go through HostRuntimeAggregate so a process engine and
    /// both registry indexes are committed atomically.
    pub fn createTerminal(
        self: *HostCore,
        connection_id: ConnectionId,
        creation_id: CreationId,
        terminal_id: TerminalId,
        launch_digest: LaunchDigest,
        columns: u16,
        rows: u16,
    ) (CoreError || std.mem.Allocator.Error)!TransitionOutcome {
        std.debug.assert(target_builtin.is_test);
        if (try self.resolveCreation(connection_id, creation_id, launch_digest)) |_| {
            return .idempotent;
        }
        try self.reserveTerminal();
        try self.commitTerminal(
            connection_id,
            creation_id,
            terminal_id,
            launch_digest,
            columns,
            rows,
        );
        return .applied;
    }

    pub fn updateTerminalConfiguration(
        self: *HostCore,
        connection_id: ConnectionId,
        terminal_id: TerminalId,
        configuration: terminal_configuration.Configuration,
    ) CoreError!TransitionOutcome {
        if (!terminal_configuration.valid(configuration)) return error.InvalidConfiguration;
        if (!self.connected(connection_id)) return error.MissingConnection;
        const terminal = try self.requireTerminal(terminal_id);
        if (!terminal.acceptsConfigurationUpdate()) return error.InvalidPhase;
        if (terminal.effectiveConfiguration().scrollback_bytes == configuration.scrollback_bytes) {
            return .idempotent;
        }
        if (terminal.configurationRevision() == std.math.maxInt(u64)) {
            return error.RevisionOverflow;
        }
        terminal.configuration_state.effective = configuration;
        terminal.configuration_state.revision += 1;
        return .applied;
    }

    pub fn requestTerminate(
        self: *HostCore,
        connection_id: ConnectionId,
        terminal_id: TerminalId,
    ) CoreError!TransitionOutcome {
        switch (try self.prepareTermination(connection_id, terminal_id, .graceful)) {
            .replay => return .idempotent,
            .dispatch => {
                self.commitPreparedTermination(terminal_id, .graceful);
                return .applied;
            },
            .containment_retry => unreachable,
        }
    }

    pub fn requestForceTerminate(
        self: *HostCore,
        connection_id: ConnectionId,
        terminal_id: TerminalId,
    ) CoreError!TransitionOutcome {
        switch (try self.prepareTermination(connection_id, terminal_id, .force)) {
            .replay => return .idempotent,
            .dispatch => {
                self.commitPreparedTermination(terminal_id, .force);
                return .applied;
            },
            .containment_retry => {
                const terminal = self.terminal_registry.find(terminal_id) orelse unreachable;
                terminal.lifecycle.commitContainmentRetry();
                return .applied;
            },
        }
    }

    pub fn prepareTermination(
        self: *HostCore,
        connection_id: ConnectionId,
        terminal_id: TerminalId,
        request: TerminationRequest,
    ) CoreError!TerminationAdmission {
        if (!self.connected(connection_id)) return error.MissingConnection;
        const terminal = try self.requireTerminal(terminal_id);
        std.debug.assert(request != .none);
        return terminal.lifecycle.prepareTermination(request);
    }

    pub fn commitPreparedTermination(
        self: *HostCore,
        terminal_id: TerminalId,
        request: TerminationRequest,
    ) void {
        std.debug.assert(request != .none);
        const terminal = self.terminal_registry.find(terminal_id) orelse unreachable;
        terminal.lifecycle.commitPreparedTermination(request);
    }

    pub fn commitTerminationDispatchFailure(
        self: *HostCore,
        terminal_id: TerminalId,
    ) void {
        const transition = self.runtimeFailed(terminal_id, .dispatch_termination) catch unreachable;
        std.debug.assert(transition == .applied);
    }

    pub fn childExited(
        self: *HostCore,
        terminal_id: TerminalId,
        result: ExitResult,
    ) CoreError!TransitionOutcome {
        const terminal = try self.requireTerminal(terminal_id);
        return terminal.lifecycle.childExit(result, terminal.observedRevision());
    }

    pub fn ptyDrained(self: *HostCore, terminal_id: TerminalId) CoreError!TransitionOutcome {
        const terminal = try self.requireTerminal(terminal_id);
        return terminal.lifecycle.ptyDrain(terminal.observedRevision());
    }

    pub fn disposeTerminal(
        self: *HostCore,
        connection_id: ConnectionId,
        terminal_id: TerminalId,
    ) CoreError!TransitionOutcome {
        const admission = try self.validateTerminalDisposal(connection_id, terminal_id);
        switch (admission) {
            .idempotent => return .idempotent,
            // The Core convenience path has no cleanup-phase payload, so both
            // containment barriers reject as incomplete containment here;
            // HostRuntimeAggregate carries the typed blocked phase to the
            // service boundary instead.
            .containment_stopping, .cleanup_blocked => return error.FailureContainmentIncomplete,
            .dispose => {},
        }
        const terminal = self.terminal_registry.get(terminal_id).?;
        // This convenience path exists for deterministic Core model tests.
        // Production sessions always carry a native engine and are removed by
        // HostRuntimeAggregate after it has completed the ownership handoff.
        if (terminal.runtime() != null) return error.InvalidPhase;
        self.commitTerminalRemoval(terminal_id);
        return .applied;
    }

    pub fn commitTerminalRemoval(self: *HostCore, terminal_id: TerminalId) void {
        const terminal = self.terminal_registry.get(terminal_id).?;
        std.debug.assert(terminal.phase() == .exited or terminal.phase() == .failed);
        std.debug.assert(terminal.pendingOutcome() == null);
        self.removeTerminalAttachments(terminal_id);
        _ = self.terminal_registry.removeAssumePresent(terminal_id, true);
    }

    pub fn takeTerminalForHostShutdown(
        self: *HostCore,
        terminal_id: TerminalId,
    ) TerminalEngine {
        self.removeTerminalAttachments(terminal_id);
        return self.terminal_registry.removeAssumePresent(terminal_id, false).engine;
    }

    pub fn validateTerminalDisposal(
        self: *HostCore,
        connection_id: ConnectionId,
        terminal_id: TerminalId,
    ) CoreError!TerminalDisposalAdmission {
        if (!self.connected(connection_id)) return error.MissingConnection;
        const terminal = self.terminal_registry.get(terminal_id) orelse return .idempotent;
        switch (try terminal.lifecycle.requireDisposable()) {
            .dispose => {},
            .containment_stopping => return .containment_stopping,
            .cleanup_blocked => |failure| return .{ .cleanup_blocked = failure },
        }
        try self.terminal_registry.requireGenerationAdvance();
        return .dispose;
    }

    pub fn attach(
        self: *HostCore,
        connection_id: ConnectionId,
        attachment_id: AttachmentId,
        terminal_id: TerminalId,
        capabilities: Capabilities,
        request_resize_lease: bool,
    ) (CoreError || std.mem.Allocator.Error)!TransitionOutcome {
        switch (try self.validateAttachmentAdmission(
            connection_id,
            attachment_id,
            terminal_id,
            capabilities,
            request_resize_lease,
        )) {
            .replay => return .idempotent,
            .create => {},
        }

        const broker = if (self.broker) |*active| active else unreachable;
        try broker.add(
            attachment_id,
            terminal_id,
            capabilities,
            request_resize_lease,
        );
        return .applied;
    }

    pub fn validateAttachmentAdmission(
        self: *HostCore,
        connection_id: ConnectionId,
        attachment_id: AttachmentId,
        terminal_id: TerminalId,
        capabilities: Capabilities,
        request_resize_lease: bool,
    ) CoreError!AttachmentAdmission {
        if (!self.connected(connection_id)) return error.MissingConnection;
        const broker = if (self.broker) |*active| active else unreachable;
        const admission = try broker.validateAdmission(
            attachment_id,
            terminal_id,
            capabilities,
            request_resize_lease,
        );
        if (admission == .replay) return .replay;

        const terminal = try self.requireTerminal(terminal_id);
        if ((capabilities.input or capabilities.resize) and !terminal.acceptsRuntimeWork()) {
            return error.InvalidPhase;
        }
        if (capabilities.read and !terminal.hasReadableRuntimeState()) {
            return error.InvalidPhase;
        }
        if (request_resize_lease and broker.hasResizeLease(terminal_id)) {
            return error.ResizeLeaseDenied;
        }
        return .create;
    }

    pub fn detach(
        self: *HostCore,
        connection_id: ConnectionId,
        attachment_id: AttachmentId,
    ) CoreError!TransitionOutcome {
        if (!self.connected(connection_id)) return error.MissingConnection;
        const broker = if (self.broker) |*active| active else unreachable;
        _ = broker.remove(attachment_id) orelse return .idempotent;
        return .applied;
    }

    pub fn authorizeInput(
        self: *HostCore,
        connection_id: ConnectionId,
        attachment_id: AttachmentId,
    ) CoreError!*const Attachment {
        const attachment = try self.resolveAttachment(connection_id, attachment_id, .input);
        const terminal = try self.requireTerminal(attachment.terminal_id);
        if (!terminal.acceptsRuntimeWork()) return error.InvalidPhase;
        return attachment;
    }

    pub fn resize(
        self: *HostCore,
        connection_id: ConnectionId,
        attachment_id: AttachmentId,
        columns: u16,
        rows: u16,
    ) CoreError!TransitionOutcome {
        const attachment = self.resolveAttachment(connection_id, attachment_id, .resize) catch |err| {
            return if (err == error.MissingResource) error.ResizeLeaseDenied else err;
        };
        if (!attachment.resize_lease) {
            return error.ResizeLeaseDenied;
        }
        const terminal = try self.requireTerminal(attachment.terminal_id);
        if (!terminal.acceptsRuntimeWork()) return error.InvalidPhase;
        const geometry = terminal.observedGeometry();
        if (geometry.columns == columns and geometry.rows == rows) return .idempotent;
        return .applied;
    }

    pub fn runtimeFailed(
        self: *HostCore,
        terminal_id: TerminalId,
        failure: RuntimeFailure,
    ) CoreError!TransitionOutcome {
        const terminal = try self.requireTerminal(terminal_id);
        return terminal.lifecycle.runtimeFailed(failure);
    }

    pub fn failureContainmentBlocked(
        self: *HostCore,
        terminal_id: TerminalId,
        cleanup_failure: RuntimeCleanupFailure,
        retry_at_ns: i128,
    ) CoreError!TransitionOutcome {
        const terminal = try self.requireTerminal(terminal_id);
        return terminal.lifecycle.containmentBlocked(cleanup_failure, retry_at_ns);
    }

    pub fn failureContained(
        self: *HostCore,
        terminal_id: TerminalId,
        observation: ProcessObservation,
    ) CoreError!TransitionOutcome {
        const terminal = try self.requireTerminal(terminal_id);
        return terminal.lifecycle.containmentCompleted(
            observation,
            terminal.observedRevision(),
        );
    }

    pub fn synchronizeRevision(self: *HostCore, terminal_id: TerminalId, revision: u64) CoreError!TransitionOutcome {
        const terminal = try self.requireTerminal(terminal_id);
        switch (terminal.phase()) {
            .running, .finishing, .containing_failure => {},
            .exited, .failed => {
                // Settled engines may still serve retained read-only state,
                // but their final revision is immutable. Accepting the exact
                // value is a coherence check, never a new mutation.
                if (revision == terminal.observedRevision()) return .idempotent;
                return error.InvalidPhase;
            },
        }
        if (revision < terminal.observedRevision()) return error.InvalidPhase;
        if (revision == terminal.observedRevision()) return .idempotent;
        terminal.engine_observation.revision = revision;
        return .applied;
    }

    pub fn synchronizeGeometry(
        self: *HostCore,
        terminal_id: TerminalId,
        columns: u16,
        rows: u16,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
        revision: u64,
    ) CoreError!void {
        const terminal = try self.requireTerminal(terminal_id);
        if (!terminal.acceptsRuntimeWork()) return error.InvalidPhase;
        if (revision < terminal.observedRevision()) return error.InvalidPhase;
        const observed_geometry = terminal.observedGeometry();
        const geometry_changed = observed_geometry.columns != columns or
            observed_geometry.rows != rows or
            observed_geometry.cell_pixel_width != cell_pixel_width or
            observed_geometry.cell_pixel_height != cell_pixel_height;
        const revision_changed = terminal.observedRevision() != revision;
        if (geometry_changed and !revision_changed) return error.InvalidPhase;
        if (!geometry_changed and !revision_changed) return;
        terminal.engine_observation.geometry = .{
            .cell_pixel_height = cell_pixel_height,
            .cell_pixel_width = cell_pixel_width,
            .columns = columns,
            .rows = rows,
        };
        terminal.engine_observation.revision = revision;
        if (geometry_changed) {
            terminal.engine_observation.geometry_changed_at_revision = revision;
        }
    }

    pub fn setProjectionDemand(
        self: *HostCore,
        connection_id: ConnectionId,
        attachment_id: AttachmentId,
        demand: ProjectionDemand,
    ) CoreError!TransitionOutcome {
        const attachment = try self.resolveMutableAttachment(connection_id, attachment_id, .read);
        if (attachment.projectionDemand() == demand) return .idempotent;
        const broker = if (self.broker) |*active| active else unreachable;
        broker.applyProjectionDemand(attachment_id, demand);
        return .applied;
    }

    pub fn commitFrame(
        self: *HostCore,
        attachment_id: AttachmentId,
        base_revision: ?u64,
        revision: u64,
        generation: u64,
        kind: ProjectionFrameKind,
    ) CoreError!InFlightFrame {
        return (try self.commitFrameTransaction(
            attachment_id,
            base_revision,
            revision,
            generation,
            kind,
        )).flight;
    }

    pub fn commitFrameTransaction(
        self: *HostCore,
        attachment_id: AttachmentId,
        base_revision: ?u64,
        revision: u64,
        generation: u64,
        kind: ProjectionFrameKind,
    ) CoreError!ProjectionFrameCommit {
        const attachment = self.mutableAttachment(attachment_id) orelse return error.MissingResource;
        if (!attachment.capabilities.read) return error.InvalidPhase;
        return attachment.projection.commitFrame(
            attachment_id,
            base_revision,
            revision,
            generation,
            kind,
        );
    }

    pub fn rollbackFrameCommit(
        self: *HostCore,
        commit: ProjectionFrameCommit,
    ) CoreError!void {
        const attachment = self.rawAttachment(commit.attachment_id) orelse
            return error.MissingResource;
        try attachment.projection.rollbackFrame(commit);
    }

    pub fn frameApplied(
        self: *HostCore,
        connection_id: ConnectionId,
        attachment_id: AttachmentId,
        sequence: u64,
        revision: u64,
        generation: u64,
    ) CoreError!TransitionOutcome {
        const attachment = try self.resolveMutableAttachment(connection_id, attachment_id, .read);
        const terminal = try self.requireTerminal(attachment.terminal_id);
        return projectionTransition(try attachment.projection.frameApplied(
            sequence,
            revision,
            generation,
            terminal.observedGeometryRevision(),
        ));
    }

    pub fn needSnapshot(
        self: *HostCore,
        connection_id: ConnectionId,
        attachment_id: AttachmentId,
        sequence: u64,
    ) CoreError!TransitionOutcome {
        const attachment = try self.resolveMutableAttachment(connection_id, attachment_id, .read);
        return projectionTransition(try attachment.projection.needSnapshot(sequence));
    }

    pub fn blockProjection(
        self: *HostCore,
        attachment_id: AttachmentId,
        reason: ProjectionBlock,
        failed_revision: u64,
    ) CoreError!void {
        _ = try self.blockProjectionTransaction(
            attachment_id,
            reason,
            failed_revision,
        );
    }

    pub fn blockProjectionTransaction(
        self: *HostCore,
        attachment_id: AttachmentId,
        reason: ProjectionBlock,
        failed_revision: u64,
    ) CoreError!ProjectionBlockCommit {
        const attachment = self.rawAttachment(attachment_id) orelse return error.MissingResource;
        const terminal = try self.requireTerminal(attachment.terminal_id);
        if (failed_revision > terminal.observedRevision()) return error.InvalidPhase;
        self.refreshAttachmentProjectionState(attachment);
        return attachment.projection.block(
            attachment_id,
            reason,
            failed_revision,
        );
    }

    pub fn rollbackProjectionBlock(
        self: *HostCore,
        commit: ProjectionBlockCommit,
    ) CoreError!void {
        const attachment = self.rawAttachment(commit.attachment_id) orelse
            return error.MissingResource;
        try attachment.projection.rollbackBlock(commit);
    }

    pub fn projectionCaughtUp(
        self: *HostCore,
        attachment_id: AttachmentId,
    ) CoreError!void {
        const attachment = self.mutableAttachment(attachment_id) orelse return error.MissingResource;
        const terminal = try self.requireTerminal(attachment.terminal_id);
        try attachment.projection.requireCaughtUp(terminal.observedRevision());
    }

    pub fn markOutcomePublished(
        self: *HostCore,
        token: OutcomePublicationToken,
    ) TransitionOutcome {
        const terminal = self.terminal_registry.find(token.terminal_id) orelse return .idempotent;
        const final_revision = terminal.settledFinalRevision() orelse return .idempotent;
        if (final_revision != token.final_revision or
            terminal.observedRevision() != final_revision)
        {
            return .idempotent;
        }
        return terminal.lifecycle.markOutcomePublished(token.kind);
    }

    pub fn findAttachment(self: *HostCore, attachment_id: AttachmentId) ?*const Attachment {
        return self.mutableAttachment(attachment_id);
    }

    fn mutableAttachment(self: *HostCore, attachment_id: AttachmentId) ?*Attachment {
        const attachment = self.rawAttachment(attachment_id) orelse return null;
        self.refreshAttachmentProjectionState(attachment);
        return attachment;
    }

    pub fn attachmentCount(self: *const HostCore) usize {
        const broker = if (self.broker) |*active| active else return 0;
        return broker.count();
    }

    pub fn attachmentKeys(self: *const HostCore) []const AttachmentId {
        const broker = if (self.broker) |*active| active else return &.{};
        return broker.keys();
    }

    pub fn attachmentValues(self: *const HostCore) []const Attachment {
        const broker = if (self.broker) |*active| active else return &.{};
        return broker.values();
    }

    pub fn findTerminal(self: *HostCore, terminal_id: TerminalId) ?*const TerminalSession {
        return self.terminal_registry.find(terminal_id);
    }

    pub fn resolveAttachment(
        self: *HostCore,
        connection_id: ConnectionId,
        attachment_id: AttachmentId,
        required: RequiredCapability,
    ) CoreError!*const Attachment {
        return self.resolveMutableAttachment(connection_id, attachment_id, required);
    }

    fn resolveMutableAttachment(
        self: *HostCore,
        connection_id: ConnectionId,
        attachment_id: AttachmentId,
        required: RequiredCapability,
    ) CoreError!*Attachment {
        if (!self.connected(connection_id)) return error.MissingConnection;
        const attachment = self.mutableAttachment(attachment_id) orelse return error.MissingResource;
        const permitted = switch (required) {
            .none => true,
            .input => attachment.capabilities.input,
            .read => attachment.capabilities.read,
            .resize => attachment.capabilities.resize,
        };
        if (!permitted) return error.MissingResource;
        _ = try self.requireTerminal(attachment.terminal_id);
        return attachment;
    }

    pub fn hasActiveProjectionDemand(self: *HostCore, terminal_id: TerminalId) bool {
        if (self.terminal_registry.get(terminal_id) == null) return false;
        const broker = if (self.broker) |*active| active else return false;
        return broker.hasActiveProjectionDemand(terminal_id);
    }

    pub fn attachmentCountForTerminal(self: *const HostCore, terminal_id: TerminalId) usize {
        const broker = if (self.broker) |*active| active else return 0;
        return broker.attachmentCountForTerminal(terminal_id);
    }

    pub fn terminalCount(self: *const HostCore) usize {
        return self.terminal_registry.count();
    }

    pub fn terminalKeys(self: *const HostCore) []const TerminalId {
        return self.terminal_registry.keys();
    }

    pub fn terminalValues(self: *const HostCore) []const TerminalSession {
        return self.terminal_registry.values();
    }

    pub fn inventoryGeneration(self: *const HostCore) u64 {
        return self.terminal_registry.generation();
    }

    pub fn connectionCount(self: *const HostCore) usize {
        return @intFromBool(self.broker != null);
    }

    pub fn connectionClientId(self: *const HostCore, connection_id: ConnectionId) ?ClientId {
        const broker = self.broker orelse return null;
        return if (broker.owns(connection_id)) broker.clientId() else null;
    }

    fn connected(self: *const HostCore, connection_id: ConnectionId) bool {
        const broker = self.broker orelse return false;
        return broker.owns(connection_id);
    }

    fn rawAttachment(self: *HostCore, attachment_id: AttachmentId) ?*Attachment {
        const broker = if (self.broker) |*active| active else return null;
        return broker.find(attachment_id);
    }

    fn requireTerminal(self: *HostCore, terminal_id: TerminalId) CoreError!*TerminalSession {
        return self.terminal_registry.find(terminal_id) orelse error.MissingResource;
    }

    fn refreshAttachmentProjectionState(self: *HostCore, attachment: *Attachment) void {
        if (!attachment.capabilities.read) return;
        const terminal = self.terminal_registry.get(attachment.terminal_id) orelse return;
        attachment.projection.refresh(
            terminal.observedRevision(),
            terminal.observedGeometryRevision(),
        );
    }

    fn removeTerminalAttachments(self: *HostCore, terminal_id: TerminalId) void {
        const broker = if (self.broker) |*active| active else return;
        broker.removeTerminalAttachments(terminal_id);
    }
};

fn projectionTransition(value: attachment_projection.Transition) TransitionOutcome {
    return switch (value) {
        .applied => .applied,
        .idempotent => .idempotent,
    };
}

pub const testing = if (@import("builtin").is_test) struct {
    pub fn creationCount(core: *const HostCore) usize {
        return terminal_registry.testing.creationCount(&core.terminal_registry);
    }

    pub fn creationKeys(core: *const HostCore) []const CreationId {
        return terminal_registry.testing.creationKeys(&core.terminal_registry);
    }

    pub fn creationValues(core: *const HostCore) []const TerminalId {
        return terminal_registry.testing.creationValues(&core.terminal_registry);
    }

    pub fn creationTerminal(core: *const HostCore, creation_id: CreationId) ?TerminalId {
        return terminal_registry.testing.creationTerminal(&core.terminal_registry, creation_id);
    }

    pub fn containsCreation(core: *const HostCore, creation_id: CreationId) bool {
        return terminal_registry.testing.containsCreation(&core.terminal_registry, creation_id);
    }

    pub fn injectCreationBinding(
        core: *HostCore,
        creation_id: CreationId,
        terminal_id: TerminalId,
    ) !void {
        try terminal_registry.testing.injectCreationBinding(
            &core.terminal_registry,
            creation_id,
            terminal_id,
        );
    }

    pub fn setInventoryGeneration(core: *HostCore, generation: u64) void {
        terminal_registry.testing.setInventoryGeneration(&core.terminal_registry, generation);
    }

    pub fn setConfigurationRevision(
        core: *HostCore,
        terminal_id: TerminalId,
        revision: u64,
    ) void {
        core.terminal_registry.find(terminal_id).?.configuration_state.revision = revision;
    }

    pub fn setProjectionState(
        core: *HostCore,
        attachment_id: AttachmentId,
        state: ProjectionState,
    ) void {
        core.rawAttachment(attachment_id).?.projection = state;
    }

    pub fn attachmentIdsForTerminal(
        core: *const HostCore,
        terminal_id: TerminalId,
    ) ?[]const AttachmentId {
        const broker = if (core.broker) |*active| active else return null;
        return broker.idsForTerminal(terminal_id);
    }

    pub fn activeReadDemandScans(core: *const HostCore) usize {
        const broker = if (core.broker) |*active| active else return core.last_active_read_demand_scans;
        return broker_session.testing.activeReadDemandScans(broker);
    }

    pub fn resetActiveReadDemandScans(core: *HostCore) void {
        core.last_active_read_demand_scans = 0;
        const broker = if (core.broker) |*active| active else return;
        broker_session.testing.resetActiveReadDemandScans(broker);
    }

    pub fn indexedAttachmentCount(core: *const HostCore) usize {
        const broker = if (core.broker) |*active| active else return 0;
        return broker_session.testing.indexedAttachmentCount(broker);
    }

    pub fn attachmentIndexReferencesRetainedTerminals(core: *const HostCore) bool {
        const broker = if (core.broker) |*active| active else return true;
        return broker_session.testing.indexReferencesOnly(broker, core.terminalKeys());
    }
} else struct {};
