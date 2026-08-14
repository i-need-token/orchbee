const harness = @import("core_harness.zig");
const std = harness.std;
const model_boundary = harness.model_boundary;
const subject = harness.subject;
const terminal_configuration = harness.terminal_configuration;
const terminal_geometry = harness.terminal_geometry;
const Capabilities = harness.Capabilities;
const AttachmentId = harness.AttachmentId;
const ClientId = harness.ClientId;
const ConnectionId = harness.ConnectionId;
const CreationId = harness.CreationId;
const HostCore = harness.HostCore;
const HostEpoch = harness.HostEpoch;
const ProjectionBlock = harness.ProjectionBlock;
const ProjectionDemand = harness.ProjectionDemand;
const TerminalId = harness.TerminalId;
const TerminalPhase = harness.TerminalPhase;
const createModel = harness.createModel;
const testId = harness.testId;
const pendingOutcomeToken = harness.pendingOutcomeToken;
const publishPendingOutcome = harness.publishPendingOutcome;
const expectCoreInvariants = harness.expectCoreInvariants;
const ExpectedAttachment = harness.ExpectedAttachment;
const initAttachmentRollbackCore = harness.initAttachmentRollbackCore;
const addExpectedAttachments = harness.addExpectedAttachments;
const expectAttachmentSnapshot = harness.expectAttachmentSnapshot;
const expectAttachOutOfMemoryPreservesSnapshot = harness.expectAttachOutOfMemoryPreservesSnapshot;
const expectActiveDemandAggregateExact = harness.expectActiveDemandAggregateExact;

test "projection demand gates one in-flight frame and coalesces later revisions" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try createModel(&core, connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.synchronizeRevision(terminal_id, 1);

    try std.testing.expectError(
        error.InvalidPhase,
        core.commitFrame(attachment_id, null, 1, 1, .snapshot),
    );
    try std.testing.expectEqual(
        .applied,
        try core.setProjectionDemand(connection, attachment_id, .active),
    );
    const first = try core.commitFrame(attachment_id, null, 1, 1, .snapshot);
    try std.testing.expectEqual(@as(u64, 1), first.sequence);
    try std.testing.expectError(
        error.InvalidPhase,
        core.commitFrame(attachment_id, null, 1, 1, .snapshot),
    );

    _ = try core.synchronizeRevision(terminal_id, 9);
    try std.testing.expect(core.findAttachment(attachment_id).?.projectionInFlight().?.dirty_after_capture);
    try std.testing.expectError(
        error.InvalidFrameAcknowledgement,
        core.frameApplied(connection, attachment_id, 1, 8, 1),
    );
    try std.testing.expectEqual(
        .applied,
        try core.frameApplied(connection, attachment_id, 1, 1, 1),
    );
    try std.testing.expectEqual(
        .idempotent,
        try core.frameApplied(connection, attachment_id, 1, 1, 1),
    );
    const second = try core.commitFrame(attachment_id, 1, 9, 1, .delta);
    try std.testing.expectEqual(@as(u64, 2), second.sequence);
    _ = try core.frameApplied(connection, attachment_id, 2, 9, 1);
    _ = try core.setProjectionDemand(connection, attachment_id, .paused);
    _ = try core.setProjectionDemand(connection, attachment_id, .active);
    const resumed = core.findAttachment(attachment_id).?;
    try std.testing.expect(resumed.projectionReady().?.forceSnapshot());
    try std.testing.expect(resumed.projectionReady().?.base() == null);
    try std.testing.expectError(
        error.InvalidPhase,
        core.commitFrame(attachment_id, 9, 9, 1, .delta),
    );
    _ = try core.commitFrame(attachment_id, null, 9, 1, .snapshot);
}
test "projection batch mutations can roll back before transport handoff" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try createModel(
        &core,
        connection,
        testId(CreationId, 3),
        terminal_id,
        [_]u8{5} ** 32,
        80,
        24,
    );
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.setProjectionDemand(connection, attachment_id, .active);

    const frame_commit = try core.commitFrameTransaction(
        attachment_id,
        null,
        1,
        1,
        .snapshot,
    );
    try std.testing.expect(core.findAttachment(attachment_id).?.projectionInFlight() != null);
    try core.rollbackFrameCommit(frame_commit);
    const rolled_back_frame = core.findAttachment(attachment_id).?;
    try std.testing.expect(rolled_back_frame.projectionInFlight() == null);
    try std.testing.expect(rolled_back_frame.projectionReady().?.forceSnapshot());
    try std.testing.expect(rolled_back_frame.projectionReady().?.base() == null);
    try std.testing.expectEqual(@as(u64, 1), rolled_back_frame.nextProjectionSequence());

    const block_commit = try core.blockProjectionTransaction(
        attachment_id,
        .capture_allocation_failed,
        0,
    );
    try std.testing.expectEqual(
        ProjectionBlock.capture_allocation_failed,
        core.findAttachment(attachment_id).?.projectionBlocked().?.reason,
    );
    try core.rollbackProjectionBlock(block_commit);
    const rolled_back_block = core.findAttachment(attachment_id).?;
    try std.testing.expect(rolled_back_block.projectionBlocked() == null);
    try std.testing.expect(rolled_back_block.projectionReady().?.forceSnapshot());
}
test "projection block records the attempted revision and refreshes from a newer observation" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try createModel(
        &core,
        connection,
        testId(CreationId, 3),
        terminal_id,
        [_]u8{5} ** 32,
        80,
        24,
    );
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.synchronizeRevision(terminal_id, 4);
    _ = try core.setProjectionDemand(connection, attachment_id, .active);
    const flight = try core.commitFrame(attachment_id, null, 4, 1, .snapshot);
    _ = try core.frameApplied(
        connection,
        attachment_id,
        flight.sequence,
        flight.revision,
        flight.generation,
    );
    _ = try core.synchronizeRevision(terminal_id, 6);

    const ready_before = core.findAttachment(attachment_id).?.projectionReady().?;
    const sequence_before = core.findAttachment(attachment_id).?.nextProjectionSequence();
    try std.testing.expectError(
        error.InvalidPhase,
        core.blockProjectionTransaction(
            attachment_id,
            .capture_allocation_failed,
            7,
        ),
    );
    const after_invalid = core.findAttachment(attachment_id).?;
    try std.testing.expectEqualDeep(ready_before, after_invalid.projectionReady().?);
    try std.testing.expectEqual(sequence_before, after_invalid.nextProjectionSequence());

    const block_commit = try core.blockProjectionTransaction(
        attachment_id,
        .capture_allocation_failed,
        5,
    );
    try std.testing.expectEqual(@as(u64, 5), block_commit.blocked.observed_revision);
    try std.testing.expectEqualDeep(ready_before.base(), block_commit.blocked.base);
    try core.rollbackProjectionBlock(block_commit);
    const rolled_back = core.findAttachment(attachment_id).?;
    try std.testing.expectEqualDeep(ready_before, rolled_back.projectionReady().?);
    try std.testing.expectEqual(sequence_before, rolled_back.nextProjectionSequence());

    const committed_block = try core.blockProjectionTransaction(
        attachment_id,
        .projection_too_large,
        5,
    );
    try std.testing.expectEqual(@as(u64, 5), committed_block.blocked.observed_revision);
    const refreshed = core.findAttachment(attachment_id).?;
    try std.testing.expect(refreshed.projectionBlocked() == null);
    try std.testing.expect(refreshed.projectionReady().?.forceSnapshot());
    try std.testing.expectEqualDeep(
        ready_before.base(),
        refreshed.projectionReady().?.base(),
    );
    try std.testing.expectEqual(sequence_before, refreshed.nextProjectionSequence());
}
test "maximum frame sequence can never be acknowledged as a phantom retry" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try createModel(&core, connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.synchronizeRevision(terminal_id, 1);
    _ = try core.setProjectionDemand(connection, attachment_id, .active);
    subject.testing.setProjectionState(&core, attachment_id, .{
        .delivery = .{ .ready = .{ .delta = .{
            .generation = 1,
            .revision = 1,
            .sequence = 1,
        } } },
        .next_sequence = std.math.maxInt(u64),
    });

    try std.testing.expectError(
        error.InvalidFrameAcknowledgement,
        core.frameApplied(
            connection,
            attachment_id,
            std.math.maxInt(u64),
            1,
            1,
        ),
    );
    try std.testing.expectError(
        error.SequenceOverflow,
        core.commitFrame(attachment_id, 1, 1, 1, .delta),
    );
    const retained_attachment = core.findAttachment(attachment_id).?;
    try std.testing.expectEqual(
        std.math.maxInt(u64),
        retained_attachment.nextProjectionSequence(),
    );
    try std.testing.expect(retained_attachment.projectionInFlight() == null);
}
test "pause abandons delivery state and keeps hidden revisions attachment lazy" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try createModel(&core, connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);

    _ = try core.synchronizeRevision(terminal_id, 1);
    try std.testing.expectEqual(
        ProjectionDemand.paused,
        core.findAttachment(attachment_id).?.projectionDemand(),
    );
    _ = try core.setProjectionDemand(connection, attachment_id, .active);
    _ = try core.commitFrame(attachment_id, null, 1, 1, .snapshot);
    _ = try core.synchronizeRevision(terminal_id, 2);
    _ = try core.setProjectionDemand(connection, attachment_id, .paused);

    const paused = core.findAttachment(attachment_id).?;
    try std.testing.expectEqual(ProjectionDemand.paused, paused.projectionDemand());
    try std.testing.expectEqual(@as(u64, 1), paused.projection.last_abandoned_sequence);
    try std.testing.expect(!core.hasActiveProjectionDemand(terminal_id));

    _ = try core.synchronizeRevision(terminal_id, 3);
    try std.testing.expectEqual(
        ProjectionDemand.paused,
        core.findAttachment(attachment_id).?.projectionDemand(),
    );
    try std.testing.expectEqual(
        .idempotent,
        try core.frameApplied(connection, attachment_id, 1, 1, 1),
    );
    try std.testing.expectEqual(
        .idempotent,
        try core.needSnapshot(connection, attachment_id, 1),
    );

    _ = try core.setProjectionDemand(connection, attachment_id, .active);
    const resumed = try core.commitFrame(attachment_id, null, 3, 1, .snapshot);
    try std.testing.expectEqual(@as(u64, 2), resumed.sequence);
    try std.testing.expectEqual(
        .idempotent,
        try core.frameApplied(connection, attachment_id, 1, 1, 1),
    );
    try std.testing.expectEqual(
        .idempotent,
        try core.needSnapshot(connection, attachment_id, 1),
    );
    try std.testing.expectEqual(
        @as(u64, 2),
        core.findAttachment(attachment_id).?.projectionInFlight().?.frame.sequence,
    );
}
test "active projection demand is aggregated across attachments" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const first_attachment = testId(AttachmentId, 6);
    const second_attachment = testId(AttachmentId, 7);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try createModel(&core, connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
    for ([_]AttachmentId{ first_attachment, second_attachment }) |attachment_id| {
        _ = try core.attach(connection, attachment_id, terminal_id, .{
            .input = false,
            .read = true,
            .resize = false,
        }, false);
        _ = try core.setProjectionDemand(connection, attachment_id, .active);
    }

    try std.testing.expect(core.hasActiveProjectionDemand(terminal_id));
    _ = try core.setProjectionDemand(connection, first_attachment, .paused);
    try std.testing.expect(core.hasActiveProjectionDemand(terminal_id));
    _ = try core.detach(connection, second_attachment);
    try std.testing.expect(!core.hasActiveProjectionDemand(terminal_id));
}
test "active-demand aggregate stays exact across demand transitions and swap removals" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 0x90));
    defer core.deinit();
    const connection: ConnectionId = 2;
    _ = try core.connect(connection, testId(ClientId, 0x91));

    const terminal_a = testId(TerminalId, 0x92);
    const terminal_b = testId(TerminalId, 0x93);
    const terminal_c = testId(TerminalId, 0x94);
    for ([_]TerminalId{ terminal_a, terminal_b, terminal_c }, 0..) |terminal_id, index| {
        _ = try createModel(
            &core,
            connection,
            testId(CreationId, @intCast(0x95 + index)),
            terminal_id,
            [_]u8{@intCast(0x95 + index)} ** 32,
            80,
            24,
        );
    }

    const read_a = testId(AttachmentId, 0xa0);
    const read_b = testId(AttachmentId, 0xa1);
    const read_c = testId(AttachmentId, 0xa2);
    const input_b = testId(AttachmentId, 0xa3);
    const read_d = testId(AttachmentId, 0xa4);
    const resize_only = testId(AttachmentId, 0xa5);
    // terminal_a: read_a, read_b; terminal_b: read_c, input_b, read_d;
    // terminal_c: resize-only lease (never read-capable).
    _ = try core.attach(connection, read_a, terminal_a, .{ .input = false, .read = true, .resize = false }, false);
    _ = try core.attach(connection, read_b, terminal_a, .{ .input = false, .read = true, .resize = false }, false);
    _ = try core.attach(connection, read_c, terminal_b, .{ .input = false, .read = true, .resize = false }, false);
    _ = try core.attach(connection, input_b, terminal_b, .{ .input = true, .read = false, .resize = false }, false);
    _ = try core.attach(connection, read_d, terminal_b, .{ .input = false, .read = true, .resize = false }, false);
    _ = try core.attach(connection, resize_only, terminal_c, .{ .input = false, .read = false, .resize = true }, true);
    try expectActiveDemandAggregateExact(&core);
    try std.testing.expect(!core.hasActiveProjectionDemand(terminal_a));
    try std.testing.expect(!core.hasActiveProjectionDemand(terminal_b));
    try std.testing.expect(!core.hasActiveProjectionDemand(terminal_c));

    // Activate a paused read on each terminal; the aggregate follows.
    _ = try core.setProjectionDemand(connection, read_a, .active);
    try std.testing.expect(core.hasActiveProjectionDemand(terminal_a));
    try expectActiveDemandAggregateExact(&core);
    _ = try core.setProjectionDemand(connection, read_d, .active);
    try std.testing.expect(core.hasActiveProjectionDemand(terminal_b));
    try expectActiveDemandAggregateExact(&core);

    // Pause one active read while another remains active.
    _ = try core.setProjectionDemand(connection, read_d, .paused);
    try std.testing.expect(core.hasActiveProjectionDemand(terminal_a));
    try std.testing.expect(!core.hasActiveProjectionDemand(terminal_b));
    try expectActiveDemandAggregateExact(&core);

    // Swap-remove a non-last paused read from terminal_a (list [read_a, read_b];
    // removing read_a swaps read_b into its slot).
    _ = try core.detach(connection, read_a);
    try std.testing.expect(!core.hasActiveProjectionDemand(terminal_a));
    try expectActiveDemandAggregateExact(&core);

    // Swap-remove a non-last input-only from terminal_b (list [read_c, input_b,
    // read_d]; removing input_b swaps read_d into its slot).
    _ = try core.detach(connection, input_b);
    try std.testing.expect(!core.hasActiveProjectionDemand(terminal_b));
    try expectActiveDemandAggregateExact(&core);

    // Activate read_c, then final-remove the last active-read attachment.
    _ = try core.setProjectionDemand(connection, read_c, .active);
    try std.testing.expect(core.hasActiveProjectionDemand(terminal_b));
    try expectActiveDemandAggregateExact(&core);
    _ = try core.detach(connection, read_c);
    try std.testing.expect(!core.hasActiveProjectionDemand(terminal_b));
    try expectActiveDemandAggregateExact(&core);

    // A replacement attachment on the same terminal re-enters cleanly.
    const replacement = testId(AttachmentId, 0xa6);
    _ = try core.attach(connection, replacement, terminal_b, .{ .input = false, .read = true, .resize = false }, false);
    try std.testing.expect(!core.hasActiveProjectionDemand(terminal_b));
    try expectActiveDemandAggregateExact(&core);
    _ = try core.setProjectionDemand(connection, replacement, .active);
    try std.testing.expect(core.hasActiveProjectionDemand(terminal_b));
    try expectActiveDemandAggregateExact(&core);

    // Disconnect drains every terminal aggregate to zero while retaining them.
    _ = core.disconnect(connection);
    for ([_]TerminalId{ terminal_a, terminal_b, terminal_c }) |terminal_id| {
        try std.testing.expect(!core.hasActiveProjectionDemand(terminal_id));
        try std.testing.expect(core.findTerminal(terminal_id) != null);
    }
    try expectActiveDemandAggregateExact(&core);
}
test "recovery is exact and blocked projection requires explicit pause resume" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try createModel(&core, connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.synchronizeRevision(terminal_id, 3);
    _ = try core.setProjectionDemand(connection, attachment_id, .active);
    _ = try core.commitFrame(attachment_id, null, 3, 1, .snapshot);

    try std.testing.expectError(
        error.InvalidFrameAcknowledgement,
        core.needSnapshot(connection, attachment_id, 2),
    );
    try std.testing.expectEqual(.applied, try core.needSnapshot(connection, attachment_id, 1));
    try std.testing.expectEqual(.idempotent, try core.needSnapshot(connection, attachment_id, 1));
    try std.testing.expect(core.findAttachment(attachment_id).?.projectionReady().?.forceSnapshot());
    _ = try core.commitFrame(attachment_id, null, 3, 1, .snapshot);
    _ = try core.frameApplied(connection, attachment_id, 2, 3, 1);

    try core.blockProjection(attachment_id, .capture_allocation_failed, 3);
    try std.testing.expectError(
        error.InvalidPhase,
        core.commitFrame(attachment_id, null, 3, 1, .snapshot),
    );
    try std.testing.expectEqual(
        .idempotent,
        try core.setProjectionDemand(connection, attachment_id, .active),
    );
    try std.testing.expect(core.findAttachment(attachment_id).?.projectionBlocked() != null);
    _ = try core.setProjectionDemand(connection, attachment_id, .paused);
    _ = try core.setProjectionDemand(connection, attachment_id, .active);
    const attachment = core.findAttachment(attachment_id).?;
    try std.testing.expect(attachment.projectionBlocked() == null);
    try std.testing.expect(attachment.projectionReady().?.forceSnapshot());
    try std.testing.expect(attachment.projectionReady().?.base() == null);
}
test "an initial projection block remains stable until a newer terminal revision" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try createModel(
        &core,
        connection,
        testId(CreationId, 3),
        terminal_id,
        [_]u8{5} ** 32,
        80,
        24,
    );
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.synchronizeRevision(terminal_id, 1);
    _ = try core.setProjectionDemand(connection, attachment_id, .active);

    try core.blockProjection(attachment_id, .projection_too_large, 1);
    const first_observation = core.findAttachment(attachment_id).?.projectionBlocked().?;
    const second_observation = core.findAttachment(attachment_id).?.projectionBlocked().?;
    try std.testing.expectEqual(@as(u64, 1), first_observation.observed_revision);
    try std.testing.expectEqual(first_observation.observed_revision, second_observation.observed_revision);
    try std.testing.expect(first_observation.base == null);

    _ = try core.synchronizeRevision(terminal_id, 2);
    const invalidated = core.findAttachment(attachment_id).?;
    try std.testing.expect(invalidated.projectionBlocked() == null);
    try std.testing.expect(invalidated.projectionReady().?.forceSnapshot());
    try std.testing.expect(invalidated.projectionReady().?.base() == null);
}
test "broker attachments advance independently and geometry invalidates every delta base" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const first_client = testId(ClientId, 2);
    const first_connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const first_attachment = testId(AttachmentId, 6);
    const second_attachment = testId(AttachmentId, 8);
    _ = try core.connect(first_connection, first_client);
    _ = try createModel(&core, first_connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.attach(first_connection, first_attachment, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.attach(first_connection, second_attachment, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.synchronizeRevision(terminal_id, 4);
    _ = try core.setProjectionDemand(first_connection, first_attachment, .active);
    _ = try core.setProjectionDemand(first_connection, second_attachment, .active);
    _ = try core.commitFrame(first_attachment, null, 4, 1, .snapshot);
    _ = try core.commitFrame(second_attachment, null, 4, 1, .snapshot);
    _ = try core.frameApplied(first_connection, first_attachment, 1, 4, 1);

    _ = try core.synchronizeRevision(terminal_id, 5);
    try std.testing.expectEqual(
        @as(u64, 4),
        core.findAttachment(first_attachment).?.projectionReady().?.base().?.revision,
    );
    try std.testing.expect(
        core.findAttachment(second_attachment).?.projectionInFlight().?.dirty_after_capture,
    );
    const first_delta = try core.commitFrame(first_attachment, 4, 5, 1, .delta);
    try std.testing.expectEqual(@as(u64, 5), first_delta.revision);
    try std.testing.expectError(
        error.InvalidPhase,
        core.commitFrame(second_attachment, 4, 5, 1, .delta),
    );

    try std.testing.expectError(
        error.InvalidPhase,
        core.synchronizeGeometry(terminal_id, 120, 40, 10, 20, 5),
    );
    try core.synchronizeGeometry(terminal_id, 120, 40, 10, 20, 6);
    try std.testing.expect(
        core.findAttachment(first_attachment).?.projectionInFlight().?.dirty_after_capture,
    );
    try std.testing.expect(
        core.findAttachment(second_attachment).?.projectionInFlight().?.dirty_after_capture,
    );

    _ = try core.frameApplied(first_connection, first_attachment, 2, 5, 1);
    try std.testing.expect(core.findAttachment(first_attachment).?.projectionReady().?.forceSnapshot());
    const replacement = try core.commitFrame(first_attachment, null, 6, 1, .snapshot);
    try std.testing.expect(core.findAttachment(first_attachment).?.projectionInFlight() != null);
    _ = try core.frameApplied(
        first_connection,
        first_attachment,
        replacement.sequence,
        replacement.revision,
        replacement.generation,
    );
    const acknowledged = core.findAttachment(first_attachment).?;
    try std.testing.expect(!acknowledged.projectionReady().?.forceSnapshot());
    try std.testing.expectEqual(@as(u64, 6), acknowledged.projectionReady().?.base().?.revision);
}
test "projection transition sequences preserve core invariants after every action" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try createModel(
        &core,
        connection,
        testId(CreationId, 3),
        terminal_id,
        [_]u8{5} ** 32,
        80,
        24,
    );
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);

    for (0..512) |step| {
        const attachment = core.findAttachment(attachment_id).?;
        switch (attachment.projection.delivery) {
            .paused => _ = try core.setProjectionDemand(connection, attachment_id, .active),
            .ready => |ready| {
                if (step % 7 == 0) {
                    try core.blockProjection(
                        attachment_id,
                        .capture_allocation_failed,
                        core.findTerminal(terminal_id).?.observedRevision(),
                    );
                } else if (step % 11 == 0) {
                    _ = try core.setProjectionDemand(connection, attachment_id, .paused);
                } else {
                    const revision = core.findTerminal(terminal_id).?.observedRevision();
                    switch (ready) {
                        .snapshot => _ = try core.commitFrame(
                            attachment_id,
                            null,
                            revision,
                            1,
                            .snapshot,
                        ),
                        .delta => |base| _ = try core.commitFrame(
                            attachment_id,
                            base.revision,
                            revision,
                            base.generation,
                            .delta,
                        ),
                    }
                }
            },
            .in_flight => |flight| {
                if (step % 5 == 0) {
                    _ = try core.synchronizeRevision(
                        terminal_id,
                        core.findTerminal(terminal_id).?.observedRevision() + 1,
                    );
                    _ = core.findAttachment(attachment_id);
                } else if (step % 5 == 1) {
                    _ = try core.needSnapshot(
                        connection,
                        attachment_id,
                        flight.frame.sequence,
                    );
                } else if (step % 5 == 2) {
                    // THCR-029 replay interleaving: while a newer frame is in
                    // flight, an acknowledgement of the retained prior base must
                    // be an idempotent no-op that never clears the newer flight.
                    if (flight.base_before) |base| {
                        try std.testing.expectEqual(
                            subject.TransitionOutcome.idempotent,
                            try core.frameApplied(
                                connection,
                                attachment_id,
                                base.sequence,
                                base.revision,
                                base.generation,
                            ),
                        );
                        try std.testing.expectEqual(
                            flight.frame.sequence,
                            core.findAttachment(attachment_id).?.projectionInFlight().?.frame.sequence,
                        );
                    }
                } else {
                    _ = try core.frameApplied(
                        connection,
                        attachment_id,
                        flight.frame.sequence,
                        flight.frame.revision,
                        flight.frame.generation,
                    );
                }
            },
            .blocked => |blocked| {
                if (step % 2 == 0) {
                    _ = try core.synchronizeRevision(
                        terminal_id,
                        blocked.observed_revision + 1,
                    );
                    _ = core.findAttachment(attachment_id);
                } else {
                    _ = try core.setProjectionDemand(connection, attachment_id, .paused);
                }
            },
        }
        try expectCoreInvariants(&core);
    }
}
test "THCR-029 replaying an earlier frame acknowledgement is idempotent while a newer flight is in flight" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try createModel(
        &core,
        connection,
        testId(CreationId, 3),
        terminal_id,
        [_]u8{5} ** 32,
        80,
        24,
    );
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.synchronizeRevision(terminal_id, 1);
    _ = try core.setProjectionDemand(connection, attachment_id, .active);

    const first = try core.commitFrame(attachment_id, null, 1, 1, .snapshot);
    try std.testing.expectEqual(@as(u64, 1), first.sequence);
    try std.testing.expectEqual(
        .applied,
        try core.frameApplied(connection, attachment_id, 1, 1, 1),
    );

    _ = try core.synchronizeRevision(terminal_id, 2);
    const second = try core.commitFrame(attachment_id, 1, 2, 1, .delta);
    try std.testing.expectEqual(@as(u64, 2), second.sequence);
    const before = core.findAttachment(attachment_id).?.projectionInFlight().?;
    try std.testing.expectEqual(
        .idempotent,
        try core.frameApplied(connection, attachment_id, 1, 1, 1),
    );
    const after = core.findAttachment(attachment_id).?.projectionInFlight().?;
    try std.testing.expectEqualDeep(before, after);
    try std.testing.expectEqual(
        .applied,
        try core.frameApplied(connection, attachment_id, 2, 2, 1),
    );
    try expectCoreInvariants(&core);
}
