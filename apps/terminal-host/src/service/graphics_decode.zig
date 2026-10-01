//! Read-only PNG decoding on a connection's existing readable attachment.
const std = @import("std");
const core = @import("terminal-host-core").state;
const engine = @import("terminal-host-engine");
const protocol = @import("terminal-protocol");
const identifiers = @import("terminal-host-model").identifiers;
const Context = @import("request_context.zig").Context;
const Outgoing = @import("outgoing.zig").Outgoing;
const response = @import("response.zig");

pub fn dispatch(
    context: *Context,
    connection_id: core.ConnectionId,
    envelope: protocol.EnvelopeSummary,
    correlation_id: identifiers.IdentifierBytes,
    outgoing: *Outgoing,
) !void {
    const payload = envelope.payload;
    if (envelope.payload_encoding != @intFromEnum(protocol.PayloadEncoding.graphics_image) or
        payload.len < 73 or payload.len > 40 + 16 * 1024 * 1024 or
        !std.mem.eql(u8, payload[0..8], "OBPI\x00\x01\x00\x00") or
        !std.mem.allEqual(u8, payload[36..40], 0) or
        std.mem.allEqual(u8, payload[8..24], 0))
    {
        return context.addFailure(outgoing, correlation_id, "invalid-image", "PNG decode request is invalid", false);
    }
    const width = std.mem.readInt(u32, payload[24..28], .big);
    const height = std.mem.readInt(u32, payload[28..32], .big);
    const length = std.mem.readInt(u32, payload[32..36], .big);
    if (width == 0 or height == 0 or width > 4096 or height > 4096 or
        length != payload.len - 40 or @as(u64, width) * height * 4 > 32 * 1024 * 1024)
    {
        return context.addFailure(outgoing, correlation_id, "invalid-image", "PNG dimensions or length exceed the decode budget", false);
    }
    const png = payload[40..];
    if (!std.mem.eql(u8, png[0..8], "\x89PNG\r\n\x1a\n") or
        std.mem.readInt(u32, png[8..12], .big) != 13 or
        !std.mem.eql(u8, png[12..16], "IHDR") or
        std.mem.readInt(u32, png[16..20], .big) != width or
        std.mem.readInt(u32, png[20..24], .big) != height)
    {
        return context.addFailure(outgoing, correlation_id, "invalid-image", "PNG header disagrees with the requested dimensions", false);
    }
    const attachment_id = core.AttachmentId.fromBytes(payload[8..24].*);
    const terminal_id = context.runtime_aggregate.graphicsDecodeTarget(connection_id, attachment_id) catch {
        return context.addFailure(outgoing, correlation_id, "missing-resource", "PNG decode attachment is not readable", false);
    };
    const rgba_length: usize = @intCast(@as(u64, width) * height * 4);
    // Reserve egress before decoding; only this transaction owns both allocations.
    var prepared = try response.prepare(outgoing, @max(56 + rgba_length, protocol.Limits.control_json_bytes), response.responseDelivery(correlation_id, context.hostEpoch()));
    defer prepared.deinit();
    const image = engine.decodeGraphicsPng(context.allocator, png) catch |err| {
        prepared.commitFailure(.{ .code = if (err == error.OutOfMemory) "resource-exhausted" else "invalid-image", .message = "PNG decoding failed", .retryable = err == error.OutOfMemory });
        return;
    };
    defer context.allocator.free(image.data);
    if (image.width != width or image.height != height or image.data.len != rgba_length) {
        prepared.commitFailure(.{ .code = "invalid-image", .message = "decoded PNG dimensions disagree", .retryable = false });
        return;
    }
    const out = prepared.payloadBuffer()[0 .. 56 + rgba_length];
    @memcpy(out[0..8], "OBPX\x00\x01\x00\x00");
    @memcpy(out[8..24], &attachment_id.bytes);
    @memcpy(out[24..40], &terminal_id.bytes);
    std.mem.writeInt(u32, out[40..44], width, .big);
    std.mem.writeInt(u32, out[44..48], height, .big);
    std.mem.writeInt(u32, out[48..52], @intCast(rgba_length), .big);
    @memset(out[52..56], 0);
    @memcpy(out[56..], image.data);
    _ = prepared.commitWrittenPayload(protocol.MessageKind.graphics_png_decoded, .graphics_image, out.len);
}
