//! Stable semantic action vocabulary owned by the composition tests.

const std = @import("std");

const core_module = @import("terminal-host-core").state;
const model = @import("terminal-host-model");

pub const Outcome = enum {
    applied,
    replayed,
    rejected,
    unknown,
};

pub const ConnectionLimits = struct {
    frame_bytes: usize,
    input_bytes: usize,
    row_window_bytes: usize,
};

pub const TerminalControlOperation = enum {
    clear,
    reset,
};

pub const TerminationMode = enum {
    graceful,
    force,
};

pub const Create = struct {
    configuration: model.configuration.Configuration,
    creation_id: core_module.CreationId,
    geometry: model.geometry.Geometry,
    launch_digest: core_module.LaunchDigest,
    model_only: bool = false,
    terminal_id: core_module.TerminalId,
};

pub const Attach = struct {
    attachment_id: core_module.AttachmentId,
    capabilities: core_module.Capabilities,
    request_resize_lease: bool,
    terminal_id: core_module.TerminalId,
};

pub const Action = union(enum) {
    connect: struct {
        client_id: core_module.ClientId,
        connection_id: core_module.ConnectionId,
        limits: ConnectionLimits,
    },
    disconnect: core_module.ConnectionId,
    create: Create,
    attach: Attach,
    claim_resize_lease: core_module.AttachmentId,
    set_projection_demand: struct {
        attachment_id: core_module.AttachmentId,
        demand: core_module.ProjectionDemand,
    },
    accept_input: struct {
        attachment_id: core_module.AttachmentId,
        byte_count: usize,
    },
    set_focus: struct {
        attachment_id: core_module.AttachmentId,
        focused: bool,
    },
    resize: struct {
        attachment_id: core_module.AttachmentId,
        geometry: model.geometry.Geometry,
    },
    update_configuration: struct {
        configuration: model.configuration.Configuration,
        terminal_id: core_module.TerminalId,
    },
    apply_terminal_control: struct {
        attachment_id: core_module.AttachmentId,
        operation: TerminalControlOperation,
    },
    request_termination: struct {
        mode: TerminationMode,
        terminal_id: core_module.TerminalId,
    },
    observe_child_exit: core_module.TerminalId,
    observe_pty_drain: core_module.TerminalId,
    observe_runtime_failure: core_module.TerminalId,
    confirm_failure_containment: struct {
        process_observed: bool,
        terminal_id: core_module.TerminalId,
    },
    publish_outcome: core_module.TerminalId,
    poll,
    detach: core_module.AttachmentId,
    dispose: core_module.TerminalId,
    begin_shutdown,
    reconcile,

    /// Stable names are the executable transition vocabulary shared by
    /// generated traces, reducers, shrink output, and formal counterexamples.
    pub fn name(self: Action) []const u8 {
        return switch (self) {
            .connect => "Connect",
            .disconnect => "Disconnect",
            .create => "Create",
            .attach => "Attach",
            .claim_resize_lease => "ClaimResizeLease",
            .set_projection_demand => "SetProjectionDemand",
            .accept_input => "AcceptInput",
            .set_focus => "SetFocus",
            .resize => "Resize",
            .update_configuration => "UpdateConfiguration",
            .apply_terminal_control => "ApplyTerminalControl",
            .request_termination => "RequestTermination",
            .observe_child_exit => "ObserveChildExit",
            .observe_pty_drain => "ObservePtyDrain",
            .observe_runtime_failure => "ObserveRuntimeFailure",
            .confirm_failure_containment => "ConfirmFailureContainment",
            .publish_outcome => "PublishOutcome",
            .poll => "Poll",
            .detach => "Detach",
            .dispose => "Dispose",
            .begin_shutdown => "BeginShutdown",
            .reconcile => "Reconcile",
        };
    }
};

pub const Step = struct {
    action: Action,
    outcome: Outcome,
    sequence: usize,
};

pub const StepInput = struct {
    action: Action,
    outcome: Outcome,
};

/// Owns only immutable test evidence. It never observes or mutates Host state.
pub const Trace = struct {
    allocator: std.mem.Allocator,
    steps: std.ArrayList(Step) = .empty,

    pub fn init(allocator: std.mem.Allocator) Trace {
        return .{ .allocator = allocator };
    }

    pub fn deinit(self: *Trace) void {
        self.steps.deinit(self.allocator);
        self.* = undefined;
    }

    pub fn append(self: *Trace, action: Action, outcome: Outcome) !Step {
        const step: Step = .{
            .action = action,
            .outcome = outcome,
            .sequence = self.steps.items.len,
        };
        try self.steps.append(self.allocator, step);
        return step;
    }

    pub fn items(self: *const Trace) []const Step {
        return self.steps.items;
    }
};

pub const vocabulary = [_][]const u8{
    "Connect",
    "Disconnect",
    "Create",
    "Attach",
    "ClaimResizeLease",
    "SetProjectionDemand",
    "AcceptInput",
    "SetFocus",
    "Resize",
    "UpdateConfiguration",
    "ApplyTerminalControl",
    "RequestTermination",
    "ObserveChildExit",
    "ObservePtyDrain",
    "ObserveRuntimeFailure",
    "ConfirmFailureContainment",
    "PublishOutcome",
    "Poll",
    "Detach",
    "Dispose",
    "BeginShutdown",
    "Reconcile",
};
