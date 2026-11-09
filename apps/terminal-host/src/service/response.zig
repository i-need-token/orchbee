const std = @import("std");

const control_encode = @import("terminal-host-control").encode;
const host_outgoing = @import("outgoing.zig");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");

const MessageKind = protocol.MessageKind;

pub const FailureDetails = struct {
    code: []const u8,
    message: []const u8,
    retryable: bool,
    reason: ?[]const u8 = null,
};

pub const SuccessResponse = struct {
    kind: u16,
    encoding: protocol.PayloadEncoding,
};

pub const ControlJsonSuccess = struct {
    kind: u16,
};

pub const RowWindowSuccess = struct {
    kind: u16,
};

pub fn resolveSuccessResponse(
    request: protocol.RequestMessageContract,
) error{InvalidSuccessResponseContract}!SuccessResponse {
    const contract = protocol.messageContract(request.success_kind) orelse
        return error.InvalidSuccessResponseContract;
    const encoding = contract.response_encoding orelse
        return error.InvalidSuccessResponseContract;
    return .{ .kind = request.success_kind, .encoding = encoding };
}

pub fn requireControlJsonSuccess(
    success: SuccessResponse,
) error{IncompatibleSuccessResponseEncoding}!ControlJsonSuccess {
    if (success.encoding != .control_json) {
        return error.IncompatibleSuccessResponseEncoding;
    }
    return .{ .kind = success.kind };
}

pub fn requireRowWindowSuccess(
    success: SuccessResponse,
) error{IncompatibleSuccessResponseEncoding}!RowWindowSuccess {
    if (success.encoding != .row_window) {
        return error.IncompatibleSuccessResponseEncoding;
    }
    return .{ .kind = success.kind };
}

/// Owns the reserved response record until a caller either commits exactly one
/// encoded payload or abandons the response. Domain effects may therefore run
/// only after bounded outgoing capacity has been reserved.
pub const PreparedResponse = struct {
    record: host_outgoing.RecordReservation,

    pub fn deinit(self: *PreparedResponse) void {
        self.record.deinit();
        self.* = undefined;
    }

    pub fn writer(self: *PreparedResponse) std.Io.Writer {
        return .fixed(self.payloadBuffer());
    }

    pub fn payloadBuffer(self: *PreparedResponse) []u8 {
        return self.record.payloadBuffer();
    }

    pub fn commitControl(
        self: *PreparedResponse,
        kind: u16,
        writer_value: *const std.Io.Writer,
    ) host_outgoing.AppendToken {
        return self.record.commitWritten(
            kind,
            .control_json,
            writer_value.buffered().len,
        );
    }

    pub fn commitControlSuccess(
        self: *PreparedResponse,
        success: ControlJsonSuccess,
        writer_value: *const std.Io.Writer,
    ) host_outgoing.AppendToken {
        return self.commitControl(success.kind, writer_value);
    }

    pub fn commitRowWindowSuccess(
        self: *PreparedResponse,
        success: RowWindowSuccess,
        payload_length: usize,
    ) host_outgoing.AppendToken {
        return self.record.commitWritten(success.kind, .row_window, payload_length);
    }

    pub fn commitPayload(
        self: *PreparedResponse,
        kind: u16,
        encoding: protocol.PayloadEncoding,
        payload: []const u8,
    ) host_outgoing.AppendToken {
        return self.record.commitPayload(kind, encoding, payload);
    }

    pub fn commitWrittenPayload(
        self: *PreparedResponse,
        kind: u16,
        encoding: protocol.PayloadEncoding,
        payload_length: usize,
    ) host_outgoing.AppendToken {
        return self.record.commitWritten(kind, encoding, payload_length);
    }

    pub fn commitFailure(
        self: *PreparedResponse,
        failure: FailureDetails,
    ) void {
        var output = self.writer();
        control_encode.writeTypedFailureWithReason(
            &output,
            failure.code,
            failure.message,
            failure.retryable,
            failure.reason,
        ) catch unreachable;
        _ = self.commitControl(MessageKind.typed_failure, &output);
    }
};

/// Reserves the maximum response capacity while retaining only a small failure
/// buffer. A successful producer transfers its exact envelope-ready allocation
/// into the reserved slot without another allocation or payload copy.
pub const PreparedOwnedResponse = struct {
    allocator: std.mem.Allocator,
    failure_allocation: ?[]u8,
    record: host_outgoing.RecordCapacityReservation,

    pub fn deinit(self: *PreparedOwnedResponse) void {
        if (self.failure_allocation) |allocation| self.allocator.free(allocation);
        self.record.deinit();
        self.* = undefined;
    }

    pub fn commitRowWindowSuccess(
        self: *PreparedOwnedResponse,
        success: RowWindowSuccess,
        allocation: []u8,
        payload: []const u8,
    ) host_outgoing.AppendToken {
        std.debug.assert(
            allocation.len == host_outgoing.owned_payload_prefix_bytes + payload.len,
        );
        const failure_allocation = self.failure_allocation.?;
        self.allocator.free(failure_allocation);
        self.failure_allocation = null;
        return self.record.commitOwnedPayload(
            success.kind,
            .row_window,
            allocation,
            payload,
        );
    }

    pub fn commitFailure(
        self: *PreparedOwnedResponse,
        failure: FailureDetails,
    ) void {
        const allocation = self.failure_allocation.?;
        const payload_buffer = allocation[host_outgoing.owned_payload_prefix_bytes..];
        var output: std.Io.Writer = .fixed(payload_buffer);
        control_encode.writeTypedFailureWithReason(
            &output,
            failure.code,
            failure.message,
            failure.retryable,
            failure.reason,
        ) catch unreachable;
        _ = self.record.commitOwnedPayload(
            MessageKind.typed_failure,
            .control_json,
            allocation,
            output.buffered(),
        );
        self.failure_allocation = null;
    }
};

pub fn requireAppended(
    outcome: host_outgoing.AppendOutcome,
) !host_outgoing.AppendToken {
    return switch (outcome) {
        .appended => |token| token,
        .full => error.OutgoingCapacityInvariant,
    };
}

pub fn appendRequired(
    outgoing: *host_outgoing.Outgoing,
    kind: u16,
    encoding: protocol.PayloadEncoding,
    payload: []const u8,
    delivery: host_outgoing.Delivery,
) !host_outgoing.AppendToken {
    return requireAppended(try outgoing.append(kind, encoding, payload, delivery));
}

pub fn appendControlSuccess(
    outgoing: *host_outgoing.Outgoing,
    success: ControlJsonSuccess,
    payload: []const u8,
    delivery: host_outgoing.Delivery,
) !host_outgoing.AppendToken {
    return appendRequired(outgoing, success.kind, .control_json, payload, delivery);
}

pub fn appendFailure(
    allocator: std.mem.Allocator,
    outgoing: *host_outgoing.Outgoing,
    failure: FailureDetails,
    delivery: host_outgoing.Delivery,
) !void {
    const payload = try control_encode.typedFailureWithReason(
        allocator,
        failure.code,
        failure.message,
        failure.retryable,
        failure.reason,
    );
    defer allocator.free(payload);
    _ = try appendRequired(
        outgoing,
        MessageKind.typed_failure,
        .control_json,
        payload,
        delivery,
    );
}

pub fn prepare(
    outgoing: *host_outgoing.Outgoing,
    maximum_payload_length: usize,
    delivery: host_outgoing.Delivery,
) !PreparedResponse {
    return .{ .record = switch (try outgoing.reserve(
        maximum_payload_length,
        delivery,
    )) {
        .full => return error.OutgoingCapacityInvariant,
        .reserved => |reservation| reservation,
    } };
}

pub fn prepareOwned(
    outgoing: *host_outgoing.Outgoing,
    maximum_payload_length: usize,
    delivery: host_outgoing.Delivery,
) !PreparedOwnedResponse {
    const reserved_payload_length = @max(
        maximum_payload_length,
        protocol.Limits.control_json_bytes,
    );
    var record = switch (try outgoing.reserveCapacity(
        reserved_payload_length,
        delivery,
    )) {
        .full => return error.OutgoingCapacityInvariant,
        .reserved => |reservation| reservation,
    };
    errdefer record.deinit();
    const allocator = outgoing.ownedPayloadAllocator();
    const failure_allocation = try allocator.alloc(
        u8,
        host_outgoing.owned_payload_prefix_bytes + protocol.Limits.control_json_bytes,
    );
    return .{
        .allocator = allocator,
        .failure_allocation = failure_allocation,
        .record = record,
    };
}

pub fn prepareControl(
    outgoing: *host_outgoing.Outgoing,
    correlation_id: identifiers.IdentifierBytes,
    host_epoch: identifiers.HostEpoch,
) !PreparedResponse {
    return prepare(
        outgoing,
        protocol.Limits.control_json_bytes,
        responseDelivery(correlation_id, host_epoch),
    );
}

pub fn eventDelivery(host_epoch: identifiers.HostEpoch) host_outgoing.Delivery {
    return .{ .event = host_epoch };
}

pub fn responseDelivery(
    correlation_id: identifiers.IdentifierBytes,
    host_epoch: identifiers.HostEpoch,
) host_outgoing.Delivery {
    return .{ .response = .{
        .correlation_id = correlation_id,
        .host_epoch = host_epoch,
    } };
}

pub fn handshakeResponseDelivery(
    correlation_id: ?identifiers.IdentifierBytes,
    host_epoch: ?identifiers.HostEpoch,
) host_outgoing.Delivery {
    return .{ .handshake_response = .{
        .correlation_id = correlation_id,
        .host_epoch = host_epoch,
    } };
}
