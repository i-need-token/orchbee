const std = @import("std");

const core_module = @import("terminal-host-core").state;
const terminal_runtime = @import("terminal-host-engine").runtime;
const terminal_configuration = @import("terminal-host-model").configuration;
const terminal_geometry = @import("terminal-host-model").geometry;
const service_boundary = @import("terminal-host-service");
const host_runtime_aggregate = service_boundary.testing.runtime_aggregate;
const host_service = service_boundary.service;

pub const ServicePhase = enum {
    running,
    shutting_down,
};

pub const ConnectionObservation = union(enum) {
    absent,
    active: struct {
        client_id: core_module.ClientId,
        connection_id: core_module.ConnectionId,
        frame_bytes: usize,
        input_bytes: usize,
        row_window_bytes: usize,
    },
    awaiting_hello,
    closing,
};

pub const TerminalObservation = struct {
    configuration: terminal_configuration.Configuration,
    configuration_revision: u64,
    child_exited: bool,
    creation_id: core_module.CreationId,
    geometry: terminal_geometry.Geometry,
    geometry_revision: u64,
    launch_digest: core_module.LaunchDigest,
    model_only: bool,
    pending_outcome: bool,
    phase: core_module.TerminalPhase,
    pty_drained: bool,
    retained_cleanup_pending: bool,
    revision: u64,
    runtime: ?terminal_runtime.testing.RuntimeObservation,
    settled_final_revision: ?u64,
    terminal_id: core_module.TerminalId,
    termination_request: core_module.TerminationRequest,
};

pub const AttachmentObservation = struct {
    admission_sequence: u64,
    attachment_id: core_module.AttachmentId,
    capabilities: core_module.Capabilities,
    projection: core_module.ProjectionState,
    requested_resize_lease: bool,
    resize_lease: bool,
    runtime_has_history_state: bool,
    runtime_has_input_ledger: bool,
    terminal_id: core_module.TerminalId,
};

pub const Observation = struct {
    active_core_connection_id: ?core_module.ConnectionId,
    allocator: std.mem.Allocator,
    attachments: []AttachmentObservation,
    connection: ConnectionObservation,
    core_connection_count: usize,
    host_epoch: core_module.HostEpoch,
    inventory_generation: u64,
    retained_cleanup: host_runtime_aggregate.RetainedCleanupStatus,
    runtime_count: usize,
    service_phase: ServicePhase,
    terminal_defaults: terminal_configuration.Configuration,
    terminal_defaults_revision: u64,
    terminals: []TerminalObservation,

    pub fn deinit(self: *Observation) void {
        self.allocator.free(self.attachments);
        self.allocator.free(self.terminals);
        self.* = undefined;
    }

    fn findTerminal(self: *const Observation, terminal_id: core_module.TerminalId) ?TerminalObservation {
        for (self.terminals) |terminal| {
            if (terminal.terminal_id.eql(terminal_id)) return terminal;
        }
        return null;
    }

    pub fn checkInvariants(self: *const Observation) CompositionInvariantError!void {
        if (self.runtime_count != nativeTerminalCount(self.terminals)) {
            return error.RuntimeCountMismatch;
        }
        switch (self.connection) {
            .active => |active| {
                const core_connection_id = self.active_core_connection_id orelse
                    return error.ActiveConnectionMissingCoreBroker;
                if (!sameConnectionId(core_connection_id, active.connection_id)) {
                    return error.ActiveConnectionIdentityMismatch;
                }
            },
            .absent, .awaiting_hello, .closing => {},
        }
        if (self.core_connection_count == 0 and self.active_core_connection_id != null) {
            return error.CoreConnectionObservationMismatch;
        }
        if (self.core_connection_count == 1 and self.active_core_connection_id == null) {
            return error.CoreConnectionObservationMismatch;
        }

        for (self.terminals) |terminal| {
            const runtime = terminal.runtime orelse {
                if (!terminal.model_only) return error.NativeTerminalMissingRuntime;
                continue;
            };
            if (terminal.model_only) return error.ModelTerminalOwnsRuntime;
            if (terminal.revision > runtime.completion.final_revision) {
                return error.CoreObservationAheadOfRuntime;
            }
            if (terminal.geometry_revision > runtime.completion.final_revision) {
                return error.CoreGeometryAheadOfRuntime;
            }
            if (!std.meta.eql(terminal.configuration, runtime.configuration)) {
                return error.ConfigurationStateDiverged;
            }
            if (terminal.revision == runtime.completion.final_revision and
                !std.meta.eql(terminal.geometry, runtime.geometry))
            {
                return error.GeometryStateDiverged;
            }
            if (terminal.settled_final_revision) |final_revision| {
                if (final_revision != terminal.revision or
                    final_revision != runtime.completion.final_revision)
                {
                    return error.SettledRevisionDiverged;
                }
            }
            switch (terminal.phase) {
                .running, .finishing => {},
                .containing_failure => if (!runtime.failure_fenced) {
                    return error.FailureLifecycleWithoutRuntimeFence;
                },
                .exited => switch (runtime.completion.classification) {
                    .normal => {},
                    .pending, .failed => return error.ExitedLifecycleWithoutNormalRuntime,
                },
                .failed => if (!runtime.failure_fenced or !runtime.containment_confirmed) {
                    return error.FailedLifecycleWithoutContainedRuntime;
                },
            }
            if (self.service_phase == .running and terminal.phase == .running and
                runtime.threads_joined)
            {
                return error.RunningTerminalHasJoinedRuntime;
            }

            var input_attachment_count: usize = 0;
            var history_attachment_count: usize = 0;
            for (self.attachments) |attachment| {
                if (!attachment.terminal_id.eql(terminal.terminal_id)) continue;
                input_attachment_count += @intFromBool(attachment.capabilities.input);
                history_attachment_count += @intFromBool(attachment.capabilities.read);
            }
            if (input_attachment_count != runtime.input.input_attachment_count) {
                return error.InputLedgerCountMismatch;
            }
            if (history_attachment_count != runtime.history_attachment_count) {
                return error.HistoryStateCountMismatch;
            }
        }

        for (self.attachments) |attachment| {
            const terminal = self.findTerminal(attachment.terminal_id) orelse
                return error.AttachmentMissingRetainedTerminal;
            if (terminal.model_only) {
                if (attachment.runtime_has_input_ledger or
                    attachment.runtime_has_history_state)
                {
                    return error.ModelAttachmentOwnsRuntimeState;
                }
            } else {
                if (attachment.runtime_has_input_ledger != attachment.capabilities.input) {
                    return error.InputCapabilityLedgerMismatch;
                }
                if (attachment.runtime_has_history_state != attachment.capabilities.read) {
                    return error.ReadCapabilityHistoryMismatch;
                }
            }
            try checkProjection(attachment.projection, terminal);
        }
    }
};

pub const CompositionInvariantError = error{
    ActiveConnectionIdentityMismatch,
    ActiveConnectionMissingCoreBroker,
    AttachmentMissingRetainedTerminal,
    ConfigurationStateDiverged,
    CoreConnectionObservationMismatch,
    CoreGeometryAheadOfRuntime,
    CoreObservationAheadOfRuntime,
    ExitedLifecycleWithoutNormalRuntime,
    FailedLifecycleWithoutContainedRuntime,
    FailureLifecycleWithoutRuntimeFence,
    GeometryStateDiverged,
    HistoryStateCountMismatch,
    InputCapabilityLedgerMismatch,
    InputLedgerCountMismatch,
    ModelAttachmentOwnsRuntimeState,
    ModelTerminalOwnsRuntime,
    NativeTerminalMissingRuntime,
    ProjectionGenerationAheadOfRuntime,
    ProjectionRevisionAheadOfCore,
    ReadCapabilityHistoryMismatch,
    RuntimeCountMismatch,
    RunningTerminalHasJoinedRuntime,
    SettledRevisionDiverged,
};

pub const InvariantError = core_module.testing.InvariantError ||
    terminal_runtime.testing.InvariantError || CompositionInvariantError;

pub fn capture(
    allocator: std.mem.Allocator,
    service: *host_service.HostService,
    connection: ?*const host_service.ConnectionState,
) std.mem.Allocator.Error!Observation {
    const aggregate = &service.runtime_aggregate;
    const core = aggregate.core();
    const terminals = try allocator.alloc(TerminalObservation, core.terminalCount());
    errdefer allocator.free(terminals);
    const attachment_observations = try allocator.alloc(
        AttachmentObservation,
        core.attachmentCount(),
    );
    errdefer allocator.free(attachment_observations);

    for (core.terminalValues(), terminals) |terminal, *observation| {
        const model_only = core_module.testing.isModelTerminal(core, terminal.terminal_id);
        const runtime = aggregate.runtimeFor(terminal.terminal_id);
        observation.* = .{
            .configuration = terminal.effectiveConfiguration(),
            .configuration_revision = terminal.configurationRevision(),
            .child_exited = terminal.childExited(),
            .creation_id = terminal.creation_id,
            .geometry = terminal.observedGeometry(),
            .geometry_revision = terminal.observedGeometryRevision(),
            .launch_digest = terminal.launch_digest,
            .model_only = model_only,
            .pending_outcome = terminal.pendingOutcome() != null,
            .phase = terminal.phase(),
            .pty_drained = terminal.ptyDrained(),
            .retained_cleanup_pending = terminal.retainedCleanupPending(),
            .revision = terminal.observedRevision(),
            .runtime = if (runtime) |engine| terminal_runtime.testing.observe(engine) else null,
            .settled_final_revision = terminal.settledFinalRevision(),
            .terminal_id = terminal.terminal_id,
            .termination_request = terminal.terminationRequest(),
        };
    }

    for (core.attachmentValues(), attachment_observations) |attachment, *observation| {
        const runtime = aggregate.runtimeFor(attachment.terminal_id);
        observation.* = .{
            .admission_sequence = attachment.admission_sequence,
            .attachment_id = attachment.attachment_id,
            .capabilities = attachment.capabilities,
            .projection = attachment.projection,
            .requested_resize_lease = attachment.requested_resize_lease,
            .resize_lease = attachment.resize_lease,
            .runtime_has_history_state = if (runtime) |engine|
                terminal_runtime.testing.hasHistoryAttachment(engine, attachment.attachment_id)
            else
                false,
            .runtime_has_input_ledger = if (runtime) |engine|
                terminal_runtime.testing.hasAttachmentInputOwnership(engine, attachment.attachment_id)
            else
                false,
            .terminal_id = attachment.terminal_id,
        };
    }

    return .{
        .active_core_connection_id = core_module.testing.activeConnectionId(core),
        .allocator = allocator,
        .attachments = attachment_observations,
        .connection = observeConnection(core, connection),
        .core_connection_count = core.connectionCount(),
        .host_epoch = aggregate.hostEpoch(),
        .inventory_generation = aggregate.inventoryGeneration(),
        .retained_cleanup = aggregate.retainedCleanupStatus(),
        .runtime_count = aggregate.runtimeCount(),
        .service_phase = if (host_service.testing.shuttingDownFn(service))
            .shutting_down
        else
            .running,
        .terminal_defaults = core.terminalDefaults(),
        .terminal_defaults_revision = core.terminalDefaultsRevision(),
        .terminals = terminals,
    };
}

pub fn checkInvariants(
    allocator: std.mem.Allocator,
    service: *host_service.HostService,
    connection: ?*const host_service.ConnectionState,
) (InvariantError || std.mem.Allocator.Error)!void {
    const aggregate = &service.runtime_aggregate;
    try core_module.testing.checkInvariants(aggregate.core());
    for (aggregate.core().terminalValues()) |terminal| {
        const runtime = aggregate.runtimeFor(terminal.terminal_id) orelse continue;
        try terminal_runtime.testing.checkInvariants(runtime);
    }
    var observation = try capture(allocator, service, connection);
    defer observation.deinit();
    try observation.checkInvariants();
}

fn observeConnection(
    core: *const core_module.HostCore,
    connection: ?*const host_service.ConnectionState,
) ConnectionObservation {
    const current = connection orelse return .absent;
    return switch (current.*) {
        .awaiting_hello => .awaiting_hello,
        .closing => .closing,
        .active => |active| .{ .active = .{
            .client_id = core.connectionClientId(active.connection_id) orelse unreachable,
            .connection_id = active.connection_id,
            .frame_bytes = active.limits.frame_bytes,
            .input_bytes = active.limits.input_bytes,
            .row_window_bytes = active.limits.row_window_bytes,
        } },
    };
}

fn nativeTerminalCount(terminals: []const TerminalObservation) usize {
    var count: usize = 0;
    for (terminals) |terminal| count += @intFromBool(!terminal.model_only);
    return count;
}

fn checkProjection(
    projection: core_module.ProjectionState,
    terminal: TerminalObservation,
) CompositionInvariantError!void {
    const runtime_generation = if (terminal.runtime) |runtime| runtime.generation else null;
    switch (projection.delivery) {
        .paused => {},
        .ready => |ready| {
            const base = switch (ready) {
                .snapshot => |value| value,
                .delta => |value| value,
            };
            if (base) |value| {
                try checkProjectionPoint(value.revision, value.generation, terminal.revision, runtime_generation);
            }
        },
        .in_flight => |flight| {
            try checkProjectionPoint(
                flight.frame.revision,
                flight.frame.generation,
                terminal.revision,
                runtime_generation,
            );
            if (flight.base_before) |base| {
                try checkProjectionPoint(base.revision, base.generation, terminal.revision, runtime_generation);
            }
        },
        .blocked => |blocked| {
            if (blocked.observed_revision > terminal.revision) {
                return error.ProjectionRevisionAheadOfCore;
            }
            if (blocked.base) |base| {
                try checkProjectionPoint(base.revision, base.generation, terminal.revision, runtime_generation);
            }
        },
    }
}

fn checkProjectionPoint(
    revision: u64,
    generation: u64,
    core_revision: u64,
    runtime_generation: ?u64,
) CompositionInvariantError!void {
    if (revision > core_revision) return error.ProjectionRevisionAheadOfCore;
    if (runtime_generation) |current_generation| {
        if (generation > current_generation) return error.ProjectionGenerationAheadOfRuntime;
    }
}

fn sameConnectionId(left: core_module.ConnectionId, right: core_module.ConnectionId) bool {
    return left == right;
}
