const harness = @import("interaction_harness.zig");
const service_harness = @import("service_harness.zig");
const std = harness.std;
const core = harness.core_module;
const protocol = harness.protocol;
const HostService = harness.service_subject.HostService;
const Outgoing = harness.host_outgoing.Outgoing;

fn readable(service: *HostService, connection_id: core.ConnectionId, attachment_id: core.AttachmentId) !harness.TestRuntime {
    if (harness.target_builtin.os.tag != .windows) return harness.createReadableRuntime(&service.runtime_aggregate, connection_id, attachment_id);
    const executable = try @import("terminal-host-platform").process.environmentVariableOwned(std.testing.allocator, "ComSpec");
    defer std.testing.allocator.free(executable);
    var launch = harness.testLaunch(&.{ "/D", "/Q", "/C", "set /P ORCHBEE_GRAPHICS_TEST=" });
    launch.executable = executable;
    const validated = try harness.terminal_launch.inspect(std.testing.allocator, launch);
    _ = try service.runtime_aggregate.core().connect(connection_id, harness.testId(core.ClientId, 0x30));
    var preparation = try service.runtime_aggregate.prepareCreateTerminal(connection_id, harness.testId(core.CreationId, 0x31), validated);
    defer preparation.deinit();
    const created = try service.runtime_aggregate.commitPreparedTerminal(&preparation);
    const terminal_id = created.identity().terminal_id;
    _ = try service.runtime_aggregate.attach(connection_id, attachment_id, terminal_id, .{ .input = false, .read = true, .resize = false }, false);
    return .{ .runtime = service.runtime_aggregate.runtimeFor(terminal_id).?, .terminal_id = terminal_id };
}
fn requestPayload(attachment_id: core.AttachmentId) ![]u8 {
    const encoded = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";
    const decoder = std.base64.standard.Decoder;
    const length = try decoder.calcSizeForSlice(encoded);
    const out = try std.testing.allocator.alloc(u8, 40 + length);
    errdefer std.testing.allocator.free(out);
    @memcpy(out[0..8], "OBPI\x00\x01\x00\x00");
    @memcpy(out[8..24], &attachment_id.bytes);
    std.mem.writeInt(u32, out[24..28], 1, .big);
    std.mem.writeInt(u32, out[28..32], 1, .big);
    std.mem.writeInt(u32, out[32..36], @intCast(length), .big);
    @memset(out[36..40], 0);
    try decoder.decode(out[40..], encoded);
    return out;
}
fn dispatch(service: *HostService, connection_id: core.ConnectionId, payload: []const u8, outgoing: *Outgoing) !void {
    const correlation = [_]u8{0x51} ** 16;
    const record = try protocol.encodeTransportEnvelope(std.testing.allocator, protocol.MessageKind.decode_graphics_png, .graphics_image, payload, correlation, service.runtime_aggregate.hostEpoch().bytes, false, false);
    defer std.testing.allocator.free(record);
    var dispatcher = service_harness.RequestDispatcher.init(service.allocator, &service.runtime_aggregate, false);
    try dispatcher.dispatch(protocol.messageContract(protocol.MessageKind.decode_graphics_png).?.request.?, connection_id, .{ .input_bytes = 1024, .row_window_bytes = 4096 }, try protocol.validateTransportEnvelope(record), correlation, outgoing);
}
test "graphics PNG decode is attachment scoped, correlated and leaves terminal text unchanged" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    const connection_id = 11;
    const attachment_id = harness.testId(core.AttachmentId, 0x12);
    const target = try readable(&service, connection_id, attachment_id);
    try std.testing.expect(harness.commitReaderBatch(target.runtime, "retain terminal text") == .committed);
    const payload = try requestPayload(attachment_id);
    defer std.testing.allocator.free(payload);
    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    try dispatch(&service, connection_id, payload, &outgoing);
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const envelope = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(protocol.MessageKind.graphics_png_decoded, envelope.kind);
    try std.testing.expectEqual(@as(?[16]u8, [_]u8{0x51} ** 16), envelope.correlation_id);
    try std.testing.expectEqualSlices(u8, &attachment_id.bytes, envelope.payload[8..24]);
    try std.testing.expectEqualSlices(u8, &target.terminal_id.bytes, envelope.payload[24..40]);
    try std.testing.expectEqualSlices(u8, &.{ 255, 0, 0, 255 }, envelope.payload[56..]);
    const text = try target.runtime.plainString(std.testing.allocator);
    defer std.testing.allocator.free(text);
    try std.testing.expect(std.mem.indexOf(u8, text, "retain terminal text") != null);
}
test "graphics PNG decode refuses a foreign viewer and malformed image without a pixel receipt" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    const connection_id = 21;
    const attachment_id = harness.testId(core.AttachmentId, 0x22);
    _ = try readable(&service, connection_id, attachment_id);
    const payload = try requestPayload(attachment_id);
    defer std.testing.allocator.free(payload);
    {
        var outgoing = Outgoing.init(std.testing.allocator);
        defer outgoing.deinit();
        try dispatch(&service, 23, payload, &outgoing);
        try service_harness.expectSingleFailureCode(&outgoing, "missing-resource");
    }
    // Keep the dimension header valid while corrupting the compressed image body.
    payload[85] ^= 0x5a;
    {
        var outgoing = Outgoing.init(std.testing.allocator);
        defer outgoing.deinit();
        try dispatch(&service, connection_id, payload, &outgoing);
        try service_harness.expectSingleFailureCode(&outgoing, "invalid-image");
    }
}
