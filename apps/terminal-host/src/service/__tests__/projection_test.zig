const std = @import("std");
const core_boundary = @import("terminal-host-core");
const engine_boundary = @import("terminal-host-engine");
const protocol = @import("terminal-protocol");
const service_boundary = @import("terminal-host-service");

const core_module = core_boundary.state;
const host_runtime_aggregate = service_boundary.testing.runtime_aggregate;
const subject = service_boundary.testing.projection;
const terminal_runtime = engine_boundary.runtime;

const MessageKind = protocol.MessageKind;
const host_outgoing = service_boundary.testing.outgoing;
const Outgoing = host_outgoing.Outgoing;

fn testIdentifier(namespace: u8, index: usize) [16]u8 {
    var result = [_]u8{0} ** 16;
    result[0] = namespace;
    std.mem.writeInt(u64, result[8..16], @intCast(index + 1), .little);
    return result;
}

fn testId(comptime Id: type, namespace: u8, index: usize) Id {
    return Id.fromBytes(testIdentifier(namespace, index));
}

test "a full dispatch batch preserves its committed projection prefix" {
    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x31, 0),
    );

    const first_terminal = testId(core_module.TerminalId, 0x32, 0);
    const second_terminal = testId(core_module.TerminalId, 0x32, 1);
    const first_attachment = testId(core_module.AttachmentId, 0x33, 0);
    const second_attachment = testId(core_module.AttachmentId, 0x33, 1);
    const terminal_ids = [_]core_module.TerminalId{ first_terminal, second_terminal };
    const attachment_ids = [_]core_module.AttachmentId{ first_attachment, second_attachment };
    for (terminal_ids, attachment_ids, 0..) |terminal_id, attachment_id, index| {
        var launch_digest = [_]u8{0} ** 32;
        launch_digest[0] = @intCast(index + 1);
        _ = try aggregate.core().createTerminal(
            connection_id,
            testId(core_module.CreationId, 0x34, index),
            terminal_id,
            launch_digest,
            80,
            24,
        );
        _ = try aggregate.core().attach(connection_id, attachment_id, terminal_id, .{
            .input = false,
            .read = true,
            .resize = false,
        }, false);
        _ = try aggregate.core().setProjectionDemand(
            connection_id,
            attachment_id,
            .active,
        );
    }

    const first_allocation = try std.testing.allocator.alloc(
        u8,
        host_outgoing.owned_payload_prefix_bytes + protocol.Limits.frame_bytes,
    );
    var first_encoded: terminal_runtime.ProjectionFrame = .{
        .allocation = first_allocation,
        .base_revision = null,
        .bytes = first_allocation[host_outgoing.owned_payload_prefix_bytes..],
        .generation = 1,
        .kind = .snapshot,
        .revision = 1,
        .sequence = 1,
    };
    @memset(first_encoded.bytes, 0);
    defer if (first_encoded.allocation.len != 0) {
        std.testing.allocator.free(first_encoded.allocation);
    };
    const second_allocation = try std.testing.allocator.alloc(
        u8,
        host_outgoing.owned_payload_prefix_bytes + protocol.Limits.frame_bytes,
    );
    var second_encoded: terminal_runtime.ProjectionFrame = .{
        .allocation = second_allocation,
        .base_revision = null,
        .bytes = second_allocation[host_outgoing.owned_payload_prefix_bytes..],
        .generation = 1,
        .kind = .snapshot,
        .revision = 1,
        .sequence = 1,
    };
    @memset(second_encoded.bytes, 0);
    defer if (second_encoded.allocation.len != 0) {
        std.testing.allocator.free(second_encoded.allocation);
    };
    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    const appendProjectionFrame = subject.testing.appendProjectionFrameFn;

    switch (try appendProjectionFrame(
        &aggregate,
        &outgoing,
        first_attachment,
        MessageKind.snapshot_frame,
        &first_encoded,
    )) {
        .appended => {},
        .batch_full => return error.ExpectedProjectionAppend,
    }
    switch (try appendProjectionFrame(
        &aggregate,
        &outgoing,
        second_attachment,
        MessageKind.snapshot_frame,
        &second_encoded,
    )) {
        .appended => return error.ExpectedFullBatch,
        .batch_full => {},
    }
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    try std.testing.expect(
        aggregate.core().findAttachment(first_attachment).?.projectionInFlight() != null,
    );
    try std.testing.expect(
        aggregate.core().findAttachment(second_attachment).?.projectionInFlight() == null,
    );
    try std.testing.expectEqual(
        @as(u64, 1),
        aggregate.core().findAttachment(second_attachment).?.nextProjectionSequence(),
    );
}

test "empty projection scheduler completes without producing a batch" {
    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    var scheduler: subject.Scheduler = .{};
    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();

    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.complete,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            1,
            protocol.Limits.frame_bytes,
            &outgoing,
        ),
    );
    try std.testing.expectEqual(@as(usize, 0), outgoing.records().len);
}
