const std = @import("std");
const build_options = @import("build-options");
const target_builtin = @import("builtin");

const candidate_admission = @import("candidate_admission.zig");
const control_encode = @import("terminal-host-control").encode;
const creation_pipeline = @import("creation_pipeline.zig");
const diagnostic_log = @import("terminal-host-platform").diagnostic_log;
const host_connection = @import("connection_workflow.zig");
const host_event_delivery = @import("event_delivery.zig");
const host_failure = @import("failure.zig");
const host_lifecycle = @import("host_lifecycle.zig");
const host_outgoing = @import("outgoing.zig");
const host_request_dispatch = @import("request_dispatch.zig");
const host_runtime_aggregate = @import("runtime_aggregate/root.zig");
const keepers_module = @import("keepers.zig");
const request_failure = @import("request_failure.zig");
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
    /// Teardown released every terminal but abandoned a creation build that never returned; only
    /// `retainedCleanupPending` and a repeated teardown may still be called.
    released_after_abandoned_build,
};

// LocalServer gives one dispatch thread ownership of HostService. Each retained
// TerminalRuntime owns the synchronization for its worker threads.
pub const HostService = struct {
    allocator: std.mem.Allocator,
    connection_workflow: host_connection.Workflow = .{},
    creations: creation_pipeline.Pipeline = .{},
    /// The turns the dispatch loop has completed; only the dispatch thread advances it.
    dispatch_turns: u64 = 0,
    event_delivery: host_event_delivery.Workflow = .{},
    /// The Keeper attachments; their links run until teardown has ended every terminal.
    keepers: keepers_module.Keepers,
    lifecycle: ServiceLifecycle = .running,
    /// The `drain` and `exit` lifecycle that queries report; only the dispatch thread changes it.
    process_state: host_lifecycle.State = .ready,
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
            .keepers = .init(allocator),
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
        if (self.lifecycle == .released_after_abandoned_build) return error.CreationBuildAbandoned;
        const first_attempt = self.beginShutdown();
        // A creation in progress settles first: a runtime its build made joins the terminals the
        // aggregate's teardown contains, and no terminal record it borrows goes before it.
        self.creations.shutdown(&self.runtime_aggregate);
        self.keepers.releaseAll(&self.runtime_aggregate);
        try self.runtime_aggregate.deinitChecked(first_attempt);
        try self.finishTeardown();
    }

    /// THDD-001: whether a failed fatal-teardown pass left an unconfirmed
    /// process tree that the Host still owns and must keep retrying cleanup for.
    pub fn retainedCleanupPending(self: *const HostService) bool {
        if (self.lifecycle == .released_after_abandoned_build) return false;
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
        try self.finishTeardown();
    }

    /// Ends teardown once the aggregate is released. THDD-001: a creation build that never
    /// returned may own a process the Host could not contain, so teardown fails instead of
    /// reporting a clean release.
    fn finishTeardown(self: *HostService) error{CreationBuildAbandoned}!void {
        // Every terminal has ended and released first, so no link still holds one.
        self.keepers.stop();
        if (self.creations.abandoned_build) {
            self.lifecycle = .released_after_abandoned_build;
            return error.CreationBuildAbandoned;
        }
        self.* = undefined;
    }

    fn beginShutdown(self: *HostService) bool {
        if (self.lifecycle == .shutting_down) return false;
        self.lifecycle = .shutting_down;
        return true;
    }

    pub fn disconnect(self: *HostService, connection: *ConnectionState) void {
        switch (connection.*) {
            .active => |active| self.creations.detach(self.allocator, active.connection_id),
            .awaiting_hello, .closing => {},
        }
        self.keepers.disconnect(switch (connection.*) {
            .active => |active| active.connection_id,
            .awaiting_hello, .closing => null,
        });
        self.connection_workflow.disconnect(&self.runtime_aggregate, connection);
    }

    /// A Worker session was admitted on the broker endpoint; `keeper` tells whether its handshake
    /// negotiated that capability.
    pub fn admitSession(self: *HostService, keeper: bool) void {
        self.keepers.admit(keeper);
    }

    /// The facts a handshake reply reports, read on the dispatch thread.
    pub fn candidateFacts(self: *const HostService) candidate_admission.Facts {
        return .{
            .terminals = std.math.cast(u32, self.runtime_aggregate.terminalCount()) orelse
                std.math.maxInt(u32),
            .state = switch (self.lifecycle) {
                .running => self.process_state,
                .shutting_down, .released_after_abandoned_build => .stopping,
            },
            .dispatch_turns = self.dispatch_turns,
        };
    }

    /// Counts one completed turn of the dispatch loop, so a query can tell a Host whose dispatch
    /// thread is held from one that is turning.
    pub fn completeDispatchTurn(self: *HostService) void {
        self.dispatch_turns +|= 1;
    }

    /// Applies a `drain` or `exit` control request on the dispatch thread and returns the
    /// resulting state. Leaving `ready` closes terminal admission for the rest of the process.
    pub fn applyControlRequest(
        self: *HostService,
        request: host_lifecycle.Request,
    ) host_lifecycle.State {
        self.process_state = host_lifecycle.transition(self.process_state, request);
        return self.process_state;
    }

    /// Whether a control request ends the Host now: `exit`, or `drain` once no terminal is
    /// retained. Teardown then ends every terminal the Host still retains.
    /// A creation admitted before a `drain` counts as a terminal until it settles.
    pub fn controlExitDue(self: *const HostService) bool {
        const creating: usize = @intFromBool(self.creations.creating());
        return host_lifecycle.endsNow(self.process_state, self.runtime_aggregate.terminalCount() + creating);
    }

    pub fn canStopWhenIdle(self: *const HostService) bool {
        return self.runtime_aggregate.isEmpty() and !self.creations.creating();
    }

    /// Advances engine-owned state while no broker is connected. Outcome
    /// delivery remains pending until a broker handoff; safety cleanup and
    /// input-credit settlement do not depend on broker availability. A
    /// creation whose requester left settles here, unanswered.
    pub fn maintain(self: *HostService) void {
        if (self.lifecycle == .shutting_down) return;
        if (self.creations.settle(&self.runtime_aggregate, null)) |unexpected| {
            var reply = unexpected;
            reply.deinit();
        }
        self.runtime_aggregate.synchronizeRuntimes();
        // Notices wait for a session's delivery, which may answer one of its requests.
        self.keepers.offer(&self.runtime_aggregate);
        diagnostic_log.flush();
    }

    /// The reply of a creation that finished building, for the session `connection` serves; a
    /// creation whose requester left settles unanswered.
    pub fn settleCreation(
        self: *HostService,
        connection: *const ConnectionState,
    ) ?creation_pipeline.Reply {
        if (self.lifecycle == .shutting_down) return null;
        return self.creations.settle(&self.runtime_aggregate, switch (connection.*) {
            .active => |active| active.connection_id,
            .awaiting_hello, .closing => null,
        });
    }

    /// The next request a settled creation no longer holds, for `handleReleased`; the caller
    /// frees it with this service's allocator.
    pub fn takeReleasedRequest(self: *HostService) ?[]u8 {
        return self.creations.takeReleased();
    }

    /// Whether the session may read another request, or must first let a creation settle.
    pub fn acceptsRequests(self: *const HostService) bool {
        return self.creations.acceptsRequests();
    }

    /// Waits up to `timeout_ns` for a creation's build to return.
    pub fn awaitCreation(self: *HostService, timeout_ns: u64) void {
        self.creations.awaitBuild(timeout_ns);
    }

    pub fn pollEvents(self: *HostService, connection: *const ConnectionState) !Outgoing {
        if (self.lifecycle == .shutting_down) return error.ServiceShuttingDown;
        var outgoing = try self.event_delivery.pollEvents(
            self.allocator,
            &self.runtime_aggregate,
            connection,
        );
        // Infallible: what does not fit stays for the next pass, so the committed batch stands.
        self.keepers.deliver(&self.runtime_aggregate, switch (connection.*) {
            .active => |active| .{ .connection_id = active.connection_id, .outgoing = &outgoing },
            .awaiting_hello, .closing => null,
        });
        return outgoing;
    }

    /// Handles one record. A request of an active session that depends on a creation still in
    /// progress is held, producing nothing until `takeReleasedRequest` returns it.
    pub fn handle(
        self: *HostService,
        connection: *ConnectionState,
        record: []const u8,
    ) !Outgoing {
        if (self.lifecycle == .shutting_down) return error.ServiceShuttingDown;
        switch (connection.*) {
            .active => if (try self.creations.holdIfDependent(self.allocator, record)) {
                return Outgoing.init(self.allocator);
            },
            .awaiting_hello, .closing => {},
        }
        return self.dispatch(connection, record);
    }

    /// Handles a request a creation held, without holding it again.
    pub fn handleReleased(
        self: *HostService,
        connection: *ConnectionState,
        record: []const u8,
    ) !Outgoing {
        if (self.lifecycle == .shutting_down) return error.ServiceShuttingDown;
        return self.dispatch(connection, record);
    }

    fn dispatch(self: *HostService, connection: *ConnectionState, record: []const u8) !Outgoing {
        return self.connection_workflow.handle(
            self.allocator,
            &self.runtime_aggregate,
            self.trace_interactions,
            host_lifecycle.terminalAdmission(self.process_state),
            &self.creations,
            self.keepers.forSession(),
            connection,
            record,
        );
    }

    /// The typed failure that answers a request whose handling failed with `err`, encoded into
    /// `storage` from the request's framed `header`; null when the connection closes instead,
    /// because it is not an active session, the Host is shutting down, or the record cannot be
    /// answered.
    pub fn answerFailedRequest(
        self: *const HostService,
        connection: *const ConnectionState,
        header: []const u8,
        err: anyerror,
        storage: *[failed_request_answer_bytes]u8,
    ) ?[]const u8 {
        if (self.lifecycle == .shutting_down) return null;
        switch (connection.*) {
            .active => {},
            .awaiting_hello, .closing => return null,
        }
        const answer = request_failure.classify(err) orelse return null;
        return request_failure.encode(storage, header, self.runtime_aggregate.hostEpoch().bytes, answer);
    }
};

pub const failed_request_answer_bytes = request_failure.answer_bytes;
/// The request kind a framed header names, for diagnostics.
pub const requestKind = request_failure.headerKind;

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
    pub const creations = creation_pipeline.testing;
    pub const CreationPipelineType = creation_pipeline.Pipeline;
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
