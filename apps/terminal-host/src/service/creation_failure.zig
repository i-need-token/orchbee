//! The typed failure a settled creation sends when its build produced no runtime, encoded into a
//! fixed record so a settle never allocates to answer.
const std = @import("std");

const control = @import("terminal-host-control");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");
const host_response = @import("response.zig");

const record_bytes = protocol.transport_envelope_header_bytes + 512;

/// A complete typed-failure record.
pub const FailureRecord = struct {
    storage: [record_bytes]u8,
    len: usize,

    pub fn bytes(self: *const FailureRecord) []const u8 {
        return self.storage[0..self.len];
    }
};

/// A build that created or started a terminal.
pub const BuildFailure = enum { allocation, spawn, missing_launch };

pub fn details(failure: BuildFailure) host_response.FailureDetails {
    return switch (failure) {
        .allocation => .{ .code = "resource-exhausted", .message = "Host could not allocate terminal runtime state", .retryable = true },
        .spawn => .{ .code = "spawn-failed", .message = "PTY or child creation failed", .retryable = true },
        .missing_launch => .{ .code = "internal-error", .message = "deferred terminal launch state is missing", .retryable = false },
    };
}

/// The record answering `correlation_id` with `failure`, whose text the Host fixes, so it fits.
pub fn encode(
    failure: host_response.FailureDetails,
    correlation_id: identifiers.IdentifierBytes,
    host_epoch: identifiers.HostEpoch,
) FailureRecord {
    var record: FailureRecord = .{ .storage = undefined, .len = 0 };
    var payload: std.Io.Writer = .fixed(record.storage[protocol.transport_envelope_header_bytes..]);
    control.encode.writeTypedFailureWithReason(&payload, failure.code, failure.message, failure.retryable, failure.reason) catch unreachable;
    record.len = (protocol.finalizeTransportEnvelope(
        &record.storage,
        protocol.MessageKind.typed_failure,
        .control_json,
        payload.buffered().len,
        correlation_id,
        host_epoch.bytes,
        true,
        false,
    ) catch unreachable).len;
    return record;
}
