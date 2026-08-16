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

test "model-only terminal construction stays behind the Core testing seam" {
    try std.testing.expect(!@hasDecl(HostCore, "createTerminal"));
    try std.testing.expect(@hasDecl(subject.testing, "createTerminal"));
    try std.testing.expect(!@hasField(HostCore, "last_active_read_demand_scans"));
    try std.testing.expect(!@hasField(subject.TerminalSession, "engine"));
    try std.testing.expect(!@hasDecl(subject.TerminalSession, "runtime"));
}
test "HostCore connection operations use concrete owner receivers" {
    inline for (.{ HostCore.connect, HostCore.disconnect }) |operation| {
        const function_info = @typeInfo(@TypeOf(operation)).@"fn";
        const receiver = function_info.params[0].type orelse
            return error.ExpectedConcreteReceiver;
        try std.testing.expect(receiver == *HostCore);
    }
    inline for (.{
        HostCore.attachmentsForConnection,
        HostCore.connectionCount,
        HostCore.connectionClientId,
    }) |operation| {
        const function_info = @typeInfo(@TypeOf(operation)).@"fn";
        const receiver = function_info.params[0].type orelse
            return error.ExpectedConcreteReceiver;
        try std.testing.expect(receiver == *const HostCore);
    }
}
test "primary runtime failure stage determines its only legal kind" {
    const FailureCase = struct {
        failure: subject.RuntimeFailure,
        kind: subject.RuntimeFailureKind,
        stage: subject.RuntimeFailureStage,
    };
    const cases = [_]FailureCase{
        .{ .failure = .admit_terminal_reply, .kind = .terminal_reply, .stage = .admit_terminal_reply },
        .{ .failure = .apply_resize, .kind = .resize, .stage = .apply_resize },
        .{ .failure = .dispatch_termination, .kind = .process_control, .stage = .dispatch_termination },
        .{ .failure = .observe_process, .kind = .process_observation, .stage = .observe_process },
        .{ .failure = .read_output, .kind = .pty_read, .stage = .read_output },
        .{ .failure = .synchronize_worker, .kind = .worker_integrity, .stage = .synchronize_worker },
        .{ .failure = .write_input, .kind = .pty_write, .stage = .write_input },
    };
    for (cases) |case| {
        try std.testing.expectEqual(case.kind, case.failure.kind());
        try std.testing.expectEqual(case.stage, case.failure.stage());
    }
    const CleanupCase = struct {
        failure: subject.RuntimeCleanupFailure,
        kind: subject.RuntimeFailureKind,
        stage: subject.RuntimeFailureStage,
    };
    const cleanup_cases = [_]CleanupCase{
        .{ .failure = .contain_process, .kind = .process_control, .stage = .contain_process },
        .{ .failure = .release_output, .kind = .pty_read, .stage = .release_output },
        .{
            .failure = .release_process_identity,
            .kind = .process_observation,
            .stage = .release_process_identity,
        },
    };
    for (cleanup_cases) |case| {
        try std.testing.expectEqual(case.kind, case.failure.kind());
        try std.testing.expectEqual(case.stage, case.failure.stage());
    }
}
test "creation lookup reports inconsistent retained state" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const creation = testId(CreationId, 3);
    _ = try core.connect(connection, testId(ClientId, 4));
    try subject.testing.injectCreationBinding(&core, creation, testId(TerminalId, 5));

    try std.testing.expectError(
        error.MissingResource,
        core.resolveCreation(connection, creation, [_]u8{6} ** 32),
    );
}
test "multi-terminal creation is idempotent without a product count cap" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const creation = testId(CreationId, 3);
    const terminal = testId(TerminalId, 4);
    const digest = [_]u8{5} ** 32;

    try std.testing.expectEqual(.applied, try core.connect(connection, testId(ClientId, 2)));
    try std.testing.expectEqual(
        .applied,
        try createModel(&core, connection, creation, terminal, digest, 80, 24),
    );
    try std.testing.expectEqual(
        .idempotent,
        try createModel(&core, connection, creation, testId(TerminalId, 9), digest, 80, 24),
    );
    try std.testing.expectError(
        error.CreationConflict,
        createModel(&core, connection, creation, terminal, [_]u8{6} ** 32, 80, 24),
    );
    try std.testing.expectEqual(
        .applied,
        try createModel(
            &core,
            connection,
            testId(CreationId, 7),
            testId(TerminalId, 8),
            digest,
            100,
            30,
        ),
    );
    try std.testing.expectEqual(@as(usize, 2), core.terminalCount());
    try std.testing.expectEqual(
        @as(u16, 100),
        core.findTerminal(testId(TerminalId, 8)).?.observedGeometry().columns,
    );
}
test "one broker owns every attachment and disconnect preserves terminals" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const client = testId(ClientId, 2);
    const first_connection: ConnectionId = 2;
    const second_connection: ConnectionId = 7;
    const terminal_id = testId(TerminalId, 4);
    _ = try core.connect(first_connection, client);
    try std.testing.expectError(
        error.ConnectionConflict,
        core.connect(second_connection, client),
    );
    _ = try createModel(&core, first_connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.attach(first_connection, testId(AttachmentId, 6), terminal_id, .{
        .input = true,
        .read = true,
        .resize = false,
    }, false);
    try std.testing.expectEqual(.idempotent, core.disconnect(second_connection));
    try std.testing.expect(core.findAttachment(testId(AttachmentId, 6)) != null);
    try std.testing.expectEqual(.applied, core.disconnect(first_connection));
    try std.testing.expectEqual(@as(usize, 0), core.attachmentCount());
    try std.testing.expect(core.findAttachment(testId(AttachmentId, 6)) == null);
    try std.testing.expectEqual(TerminalPhase.running, core.findTerminal(terminal_id).?.phase());
    try std.testing.expectEqual(.none, core.findTerminal(terminal_id).?.terminationRequest());
}
test "attachment routing and terminal disposal are exact across registries" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const first_connection: ConnectionId = 2;
    const first_terminal = testId(TerminalId, 4);
    const second_terminal = testId(TerminalId, 5);
    const first_attachment = testId(AttachmentId, 6);
    const second_attachment = testId(AttachmentId, 7);
    _ = try core.connect(first_connection, testId(ClientId, 8));
    _ = try createModel(&core, first_connection, testId(CreationId, 9), first_terminal, [_]u8{10} ** 32, 80, 24);
    _ = try createModel(&core, first_connection, testId(CreationId, 11), second_terminal, [_]u8{12} ** 32, 100, 30);
    _ = try core.attach(first_connection, first_attachment, first_terminal, .{
        .input = true,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.attach(first_connection, second_attachment, second_terminal, .{
        .input = true,
        .read = true,
        .resize = false,
    }, false);

    try std.testing.expect(
        first_terminal.eql(
            (try core.resolveAttachment(first_connection, first_attachment, .input)).terminal_id,
        ),
    );
    try std.testing.expect(
        second_terminal.eql(
            (try core.resolveAttachment(first_connection, second_attachment, .read)).terminal_id,
        ),
    );
    try std.testing.expectError(
        error.MissingResource,
        core.resolveAttachment(
            first_connection,
            testId(AttachmentId, 99),
            .none,
        ),
    );

    _ = try core.childExited(first_terminal, .{ .exit_code = 0 });
    _ = try core.ptyDrained(first_terminal);
    try publishPendingOutcome(&core, first_terminal);
    _ = try core.disposeTerminal(first_connection, first_terminal);
    try std.testing.expect(core.findTerminal(first_terminal) == null);
    try std.testing.expect(core.findAttachment(first_attachment) == null);
    try std.testing.expect(core.findTerminal(second_terminal) != null);
    try std.testing.expect(core.findAttachment(second_attachment) != null);
}
test "terminal disposal preserves pending exited and failed outcomes" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 0xd0));
    defer core.deinit();
    const connection: ConnectionId = 2;
    _ = try core.connect(connection, testId(ClientId, 0xd1));

    for (0..2) |case_index| {
        const identity_byte: u8 = @intCast(0xd2 + case_index);
        const terminal_id = testId(TerminalId, identity_byte);
        const attachment_id = testId(AttachmentId, identity_byte + 0x10);
        _ = try createModel(
            &core,
            connection,
            testId(CreationId, identity_byte),
            terminal_id,
            [_]u8{identity_byte} ** 32,
            80,
            24,
        );
        _ = try core.attach(
            connection,
            attachment_id,
            terminal_id,
            .{ .input = false, .read = true, .resize = false },
            false,
        );
        if (case_index == 0) {
            _ = try core.childExited(terminal_id, .{ .exit_code = 0 });
            _ = try core.ptyDrained(terminal_id);
        } else {
            _ = try core.runtimeFailed(terminal_id, .read_output);
            _ = try core.failureContained(
                terminal_id,
                .{ .observed = .{ .exit_code = 1 } },
            );
        }

        const generation_before = core.inventoryGeneration();
        const token = try pendingOutcomeToken(&core, terminal_id);
        try std.testing.expectError(
            error.OutcomePublicationPending,
            core.disposeTerminal(connection, terminal_id),
        );
        try std.testing.expectEqual(generation_before, core.inventoryGeneration());
        try std.testing.expect(core.findTerminal(terminal_id) != null);
        try std.testing.expect(core.findAttachment(attachment_id) != null);
        try std.testing.expect(core.findTerminal(terminal_id).?.pendingOutcome() != null);

        var mismatched_token = token;
        mismatched_token.final_revision += 1;
        try std.testing.expectEqual(
            subject.TransitionOutcome.idempotent,
            core.markOutcomePublished(mismatched_token),
        );
        try std.testing.expectError(
            error.OutcomePublicationPending,
            core.disposeTerminal(connection, terminal_id),
        );
        try std.testing.expectEqual(
            subject.TransitionOutcome.applied,
            core.markOutcomePublished(token),
        );
        try std.testing.expectEqual(
            subject.TransitionOutcome.idempotent,
            core.markOutcomePublished(token),
        );
        try std.testing.expectEqual(
            subject.TransitionOutcome.applied,
            try core.disposeTerminal(connection, terminal_id),
        );
        try std.testing.expect(core.findAttachment(attachment_id) == null);
        try std.testing.expectEqual(
            subject.TransitionOutcome.idempotent,
            try core.disposeTerminal(connection, terminal_id),
        );
    }
}
test "first attachment allocation failures leave no broker commit" {
    const connection_id: ConnectionId = 0xa4;
    const terminal_id = testId(TerminalId, 0xa5);
    const candidate: ExpectedAttachment = .{
        .attachment_id = testId(AttachmentId, 0xa6),
        .capabilities = .{ .input = true, .read = true, .resize = true },
        .resize_lease = true,
    };

    var attach_allocation_count: usize = 0;
    {
        var allocation_counter = std.testing.FailingAllocator.init(
            std.testing.allocator,
            .{},
        );
        var core = try initAttachmentRollbackCore(
            allocation_counter.allocator(),
            connection_id,
            terminal_id,
        );
        defer core.deinit();

        const allocation_start = allocation_counter.alloc_index;
        const resize_start = allocation_counter.resize_index;
        try addExpectedAttachments(&core, connection_id, terminal_id, &.{candidate});
        attach_allocation_count = allocation_counter.alloc_index - allocation_start;
        // The O(1) active-read-demand aggregate reserves its per-terminal
        // entry at first attach, so a fresh terminal costs four allocations:
        // the per-terminal ID list, its first entry, the demand-aggregate
        // entry, and the attachment map entry.
        try std.testing.expectEqual(@as(usize, 4), attach_allocation_count);
        try std.testing.expectEqual(
            resize_start,
            allocation_counter.resize_index,
        );
        try expectAttachmentSnapshot(&core, terminal_id, &.{candidate}, null);
    }

    for (0..attach_allocation_count) |allocation_offset| {
        try expectAttachOutOfMemoryPreservesSnapshot(
            connection_id,
            terminal_id,
            &.{},
            candidate,
            allocation_offset,
            false,
        );
    }
}
test "populated attachment growth rolls back every allocation failure" {
    const connection_id: ConnectionId = 0xb0;
    const terminal_id = testId(TerminalId, 0xb1);
    // Zig 0.15.2 gives an unmanaged ArrayList its first capacity from one
    // cache line. Fill that capacity so the candidate append must grow the
    // per-terminal ID list; the allocator deltas below verify the growth.
    const populated_count = @max(1, std.atomic.cache_line / @sizeOf(AttachmentId));
    var expected: [populated_count + 1]ExpectedAttachment = undefined;
    for (expected[0..populated_count], 0..) |*attachment, index| {
        attachment.* = .{
            .attachment_id = testId(AttachmentId, @intCast(0xc0 + index)),
            .capabilities = switch (index % 3) {
                0 => .{ .input = true, .read = true, .resize = true },
                1 => .{ .input = true, .read = false, .resize = false },
                2 => .{ .input = false, .read = true, .resize = false },
                else => unreachable,
            },
            .resize_lease = index == 0,
        };
    }
    expected[populated_count] = .{
        .attachment_id = testId(AttachmentId, 0xf0),
        .capabilities = .{ .input = true, .read = true, .resize = true },
        .resize_lease = false,
    };
    const preexisting = expected[0..populated_count];
    const candidate = expected[populated_count];

    var ordinary_allocation_count: usize = 0;
    var ordinary_resize_count: usize = 0;
    {
        var allocation_counter = std.testing.FailingAllocator.init(
            std.testing.allocator,
            .{},
        );
        var core = try initAttachmentRollbackCore(
            allocation_counter.allocator(),
            connection_id,
            terminal_id,
        );
        defer core.deinit();
        try addExpectedAttachments(&core, connection_id, terminal_id, preexisting);

        const allocation_start = allocation_counter.alloc_index;
        const resize_start = allocation_counter.resize_index;
        try addExpectedAttachments(&core, connection_id, terminal_id, &.{candidate});
        ordinary_allocation_count = allocation_counter.alloc_index - allocation_start;
        ordinary_resize_count = allocation_counter.resize_index - resize_start;
        try std.testing.expect(
            ordinary_allocation_count > 0 or ordinary_resize_count > 0,
        );
        try expectAttachmentSnapshot(&core, terminal_id, &expected, null);
    }

    for (0..ordinary_allocation_count) |allocation_offset| {
        try expectAttachOutOfMemoryPreservesSnapshot(
            connection_id,
            terminal_id,
            preexisting,
            candidate,
            allocation_offset,
            false,
        );
    }

    // ArrayList growth tries allocator.remap before allocating a replacement.
    // Pinning the current resize index rejects every candidate-time remap;
    // the measured allocation delta therefore enumerates every fallback and
    // later allocation that can return OutOfMemory on this commit path.
    var remap_fallback_allocation_count: usize = 0;
    {
        var allocation_counter = std.testing.FailingAllocator.init(
            std.testing.allocator,
            .{},
        );
        var core = try initAttachmentRollbackCore(
            allocation_counter.allocator(),
            connection_id,
            terminal_id,
        );
        defer core.deinit();
        try addExpectedAttachments(&core, connection_id, terminal_id, preexisting);

        const allocation_start = allocation_counter.alloc_index;
        const resize_start = allocation_counter.resize_index;
        allocation_counter.resize_fail_index = resize_start;
        try addExpectedAttachments(&core, connection_id, terminal_id, &.{candidate});
        remap_fallback_allocation_count =
            allocation_counter.alloc_index - allocation_start;
        try std.testing.expect(remap_fallback_allocation_count > 0);
        try std.testing.expectEqual(resize_start, allocation_counter.resize_index);
        allocation_counter.resize_fail_index = std.math.maxInt(usize);
        try expectAttachmentSnapshot(&core, terminal_id, &expected, null);
    }

    for (0..remap_fallback_allocation_count) |allocation_offset| {
        try expectAttachOutOfMemoryPreservesSnapshot(
            connection_id,
            terminal_id,
            preexisting,
            candidate,
            allocation_offset,
            true,
        );
    }
}
test "large inventory keeps session addresses stable while swap-removing both indexes" {
    const inventory_count = 192;
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    _ = try core.connect(connection, testId(ClientId, 3));

    var creation_ids: [inventory_count]CreationId = undefined;
    var terminal_ids: [inventory_count]TerminalId = undefined;
    var first_session_address: usize = 0;
    for (&creation_ids, &terminal_ids, 0..) |*creation_id, *terminal_id, index| {
        const identity_byte: u8 = @intCast(index + 1);
        creation_id.* = testId(CreationId, identity_byte);
        terminal_id.* = testId(TerminalId, identity_byte);
        try std.testing.expectEqual(
            .applied,
            try createModel(
                &core,
                connection,
                creation_id.*,
                terminal_id.*,
                [_]u8{identity_byte} ** 32,
                80,
                24,
            ),
        );
        if (index == 0) {
            first_session_address = @intFromPtr(core.findTerminal(terminal_id.*).?);
        }
        _ = try core.childExited(terminal_id.*, .{ .exit_code = @intCast(index) });
        _ = try core.ptyDrained(terminal_id.*);
    }
    try std.testing.expectEqual(
        first_session_address,
        @intFromPtr(core.findTerminal(terminal_ids[0]).?),
    );
    try expectCoreInvariants(&core);

    const former_final_creation = creation_ids[inventory_count - 1];
    const former_final_terminal = terminal_ids[inventory_count - 1];
    const former_final_session_address = @intFromPtr(core.findTerminal(former_final_terminal).?);
    for (creation_ids, terminal_ids, 0..) |creation_id, terminal_id, removed_index| {
        const generation_before = core.inventoryGeneration();
        try publishPendingOutcome(&core, terminal_id);
        try std.testing.expectEqual(
            .applied,
            try core.disposeTerminal(connection, terminal_id),
        );
        const generation_after = core.inventoryGeneration();
        try std.testing.expectEqual(generation_before + 1, generation_after);
        try std.testing.expectEqual(inventory_count - removed_index - 1, core.terminalCount());
        try std.testing.expectEqual(
            inventory_count - removed_index - 1,
            subject.testing.creationCount(&core),
        );
        try std.testing.expect(core.findTerminal(terminal_id) == null);
        try std.testing.expect(!subject.testing.containsCreation(&core, creation_id));
        try std.testing.expect(subject.testing.creationTerminal(&core, creation_id) == null);

        if (removed_index == 0) {
            try std.testing.expectEqual(
                former_final_terminal.bytes,
                core.terminalKeys()[0].bytes,
            );
            try std.testing.expectEqual(
                former_final_terminal.bytes,
                core.terminalValues()[0].terminal_id.bytes,
            );
            try std.testing.expectEqual(
                former_final_session_address,
                @intFromPtr(core.findTerminal(former_final_terminal).?),
            );
            try std.testing.expectEqual(
                former_final_creation.bytes,
                subject.testing.creationKeys(&core)[0].bytes,
            );
            try std.testing.expectEqual(
                former_final_terminal.bytes,
                subject.testing.creationValues(&core)[0].bytes,
            );
        }

        for (creation_ids[0 .. removed_index + 1], terminal_ids[0 .. removed_index + 1]) |removed_creation_id, removed_terminal_id| {
            try std.testing.expect(core.findTerminal(removed_terminal_id) == null);
            try std.testing.expect(!subject.testing.containsCreation(&core, removed_creation_id));
        }
        for (creation_ids[removed_index + 1 ..], terminal_ids[removed_index + 1 ..]) |retained_creation_id, retained_terminal_id| {
            try std.testing.expect(core.findTerminal(retained_terminal_id) != null);
            const bound_terminal_id = subject.testing.creationTerminal(
                &core,
                retained_creation_id,
            ) orelse return error.TestUnexpectedResult;
            try std.testing.expect(bound_terminal_id.eql(retained_terminal_id));
        }
        try expectCoreInvariants(&core);

        try std.testing.expectEqual(
            .idempotent,
            try core.disposeTerminal(connection, terminal_id),
        );
        try std.testing.expectEqual(generation_after, core.inventoryGeneration());
    }
}
test "inventory generation exhaustion leaves membership unchanged" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    _ = try core.connect(connection, testId(ClientId, 3));
    subject.testing.setInventoryGeneration(&core, std.math.maxInt(u64));

    try std.testing.expectError(
        error.InventoryGenerationOverflow,
        createModel(
            &core,
            connection,
            testId(CreationId, 4),
            testId(TerminalId, 5),
            [_]u8{6} ** 32,
            80,
            24,
        ),
    );
    try std.testing.expectEqual(@as(usize, 0), core.terminalCount());
    try std.testing.expectEqual(@as(usize, 0), subject.testing.creationCount(&core));
    try expectCoreInvariants(&core);
}
test "invalid resize lease replay cannot bypass capability validation" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 3);
    const attachment_id = testId(AttachmentId, 4);
    const capabilities: Capabilities = .{
        .input = true,
        .read = true,
        .resize = false,
    };
    _ = try core.connect(connection, testId(ClientId, 5));
    _ = try createModel(
        &core,
        connection,
        testId(CreationId, 6),
        terminal_id,
        [_]u8{7} ** 32,
        80,
        24,
    );
    try std.testing.expectError(
        error.InvalidCapabilities,
        core.attach(
            connection,
            testId(AttachmentId, 8),
            terminal_id,
            .{ .input = false, .read = false, .resize = false },
            false,
        ),
    );
    _ = try core.attach(
        connection,
        attachment_id,
        terminal_id,
        capabilities,
        false,
    );

    try std.testing.expectError(
        error.ResizeLeaseDenied,
        core.attach(connection, attachment_id, terminal_id, capabilities, true),
    );
    try std.testing.expect(!core.findAttachment(attachment_id).?.resize_lease);
    try expectCoreInvariants(&core);
}
test "detaching the resize owner promotes the earliest capable observer" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 3);
    const owner_id = testId(AttachmentId, 4);
    const first_observer_id = testId(AttachmentId, 5);
    const second_observer_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 7));
    _ = try createModel(
        &core,
        connection,
        testId(CreationId, 8),
        terminal_id,
        [_]u8{9} ** 32,
        80,
        24,
    );
    _ = try core.attach(
        connection,
        owner_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = true },
        true,
    );
    _ = try core.attach(
        connection,
        first_observer_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = true },
        false,
    );
    _ = try core.attach(
        connection,
        second_observer_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = true },
        false,
    );

    const detached = try core.detach(connection, owner_id);

    try std.testing.expectEqual(subject.TransitionOutcome.applied, detached.outcome);
    try std.testing.expect(detached.resize_lease_change.?.attachment_id.eql(first_observer_id));
    try std.testing.expect(detached.resize_lease_change.?.terminal_id.eql(terminal_id));
    try std.testing.expect(detached.resize_lease_change.?.resize_lease);
    try std.testing.expect(core.findAttachment(first_observer_id).?.resize_lease);
    try std.testing.expect(!core.findAttachment(second_observer_id).?.resize_lease);
    try std.testing.expectEqual(
        subject.TransitionOutcome.idempotent,
        try core.attach(
            connection,
            first_observer_id,
            terminal_id,
            .{ .input = true, .read = true, .resize = true },
            false,
        ),
    );
    try std.testing.expectError(
        error.AttachmentConflict,
        core.attach(
            connection,
            first_observer_id,
            terminal_id,
            .{ .input = true, .read = true, .resize = true },
            true,
        ),
    );
    try expectCoreInvariants(&core);
}
test "exact attachment replay survives phase change but new runtime authority does not" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 3);
    const attachment_id = testId(AttachmentId, 4);
    const capabilities: Capabilities = .{
        .input = true,
        .read = false,
        .resize = false,
    };
    _ = try core.connect(connection, testId(ClientId, 5));
    _ = try createModel(
        &core,
        connection,
        testId(CreationId, 6),
        terminal_id,
        [_]u8{7} ** 32,
        80,
        24,
    );
    _ = try core.attach(
        connection,
        attachment_id,
        terminal_id,
        capabilities,
        false,
    );
    _ = try core.childExited(terminal_id, .{ .exit_code = 0 });

    try std.testing.expectEqual(
        .idempotent,
        try core.attach(
            connection,
            attachment_id,
            terminal_id,
            capabilities,
            false,
        ),
    );
    try std.testing.expectError(
        error.InvalidPhase,
        core.attach(
            connection,
            testId(AttachmentId, 8),
            terminal_id,
            capabilities,
            false,
        ),
    );
    try std.testing.expectEqual(@as(usize, 1), core.attachmentCount());
    try expectCoreInvariants(&core);
}
