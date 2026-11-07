const std = @import("std");
const target_builtin = @import("builtin");
const core_module = @import("terminal-host-core").state;
const terminal_runtime = @import("terminal-host-engine").runtime;
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const terminal_configuration = model.configuration;
const host_runtime_maintenance = @import("../runtime_maintenance/root.zig");

const attachments = @import("attachments.zig");
const configuration = @import("configuration.zig");
const creation = @import("creation.zig");
const geometry = @import("geometry.zig");
const input_admission = @import("input_admission.zig");
const interaction_access = @import("interaction_access.zig");
const inventory = @import("inventory.zig");
const maintenance_api = @import("maintenance_api.zig");
const ownership = @import("ownership.zig");
const projection = @import("projection.zig");
const start = @import("start.zig");
const terminal_control = @import("terminal_control.zig");
const theme = @import("theme.zig");
const termination = @import("termination.zig");

pub const HostRuntimeAggregateTestingState = if (target_builtin.is_test) struct {
    after_configuration_commit: ?*const fn (?*anyopaque, *terminal_runtime.TerminalRuntime) void = null,
    after_configuration_commit_context: ?*anyopaque = null,
    after_resize_commit: ?*const fn (?*anyopaque, *terminal_runtime.TerminalRuntime) void = null,
    after_resize_commit_context: ?*anyopaque = null,
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
    staged_output: @import("../output_staging.zig").StagedOutput = .{},
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
    pub const pendingStartCleanup = ownership.pendingStartCleanup;
    pub const releaseStartResources = ownership.releaseStartResources;

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

    // geometry boundary.
    pub const resizeTerminal = geometry.resizeTerminal;

    // terminal-control boundary.
    pub const applyTerminalControl = terminal_control.applyTerminalControl;

    // terminal-theme boundary.
    pub const applyTerminalTheme = theme.applyTerminalTheme;

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
    pub const prepareCreateTerminalWithScrollback = creation.prepareCreateTerminalWithScrollback;
    pub const commitPreparedTerminal = creation.commitPreparedTerminal;

    // start boundary.
    pub const prepareStartTerminal = start.prepareStartTerminal;
    pub const commitPreparedTerminalStart = start.commitPreparedTerminalStart;

    // termination boundary.
    pub const requestTermination = termination.requestTermination;
    pub const disposeTerminal = termination.disposeTerminal;

    // attachment-lifecycle boundary.
    pub const attach = attachments.attach;
    pub const claimResizeLease = attachments.claimResizeLease;
    pub const updateAttachmentInputCapability = attachments.updateAttachmentInputCapability;
    pub const detach = attachments.detach;
    pub const disconnect = attachments.disconnect;

    // input-admission boundary.
    pub const acceptRawInput = input_admission.acceptRawInput;
    pub const prepareRawInput = input_admission.prepareRawInput;
    pub const acceptSemanticInput = input_admission.acceptSemanticInput;
    pub const prepareSemanticInput = input_admission.prepareSemanticInput;

    // interaction-access boundary.
    pub const graphicsDecodeTarget = interaction_access.graphicsDecodeTarget;
    pub const historyAccess = interaction_access.historyAccess;
    pub const sampleTerminalProcesses = interaction_access.sampleTerminalProcesses;
    pub const selectionAccess = interaction_access.selectionAccess;
};
