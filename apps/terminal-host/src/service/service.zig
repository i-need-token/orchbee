const std = @import("std");
const build_options = @import("build-options");
const target_builtin = @import("builtin");

const control = @import("terminal-host-control");
const control_decode = control.decode;
const control_encode = control.encode;
const core_module = @import("terminal-host-core").state;
const host_failure = @import("failure.zig");
const host_interaction = @import("interaction.zig");
const host_outgoing = @import("outgoing.zig");
const host_projection = @import("projection.zig");
const host_request_context = @import("request_context.zig");
const host_request_dispatch = @import("request_dispatch.zig");
const host_response = @import("response.zig");
const host_runtime_aggregate = @import("runtime_aggregate.zig");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const protocol = @import("terminal-protocol");
const terminal_configuration = model.configuration;

const MessageKind = protocol.MessageKind;
const WireIdentifier = identifiers.IdentifierBytes;

pub const host_version = build_options.host_version;

const Outgoing = host_outgoing.Outgoing;
const InputFailure = host_failure.InputFailure;
const appendRequired = host_response.appendRequired;
const eventDelivery = host_response.eventDelivery;
const handshakeResponseDelivery = host_response.handshakeResponseDelivery;
const prepareResponse = host_response.prepare;
const responseDelivery = host_response.responseDelivery;
const cleanupFailureDetails = host_failure.fromCleanup;
const coreError = host_failure.fromCore;
const inputFailure = host_failure.fromInput;
const protocolErrorCode = host_failure.protocolCode;

const EgressPriority = enum {
    interaction,
    projection,

    fn next(self: EgressPriority) EgressPriority {
        return switch (self) {
            .interaction => .projection,
            .projection => .interaction,
        };
    }
};

pub const ConnectionState = union(enum) {
    awaiting_hello,
    active: Active,
    closing,

    pub const Active = struct {
        connection_id: core_module.ConnectionId,
        limits: NegotiatedLimits,
    };

    pub const NegotiatedLimits = struct {
        frame_bytes: usize,
        input_bytes: usize,
        row_window_bytes: usize,
    };

    pub fn isAwaitingHello(self: *const ConnectionState) bool {
        return switch (self.*) {
            .awaiting_hello => true,
            .active, .closing => false,
        };
    }

    pub fn shouldClose(self: *const ConnectionState) bool {
        return switch (self.*) {
            .closing => true,
            .active, .awaiting_hello => false,
        };
    }
};

pub const Options = struct {
    terminal_defaults: terminal_configuration.Configuration = .{},
    trace_interactions: bool = false,
};

const ServiceLifecycle = enum {
    running,
    shutting_down,
};

const OutcomePublicationHandoff = struct {
    count: usize = 0,
    tokens: [64]core_module.OutcomePublicationToken = undefined,

    fn add(
        self: *OutcomePublicationHandoff,
        token: core_module.OutcomePublicationToken,
    ) void {
        std.debug.assert(self.count < self.tokens.len);
        std.debug.assert(!self.contains(token));
        self.tokens[self.count] = token;
        self.count += 1;
    }

    fn contains(
        self: *const OutcomePublicationHandoff,
        token: core_module.OutcomePublicationToken,
    ) bool {
        for (self.tokens[0..self.count]) |scheduled| {
            if (scheduled.kind == token.kind and
                scheduled.final_revision == token.final_revision and
                scheduled.terminal_id.eql(token.terminal_id))
            {
                return true;
            }
        }
        return false;
    }

    fn commit(
        self: *OutcomePublicationHandoff,
        core: *core_module.HostCore,
    ) void {
        for (self.tokens[0..self.count]) |token| {
            _ = core.markOutcomePublished(token);
        }
        self.count = 0;
    }
};

// LocalServer gives one dispatch thread ownership of HostService. Each retained
// TerminalRuntime owns the synchronization for its worker threads.
pub const HostService = struct {
    allocator: std.mem.Allocator,
    interaction_scheduler: host_interaction.Scheduler = .{},
    lifecycle: ServiceLifecycle = .running,
    next_egress_priority: EgressPriority = .projection,
    next_connection_id: core_module.ConnectionId = 1,
    projection_scheduler: host_projection.Scheduler = .{},
    runtime_aggregate: host_runtime_aggregate.HostRuntimeAggregate,
    terminal_defaults: terminal_configuration.Configuration,
    terminal_defaults_revision: u64 = 1,
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
            .runtime_aggregate = .init(allocator),
            .terminal_defaults = options.terminal_defaults,
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

    fn beginShutdown(self: *HostService) bool {
        if (self.lifecycle == .shutting_down) return false;
        self.lifecycle = .shutting_down;
        return true;
    }

    pub fn disconnect(self: *HostService, connection: *ConnectionState) void {
        const connection_id: ?core_module.ConnectionId = switch (connection.*) {
            .active => |active| active.connection_id,
            .awaiting_hello, .closing => null,
        };
        if (connection_id) |active_connection_id| {
            _ = self.runtime_aggregate.disconnect(active_connection_id);
        }
        connection.* = .awaiting_hello;
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
        var outcome_handoff: OutcomePublicationHandoff = .{};
        const outgoing = try self.pollEventsPreparing(connection, &outcome_handoff);
        outcome_handoff.commit(self.core());
        return outgoing;
    }

    fn pollEventsPreparing(
        self: *HostService,
        connection: *const ConnectionState,
        outcome_handoff: *OutcomePublicationHandoff,
    ) !Outgoing {
        if (self.lifecycle == .shutting_down) return error.ServiceShuttingDown;
        var outgoing = Outgoing.init(self.allocator);
        errdefer outgoing.deinit();
        const active: *const ConnectionState.Active = switch (connection.*) {
            .active => |*active| active,
            .awaiting_hello, .closing => return outgoing,
        };
        try self.synchronizeRuntimeLifecycle(&outgoing, outcome_handoff);
        try self.scheduleEgress(active, &outgoing);
        return outgoing;
    }

    pub fn handle(
        self: *HostService,
        connection: *ConnectionState,
        record: []const u8,
    ) !Outgoing {
        return self.handlePreparing(connection, record);
    }

    fn handlePreparing(
        self: *HostService,
        connection: *ConnectionState,
        record: []const u8,
    ) !Outgoing {
        if (self.lifecycle == .shutting_down) return error.ServiceShuttingDown;
        var outgoing = Outgoing.init(self.allocator);
        errdefer outgoing.deinit();

        const envelope = protocol.validateTransportEnvelope(record) catch return error.InvalidTransportRecord;
        const active: ?*ConnectionState.Active = switch (connection.*) {
            .awaiting_hello => null,
            .active => |*active| active,
            .closing => return error.ConnectionClosing,
        };
        if (active != null and envelope.correlation_id == null) {
            return error.InvalidTransportRecord;
        }
        if (active != null) {
            // Runtime observations must be linearized before command authority
            // and phase checks, but their asynchronous events are emitted only
            // by pollEvents. They cannot consume or invalidate this reply batch.
            self.runtime_aggregate.synchronizeRuntimes();
        }
        if (envelope.event or envelope.response) {
            if (active == null) {
                try self.addHandshakeFailure(&outgoing, envelope.correlation_id, "invalid-envelope-role", "client record must be a request", false);
                connection.* = .closing;
            } else {
                try self.addFailure(&outgoing, envelope.correlation_id.?, "invalid-envelope-role", "client record must be a request", false);
            }
            return outgoing;
        }
        if (active != null and
            !usesCurrentProtocolVersion(envelope))
        {
            try self.addFailure(&outgoing, envelope.correlation_id.?, "unsupported-version", "unsupported envelope protocol version", false);
            return outgoing;
        }

        const active_connection = active orelse {
            try self.handleHello(connection, envelope, &outgoing);
            return outgoing;
        };
        if (envelope.kind == MessageKind.hello) {
            try self.addFailure(&outgoing, envelope.correlation_id.?, "repeated-hello", "Hello is allowed exactly once per connection", false);
            return outgoing;
        }
        const correlation_id = envelope.correlation_id.?;
        if (envelope.host_epoch == null or
            !core_module.HostEpoch.fromBytes(envelope.host_epoch.?).eql(self.core().epoch))
        {
            try self.addFailure(&outgoing, correlation_id, "stale-epoch", "request HostEpoch is not current", false);
            return outgoing;
        }
        if (envelope.optional_message and !protocol.isKnownMessageKind(envelope.kind)) return outgoing;
        const request_contract = protocol.messageContract(envelope.kind).?.request orelse {
            try self.addFailure(&outgoing, correlation_id, "invalid-envelope-role", "message kind is not a client request", false);
            return outgoing;
        };
        if (envelope.payload_encoding != @intFromEnum(request_contract.payload_encoding)) {
            try self.addFailure(&outgoing, correlation_id, "invalid-field", "message kind uses the wrong payload encoding", false);
            return outgoing;
        }

        var request_dispatcher = host_request_dispatch.Dispatcher.init(
            self.allocator,
            &self.runtime_aggregate,
            &self.terminal_defaults,
            &self.terminal_defaults_revision,
            self.trace_interactions,
        );
        try request_dispatcher.dispatch(
            request_contract.operation,
            active_connection.connection_id,
            .{
                .input_bytes = active_connection.limits.input_bytes,
                .row_window_bytes = active_connection.limits.row_window_bytes,
            },
            envelope,
            correlation_id,
            &outgoing,
        );
        // A request dispatch contains only records caused by that request.
        // Lifecycle, projection, and interaction events are polled separately,
        // so unrelated fallible work cannot discard a committed command result.
        return outgoing;
    }

    fn handleHello(
        self: *HostService,
        connection: *ConnectionState,
        envelope: protocol.EnvelopeSummary,
        outgoing: *Outgoing,
    ) !void {
        if (envelope.kind != MessageKind.hello or envelope.payload_encoding != @intFromEnum(protocol.PayloadEncoding.control_json) or envelope.host_epoch != null) {
            try self.addHandshakeFailure(outgoing, envelope.correlation_id, "handshake-required", "first request must be Hello without HostEpoch", false);
            connection.* = .closing;
            return;
        }
        if (!usesCurrentProtocolVersion(envelope)) {
            const payload = try control_encode.protocolRejected(
                self.allocator,
                envelope.protocol_major,
                envelope.protocol_minor,
                envelope.protocol_patch,
                protocol.current_protocol_major,
                protocol.current_protocol_minor,
                protocol.current_protocol_patch,
            );
            defer self.allocator.free(payload);
            _ = try appendRequired(
                outgoing,
                MessageKind.protocol_rejected,
                .control_json,
                payload,
                handshakeResponseDelivery(envelope.correlation_id, null),
            );
            connection.* = .closing;
            return;
        }
        var decoded = control_decode.decodeHello(self.allocator, envelope.payload) catch |err| {
            const failure = host_request_context.classifyControlDecodeError(err) catch
                return error.OutOfMemory;
            const message = switch (failure.kind) {
                .inspection_resource => "Host could not inspect the Hello command",
                .invalid_identifier => "ClientId is invalid",
                .invalid_payload => "Hello control payload is invalid",
            };
            try self.addHandshakeFailure(
                outgoing,
                envelope.correlation_id,
                failure.code,
                message,
                failure.retryable,
            );
            connection.* = .closing;
            return;
        };
        defer decoded.deinit();
        const hello = decoded.value;
        if (envelope.protocol_major != hello.protocol_version.major or
            envelope.protocol_minor != hello.protocol_version.minor or
            envelope.protocol_patch != hello.protocol_version.patch)
        {
            try self.addHandshakeFailure(outgoing, envelope.correlation_id, "invalid-field", "Hello envelope and payload versions differ", false);
            connection.* = .closing;
            return;
        }
        const limits: ConnectionState.NegotiatedLimits = .{
            .frame_bytes = hello.frame_bytes,
            .input_bytes = hello.input_bytes,
            .row_window_bytes = hello.row_window_bytes,
        };
        var response = try prepareResponse(
            outgoing,
            protocol.Limits.control_json_bytes,
            handshakeResponseDelivery(envelope.correlation_id, self.core().epoch),
        );
        defer response.deinit();
        const connection_id = self.prepareConnectionId() catch {
            try self.addHandshakeFailure(outgoing, envelope.correlation_id, "resource-exhausted", "Host connection identity space is exhausted", false);
            connection.* = .closing;
            return;
        };
        _ = self.core().connect(connection_id, hello.client_id) catch |err| {
            const failure = coreError(err);
            try self.addHandshakeFailure(
                outgoing,
                envelope.correlation_id,
                failure.code,
                failure.message,
                failure.retryable,
            );
            connection.* = .closing;
            return;
        };
        self.commitConnectionId(connection_id);
        connection.* = .{ .active = .{
            .connection_id = connection_id,
            .limits = limits,
        } };
        var output = response.writer();
        control_encode.writeHelloAccepted(
            &output,
            limits,
            self.core().epoch,
            host_version,
        ) catch unreachable;
        _ = response.commitControl(MessageKind.hello_accepted, &output);
    }

    fn prepareConnectionId(self: *const HostService) error{ConnectionIdentityExhausted}!core_module.ConnectionId {
        if (self.next_connection_id == std.math.maxInt(core_module.ConnectionId)) {
            return error.ConnectionIdentityExhausted;
        }
        return self.next_connection_id;
    }

    fn commitConnectionId(self: *HostService, connection_id: core_module.ConnectionId) void {
        std.debug.assert(connection_id == self.next_connection_id);
        self.next_connection_id = connection_id + 1;
    }

    fn synchronizeRuntimeLifecycle(
        self: *HostService,
        outgoing: *Outgoing,
        outcome_handoff: *OutcomePublicationHandoff,
    ) !void {
        self.runtime_aggregate.synchronizeRuntimes();
        try self.appendPendingOutcomes(outgoing, outcome_handoff);
    }

    fn appendPendingOutcomes(
        self: *HostService,
        outgoing: *Outgoing,
        outcome_handoff: *OutcomePublicationHandoff,
    ) !void {
        const remaining = outcome_handoff.tokens.len - outcome_handoff.count;
        if (remaining == 0) return;
        const pending_outcomes = self.runtime_aggregate.pendingOutcomes(remaining);
        for (pending_outcomes.outcomes()) |outcome| {
            const token = outcome.token();
            if (outcome_handoff.contains(token)) continue;
            if (outcome_handoff.count == outcome_handoff.tokens.len) break;
            const payload = switch (outcome) {
                .exited => |notice| try control_encode.terminalExited(
                    self.allocator,
                    self.core().epoch,
                    notice,
                ),
                .failed => |notice| try control_encode.terminalFailed(
                    self.allocator,
                    self.core().epoch,
                    notice,
                ),
            };
            defer self.allocator.free(payload);
            switch (try outgoing.append(
                switch (outcome) {
                    .exited => MessageKind.terminal_exited,
                    .failed => MessageKind.terminal_failed,
                },
                .control_json,
                payload,
                eventDelivery(self.core().epoch),
            )) {
                .appended => outcome_handoff.add(token),
                .full => break,
            }
        }
    }

    fn scheduleEgress(
        self: *HostService,
        connection: *const ConnectionState.Active,
        outgoing: *Outgoing,
    ) !void {
        const first = self.next_egress_priority;
        const record_count_before = outgoing.records().len;
        const first_outcome = switch (first) {
            .interaction => try self.interaction_scheduler.schedule(
                self.allocator,
                &self.runtime_aggregate,
                connection.connection_id,
                outgoing,
            ),
            .projection => try self.projection_scheduler.schedule(
                self.allocator,
                &self.runtime_aggregate,
                connection.connection_id,
                connection.limits.frame_bytes,
                outgoing,
            ),
        };

        // Once one class emits records, end this dispatch with a coherent
        // scheduler transaction. The other class leads the next dispatch, so
        // a stream of full projection batches cannot starve interactions.
        if (first_outcome == .complete and
            outgoing.records().len == record_count_before)
        {
            _ = switch (first.next()) {
                .interaction => try self.interaction_scheduler.schedule(
                    self.allocator,
                    &self.runtime_aggregate,
                    connection.connection_id,
                    outgoing,
                ),
                .projection => try self.projection_scheduler.schedule(
                    self.allocator,
                    &self.runtime_aggregate,
                    connection.connection_id,
                    connection.limits.frame_bytes,
                    outgoing,
                ),
            };
        }
        self.next_egress_priority = first.next();
    }

    fn core(self: *HostService) *core_module.HostCore {
        return self.runtime_aggregate.core();
    }

    fn addFailure(
        self: *HostService,
        outgoing: *Outgoing,
        correlation_id: WireIdentifier,
        code: []const u8,
        message: []const u8,
        retryable: bool,
    ) !void {
        try self.addTypedFailure(
            outgoing,
            code,
            message,
            retryable,
            responseDelivery(correlation_id, self.core().epoch),
        );
    }

    fn addHandshakeFailure(
        self: *HostService,
        outgoing: *Outgoing,
        correlation_id: ?WireIdentifier,
        code: []const u8,
        message: []const u8,
        retryable: bool,
    ) !void {
        try self.addTypedFailure(
            outgoing,
            code,
            message,
            retryable,
            handshakeResponseDelivery(correlation_id, null),
        );
    }

    fn addTypedFailure(
        self: *HostService,
        outgoing: *Outgoing,
        code: []const u8,
        message: []const u8,
        retryable: bool,
        delivery: host_outgoing.Delivery,
    ) !void {
        try host_response.appendFailure(
            self.allocator,
            outgoing,
            .{ .code = code, .message = message, .retryable = retryable },
            delivery,
        );
    }
};

fn usesCurrentProtocolVersion(envelope: protocol.EnvelopeSummary) bool {
    return envelope.protocol_major == protocol.current_protocol_major and
        envelope.protocol_minor == protocol.current_protocol_minor and
        envelope.protocol_patch == protocol.current_protocol_patch;
}

pub const testing = if (target_builtin.is_test) struct {
    pub const beginShutdownFn = HostService.beginShutdown;
    pub const InputFailureType = InputFailure;
    pub const OutcomePublicationHandoffType = OutcomePublicationHandoff;
    pub const commitOutcomePublicationHandoffFn = OutcomePublicationHandoff.commit;
    pub const cleanupFailureDetailsFn = cleanupFailureDetails;
    pub const identifierTextFn = control_encode.identifierText;
    pub const inputFailureFn = inputFailure;
    pub const protocolErrorCodeFn = protocolErrorCode;
    pub const scheduleEgressFn = HostService.scheduleEgress;
    pub const synchronizeRuntimeLifecycleFn = HostService.synchronizeRuntimeLifecycle;
} else struct {};
