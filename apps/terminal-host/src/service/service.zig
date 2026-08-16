const std = @import("std");
const build_options = @import("build-options");
const target_builtin = @import("builtin");

const control_encode = @import("terminal-host-control").encode;
const host_connection = @import("connection_workflow.zig");
const host_event_delivery = @import("event_delivery.zig");
const host_failure = @import("failure.zig");
const host_outgoing = @import("outgoing.zig");
const host_request_dispatch = @import("request_dispatch.zig");
const host_runtime_aggregate = @import("runtime_aggregate/root.zig");
const terminal_configuration = @import("terminal-host-model").configuration;

pub const host_version = build_options.host_version;
pub const ConnectionState = host_connection.ConnectionState;

const Outgoing = host_outgoing.Outgoing;
const InputFailure = host_failure.InputFailure;

pub const Options = struct {
    terminal_defaults: terminal_configuration.Configuration = .{},
    trace_interactions: bool = false,
};

const ServiceLifecycle = enum {
    running,
    shutting_down,
};

// LocalServer gives one dispatch thread ownership of HostService. Each retained
// TerminalRuntime owns the synchronization for its worker threads.
pub const HostService = struct {
    allocator: std.mem.Allocator,
    connection_workflow: host_connection.Workflow = .{},
    event_delivery: host_event_delivery.Workflow = .{},
    lifecycle: ServiceLifecycle = .running,
    runtime_aggregate: host_runtime_aggregate.HostRuntimeAggregate,
    trace_interactions: bool,

    pub fn init(allocator: std.mem.Allocator) HostService {
        return initWithOptions(allocator, .{});
    }

    pub fn initWithOptions(
        allocator: std.mem.Allocator,
        options: Options,
    ) HostService {
        std.debug.assert(terminal_configuration.valid(options.terminal_defaults));
        return .{
            .allocator = allocator,
            .runtime_aggregate = .initWithDefaults(allocator, options.terminal_defaults),
            .trace_interactions = options.trace_interactions,
        };
    }

    pub fn deinit(self: *HostService) void {
        self.deinitChecked() catch |err| {
            // The Core/runtime aggregate remains intact so shutdown can be
            // retried. Dispatch stays disabled because some runtimes may
            // already have joined their workers. Callers that need an
            // observable shutdown result can use deinitChecked directly.
            std.log.err("retaining terminal host after shutdown failure: {s}", .{@errorName(err)});
        };
    }

    pub fn deinitChecked(self: *HostService) !void {
        const first_attempt = self.beginShutdown();
        try self.runtime_aggregate.deinitChecked(first_attempt);
        self.* = undefined;
    }

    /// THDD-001: whether a failed fatal-teardown pass left an unconfirmed
    /// process tree that the Host still owns and must keep retrying cleanup for.
    pub fn retainedCleanupPending(self: *const HostService) bool {
        return self.runtime_aggregate.retainedCleanupPending();
    }

    /// THDD-001: bounded retained-cleanup window. Runs while the endpoint and
    /// lifetime lock stay held by the caller; the aggregate reuses the existing
    /// bounded cleanup-retry scheduler so unconfirmed trees are retried on its
    /// bounded backoff cadence instead of being abandoned after one failed pass.
    pub fn runRetainedCleanupWindow(self: *HostService) host_runtime_aggregate.RetainedCleanupResult {
        return self.runtime_aggregate.runRetainedCleanupWindow();
    }

    /// THDD-001: completes teardown once every retained process tree is
    /// containment-confirmed, mirroring the successful deinitChecked path.
    pub fn finishRetainedCleanup(self: *HostService) !void {
        try self.runtime_aggregate.finishRetainedCleanup();
        self.* = undefined;
    }

    fn beginShutdown(self: *HostService) bool {
        if (self.lifecycle == .shutting_down) return false;
        self.lifecycle = .shutting_down;
        return true;
    }

    pub fn disconnect(self: *HostService, connection: *ConnectionState) void {
        self.connection_workflow.disconnect(&self.runtime_aggregate, connection);
    }

    pub fn canStopWhenIdle(self: *const HostService) bool {
        return self.runtime_aggregate.isEmpty();
    }

    /// Advances engine-owned state while no broker is connected. Outcome
    /// delivery remains pending until a broker handoff; safety cleanup and
    /// input-credit settlement do not depend on broker availability.
    pub fn maintain(self: *HostService) void {
        if (self.lifecycle == .shutting_down) return;
        self.runtime_aggregate.synchronizeRuntimes();
    }

    pub fn pollEvents(self: *HostService, connection: *const ConnectionState) !Outgoing {
        if (self.lifecycle == .shutting_down) return error.ServiceShuttingDown;
        return self.event_delivery.pollEvents(
            self.allocator,
            &self.runtime_aggregate,
            connection,
        );
    }

    pub fn handle(
        self: *HostService,
        connection: *ConnectionState,
        record: []const u8,
    ) !Outgoing {
        if (self.lifecycle == .shutting_down) return error.ServiceShuttingDown;
        return self.connection_workflow.handle(
            self.allocator,
            &self.runtime_aggregate,
            self.trace_interactions,
            connection,
            record,
        );
    }
};

fn scheduleEgressForTesting(
    service: *HostService,
    connection: *const ConnectionState.Active,
    outgoing: *Outgoing,
) !void {
    return service.event_delivery.scheduleEgress(
        service.allocator,
        &service.runtime_aggregate,
        connection,
        outgoing,
    );
}

fn synchronizeRuntimeLifecycleForTesting(
    service: *HostService,
    outgoing: *Outgoing,
    outcome_handoff: *host_event_delivery.OutcomePublicationHandoff,
) !void {
    return service.event_delivery.synchronizeRuntimeLifecycle(
        service.allocator,
        &service.runtime_aggregate,
        outgoing,
        outcome_handoff,
    );
}

fn setNextProjectionForTesting(service: *HostService) void {
    service.event_delivery.setNextProjectionForTesting();
}

fn shuttingDownForTesting(service: *const HostService) bool {
    return service.lifecycle == .shutting_down;
}

pub const testing = if (target_builtin.is_test) struct {
    pub const beginShutdownFn = HostService.beginShutdown;
    pub const InputFailureType = InputFailure;
    pub const OutcomePublicationHandoffType = host_event_delivery.OutcomePublicationHandoff;
    pub const RequestDispatcherType = host_request_dispatch.Dispatcher;
    pub const commitOutcomePublicationHandoffFn = host_event_delivery.OutcomePublicationHandoff.commit;
    pub const cleanupFailureDetailsFn = host_failure.fromCleanup;
    pub const identifierTextFn = control_encode.identifierText;
    pub const inputFailureFn = host_failure.fromInput;
    pub const protocolErrorCodeFn = host_failure.protocolCode;
    pub const scheduleEgressFn = scheduleEgressForTesting;
    pub const setNextProjectionFn = setNextProjectionForTesting;
    pub const shuttingDownFn = shuttingDownForTesting;
    pub const synchronizeRuntimeLifecycleFn = synchronizeRuntimeLifecycleForTesting;
} else struct {};
