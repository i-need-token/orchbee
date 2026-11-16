//! The answer to one Worker request the Host could not carry out: a typed failure on that
//! request's correlation id, encoded into storage the caller owns, so an answer needs no memory
//! even when the failure was running out of it. A request whose error escapes dispatch has
//! changed nothing, because every request prepares before its infallible commit, so answering it
//! and serving the next request is safe.
const std = @import("std");

const control_encode = @import("terminal-host-control").encode;
const protocol = @import("terminal-protocol");

/// Room for the header and the longest answer below.
pub const answer_bytes = protocol.transport_envelope_header_bytes + 512;

pub const Answer = enum {
    resource_exhausted,
    invalid_envelope,
    internal_error,

    fn code(self: Answer) []const u8 {
        return switch (self) {
            .resource_exhausted => "resource-exhausted",
            .invalid_envelope => "invalid-envelope",
            .internal_error => "internal-error",
        };
    }

    fn message(self: Answer) []const u8 {
        return switch (self) {
            .resource_exhausted => "Host could not hold this request; retry it",
            .invalid_envelope => "request envelope has a payload encoding this Host does not know",
            .internal_error => "Host could not carry out this request",
        };
    }

    fn retryable(self: Answer) bool {
        return self == .resource_exhausted;
    }
};

/// How the Host answers an error that request handling returned on an active connection, or
/// null when the connection closes: a record that is not a request the session can answer, or a
/// Host that is shutting down.
pub fn classify(err: anyerror) ?Answer {
    return switch (err) {
        error.InvalidTransportRecord, error.ConnectionClosing, error.ServiceShuttingDown => null,
        // `HeldRequestLimit`: more requests wait behind a creation than the Host holds.
        error.OutOfMemory, error.OutgoingCapacityInvariant, error.HeldRequestLimit => .resource_exhausted,
        // A framed record of an unknown payload encoding, read whole and discarded.
        error.UnknownPayloadEncoding => .invalid_envelope,
        else => .internal_error,
    };
}

/// The correlation id in a framed record's header, or null when it carries none.
pub fn headerCorrelationId(header: []const u8) ?[16]u8 {
    if (header.len < protocol.transport_envelope_header_bytes) return null;
    const bytes = header[24..40];
    if (std.mem.allEqual(u8, bytes, 0)) return null;
    return bytes.*;
}

/// The request kind in a framed record's header.
pub fn headerKind(header: []const u8) ?u16 {
    if (header.len < protocol.transport_envelope_header_bytes) return null;
    return std.mem.readInt(u16, header[20..22], .little);
}

/// Encodes `answer` for the request whose header is `header` into `storage`; null when the
/// header carries no correlation id to answer.
pub fn encode(
    storage: *[answer_bytes]u8,
    header: []const u8,
    host_epoch: [16]u8,
    answer: Answer,
) ?[]const u8 {
    const correlation_id = headerCorrelationId(header) orelse return null;
    var payload: std.Io.Writer = .fixed(storage[protocol.transport_envelope_header_bytes..]);
    control_encode.writeTypedFailure(
        &payload,
        answer.code(),
        answer.message(),
        answer.retryable(),
    ) catch return null;
    return protocol.finalizeTransportEnvelope(
        storage,
        protocol.MessageKind.typed_failure,
        .control_json,
        payload.buffered().len,
        correlation_id,
        host_epoch,
        true,
        false,
    ) catch null;
}
