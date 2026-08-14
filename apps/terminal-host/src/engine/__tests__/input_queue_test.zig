const std = @import("std");

const subject = @import("terminal-host-engine").testing.input_queue;
const Entry = subject.testing.EntryType;
const Queue = subject.Queue;

test "terminal replies preempt client slices without reordering either source" {
    var first_client = [_]u8{'a'};
    var reply = [_]u8{'r'};
    var second_client = [_]u8{'b'};
    var entries = [_]Entry{
        .{
            .bytes = first_client[0..],
            .identifier = 1,
            .reserved_bytes = first_client.len,
            .source = .client,
        },
        .{
            .bytes = reply[0..],
            .identifier = 2,
            .reserved_bytes = reply.len,
            .source = .terminal,
        },
        .{
            .bytes = second_client[0..],
            .identifier = 3,
            .reserved_bytes = second_client.len,
            .source = .client,
        },
    };

    try std.testing.expectEqual(
        @as(?usize, 1),
        subject.testing.selectWritableEntryFn(&entries, subject.terminal_reply_burst_slices),
    );

    entries[1].bytes = null;
    try std.testing.expectEqual(
        @as(?usize, null),
        subject.testing.selectWritableEntryFn(&entries, subject.terminal_reply_burst_slices),
    );

    entries[1].source = .client;
    try std.testing.expectEqual(
        @as(?usize, 0),
        subject.testing.selectWritableEntryFn(&entries, subject.terminal_reply_burst_slices),
    );

    entries[0].bytes = null;
    try std.testing.expectEqual(
        @as(?usize, null),
        subject.testing.selectWritableEntryFn(&entries, subject.terminal_reply_burst_slices),
    );
}

test "exhausted reply burst yields to a waiting client entry" {
    var client = [_]u8{'c'};
    var reply = [_]u8{'r'};
    const entries = [_]Entry{
        .{
            .bytes = client[0..],
            .identifier = 1,
            .reserved_bytes = client.len,
            .source = .client,
        },
        .{
            .bytes = reply[0..],
            .identifier = 2,
            .reserved_bytes = reply.len,
            .source = .terminal,
        },
    };

    try std.testing.expectEqual(
        @as(?usize, 0),
        subject.testing.selectWritableEntryFn(&entries, 0),
    );
}

test "reservation publication and writer accounting have one charge owner" {
    var queue = Queue.init(std.testing.allocator);
    defer queue.deinit();

    const reservation = try queue.reserve(3, .client, .{});
    try std.testing.expectEqual(@as(usize, 3), queue.pendingBytes());
    try std.testing.expectEqual(@as(usize, 1), queue.charge(.client).entries);

    const bytes = try std.testing.allocator.dupe(u8, "abc");
    try std.testing.expect(queue.publish(reservation, bytes));

    var buffer: [subject.writer_slice_bytes]u8 = undefined;
    const slice = queue.copyNextWriterSlice(&buffer);
    try std.testing.expectEqualStrings("abc", buffer[0..slice.length]);
    try std.testing.expect(queue.ownsWriterSlice(slice));
    try std.testing.expectEqual(
        subject.AccountOutcome.completed,
        queue.accountWriterSlice(slice, slice.length),
    );
    try std.testing.expectEqual(@as(usize, 0), queue.pendingBytes());
    try std.testing.expectEqual(@as(usize, 0), queue.charge(.client).entries);
}

test "cancelling an unpublished reservation releases its budget and FIFO slot" {
    var queue = Queue.init(std.testing.allocator);
    defer queue.deinit();

    const reservation = try queue.reserve(subject.client_capacity_bytes, .client, .{});
    try std.testing.expectError(
        error.InputBackpressure,
        queue.reserve(1, .client, .{}),
    );
    try std.testing.expect(queue.cancel(reservation.identifier));
    try std.testing.expectEqual(@as(usize, 0), queue.pendingBytes());

    const replacement = try queue.reserve(1, .client, .{});
    try std.testing.expect(replacement.identifier > reservation.identifier);
}
