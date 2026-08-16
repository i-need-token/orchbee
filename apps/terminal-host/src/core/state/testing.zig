const std = @import("std");
const broker_session = @import("../broker_session.zig");
const model = @import("terminal-host-model");
const terminal_registry = @import("../terminal_registry.zig");
const contracts = @import("contracts.zig");
const host_core = @import("host_core.zig");
const core_storage = @import("storage.zig");

const terminal_configuration = model.configuration;
const terminal_geometry = model.geometry;

const AttachmentId = contracts.AttachmentId;
const ConnectionId = contracts.ConnectionId;
const CoreError = contracts.CoreError;
const CreationId = contracts.CreationId;
const LaunchDigest = contracts.LaunchDigest;
const ProjectionState = contracts.ProjectionState;
const TerminalId = contracts.TerminalId;
const TerminalSession = contracts.TerminalSession;
const TransitionOutcome = contracts.TransitionOutcome;
const HostCore = host_core.HostCore;
const TestingTerminalStorage = host_core.TestingTerminalStorage;

pub const testing = if (@import("builtin").is_test) struct {
    pub const InvariantError = error{
        AttachmentIndexCountMismatch,
        AttachmentIndexReferencesMissingTerminal,
        AttachmentMissingConnection,
        AttachmentMissingTerminal,
        AttachmentsWithoutConnection,
        CreationBindingMismatch,
        CreationIndexCountMismatch,
        DuplicateAttachmentAdmissionSequence,
        GeometryRevisionAheadOfObservation,
        InvalidAttachmentAdmissionSequence,
        InvalidBlockedProjectionBase,
        InvalidConfigurationRevision,
        InvalidContainingFailureLifecycle,
        InvalidExitedLifecycle,
        InvalidFailedLifecycle,
        InvalidFinishingLifecycle,
        InvalidInFlightProjectionBase,
        InvalidProjectionSequence,
        InvalidRequestedResizeLease,
        InvalidResizeLease,
        InvalidRunningLifecycle,
        InvalidTerminalConfiguration,
        InvalidTerminalGeometry,
        MissingCreationBinding,
        MissingCreationTerminal,
        MissingTerminalAttachmentIndex,
        MultipleConnections,
        MultipleResizeLeases,
        ProjectionWithoutReadCapability,
        TerminalAttachmentIndexMismatch,
        TerminalKeyMismatch,
        TerminalProjectionDemandMismatch,
    };

    /// Creates a model-only terminal without a native engine. Product creation
    /// goes through HostRuntimeAggregate so the engine and indexes commit
    /// atomically.
    pub fn createTerminal(
        core: *HostCore,
        connection_id: ConnectionId,
        creation_id: CreationId,
        terminal_id: TerminalId,
        launch_digest: LaunchDigest,
        columns: u16,
        rows: u16,
    ) (CoreError || std.mem.Allocator.Error)!TransitionOutcome {
        if (try core.resolveCreation(connection_id, creation_id, launch_digest)) |_| {
            return .idempotent;
        }
        try core.reserveTerminal();
        try core.terminal_registry.validateAdmission(creation_id, terminal_id);
        const storage = try core.allocator.create(TestingTerminalStorage);
        errdefer core.allocator.destroy(storage);
        storage.* = .{};
        core.commitRunningTerminal(
            connection_id,
            creation_id,
            terminal_id,
            launch_digest,
            columns,
            rows,
            1,
            1,
            .{},
            &storage.session,
        );
        storage.next = core.testing_state.terminal_storage;
        core.testing_state.terminal_storage = storage;
        return .applied;
    }

    pub fn isModelTerminal(core: *const HostCore, terminal_id: TerminalId) bool {
        const terminal = core.terminal_registry.get(terminal_id) orelse return false;
        return core_storage.ownsTestingTerminalStorage(core, terminal);
    }

    pub fn destroyModelTerminal(core: *HostCore, session: *TerminalSession) void {
        core_storage.destroyTestingTerminalStorage(core, session);
    }

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
        core_storage.rawAttachment(core, attachment_id).?.projection = state;
    }

    pub fn attachmentIdsForTerminal(
        core: *const HostCore,
        terminal_id: TerminalId,
    ) ?[]const AttachmentId {
        const broker = if (core.broker) |*active| active else return null;
        return broker.idsForTerminal(terminal_id);
    }

    pub fn activeReadDemandScans(core: *const HostCore) usize {
        const broker = if (core.broker) |*active| active else return core.testing_state.last_active_read_demand_scans;
        return broker_session.testing.activeReadDemandScans(broker);
    }

    pub fn resetActiveReadDemandScans(core: *HostCore) void {
        core.testing_state.last_active_read_demand_scans = 0;
        const broker = if (core.broker) |*active| active else return;
        broker_session.testing.resetActiveReadDemandScans(broker);
    }

    pub fn setTerminalDefaultsRevision(core: *HostCore, revision: u64) void {
        core.host_settings.revision = revision;
    }

    pub fn indexedAttachmentCount(core: *const HostCore) usize {
        const broker = if (core.broker) |*active| active else return 0;
        return broker_session.testing.indexedAttachmentCount(broker);
    }

    pub fn attachmentIndexReferencesRetainedTerminals(core: *const HostCore) bool {
        const broker = if (core.broker) |*active| active else return true;
        return broker_session.testing.indexReferencesOnly(broker, core.terminalKeys());
    }

    pub fn activeConnectionId(core: *const HostCore) ?ConnectionId {
        const broker = core.broker orelse return null;
        return broker.connection_id;
    }

    /// Checks the complete owner-local Core invariant set after one semantic
    /// action. Service composition tests call this same oracle before checking
    /// Core-to-runtime relationships, so local and assembled evidence cannot
    /// silently use different definitions of registry correctness.
    pub fn checkInvariants(core: *HostCore) InvariantError!void {
        if (core.connectionCount() > 1) return error.MultipleConnections;
        if (core.connectionCount() == 0 and core.attachmentCount() != 0) {
            return error.AttachmentsWithoutConnection;
        }
        if (core.terminalCount() != creationCount(core)) {
            return error.CreationIndexCountMismatch;
        }

        for (core.terminalKeys(), core.terminalValues()) |terminal_id, terminal| {
            if (!terminal_id.eql(terminal.terminal_id)) return error.TerminalKeyMismatch;
            const retained_terminal_id = creationTerminal(core, terminal.creation_id) orelse
                return error.MissingCreationBinding;
            if (!retained_terminal_id.eql(terminal_id)) return error.CreationBindingMismatch;
            if (!terminal_configuration.valid(terminal.effectiveConfiguration())) {
                return error.InvalidTerminalConfiguration;
            }
            if (terminal.configurationRevision() == 0) {
                return error.InvalidConfigurationRevision;
            }
            if (!terminal_geometry.isValid(terminal.observedGeometry())) {
                return error.InvalidTerminalGeometry;
            }
            if (terminal.observedGeometryRevision() > terminal.observedRevision()) {
                return error.GeometryRevisionAheadOfObservation;
            }
            switch (terminal.lifecycle) {
                .running => if (terminal.childExited() or
                    terminal.ptyDrained() or
                    terminal.exitResult() != null or
                    terminal.settledFinalRevision() != null)
                {
                    return error.InvalidRunningLifecycle;
                },
                .finishing => if (terminal.childExited() == terminal.ptyDrained() or
                    terminal.settledFinalRevision() != null)
                {
                    return error.InvalidFinishingLifecycle;
                },
                .containing_failure => if (terminal.ptyDrained() or
                    terminal.exitResult() != null or
                    terminal.runtimeFailure() == null or
                    terminal.failureContainment() == null or
                    terminal.pendingOutcome() != null or
                    terminal.settledFinalRevision() != null)
                {
                    return error.InvalidContainingFailureLifecycle;
                },
                .exited => if (!terminal.childExited() or
                    !terminal.ptyDrained() or
                    terminal.exitResult() == null or
                    terminal.settledFinalRevision() != terminal.observedRevision())
                {
                    return error.InvalidExitedLifecycle;
                },
                .failed => if (terminal.ptyDrained() or
                    terminal.exitResult() != null or
                    terminal.runtimeFailure() == null or
                    terminal.failedProcess() == null or
                    terminal.settledFinalRevision() != terminal.observedRevision())
                {
                    return error.InvalidFailedLifecycle;
                },
            }

            var has_active_projection_demand = false;
            for (core.attachmentValues()) |attachment| {
                if (!attachment.terminal_id.eql(terminal_id)) continue;
                if (attachment.capabilities.read and attachment.projectionDemand() == .active) {
                    has_active_projection_demand = true;
                }
            }
            if (has_active_projection_demand != core.hasActiveProjectionDemand(terminal_id)) {
                return error.TerminalProjectionDemandMismatch;
            }
        }

        for (creationKeys(core), creationValues(core)) |creation_id, terminal_id| {
            const terminal = core.findTerminal(terminal_id) orelse
                return error.MissingCreationTerminal;
            if (!creation_id.eql(terminal.creation_id)) return error.CreationBindingMismatch;
        }

        for (core.attachmentValues()) |attachment| {
            if (attachment.admission_sequence == 0) {
                return error.InvalidAttachmentAdmissionSequence;
            }
            if (core.connectionCount() != 1) return error.AttachmentMissingConnection;
            if (core.findTerminal(attachment.terminal_id) == null) {
                return error.AttachmentMissingTerminal;
            }
            if (attachment.requested_resize_lease and !attachment.capabilities.resize) {
                return error.InvalidRequestedResizeLease;
            }
            if (attachment.resize_lease and !attachment.capabilities.resize) {
                return error.InvalidResizeLease;
            }
            const terminal_ids = attachmentIdsForTerminal(
                core,
                attachment.terminal_id,
            ) orelse return error.MissingTerminalAttachmentIndex;
            if (attachment.terminal_attachment_index >= terminal_ids.len or
                !terminal_ids[attachment.terminal_attachment_index].eql(attachment.attachment_id))
            {
                return error.TerminalAttachmentIndexMismatch;
            }
            switch (attachment.projection.delivery) {
                .paused => {},
                .ready => |ready| {
                    if (!attachment.capabilities.read) {
                        return error.ProjectionWithoutReadCapability;
                    }
                    const base = switch (ready) {
                        .snapshot => |value| value,
                        .delta => |value| value,
                    };
                    if (base) |value| {
                        if (value.sequence >= attachment.projection.next_sequence) {
                            return error.InvalidProjectionSequence;
                        }
                    }
                },
                .in_flight => |flight| {
                    if (!attachment.capabilities.read) {
                        return error.ProjectionWithoutReadCapability;
                    }
                    if (flight.frame.sequence >= attachment.projection.next_sequence) {
                        return error.InvalidProjectionSequence;
                    }
                    if (flight.base_before) |base| {
                        if (base.sequence >= flight.frame.sequence) {
                            return error.InvalidInFlightProjectionBase;
                        }
                    }
                },
                .blocked => |blocked| {
                    if (!attachment.capabilities.read) {
                        return error.ProjectionWithoutReadCapability;
                    }
                    if (blocked.base) |base| {
                        if (base.sequence >= attachment.projection.next_sequence) {
                            return error.InvalidBlockedProjectionBase;
                        }
                    }
                },
            }

            var resize_lease_count: usize = 0;
            for (core.attachmentValues()) |candidate| {
                if (!candidate.attachment_id.eql(attachment.attachment_id) and
                    candidate.admission_sequence == attachment.admission_sequence)
                {
                    return error.DuplicateAttachmentAdmissionSequence;
                }
                if (candidate.resize_lease and
                    candidate.terminal_id.eql(attachment.terminal_id))
                {
                    resize_lease_count += 1;
                }
            }
            if (resize_lease_count > 1) return error.MultipleResizeLeases;
        }

        if (!attachmentIndexReferencesRetainedTerminals(core)) {
            return error.AttachmentIndexReferencesMissingTerminal;
        }
        if (core.attachmentCount() != indexedAttachmentCount(core)) {
            return error.AttachmentIndexCountMismatch;
        }
    }
} else struct {};
