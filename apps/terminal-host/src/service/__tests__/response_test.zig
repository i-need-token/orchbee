const std = @import("std");

const model = @import("terminal-host-model");
const protocol = @import("terminal-protocol");
const service_boundary = @import("terminal-host-service");

const host_outgoing = service_boundary.testing.outgoing;
const host_response = service_boundary.testing.response;

fn requestContract(kind: u16) protocol.RequestMessageContract {
    return protocol.messageContract(kind).?.request.?;
}

test "success response projection follows the registered Hello and ReadRows contracts" {
    const hello = try host_response.resolveSuccessResponse(
        requestContract(protocol.MessageKind.hello),
    );
    try std.testing.expectEqual(protocol.MessageKind.hello_accepted, hello.kind);
    try std.testing.expectEqual(protocol.PayloadEncoding.control_json, hello.encoding);

    const read_rows = try host_response.resolveSuccessResponse(
        requestContract(protocol.MessageKind.read_rows),
    );
    try std.testing.expectEqual(protocol.MessageKind.rows_snapshot, read_rows.kind);
    try std.testing.expectEqual(protocol.PayloadEncoding.row_window, read_rows.encoding);
}

test "success response projection rejects unknown and non-response kinds" {
    var request = requestContract(protocol.MessageKind.ping);
    request.success_kind = std.math.maxInt(u16);
    try std.testing.expectError(
        error.InvalidSuccessResponseContract,
        host_response.resolveSuccessResponse(request),
    );

    request.success_kind = protocol.MessageKind.terminal_started;
    try std.testing.expectError(
        error.InvalidSuccessResponseContract,
        host_response.resolveSuccessResponse(request),
    );
}

test "success response codec tokens reject control and row-window mismatches" {
    const control = try host_response.resolveSuccessResponse(
        requestContract(protocol.MessageKind.hello),
    );
    try std.testing.expectError(
        error.IncompatibleSuccessResponseEncoding,
        host_response.requireRowWindowSuccess(control),
    );

    const row_window = try host_response.resolveSuccessResponse(
        requestContract(protocol.MessageKind.read_rows),
    );
    try std.testing.expectError(
        error.IncompatibleSuccessResponseEncoding,
        host_response.requireControlJsonSuccess(row_window),
    );
}

test "success append and commit preserve registry-derived metadata" {
    const host_epoch = model.identifiers.HostEpoch.fromBytes([_]u8{0x21} ** 16);
    const correlation_id = [_]u8{0x22} ** 16;
    const delivery = host_response.responseDelivery(correlation_id, host_epoch);
    var outgoing = host_outgoing.Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();

    const ping = try host_response.requireControlJsonSuccess(
        try host_response.resolveSuccessResponse(requestContract(protocol.MessageKind.ping)),
    );
    _ = try host_response.appendControlSuccess(&outgoing, ping, "{}", delivery);

    const hello = try host_response.requireControlJsonSuccess(
        try host_response.resolveSuccessResponse(requestContract(protocol.MessageKind.hello)),
    );
    var control = try host_response.prepare(
        &outgoing,
        protocol.Limits.control_json_bytes,
        delivery,
    );
    defer control.deinit();
    var control_output = control.writer();
    try control_output.writeAll("{}");
    _ = control.commitControlSuccess(hello, &control_output);

    const read_rows = try host_response.requireRowWindowSuccess(
        try host_response.resolveSuccessResponse(requestContract(protocol.MessageKind.read_rows)),
    );
    var rows = try host_response.prepare(&outgoing, 4, delivery);
    defer rows.deinit();
    @memcpy(rows.payloadBuffer()[0..4], "rows");
    _ = rows.commitRowWindowSuccess(read_rows, 4);

    const expected = [_]host_response.SuccessResponse{
        try host_response.resolveSuccessResponse(requestContract(protocol.MessageKind.ping)),
        try host_response.resolveSuccessResponse(requestContract(protocol.MessageKind.hello)),
        try host_response.resolveSuccessResponse(requestContract(protocol.MessageKind.read_rows)),
    };
    try std.testing.expectEqual(expected.len, outgoing.records().len);
    for (outgoing.records(), expected) |record, metadata| {
        const envelope = try protocol.validateTransportEnvelope(record);
        try std.testing.expectEqual(metadata.kind, envelope.kind);
        try std.testing.expectEqual(@intFromEnum(metadata.encoding), envelope.payload_encoding);
    }
}
