const std = @import("std");
const target_builtin = @import("builtin");
const core_module = @import("terminal-host-core").state;
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const terminal_configuration = model.configuration;
const host_runtime_maintenance = @import("../runtime_maintenance/root.zig");

const attachments = @import("attachments.zig");
const configuration = @import("configuration.zig");
const creation = @import("creation.zig");
const inventory = @import("inventory.zig");
const maintenance_api = @import("maintenance_api.zig");
const ownership = @import("ownership.zig");
const projection = @import("projection.zig");
const termination = @import("termination.zig");

pub const HostRuntimeAggregateTestingState = if (target_builtin.is_test) struct {
    /// Counts projection-release decisions so disconnect teardown can prove
    /// that each affected terminal is considered at most once.
    projection_release_decisions: usize = 0,
} else struct {};

// The aggregate remains the single owner of each co-located retained terminal;
// operation modules receive this complete owner and cannot outlive it.
pub const HostRuntimeAggregate = struct {
    allocator: std.mem.Allocator,
    core_state: core_module.HostCore,
    maintenance: host_runtime_maintenance.Maintenance = .{},
    testing_state: HostRuntimeAggregateTestingState = .{},

    pub fn init(allocator: std.mem.Allocator) HostRuntimeAggregate {
        return initWithDefaults(allocator, .{});
    }

    pub fn initWithDefaults(
        allocator: std.mem.Allocator,
        defaults: terminal_configuration.Configuration,
    ) HostRuntimeAggregate {
        return .{
            .allocator = allocator,
            .core_state = .initWithDefaults(
                allocator,
                identifiers.HostEpoch.random(),
                defaults,
            ),
        };
    }

    // ownership boundary.
    pub const deinitChecked = ownership.deinitChecked;
    pub const retainedCleanupPending = ownership.retainedCleanupPending;
    pub const retainedCleanupStatus = ownership.retainedCleanupStatus;
    pub const runRetainedCleanupPass = ownership.runRetainedCleanupPass;
    pub const runRetainedCleanupWindow = ownership.runRetainedCleanupWindow;
    pub const finishRetainedCleanup = ownership.finishRetainedCleanup;
    pub const isEmpty = ownership.isEmpty;
    pub const runtimeCount = ownership.runtimeCount;
    pub const runtimeFor = ownership.runtimeFor;

    // inventory boundary.
    pub const core = inventory.core;
    pub const coreRead = inventory.coreRead;
    pub const hostEpoch = inventory.hostEpoch;
    pub const inventoryGeneration = inventory.inventoryGeneration;
    pub const terminalCount = inventory.terminalCount;
    pub const terminalInventory = inventory.terminalInventory;
    pub const connect = inventory.connect;
    pub const commitOutcomePublications = inventory.commitOutcomePublications;

    // configuration boundary.
    pub const prepareTerminalDefaultsUpdate = configuration.prepareTerminalDefaultsUpdate;
    pub const commitTerminalDefaultsUpdate = configuration.commitTerminalDefaultsUpdate;
    pub const updateTerminalConfiguration = configuration.updateTerminalConfiguration;
    pub const resizeTerminal = configuration.resizeTerminal;
    pub const applyTerminalControl = configuration.applyTerminalControl;

    // projection boundary.
    pub const frameApplied = projection.frameApplied;
    pub const needSnapshot = projection.needSnapshot;
    pub const nextProjectionAt = projection.nextProjectionAt;
    pub const setNextProjectionAt = projection.setNextProjectionAt;
    pub const setProjectionDemand = projection.setProjectionDemand;

    // maintenance boundary.
    pub const synchronizeRuntimeObservation = maintenance_api.synchronizeRuntimeObservation;
    pub const synchronizeRuntimes = maintenance_api.synchronizeRuntimes;
    pub const synchronizeTerminalRange = maintenance_api.synchronizeTerminalRange;
    pub const pendingOutcomes = maintenance_api.pendingOutcomes;
    pub const synchronizeTerminal = maintenance_api.synchronizeTerminal;

    // creation boundary.
    pub const prepareCreateTerminal = creation.prepareCreateTerminal;
    pub const commitPreparedTerminal = creation.commitPreparedTerminal;

    // termination boundary.
    pub const requestTermination = termination.requestTermination;
    pub const disposeTerminal = termination.disposeTerminal;

    // attachments boundary.
    pub const readableAttachment = attachments.readableAttachment;
    pub const attach = attachments.attach;
    pub const acceptRawInput = attachments.acceptRawInput;
    pub const prepareRawInput = attachments.prepareRawInput;
    pub const acceptSemanticInput = attachments.acceptSemanticInput;
    pub const prepareSemanticInput = attachments.prepareSemanticInput;
    pub const detach = attachments.detach;
    pub const disconnect = attachments.disconnect;
};
