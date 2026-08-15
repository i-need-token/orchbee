const std = @import("std");
const build_options = @import("build-options");

const control = @import("terminal-host-control");
const control_decode = control.decode;
const control_encode = control.encode;
const core_module = @import("terminal-host-core").state;
const host_failure = @import("failure.zig");
const host_outgoing = @import("outgoing.zig");
const host_request_context = @import("request_context.zig");
const host_request_dispatch = @import("request_dispatch.zig");
const host_response = @import("response.zig");
const host_runtime_aggregate = @import("runtime_aggregate/root.zig");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");

const MessageKind = protocol.MessageKind;
const Outgoing = host_outgoing.Outgoing;
const WireIdentifier = identifiers.IdentifierBytes;

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

/// Owns physical-connection admission identity and the complete ingress path:
/// envelope validation, one-shot handshake, active-session authorization, and
/// request dispatch. It never emits asynchronous runtime events.
pub const Workflow = struct {
    next_connection_id: core_module.ConnectionId = 1,

    pub fn disconnect(
        _: *Workflow,
        runtime_aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
        connection: *ConnectionState,
    ) void {
        const connection_id: ?core_module.ConnectionId = switch (connection.*) {
            .active => |active| active.connection_id,
            .awaiting_hello, .closing => null,
        };
        if (connection_id) |active_connection_id| {
            _ = runtime_aggregate.disconnect(active_connection_id);
        }
        connection.* = .awaiting_hello;
    }

    pub fn handle(
        self: *Workflow,
        allocator: std.mem.Allocator,
        runtime_aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
        trace_interactions: bool,
        connection: *ConnectionState,
        record: []const u8,
    ) !Outgoing {
        var outgoing = Outgoing.init(allocator);
        errdefer outgoing.deinit();

        const envelope = protocol.validateTransportEnvelope(record) catch
            return error.InvalidTransportRecord;
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
            // and phase checks, but asynchronous events are emitted only by
            // the event-delivery workflow.
            runtime_aggregate.synchronizeRuntimes();
        }
        if (envelope.event or envelope.response) {
            if (active == null) {
                try addHandshakeFailure(
                    allocator,
                    runtime_aggregate,
                    &outgoing,
                    envelope.correlation_id,
                    "invalid-envelope-role",
                    "client record must be a request",
                    false,
                );
                connection.* = .closing;
            } else {
                try addFailure(
                    allocator,
                    runtime_aggregate,
                    &outgoing,
                    envelope.correlation_id.?,
                    "invalid-envelope-role",
                    "client record must be a request",
                    false,
                );
            }
            return outgoing;
        }
        if (active != null and !usesCurrentProtocolVersion(envelope)) {
            try addFailure(
                allocator,
                runtime_aggregate,
                &outgoing,
                envelope.correlation_id.?,
                "unsupported-version",
                "unsupported envelope protocol version",
                false,
            );
            return outgoing;
        }

        const active_connection = active orelse {
            try self.handleHello(allocator, runtime_aggregate, connection, envelope, &outgoing);
            return outgoing;
        };
        if (envelope.kind == MessageKind.hello) {
            try addFailure(
                allocator,
                runtime_aggregate,
                &outgoing,
                envelope.correlation_id.?,
                "repeated-hello",
                "Hello is allowed exactly once per connection",
                false,
            );
            return outgoing;
        }
        const correlation_id = envelope.correlation_id.?;
        if (envelope.host_epoch == null or
            !core_module.HostEpoch.fromBytes(envelope.host_epoch.?).eql(runtime_aggregate.hostEpoch()))
        {
            try addFailure(
                allocator,
                runtime_aggregate,
                &outgoing,
                correlation_id,
                "stale-epoch",
                "request HostEpoch is not current",
                false,
            );
            return outgoing;
        }
        if (envelope.optional_message and !protocol.isKnownMessageKind(envelope.kind)) return outgoing;
        const request_contract = protocol.messageContract(envelope.kind).?.request orelse {
            try addFailure(
                allocator,
                runtime_aggregate,
                &outgoing,
                correlation_id,
                "invalid-envelope-role",
                "message kind is not a client request",
                false,
            );
            return outgoing;
        };
        if (envelope.payload_encoding != @intFromEnum(request_contract.payload_encoding)) {
            try addFailure(
                allocator,
                runtime_aggregate,
                &outgoing,
                correlation_id,
                "invalid-field",
                "message kind uses the wrong payload encoding",
                false,
            );
            return outgoing;
        }

        var request_dispatcher = host_request_dispatch.Dispatcher.init(
            allocator,
            runtime_aggregate,
            trace_interactions,
        );
        try request_dispatcher.dispatch(
            request_contract,
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
        self: *Workflow,
        allocator: std.mem.Allocator,
        runtime_aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
        connection: *ConnectionState,
        envelope: protocol.EnvelopeSummary,
        outgoing: *Outgoing,
    ) !void {
        if (envelope.kind != MessageKind.hello or
            envelope.host_epoch != null)
        {
            try addHandshakeFailure(
                allocator,
                runtime_aggregate,
                outgoing,
                envelope.correlation_id,
                "handshake-required",
                "first request must be Hello without HostEpoch",
                false,
            );
            connection.* = .closing;
            return;
        }
        if (!usesCurrentProtocolVersion(envelope)) {
            const payload = try control_encode.protocolRejected(
                allocator,
                envelope.protocol_major,
                envelope.protocol_minor,
                envelope.protocol_patch,
                protocol.current_protocol_major,
                protocol.current_protocol_minor,
                protocol.current_protocol_patch,
            );
            defer allocator.free(payload);
            _ = try host_response.appendRequired(
                outgoing,
                MessageKind.protocol_rejected,
                .control_json,
                payload,
                host_response.handshakeResponseDelivery(envelope.correlation_id, null),
            );
            connection.* = .closing;
            return;
        }
        const hello_request = protocol.messageContract(MessageKind.hello).?.request.?;
        if (envelope.payload_encoding != @intFromEnum(hello_request.payload_encoding)) {
            try addHandshakeFailure(
                allocator,
                runtime_aggregate,
                outgoing,
                envelope.correlation_id,
                "handshake-required",
                "first request must be Hello without HostEpoch",
                false,
            );
            connection.* = .closing;
            return;
        }
        const success_response = try host_response.resolveSuccessResponse(hello_request);
        const success = try host_response.requireControlJsonSuccess(success_response);
        var decoded = control_decode.decodeHello(allocator, envelope.payload) catch |err| {
            const failure = host_request_context.classifyControlDecodeError(err) catch
                return error.OutOfMemory;
            const message = switch (failure.kind) {
                .inspection_resource => "Host could not inspect the Hello command",
                .invalid_identifier => "ClientId is invalid",
                .invalid_payload => "Hello control payload is invalid",
            };
            try addHandshakeFailure(
                allocator,
                runtime_aggregate,
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
            try addHandshakeFailure(
                allocator,
                runtime_aggregate,
                outgoing,
                envelope.correlation_id,
                "invalid-field",
                "Hello envelope and payload versions differ",
                false,
            );
            connection.* = .closing;
            return;
        }
        const limits: ConnectionState.NegotiatedLimits = .{
            .frame_bytes = hello.frame_bytes,
            .input_bytes = hello.input_bytes,
            .row_window_bytes = hello.row_window_bytes,
        };
        var response = try host_response.prepare(
            outgoing,
            protocol.Limits.control_json_bytes,
            host_response.handshakeResponseDelivery(envelope.correlation_id, runtime_aggregate.hostEpoch()),
        );
        defer response.deinit();
        const connection_id = self.prepareConnectionId() catch {
            try addHandshakeFailure(
                allocator,
                runtime_aggregate,
                outgoing,
                envelope.correlation_id,
                "resource-exhausted",
                "Host connection identity space is exhausted",
                false,
            );
            connection.* = .closing;
            return;
        };
        _ = runtime_aggregate.connect(connection_id, hello.client_id) catch |err| {
            const failure = host_failure.fromCore(err);
            try addHandshakeFailure(
                allocator,
                runtime_aggregate,
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
            runtime_aggregate.hostEpoch(),
            build_options.host_version,
        ) catch unreachable;
        _ = response.commitControlSuccess(success, &output);
    }

    fn prepareConnectionId(
        self: *const Workflow,
    ) error{ConnectionIdentityExhausted}!core_module.ConnectionId {
        if (self.next_connection_id == std.math.maxInt(core_module.ConnectionId)) {
            return error.ConnectionIdentityExhausted;
        }
        return self.next_connection_id;
    }

    fn commitConnectionId(self: *Workflow, connection_id: core_module.ConnectionId) void {
        std.debug.assert(connection_id == self.next_connection_id);
        self.next_connection_id = connection_id + 1;
    }
};

fn usesCurrentProtocolVersion(envelope: protocol.EnvelopeSummary) bool {
    return envelope.protocol_major == protocol.current_protocol_major and
        envelope.protocol_minor == protocol.current_protocol_minor and
        envelope.protocol_patch == protocol.current_protocol_patch;
}

fn addFailure(
    allocator: std.mem.Allocator,
    runtime_aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    outgoing: *Outgoing,
    correlation_id: WireIdentifier,
    code: []const u8,
    message: []const u8,
    retryable: bool,
) !void {
    try addTypedFailure(
        allocator,
        outgoing,
        code,
        message,
        retryable,
        host_response.responseDelivery(correlation_id, runtime_aggregate.hostEpoch()),
    );
}

fn addHandshakeFailure(
    allocator: std.mem.Allocator,
    _: *host_runtime_aggregate.HostRuntimeAggregate,
    outgoing: *Outgoing,
    correlation_id: ?WireIdentifier,
    code: []const u8,
    message: []const u8,
    retryable: bool,
) !void {
    try addTypedFailure(
        allocator,
        outgoing,
        code,
        message,
        retryable,
        host_response.handshakeResponseDelivery(correlation_id, null),
    );
}

fn addTypedFailure(
    allocator: std.mem.Allocator,
    outgoing: *Outgoing,
    code: []const u8,
    message: []const u8,
    retryable: bool,
    delivery: host_outgoing.Delivery,
) !void {
    try host_response.appendFailure(
        allocator,
        outgoing,
        .{ .code = code, .message = message, .retryable = retryable },
        delivery,
    );
}
