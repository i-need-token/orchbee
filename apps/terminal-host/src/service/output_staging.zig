//! One staged output seed for the next creation on a broker connection.
//!
//! `StageTerminalOutput` admits earlier output for exactly one creation identity. The next
//! `CreateTerminal` on the same connection takes the slot: a new terminal with that creation
//! identity starts with the seed; any other creation, or a replay of an existing one,
//! discards it. Staging again replaces the slot, so the Host retains at most one seed.
const std = @import("std");
const core = @import("terminal-host-core").state;
const engine = @import("terminal-host-engine");
const protocol = @import("terminal-protocol");
const identifiers = @import("terminal-host-model").identifiers;
const Context = @import("request_context.zig").Context;
const Outgoing = @import("outgoing.zig").Outgoing;
const response = @import("response.zig");

pub const request_magic = "OBSO\x00\x01\x00\x00";
pub const response_magic = "OBSS\x00\x01\x00\x00";
pub const header_bytes = 32;

pub const StagedOutput = struct {
    slot: ?Slot = null,

    const Slot = struct {
        connection_id: core.ConnectionId,
        creation_id: core.CreationId,
        seed: []u8,
    };

    /// Replaces the slot with an owned seed.
    pub fn stage(
        self: *StagedOutput,
        allocator: std.mem.Allocator,
        connection_id: core.ConnectionId,
        creation_id: core.CreationId,
        seed: []u8,
    ) void {
        self.deinit(allocator);
        self.slot = .{ .connection_id = connection_id, .creation_id = creation_id, .seed = seed };
    }

    /// Empties the slot for a creation on `connection_id`; the seed is returned only when it was
    /// staged for `creation_id`, otherwise it is freed.
    pub fn take(
        self: *StagedOutput,
        allocator: std.mem.Allocator,
        connection_id: core.ConnectionId,
        creation_id: core.CreationId,
    ) ?[]u8 {
        const slot = self.slot orelse return null;
        if (slot.connection_id != connection_id) return null;
        self.slot = null;
        if (slot.creation_id.eql(creation_id)) return slot.seed;
        allocator.free(slot.seed);
        return null;
    }

    pub fn deinit(self: *StagedOutput, allocator: std.mem.Allocator) void {
        if (self.slot) |slot| allocator.free(slot.seed);
        self.slot = null;
    }
};

pub fn dispatch(
    context: *Context,
    connection_id: core.ConnectionId,
    envelope: protocol.EnvelopeSummary,
    correlation_id: identifiers.IdentifierBytes,
    outgoing: *Outgoing,
) !void {
    const payload = envelope.payload;
    if (envelope.payload_encoding != @intFromEnum(protocol.PayloadEncoding.raw_bytes) or
        payload.len <= header_bytes or
        payload.len > header_bytes + engine.output_seed.max_bytes or
        !std.mem.eql(u8, payload[0..8], request_magic) or
        std.mem.allEqual(u8, payload[8..24], 0) or
        std.mem.readInt(u32, payload[24..28], .big) != payload.len - header_bytes or
        !std.mem.allEqual(u8, payload[28..32], 0) or
        !engine.output_seed.admit(payload[header_bytes..]))
    {
        return context.addFailure(outgoing, correlation_id, "invalid-field", "staged output is invalid", false);
    }
    var prepared = try response.prepare(outgoing, protocol.Limits.control_json_bytes, response.responseDelivery(correlation_id, context.hostEpoch()));
    defer prepared.deinit();
    const seed = context.allocator.dupe(u8, payload[header_bytes..]) catch {
        prepared.commitFailure(.{ .code = "resource-exhausted", .message = "Host could not retain staged output", .retryable = true });
        return;
    };
    const creation_id = core.CreationId.fromBytes(payload[8..24].*);
    context.runtime_aggregate.staged_output.stage(context.allocator, connection_id, creation_id, seed);
    const out = prepared.payloadBuffer()[0..header_bytes];
    @memcpy(out[0..8], response_magic);
    @memcpy(out[8..24], &creation_id.bytes);
    std.mem.writeInt(u32, out[24..28], @intCast(seed.len), .big);
    @memset(out[28..32], 0);
    _ = prepared.commitWrittenPayload(protocol.MessageKind.terminal_output_staged, .raw_bytes, out.len);
}
