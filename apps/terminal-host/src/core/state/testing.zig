const std = @import("std");
const broker_session = @import("../broker_session.zig");
const terminal_registry = @import("../terminal_registry.zig");
const contracts = @import("contracts.zig");
const host_core = @import("host_core.zig");
const core_storage = @import("storage.zig");

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
} else struct {};
