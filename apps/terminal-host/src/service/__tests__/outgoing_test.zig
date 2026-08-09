const std = @import("std");

const model_boundary = @import("terminal-host-model");
const protocol = @import("terminal-protocol");
const subject = @import("terminal-host-service").testing.outgoing;

const HostEpoch = model_boundary.identifiers.HostEpoch;
const MessageKind = protocol.MessageKind;
const Outgoing = subject.Outgoing;

fn testEpoch(byte: u8) HostEpoch {
    return HostEpoch.fromBytes([_]u8{byte} ** 16);
}

fn expectAppended(outcome: subject.AppendOutcome) !subject.AppendToken {
    return switch (outcome) {
        .appended => |token| token,
        .full => error.ExpectedAppend,
    };
}

fn expectFull(outcome: subject.AppendOutcome) !void {
    switch (outcome) {
        .appended => return error.ExpectedFull,
        .full => {},
    }
}

fn expectReserved(outcome: subject.ReserveOutcome) !subject.RecordReservation {
    return switch (outcome) {
        .full => error.ExpectedReservation,
        .reserved => |reservation| reservation,
    };
}

fn expectPrepared(outcome: subject.AdoptionOutcome) !subject.PayloadAdoption {
    return switch (outcome) {
        .full => error.ExpectedAdoptionPreparation,
        .prepared => |prepared| prepared,
    };
}

test "Outgoing encodes valid delivery identities and rolls back only the matching append" {
    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    try std.testing.expectError(
        error.InvalidAppendToken,
        outgoing.rollback(.{
            .append_id = 0,
            .byte_count_before = 0,
            .previous_append_id = 0,
            .record_count_before = 0,
        }),
    );

    const correlation_id = [_]u8{0x33} ** 16;
    const host_epoch = testEpoch(0x44);
    const response_token = try expectAppended(try outgoing.append(
        MessageKind.ping,
        .control_json,
        "{}",
        .{ .response = .{
            .correlation_id = correlation_id,
            .host_epoch = host_epoch,
        } },
    ));
    const response_length = outgoing.records()[0].len;
    const response = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(MessageKind.ping, response.kind);
    try std.testing.expect(response.response);
    try std.testing.expect(!response.event);
    try std.testing.expectEqualSlices(u8, &correlation_id, &response.correlation_id.?);
    try std.testing.expectEqualSlices(u8, &host_epoch.bytes, &response.host_epoch.?);
    try std.testing.expectEqualStrings("{}", response.payload);

    const event_payload = "{\"terminalId\":\"11111111111111111111111111111111\"}";
    const event_token = try expectAppended(try outgoing.append(
        MessageKind.terminal_started,
        .control_json,
        event_payload,
        .{ .event = host_epoch },
    ));
    const event = try protocol.validateTransportEnvelope(outgoing.records()[1]);
    try std.testing.expectEqual(MessageKind.terminal_started, event.kind);
    try std.testing.expect(!event.response);
    try std.testing.expect(event.event);
    try std.testing.expect(event.correlation_id == null);
    try std.testing.expectEqualSlices(u8, &host_epoch.bytes, &event.host_epoch.?);
    try std.testing.expectEqual(@as(usize, 2), outgoing.records().len);

    try outgoing.rollback(event_token);
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    try std.testing.expectEqual(response_length, subject.testing.byteCount(&outgoing));

    const replacement_token = try expectAppended(try outgoing.append(
        MessageKind.terminal_started,
        .control_json,
        event_payload,
        .{ .event = host_epoch },
    ));
    try std.testing.expectError(
        error.InvalidAppendToken,
        outgoing.rollback(event_token),
    );
    try std.testing.expectEqual(@as(usize, 2), outgoing.records().len);
    try outgoing.rollback(replacement_token);
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    try outgoing.rollback(response_token);
    try std.testing.expectEqual(@as(usize, 0), outgoing.records().len);
}

test "Outgoing encodes identity-free handshake responses explicitly" {
    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();

    _ = try expectAppended(try outgoing.append(
        MessageKind.protocol_rejected,
        .control_json,
        "{}",
        .{ .handshake_response = .{
            .correlation_id = null,
            .host_epoch = null,
        } },
    ));
    const envelope = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expect(envelope.response);
    try std.testing.expect(!envelope.event);
    try std.testing.expect(envelope.correlation_id == null);
    try std.testing.expect(envelope.host_epoch == null);
}

test "Outgoing returns full without discarding an appendable prefix" {
    const maximum_frame = try std.testing.allocator.alloc(u8, protocol.Limits.frame_bytes);
    defer std.testing.allocator.free(maximum_frame);
    @memset(maximum_frame, 0);
    const exact_remainder_payload = try std.testing.allocator.alloc(
        u8,
        protocol.Limits.frame_bytes - 2 * protocol.transport_envelope_header_bytes,
    );
    defer std.testing.allocator.free(exact_remainder_payload);
    @memset(exact_remainder_payload, 0);

    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    const host_epoch = testEpoch(0x45);
    _ = try expectAppended(try outgoing.append(
        MessageKind.snapshot_frame,
        .terminal_frame,
        maximum_frame,
        .{ .event = host_epoch },
    ));
    try std.testing.expectEqual(
        protocol.Limits.frame_bytes - protocol.transport_envelope_header_bytes,
        subject.testing.byte_limit - subject.testing.byteCount(&outgoing),
    );

    try expectFull(try outgoing.append(
        MessageKind.snapshot_frame,
        .terminal_frame,
        maximum_frame,
        .{ .event = host_epoch },
    ));
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);

    const remainder_token = try expectAppended(try outgoing.append(
        MessageKind.snapshot_frame,
        .terminal_frame,
        exact_remainder_payload,
        .{ .event = host_epoch },
    ));
    try std.testing.expectEqual(subject.testing.byte_limit, subject.testing.byteCount(&outgoing));
    try std.testing.expectEqual(@as(usize, 2), outgoing.records().len);
    try expectFull(try outgoing.append(
        MessageKind.terminal_started,
        .control_json,
        "",
        .{ .event = host_epoch },
    ));
    try outgoing.rollback(remainder_token);
    _ = try expectAppended(try outgoing.append(
        MessageKind.terminal_started,
        .control_json,
        "",
        .{ .event = host_epoch },
    ));
}

test "Outgoing enforces the record limit through actual appends" {
    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    const host_epoch = testEpoch(0x46);

    for (0..subject.testing.record_limit) |_| {
        _ = try expectAppended(try outgoing.append(
            MessageKind.terminal_started,
            .control_json,
            "{}",
            .{ .event = host_epoch },
        ));
    }
    try std.testing.expectEqual(subject.testing.record_limit, outgoing.records().len);
    try expectFull(try outgoing.append(
        MessageKind.terminal_started,
        .control_json,
        "{}",
        .{ .event = host_epoch },
    ));
}

test "Outgoing reserves capacity before payload construction and commits without allocation" {
    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    const host_epoch = testEpoch(0x47);
    const correlation_id = [_]u8{0x37} ** 16;

    var reservation = try expectReserved(try outgoing.reserve(
        protocol.Limits.control_json_bytes,
        .{ .response = .{
            .correlation_id = correlation_id,
            .host_epoch = host_epoch,
        } },
    ));
    defer reservation.deinit();
    try std.testing.expectEqual(@as(usize, 0), outgoing.records().len);
    try std.testing.expectEqual(@as(usize, 1), subject.testing.reservedRecordCount(&outgoing));
    try std.testing.expect(subject.testing.reservedByteCount(&outgoing) > 0);

    var writer: std.Io.Writer = .fixed(reservation.payloadBuffer());
    try writer.writeAll("{\"prepared\":true}");
    const token = reservation.commitWritten(
        MessageKind.ping,
        .control_json,
        writer.buffered().len,
    );
    try std.testing.expectEqual(@as(usize, 0), subject.testing.reservedRecordCount(&outgoing));
    try std.testing.expectEqual(@as(usize, 0), subject.testing.reservedByteCount(&outgoing));
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const envelope = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqualStrings("{\"prepared\":true}", envelope.payload);
    try outgoing.rollback(token);
}

test "Outgoing adoption retains capacity promised to a pending reservation" {
    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    const host_epoch = testEpoch(0x48);

    for ([_][]const u8{ "{\"record\":1}", "{\"record\":2}", "{\"record\":3}" }) |payload| {
        _ = try expectAppended(try outgoing.append(
            MessageKind.terminal_started,
            .control_json,
            payload,
            .{ .event = host_epoch },
        ));
    }

    const correlation_id = [_]u8{0x38} ** 16;
    const reserved_payload = "reserved response";
    var reservation = try expectReserved(try outgoing.reserve(
        reserved_payload.len,
        .{ .response = .{
            .correlation_id = correlation_id,
            .host_epoch = host_epoch,
        } },
    ));
    defer reservation.deinit();

    const adopted_payload = "adopted projection";
    const adopted_allocation = try outgoing.ownedPayloadAllocator().alloc(
        u8,
        subject.owned_payload_prefix_bytes + adopted_payload.len,
    );
    var caller_owns_adopted_allocation = true;
    defer if (caller_owns_adopted_allocation) {
        outgoing.ownedPayloadAllocator().free(adopted_allocation);
    };
    const adopted_payload_storage = adopted_allocation[subject.owned_payload_prefix_bytes..];
    @memcpy(adopted_payload_storage, adopted_payload);

    _ = try expectAppended(try outgoing.adoptPayload(
        MessageKind.snapshot_frame,
        .terminal_frame,
        adopted_allocation,
        adopted_payload_storage,
        .{ .event = host_epoch },
    ));
    caller_owns_adopted_allocation = false;
    _ = reservation.commitPayload(
        MessageKind.ping,
        .control_json,
        reserved_payload,
    );

    try std.testing.expectEqual(@as(usize, 5), outgoing.records().len);
    try std.testing.expectEqual(@as(usize, 0), subject.testing.reservedRecordCount(&outgoing));
    try std.testing.expectEqual(@as(usize, 0), subject.testing.reservedByteCount(&outgoing));
    for (outgoing.records()) |record| {
        _ = try protocol.validateTransportEnvelope(record);
    }
    const adopted_envelope = try protocol.validateTransportEnvelope(outgoing.records()[3]);
    try std.testing.expectEqualStrings(adopted_payload, adopted_envelope.payload);
    try std.testing.expect(adopted_envelope.payload.ptr == adopted_payload_storage.ptr);
    try std.testing.expect(!caller_owns_adopted_allocation);
    const reserved_envelope = try protocol.validateTransportEnvelope(outgoing.records()[4]);
    try std.testing.expectEqualStrings(reserved_payload, reserved_envelope.payload);
}

test "Outgoing adoption allocation failure preserves caller ownership and reservations" {
    var failing = std.testing.FailingAllocator.init(std.testing.allocator, .{});
    var outgoing = Outgoing.init(failing.allocator());
    defer outgoing.deinit();
    const host_epoch = testEpoch(0x48);

    for (0..7) |_| {
        _ = try expectAppended(try outgoing.append(
            MessageKind.terminal_started,
            .control_json,
            "{}",
            .{ .event = host_epoch },
        ));
    }

    var reservation = try expectReserved(try outgoing.reserve(
        1,
        .{ .event = host_epoch },
    ));
    defer reservation.deinit();
    const reserved_bytes_before = subject.testing.reservedByteCount(&outgoing);
    const byte_count_before = subject.testing.byteCount(&outgoing);

    const payload = "caller owned";
    const allocation = try outgoing.ownedPayloadAllocator().alloc(
        u8,
        subject.owned_payload_prefix_bytes + payload.len,
    );
    var caller_owns_allocation = true;
    defer if (caller_owns_allocation) outgoing.ownedPayloadAllocator().free(allocation);
    const payload_storage = allocation[subject.owned_payload_prefix_bytes..];
    @memcpy(payload_storage, payload);

    failing.fail_index = failing.alloc_index;
    failing.resize_fail_index = failing.resize_index;
    const adoption_result = outgoing.adoptPayload(
        MessageKind.snapshot_frame,
        .terminal_frame,
        allocation,
        payload_storage,
        .{ .event = host_epoch },
    );
    if (adoption_result) |outcome| {
        switch (outcome) {
            .appended => caller_owns_allocation = false,
            .full => {},
        }
    } else |_| {}
    try std.testing.expectError(
        error.OutOfMemory,
        adoption_result,
    );

    try std.testing.expect(failing.has_induced_failure);
    try std.testing.expectEqualStrings(payload, payload_storage);
    try std.testing.expectEqual(@as(usize, 7), outgoing.records().len);
    try std.testing.expectEqual(byte_count_before, subject.testing.byteCount(&outgoing));
    try std.testing.expectEqual(@as(usize, 1), subject.testing.reservedRecordCount(&outgoing));
    try std.testing.expectEqual(reserved_bytes_before, subject.testing.reservedByteCount(&outgoing));
}

test "Outgoing cancellation releases reserved batch capacity" {
    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    const host_epoch = testEpoch(0x48);

    var reservation = try expectReserved(try outgoing.reserve(
        protocol.Limits.frame_bytes,
        .{ .event = host_epoch },
    ));
    try expectFull(try outgoing.append(
        MessageKind.snapshot_frame,
        .terminal_frame,
        reservation.payloadBuffer(),
        .{ .event = host_epoch },
    ));
    reservation.deinit();
    try std.testing.expectEqual(@as(usize, 0), subject.testing.reservedRecordCount(&outgoing));
    _ = try expectAppended(try outgoing.append(
        MessageKind.terminal_started,
        .control_json,
        "{}",
        .{ .event = host_epoch },
    ));
}

test "Outgoing adopts an envelope-ready payload allocation without copying" {
    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    const host_epoch = testEpoch(0x49);
    const payload = "owned projection payload";
    const allocation = try outgoing.ownedPayloadAllocator().alloc(
        u8,
        subject.owned_payload_prefix_bytes + payload.len,
    );
    var caller_owns_allocation = true;
    defer if (caller_owns_allocation) outgoing.ownedPayloadAllocator().free(allocation);
    const payload_storage = allocation[subject.owned_payload_prefix_bytes..];
    @memcpy(payload_storage, payload);

    const token = try expectAppended(try outgoing.adoptPayload(
        MessageKind.snapshot_frame,
        .terminal_frame,
        allocation,
        payload_storage,
        .{ .event = host_epoch },
    ));
    caller_owns_allocation = false;
    const envelope = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqualStrings(payload, envelope.payload);
    try std.testing.expect(envelope.payload.ptr == payload_storage.ptr);
    try outgoing.rollback(token);
}

test "Outgoing cancellation releases a provisional adoption and its exact capacity" {
    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    const host_epoch = testEpoch(0x4a);
    const payload = "provisional projection";
    const allocation = try outgoing.ownedPayloadAllocator().alloc(
        u8,
        subject.owned_payload_prefix_bytes + payload.len,
    );
    const payload_storage = allocation[subject.owned_payload_prefix_bytes..];
    @memcpy(payload_storage, payload);

    var adoption = try expectPrepared(try outgoing.prepareAdoptPayload(
        MessageKind.snapshot_frame,
        .terminal_frame,
        allocation,
        payload_storage,
        .{ .event = host_epoch },
    ));
    try std.testing.expectEqual(@as(usize, 0), outgoing.records().len);
    try std.testing.expectEqual(@as(usize, 1), subject.testing.reservedRecordCount(&outgoing));
    try std.testing.expectEqual(
        allocation.len,
        subject.testing.reservedByteCount(&outgoing),
    );

    adoption.deinit();
    try std.testing.expectEqual(@as(usize, 0), outgoing.records().len);
    try std.testing.expectEqual(@as(usize, 0), subject.testing.byteCount(&outgoing));
    try std.testing.expectEqual(@as(usize, 0), subject.testing.reservedRecordCount(&outgoing));
    try std.testing.expectEqual(@as(usize, 0), subject.testing.reservedByteCount(&outgoing));
}

test "Outgoing leaves no record or reservation when preparation allocation fails" {
    var failing = std.testing.FailingAllocator.init(
        std.testing.allocator,
        .{ .fail_index = 1 },
    );
    var outgoing = Outgoing.init(failing.allocator());
    defer outgoing.deinit();
    const host_epoch = testEpoch(0x47);

    try std.testing.expectError(
        error.OutOfMemory,
        outgoing.append(
            MessageKind.terminal_started,
            .control_json,
            "{}",
            .{ .event = host_epoch },
        ),
    );
    try std.testing.expectEqual(@as(usize, 0), outgoing.records().len);
    try std.testing.expectEqual(@as(usize, 0), subject.testing.byteCount(&outgoing));
    try std.testing.expectEqual(@as(usize, 0), subject.testing.reservedRecordCount(&outgoing));
}
