const std = @import("std");
const target_builtin = @import("builtin");
const broker_session = @import("../broker_session.zig");
const host_settings = @import("../host_settings.zig");
const model = @import("terminal-host-model");
const terminal_configuration = model.configuration;
const contracts = @import("contracts.zig");
const attachments = @import("attachments.zig");
const connections = @import("connections.zig");
const projection = @import("projection.zig");
const storage = @import("storage.zig");
const terminals = @import("terminals.zig");

const BrokerSession = broker_session.Session;
const HostEpoch = contracts.HostEpoch;
const PreparedTerminalDefaultsUpdate = contracts.PreparedTerminalDefaultsUpdate;
const TerminalRegistry = contracts.TerminalRegistry;
const TerminalSession = contracts.TerminalSession;
const CoreError = contracts.CoreError;

pub const TestingTerminalStorage = if (target_builtin.is_test) struct {
    next: ?*@This() = null,
    session: TerminalSession = undefined,
} else struct {};

pub const HostCoreTestingState = if (target_builtin.is_test) struct {
    /// Preserves the broker-owned scan count across disconnect teardown.
    last_active_read_demand_scans: usize = 0,
    terminal_storage: ?*TestingTerminalStorage = null,
} else struct {};

pub const HostCore = struct {
    allocator: std.mem.Allocator,
    broker: ?BrokerSession = null,
    epoch: HostEpoch,
    host_settings: host_settings.State,
    terminal_registry: TerminalRegistry,
    testing_state: HostCoreTestingState = .{},

    pub fn init(allocator: std.mem.Allocator, epoch: HostEpoch) HostCore {
        return initWithDefaults(allocator, epoch, .{});
    }

    pub fn initWithDefaults(
        allocator: std.mem.Allocator,
        epoch: HostEpoch,
        defaults: terminal_configuration.Configuration,
    ) HostCore {
        return .{
            .allocator = allocator,
            .epoch = epoch,
            .host_settings = .init(defaults),
            .terminal_registry = .init(allocator),
        };
    }

    pub fn deinit(self: *HostCore) void {
        if (target_builtin.is_test) {
            for (self.terminal_registry.values()) |terminal| {
                std.debug.assert(storage.ownsTestingTerminalStorage(self, terminal));
            }
        } else {
            // HostRuntimeAggregate must transfer and destroy every retained
            // terminal before releasing the semantic registry.
            std.debug.assert(self.terminal_registry.count() == 0);
        }
        if (self.broker) |*broker| broker.deinit();
        self.terminal_registry.deinit();
        if (target_builtin.is_test) {
            while (self.testing_state.terminal_storage) |testing_storage| {
                self.testing_state.terminal_storage = testing_storage.next;
                self.allocator.destroy(testing_storage);
            }
        }
        self.* = undefined;
    }

    pub fn terminalDefaults(self: *const HostCore) terminal_configuration.Configuration {
        return self.host_settings.defaults;
    }

    pub fn terminalDefaultsRevision(self: *const HostCore) u64 {
        return self.host_settings.revision;
    }

    pub fn prepareTerminalDefaultsUpdate(
        self: *const HostCore,
        configuration: terminal_configuration.Configuration,
    ) CoreError!PreparedTerminalDefaultsUpdate {
        return self.host_settings.prepareDefaultsUpdate(configuration);
    }

    pub fn commitTerminalDefaultsUpdate(
        self: *HostCore,
        prepared: PreparedTerminalDefaultsUpdate,
    ) void {
        self.host_settings.commitDefaultsUpdate(prepared);
    }

    // Connections boundary.
    pub const connect = connections.connect;
    pub const disconnect = connections.disconnect;
    pub const attachmentsForConnection = connections.attachmentsForConnection;
    pub const connectionCount = connections.connectionCount;
    pub const connectionClientId = connections.connectionClientId;

    // Terminals boundary.
    pub const resolveCreation = terminals.resolveCreation;
    pub const reserveTerminal = terminals.reserveTerminal;
    pub const commitRunningTerminal = terminals.commitRunningTerminal;
    pub const updateTerminalConfiguration = terminals.updateTerminalConfiguration;
    pub const requestTerminate = terminals.requestTerminate;
    pub const requestForceTerminate = terminals.requestForceTerminate;
    pub const prepareTermination = terminals.prepareTermination;
    pub const commitPreparedTermination = terminals.commitPreparedTermination;
    pub const commitTerminationDispatchFailure = terminals.commitTerminationDispatchFailure;
    pub const childExited = terminals.childExited;
    pub const ptyDrained = terminals.ptyDrained;
    pub const disposeTerminal = terminals.disposeTerminal;
    pub const commitTerminalRemoval = terminals.commitTerminalRemoval;
    pub const takeTerminalForHostShutdown = terminals.takeTerminalForHostShutdown;
    pub const validateTerminalDisposal = terminals.validateTerminalDisposal;
    pub const runtimeFailed = terminals.runtimeFailed;
    pub const failureContainmentBlocked = terminals.failureContainmentBlocked;
    pub const failureContained = terminals.failureContained;
    pub const retainedCleanupBlocked = terminals.retainedCleanupBlocked;
    pub const retainedCleanupCompleted = terminals.retainedCleanupCompleted;
    pub const retainedCleanupExhausted = terminals.retainedCleanupExhausted;
    pub const synchronizeRevision = terminals.synchronizeRevision;
    pub const synchronizeGeometry = terminals.synchronizeGeometry;
    pub const markOutcomePublished = terminals.markOutcomePublished;
    pub const findTerminal = terminals.findTerminal;
    pub const terminalCount = terminals.terminalCount;
    pub const terminalKeys = terminals.terminalKeys;
    pub const terminalValues = terminals.terminalValues;
    pub const inventoryGeneration = terminals.inventoryGeneration;

    // Attachments boundary.
    pub const attach = attachments.attach;
    pub const attachAtBellSequence = attachments.attachAtBellSequence;
    pub const prepareBellDelivery = attachments.prepareBellDelivery;
    pub const commitBellDelivery = attachments.commitBellDelivery;
    pub const validateAttachmentAdmission = attachments.validateAttachmentAdmission;
    pub const detach = attachments.detach;
    pub const authorizeInput = attachments.authorizeInput;
    pub const resize = attachments.resize;
    pub const findAttachment = attachments.findAttachment;
    pub const attachmentCount = attachments.attachmentCount;
    pub const attachmentKeys = attachments.attachmentKeys;
    pub const attachmentValues = attachments.attachmentValues;
    pub const resolveAttachment = attachments.resolveAttachment;
    pub const hasActiveProjectionDemand = attachments.hasActiveProjectionDemand;
    pub const attachmentCountForTerminal = attachments.attachmentCountForTerminal;

    // Projection boundary.
    pub const setProjectionDemand = projection.setProjectionDemand;
    pub const commitFrame = projection.commitFrame;
    pub const commitFrameTransaction = projection.commitFrameTransaction;
    pub const rollbackFrameCommit = projection.rollbackFrameCommit;
    pub const frameApplied = projection.frameApplied;
    pub const needSnapshot = projection.needSnapshot;
    pub const blockProjection = projection.blockProjection;
    pub const blockProjectionTransaction = projection.blockProjectionTransaction;
    pub const rollbackProjectionBlock = projection.rollbackProjectionBlock;
    pub const projectionCaughtUp = projection.projectionCaughtUp;
};
