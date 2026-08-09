const std = @import("std");
const model_boundary = @import("terminal-host-model");
const subject = @import("terminal-host-core").state;
const terminal_configuration = model_boundary.configuration;
const terminal_geometry = model_boundary.geometry;

const Capabilities = subject.Capabilities;
const AttachmentId = subject.AttachmentId;
const ClientId = subject.ClientId;
const ConnectionId = subject.ConnectionId;
const CreationId = subject.CreationId;
const HostCore = subject.HostCore;
const HostEpoch = subject.HostEpoch;
const ProjectionBlock = subject.ProjectionBlock;
const ProjectionDemand = subject.ProjectionDemand;
const TerminalId = subject.TerminalId;
const TerminalPhase = subject.TerminalPhase;

fn testId(comptime Id: type, byte: u8) Id {
    return Id.fromBytes([_]u8{byte} ** 16);
}

fn expectCoreInvariants(core: *HostCore) !void {
    try std.testing.expect(core.connectionCount() <= 1);
    if (core.connectionCount() == 0) {
        try std.testing.expectEqual(@as(usize, 0), core.attachmentCount());
    }
    try std.testing.expectEqual(core.terminalCount(), subject.testing.creationCount(core));

    for (core.terminalKeys(), core.terminalValues()) |terminal_id, terminal| {
        try std.testing.expect(terminal_id.eql(terminal.terminal_id));
        try std.testing.expect(terminal.runtime() == null);
        const retained_terminal_id = subject.testing.creationTerminal(
            core,
            terminal.creation_id,
        ) orelse {
            return error.TestUnexpectedResult;
        };
        try std.testing.expect(retained_terminal_id.eql(terminal_id));
        try std.testing.expect(terminal_configuration.valid(terminal.effectiveConfiguration()));
        try std.testing.expect(terminal.configurationRevision() > 0);
        try std.testing.expect(terminal_geometry.isValid(terminal.observedGeometry()));
        try std.testing.expect(
            terminal.observedGeometryRevision() <= terminal.observedRevision(),
        );
        switch (terminal.lifecycle) {
            .running => {
                try std.testing.expect(!terminal.childExited());
                try std.testing.expect(!terminal.ptyDrained());
                try std.testing.expect(terminal.exitResult() == null);
                try std.testing.expect(terminal.settledFinalRevision() == null);
            },
            .finishing => {
                try std.testing.expect(terminal.childExited() != terminal.ptyDrained());
                try std.testing.expect(terminal.settledFinalRevision() == null);
            },
            .containing_failure => {
                try std.testing.expect(!terminal.ptyDrained());
                try std.testing.expect(terminal.exitResult() == null);
                try std.testing.expect(terminal.runtimeFailure() != null);
                try std.testing.expect(terminal.failureContainment() != null);
                try std.testing.expect(terminal.pendingOutcome() == null);
                try std.testing.expect(terminal.settledFinalRevision() == null);
            },
            .exited => {
                try std.testing.expect(terminal.childExited());
                try std.testing.expect(terminal.ptyDrained());
                try std.testing.expect(terminal.exitResult() != null);
                try std.testing.expectEqual(
                    terminal.observedRevision(),
                    terminal.settledFinalRevision().?,
                );
            },
            .failed => {
                try std.testing.expect(!terminal.ptyDrained());
                try std.testing.expect(terminal.exitResult() == null);
                try std.testing.expect(terminal.runtimeFailure() != null);
                try std.testing.expect(terminal.failedProcess() != null);
                try std.testing.expectEqual(
                    terminal.observedRevision(),
                    terminal.settledFinalRevision().?,
                );
            },
        }

        var has_active_projection_demand = false;
        for (core.attachmentValues()) |attachment| {
            if (!attachment.terminal_id.eql(terminal_id)) continue;
            if (attachment.capabilities.read and attachment.projectionDemand() == .active) {
                has_active_projection_demand = true;
            }
        }
        try std.testing.expectEqual(
            has_active_projection_demand,
            core.hasActiveProjectionDemand(terminal_id),
        );
    }

    for (
        subject.testing.creationKeys(core),
        subject.testing.creationValues(core),
    ) |creation_id, terminal_id| {
        const terminal = core.findTerminal(terminal_id) orelse return error.TestUnexpectedResult;
        try std.testing.expect(creation_id.eql(terminal.creation_id));
    }

    for (core.attachmentValues()) |attachment| {
        try std.testing.expectEqual(@as(usize, 1), core.connectionCount());
        try std.testing.expect(core.findTerminal(attachment.terminal_id) != null);
        try std.testing.expect(!attachment.resize_lease or attachment.capabilities.resize);
        const terminal_ids = subject.testing.attachmentIdsForTerminal(
            core,
            attachment.terminal_id,
        ) orelse {
            return error.TestUnexpectedResult;
        };
        try std.testing.expect(attachment.terminal_attachment_index < terminal_ids.len);
        try std.testing.expect(
            terminal_ids[attachment.terminal_attachment_index].eql(attachment.attachment_id),
        );
        switch (attachment.projection.delivery) {
            .paused => {},
            .ready => |ready| {
                try std.testing.expect(attachment.capabilities.read);
                switch (ready) {
                    .snapshot => |base| if (base) |value| {
                        try std.testing.expect(value.sequence < attachment.projection.next_sequence);
                    },
                    .delta => |base| {
                        try std.testing.expect(base.sequence < attachment.projection.next_sequence);
                    },
                }
            },
            .in_flight => |flight| {
                try std.testing.expect(attachment.capabilities.read);
                try std.testing.expect(flight.frame.sequence < attachment.projection.next_sequence);
                if (flight.base_before) |base| {
                    try std.testing.expect(base.sequence < flight.frame.sequence);
                }
            },
            .blocked => |blocked| {
                try std.testing.expect(attachment.capabilities.read);
                if (blocked.base) |base| {
                    try std.testing.expect(base.sequence < attachment.projection.next_sequence);
                }
            },
        }

        var resize_lease_count: usize = 0;
        for (core.attachmentValues()) |candidate| {
            if (candidate.resize_lease and
                candidate.terminal_id.eql(attachment.terminal_id))
            {
                resize_lease_count += 1;
            }
        }
        try std.testing.expect(resize_lease_count <= 1);
    }

    try std.testing.expect(subject.testing.attachmentIndexReferencesRetainedTerminals(core));
    try std.testing.expectEqual(
        core.attachmentCount(),
        subject.testing.indexedAttachmentCount(core),
    );
}

const ExpectedAttachment = struct {
    attachment_id: AttachmentId,
    capabilities: Capabilities,
    resize_lease: bool,
};

fn initAttachmentRollbackCore(
    allocator: std.mem.Allocator,
    connection_id: ConnectionId,
    terminal_id: TerminalId,
) !HostCore {
    var core = HostCore.init(allocator, testId(HostEpoch, 0xa0));
    errdefer core.deinit();
    try std.testing.expectEqual(
        .applied,
        try core.connect(connection_id, testId(ClientId, 0xa1)),
    );
    try std.testing.expectEqual(
        .applied,
        try core.createTerminal(
            connection_id,
            testId(CreationId, 0xa2),
            terminal_id,
            [_]u8{0xa3} ** 32,
            80,
            24,
        ),
    );
    return core;
}

fn addExpectedAttachments(
    core: *HostCore,
    connection_id: ConnectionId,
    terminal_id: TerminalId,
    expected: []const ExpectedAttachment,
) !void {
    for (expected) |attachment| {
        try std.testing.expectEqual(
            .applied,
            try core.attach(
                connection_id,
                attachment.attachment_id,
                terminal_id,
                attachment.capabilities,
                attachment.resize_lease,
            ),
        );
    }
}

fn expectAttachmentSnapshot(
    core: *HostCore,
    terminal_id: TerminalId,
    expected: []const ExpectedAttachment,
    absent_attachment_id: ?AttachmentId,
) !void {
    try std.testing.expectEqual(expected.len, core.attachmentCount());
    try std.testing.expectEqual(
        expected.len,
        subject.testing.indexedAttachmentCount(core),
    );

    const indexed_ids = subject.testing.attachmentIdsForTerminal(core, terminal_id);
    if (expected.len == 0) {
        try std.testing.expect(indexed_ids == null);
    } else {
        const ids = indexed_ids orelse return error.TestUnexpectedResult;
        try std.testing.expectEqual(expected.len, ids.len);
        for (expected, 0..) |attachment, index| {
            try std.testing.expect(ids[index].eql(attachment.attachment_id));
        }
    }

    var expected_resize_leases: usize = 0;
    for (expected, 0..) |attachment, index| {
        const retained = core.findAttachment(attachment.attachment_id) orelse
            return error.TestUnexpectedResult;
        try std.testing.expect(retained.attachment_id.eql(attachment.attachment_id));
        try std.testing.expect(retained.terminal_id.eql(terminal_id));
        try std.testing.expect(retained.capabilities == attachment.capabilities);
        try std.testing.expectEqual(attachment.resize_lease, retained.resize_lease);
        try std.testing.expectEqual(index, retained.terminal_attachment_index);
        expected_resize_leases += @intFromBool(attachment.resize_lease);
    }

    var actual_resize_leases: usize = 0;
    for (core.attachmentValues()) |attachment| {
        if (attachment.terminal_id.eql(terminal_id) and attachment.resize_lease) {
            actual_resize_leases += 1;
        }
    }
    try std.testing.expectEqual(expected_resize_leases, actual_resize_leases);
    if (absent_attachment_id) |attachment_id| {
        try std.testing.expect(core.findAttachment(attachment_id) == null);
    }
    try expectCoreInvariants(core);
}

fn expectAttachOutOfMemoryPreservesSnapshot(
    connection_id: ConnectionId,
    terminal_id: TerminalId,
    preexisting: []const ExpectedAttachment,
    candidate: ExpectedAttachment,
    allocation_offset: usize,
    reject_remap: bool,
) !void {
    var failing = std.testing.FailingAllocator.init(std.testing.allocator, .{});
    var core = try initAttachmentRollbackCore(
        failing.allocator(),
        connection_id,
        terminal_id,
    );
    defer core.deinit();
    try addExpectedAttachments(&core, connection_id, terminal_id, preexisting);

    const allocation_start = failing.alloc_index;
    const resize_start = failing.resize_index;
    failing.fail_index = allocation_start + allocation_offset;
    if (reject_remap) failing.resize_fail_index = resize_start;
    try std.testing.expectError(
        error.OutOfMemory,
        core.attach(
            connection_id,
            candidate.attachment_id,
            terminal_id,
            candidate.capabilities,
            candidate.resize_lease,
        ),
    );
    try std.testing.expect(failing.has_induced_failure);
    try std.testing.expectEqual(failing.fail_index, failing.alloc_index);
    if (reject_remap) {
        try std.testing.expectEqual(resize_start, failing.resize_index);
    }
    failing.fail_index = std.math.maxInt(usize);
    failing.resize_fail_index = std.math.maxInt(usize);

    try expectAttachmentSnapshot(
        &core,
        terminal_id,
        preexisting,
        candidate.attachment_id,
    );
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
        try core.createTerminal(connection, creation, terminal, digest, 80, 24),
    );
    try std.testing.expectEqual(
        .idempotent,
        try core.createTerminal(connection, creation, testId(TerminalId, 9), digest, 80, 24),
    );
    try std.testing.expectError(
        error.CreationConflict,
        core.createTerminal(connection, creation, terminal, [_]u8{6} ** 32, 80, 24),
    );
    try std.testing.expectEqual(
        .applied,
        try core.createTerminal(
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
    _ = try core.createTerminal(first_connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
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
    _ = try core.createTerminal(first_connection, testId(CreationId, 9), first_terminal, [_]u8{10} ** 32, 80, 24);
    _ = try core.createTerminal(first_connection, testId(CreationId, 11), second_terminal, [_]u8{12} ** 32, 100, 30);
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
    _ = try core.disposeTerminal(first_connection, first_terminal);
    try std.testing.expect(core.findTerminal(first_terminal) == null);
    try std.testing.expect(core.findAttachment(first_attachment) == null);
    try std.testing.expect(core.findTerminal(second_terminal) != null);
    try std.testing.expect(core.findAttachment(second_attachment) != null);
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
        try std.testing.expectEqual(@as(usize, 3), attach_allocation_count);
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

test "large inventory disposal swap-removes both indexes and advances one generation per terminal" {
    const inventory_count = 192;
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    _ = try core.connect(connection, testId(ClientId, 3));

    var creation_ids: [inventory_count]CreationId = undefined;
    var terminal_ids: [inventory_count]TerminalId = undefined;
    for (&creation_ids, &terminal_ids, 0..) |*creation_id, *terminal_id, index| {
        const identity_byte: u8 = @intCast(index + 1);
        creation_id.* = testId(CreationId, identity_byte);
        terminal_id.* = testId(TerminalId, identity_byte);
        try std.testing.expectEqual(
            .applied,
            try core.createTerminal(
                connection,
                creation_id.*,
                terminal_id.*,
                [_]u8{identity_byte} ** 32,
                80,
                24,
            ),
        );
        _ = try core.childExited(terminal_id.*, .{ .exit_code = @intCast(index) });
        _ = try core.ptyDrained(terminal_id.*);
    }
    try expectCoreInvariants(&core);

    const former_final_creation = creation_ids[inventory_count - 1];
    const former_final_terminal = terminal_ids[inventory_count - 1];
    for (creation_ids, terminal_ids, 0..) |creation_id, terminal_id, removed_index| {
        const generation_before = core.inventoryGeneration();
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
        core.createTerminal(
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
    _ = try core.createTerminal(
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
    _ = try core.createTerminal(
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

test "input authority, resize, and reordered completion preserve bounds" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try core.createTerminal(connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = true,
        .read = true,
        .resize = true,
    }, true);

    _ = try core.authorizeInput(connection, attachment_id);
    _ = try core.resize(connection, attachment_id, 120, 40);
    try std.testing.expectEqual(@as(u64, 0), core.findTerminal(terminal_id).?.observedRevision());

    try std.testing.expectEqual(.applied, try core.requestForceTerminate(connection, terminal_id));
    try std.testing.expectEqual(.idempotent, try core.requestTerminate(connection, terminal_id));
    try std.testing.expectEqual(.idempotent, try core.requestForceTerminate(connection, terminal_id));
    try std.testing.expectEqual(.force, core.findTerminal(terminal_id).?.terminationRequest());
    _ = try core.ptyDrained(terminal_id);
    try std.testing.expectEqual(TerminalPhase.finishing, core.findTerminal(terminal_id).?.phase());
    _ = try core.childExited(terminal_id, .{ .exit_code = 0 });
    try std.testing.expectEqual(TerminalPhase.exited, core.findTerminal(terminal_id).?.phase());
    try std.testing.expectError(
        error.InvalidPhase,
        core.synchronizeGeometry(terminal_id, 120, 40, 10, 20, 1),
    );
    try std.testing.expectEqual(.idempotent, try core.disposeTerminal(connection, testId(TerminalId, 9)));
    _ = try core.disposeTerminal(connection, terminal_id);
    try std.testing.expect(core.findTerminal(terminal_id) == null);
}

test "retained runtime capabilities separate live writes from retained reads" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try core.createTerminal(
        connection,
        testId(CreationId, 3),
        terminal_id,
        [_]u8{5} ** 32,
        80,
        24,
    );

    const running = core.findTerminal(terminal_id).?;
    try std.testing.expect(running.acceptsRuntimeWork());
    try std.testing.expect(running.acceptsConfigurationUpdate());
    try std.testing.expect(running.hasReadableRuntimeState());
    try std.testing.expectError(
        error.InvalidConfiguration,
        core.updateTerminalConfiguration(
            connection,
            terminal_id,
            .{ .scrollback_bytes = std.math.maxInt(usize) },
        ),
    );

    _ = try core.childExited(terminal_id, .{ .exit_code = 0 });
    const finishing = core.findTerminal(terminal_id).?;
    try std.testing.expect(!finishing.acceptsRuntimeWork());
    try std.testing.expect(finishing.acceptsConfigurationUpdate());
    try std.testing.expect(finishing.hasReadableRuntimeState());
    try std.testing.expectEqual(
        .applied,
        try core.updateTerminalConfiguration(
            connection,
            terminal_id,
            .{ .scrollback_bytes = 0 },
        ),
    );

    _ = try core.ptyDrained(terminal_id);
    const exited = core.findTerminal(terminal_id).?;
    try std.testing.expect(!exited.acceptsRuntimeWork());
    try std.testing.expect(!exited.acceptsConfigurationUpdate());
    try std.testing.expect(exited.hasReadableRuntimeState());
    try std.testing.expectError(
        error.InvalidPhase,
        core.updateTerminalConfiguration(
            connection,
            terminal_id,
            .{ .scrollback_bytes = 1 },
        ),
    );
    try std.testing.expectEqual(
        @as(u64, 2),
        core.findTerminal(terminal_id).?.configurationRevision(),
    );
    try std.testing.expectEqual(
        core.findTerminal(terminal_id).?.observedRevision(),
        core.findTerminal(terminal_id).?.settledFinalRevision().?,
    );
}

test "invalid zero signal cannot enter retained lifecycle state" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try core.createTerminal(
        connection,
        testId(CreationId, 3),
        terminal_id,
        [_]u8{5} ** 32,
        80,
        24,
    );

    try std.testing.expectError(
        error.InvalidProcessObservation,
        core.childExited(terminal_id, .{ .signal = 0 }),
    );
    try std.testing.expectEqual(TerminalPhase.running, core.findTerminal(terminal_id).?.phase());
    _ = try core.runtimeFailed(terminal_id, .observe_process);
    try std.testing.expectError(
        error.InvalidProcessObservation,
        core.failureContained(terminal_id, .{ .observed = .{ .signal = 0 } }),
    );
    try std.testing.expectEqual(
        TerminalPhase.containing_failure,
        core.findTerminal(terminal_id).?.phase(),
    );
}

test "either finishing order remains terminable before force" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    _ = try core.connect(connection, testId(ClientId, 3));

    for (0..2) |order| {
        const terminal_id = testId(TerminalId, @intCast(10 + order));
        _ = try core.createTerminal(
            connection,
            testId(CreationId, @intCast(20 + order)),
            terminal_id,
            [_]u8{@intCast(30 + order)} ** 32,
            80,
            24,
        );
        if (order == 0) {
            _ = try core.childExited(terminal_id, .{ .exit_code = 0 });
        } else {
            _ = try core.ptyDrained(terminal_id);
        }
        try std.testing.expectEqual(TerminalPhase.finishing, core.findTerminal(terminal_id).?.phase());
        try std.testing.expectEqual(.applied, try core.requestTerminate(connection, terminal_id));
        try std.testing.expectEqual(.applied, try core.requestForceTerminate(connection, terminal_id));
        if (order == 0) {
            _ = try core.ptyDrained(terminal_id);
        } else {
            _ = try core.childExited(terminal_id, .{ .exit_code = 0 });
        }
        try std.testing.expectEqual(TerminalPhase.exited, core.findTerminal(terminal_id).?.phase());
        try expectCoreInvariants(&core);
    }
}

test "termination retries remain idempotent after terminal completion" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try core.createTerminal(
        connection,
        testId(CreationId, 3),
        terminal_id,
        [_]u8{5} ** 32,
        80,
        24,
    );
    _ = try core.childExited(terminal_id, .{ .exit_code = 0 });
    _ = try core.ptyDrained(terminal_id);

    try std.testing.expectEqual(TerminalPhase.exited, core.findTerminal(terminal_id).?.phase());
    try std.testing.expectEqual(
        .idempotent,
        try core.requestTerminate(connection, terminal_id),
    );
    try std.testing.expectEqual(
        .idempotent,
        try core.requestForceTerminate(connection, terminal_id),
    );
    try std.testing.expectEqual(.none, core.findTerminal(terminal_id).?.terminationRequest());
}

test "a settled normal outcome cannot be rewritten as a runtime failure" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try core.createTerminal(
        connection,
        testId(CreationId, 3),
        terminal_id,
        [_]u8{5} ** 32,
        80,
        24,
    );
    _ = try core.childExited(terminal_id, .{ .exit_code = 0 });
    _ = try core.ptyDrained(terminal_id);

    try std.testing.expectEqual(
        .idempotent,
        try core.runtimeFailed(terminal_id, .observe_process),
    );
    const terminal = core.findTerminal(terminal_id).?;
    try std.testing.expectEqual(TerminalPhase.exited, terminal.phase());
    const outcome = terminal.pendingOutcome() orelse return error.TestUnexpectedResult;
    switch (outcome) {
        .exited => |exited| switch (exited.result) {
            .exit_code => |code| try std.testing.expectEqual(@as(u32, 0), code),
            .signal, .unknown => return error.ExpectedExitCode,
        },
        .failed => return error.ExpectedNormalOutcome,
    }
    try expectCoreInvariants(&core);
}

test "failed termination dispatch enters explicit failure containment" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try core.createTerminal(
        connection,
        testId(CreationId, 3),
        terminal_id,
        [_]u8{5} ** 32,
        80,
        24,
    );

    try std.testing.expectEqual(
        subject.TerminationAdmission.dispatch,
        try core.prepareTermination(connection, terminal_id, .graceful),
    );
    try std.testing.expectEqual(
        subject.TerminationRequest.none,
        core.findTerminal(terminal_id).?.terminationRequest(),
    );
    core.commitTerminationDispatchFailure(terminal_id);
    const failed = core.findTerminal(terminal_id).?;
    try std.testing.expectEqual(TerminalPhase.containing_failure, failed.phase());
    try std.testing.expectEqual(.force, failed.terminationRequest());
    const failure = failed.runtimeFailure() orelse return error.TestUnexpectedResult;
    try std.testing.expectEqual(subject.RuntimeFailureKind.process_control, failure.kind());
    try std.testing.expectEqual(subject.RuntimeFailureStage.dispatch_termination, failure.stage());
    try std.testing.expectEqual(
        .idempotent,
        try core.requestForceTerminate(connection, terminal_id),
    );
    try expectCoreInvariants(&core);
}

test "runtime failure fences work without inventing a completion signal" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try core.createTerminal(
        connection,
        testId(CreationId, 3),
        terminal_id,
        [_]u8{5} ** 32,
        80,
        24,
    );
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = true,
        .read = false,
        .resize = true,
    }, true);

    try std.testing.expectEqual(
        .applied,
        try core.runtimeFailed(terminal_id, .apply_resize),
    );
    const failed = core.findTerminal(terminal_id).?;
    try std.testing.expectEqual(TerminalPhase.containing_failure, failed.phase());
    try std.testing.expect(!failed.childExited());
    try std.testing.expect(!failed.ptyDrained());
    try std.testing.expect(!failed.acceptsRuntimeWork());
    try std.testing.expect(!failed.acceptsConfigurationUpdate());
    try std.testing.expect(!failed.hasReadableRuntimeState());
    try std.testing.expectEqual(.force, failed.terminationRequest());
    const failure = failed.runtimeFailure() orelse return error.TestUnexpectedResult;
    try std.testing.expectEqual(subject.RuntimeFailureKind.resize, failure.kind());
    try std.testing.expectEqual(subject.RuntimeFailureStage.apply_resize, failure.stage());
    try std.testing.expectError(
        error.InvalidPhase,
        core.authorizeInput(connection, attachment_id),
    );
    try std.testing.expectError(
        error.InvalidPhase,
        core.resize(connection, attachment_id, 120, 40),
    );
    try std.testing.expectEqual(
        .idempotent,
        try core.runtimeFailed(terminal_id, .apply_resize),
    );
    try std.testing.expectEqual(
        .applied,
        try core.failureContainmentBlocked(terminal_id, .contain_process, 123),
    );
    const blocked = core.findTerminal(terminal_id).?.failureContainment() orelse
        return error.TestUnexpectedResult;
    switch (blocked) {
        .blocked => |state| {
            try std.testing.expectEqual(subject.RuntimeCleanupFailure.contain_process, state.cleanup_failure);
            try std.testing.expectEqual(@as(i128, 123), state.retry_at_ns);
        },
        .stopping => return error.TestUnexpectedResult,
    }
    try std.testing.expectEqual(
        .applied,
        try core.requestForceTerminate(connection, terminal_id),
    );
    try std.testing.expectEqual(
        .applied,
        try core.failureContained(terminal_id, .{ .observed = .{ .exit_code = 1 } }),
    );
    const contained = core.findTerminal(terminal_id).?;
    try std.testing.expectEqual(TerminalPhase.failed, contained.phase());
    try std.testing.expect(contained.childExited());
    try std.testing.expect(!contained.ptyDrained());
    try std.testing.expect(!contained.hasReadableRuntimeState());
    const outcome = contained.pendingOutcome() orelse return error.TestUnexpectedResult;
    const token = switch (outcome) {
        .failed => |value| value.token,
        .exited => return error.TestUnexpectedResult,
    };
    var mismatched_token = token;
    mismatched_token.kind = .exited;
    try std.testing.expectEqual(.idempotent, core.markOutcomePublished(mismatched_token));
    try std.testing.expect(contained.pendingOutcome() != null);
    try std.testing.expectEqual(.applied, core.markOutcomePublished(token));
    try std.testing.expectEqual(.idempotent, core.markOutcomePublished(token));
    try std.testing.expect(contained.pendingOutcome() == null);
    try expectCoreInvariants(&core);
}

// Stateful transition property suite for TH-001, TH-005, TH-008, TH-009,
// and TH-013. It exercises Create, graceful/force intent, both normal
// completion observations, failure containment, outcome publication, and
// Dispose while checking the Core invariants after every action.
test "lifecycle command sequences preserve invariants after every transition" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    _ = try core.connect(connection, testId(ClientId, 3));

    for (0..64) |cycle| {
        const terminal_id = testId(TerminalId, @intCast(cycle + 10));
        const attachment_id = testId(AttachmentId, @intCast(cycle + 80));
        _ = try core.createTerminal(
            connection,
            testId(CreationId, @intCast(cycle + 10)),
            terminal_id,
            [_]u8{@intCast(cycle + 1)} ** 32,
            80,
            24,
        );
        _ = try core.attach(connection, attachment_id, terminal_id, .{
            .input = true,
            .read = true,
            .resize = true,
        }, true);

        var containment_blocked_once = false;
        var step: usize = 0;
        while (core.findTerminal(terminal_id) != null and step < 32) : (step += 1) {
            const terminal = core.findTerminal(terminal_id).?;
            switch (terminal.lifecycle) {
                .running => {
                    switch ((step + cycle) % 5) {
                        0, 1 => {
                            const request: subject.TerminationRequest = if ((step + cycle) % 5 == 0)
                                .graceful
                            else
                                .force;
                            const before = terminal.terminationRequest();
                            switch (try core.prepareTermination(connection, terminal_id, request)) {
                                .dispatch => {
                                    try std.testing.expectEqual(
                                        before,
                                        core.findTerminal(terminal_id).?.terminationRequest(),
                                    );
                                    core.commitPreparedTermination(terminal_id, request);
                                },
                                .replay => {},
                                .containment_retry => return error.UnexpectedContainmentRetry,
                            }
                        },
                        2 => _ = try core.childExited(
                            terminal_id,
                            .{ .exit_code = @intCast(cycle) },
                        ),
                        3 => _ = try core.ptyDrained(terminal_id),
                        4 => _ = try core.runtimeFailed(terminal_id, .read_output),
                        else => unreachable,
                    }
                },
                .finishing => |state| {
                    switch ((step + cycle) % 4) {
                        0 => {
                            switch (try core.prepareTermination(connection, terminal_id, .force)) {
                                .dispatch => core.commitPreparedTermination(terminal_id, .force),
                                .replay => {},
                                .containment_retry => return error.UnexpectedContainmentRetry,
                            }
                        },
                        1 => _ = try core.runtimeFailed(terminal_id, .write_input),
                        2, 3 => switch (state.first) {
                            .child_exited => _ = try core.ptyDrained(terminal_id),
                            .pty_drained => _ = try core.childExited(
                                terminal_id,
                                .{ .exit_code = @intCast(cycle) },
                            ),
                        },
                        else => unreachable,
                    }
                },
                .containing_failure => |state| switch (state.containment) {
                    .stopping => {
                        if (cycle % 2 == 0 and !containment_blocked_once) {
                            containment_blocked_once = true;
                            _ = try core.failureContainmentBlocked(
                                terminal_id,
                                .release_output,
                                @intCast(cycle),
                            );
                        } else {
                            _ = try core.failureContained(
                                terminal_id,
                                if (cycle % 3 == 0)
                                    .{ .observed = .{ .exit_code = @intCast(cycle) } }
                                else
                                    .{ .unavailable = .observe_process },
                            );
                        }
                    },
                    .blocked => try std.testing.expectEqual(
                        subject.TransitionOutcome.applied,
                        try core.requestForceTerminate(connection, terminal_id),
                    ),
                },
                .exited, .failed => {
                    const pending = terminal.pendingOutcome() orelse
                        return error.ExpectedPendingOutcome;
                    const token = switch (pending) {
                        .exited => |outcome| outcome.token,
                        .failed => |outcome| outcome.token,
                    };
                    try std.testing.expectEqual(
                        subject.TransitionOutcome.applied,
                        core.markOutcomePublished(token),
                    );
                    try std.testing.expectEqual(
                        subject.TransitionOutcome.idempotent,
                        core.markOutcomePublished(token),
                    );
                    _ = try core.disposeTerminal(connection, terminal_id);
                },
            }
            try expectCoreInvariants(&core);
        }
        try std.testing.expect(step < 32);
        try std.testing.expect(core.findTerminal(terminal_id) == null);
    }
}

// Stateful transition property suite for TH-002, TH-003, TH-004, TH-011,
// and TH-012. It exercises Attach, capability use, demand, acknowledgement,
// Detach, and broker replacement while checking both owner indexes.
test "attachment capability sequences preserve broker and terminal indexes" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    var connection: ConnectionId = 2;
    _ = try core.connect(connection, testId(ClientId, 3));
    const terminal_id = testId(TerminalId, 4);
    _ = try core.createTerminal(
        connection,
        testId(CreationId, 5),
        terminal_id,
        [_]u8{6} ** 32,
        80,
        24,
    );

    for (0..512) |step| {
        const attachment_id = testId(AttachmentId, @intCast(20 + step % 4));
        const capabilities: Capabilities = switch (step % 4) {
            0 => .{ .input = false, .read = true, .resize = false },
            1 => .{ .input = true, .read = false, .resize = false },
            2 => .{ .input = false, .read = true, .resize = true },
            3 => .{ .input = true, .read = true, .resize = true },
            else => unreachable,
        };
        switch (step % 9) {
            0 => {
                if (core.findAttachment(attachment_id) == null) {
                    var resize_lease_exists = false;
                    for (core.attachmentValues()) |candidate| {
                        if (candidate.terminal_id.eql(terminal_id) and candidate.resize_lease) {
                            resize_lease_exists = true;
                            break;
                        }
                    }
                    const request_lease = capabilities.resize and !resize_lease_exists;
                    _ = try core.attach(
                        connection,
                        attachment_id,
                        terminal_id,
                        capabilities,
                        request_lease,
                    );
                }
            },
            1 => {
                if (core.findAttachment(attachment_id)) |attachment| {
                    try std.testing.expectEqual(
                        subject.TransitionOutcome.idempotent,
                        try core.attach(
                            connection,
                            attachment_id,
                            terminal_id,
                            attachment.capabilities,
                            attachment.resize_lease,
                        ),
                    );
                }
            },
            2 => {
                if (core.findAttachment(attachment_id)) |attachment| {
                    var conflicting = attachment.capabilities;
                    conflicting.input = !conflicting.input;
                    try std.testing.expectError(
                        error.AttachmentConflict,
                        core.attach(
                            connection,
                            attachment_id,
                            terminal_id,
                            conflicting,
                            attachment.resize_lease,
                        ),
                    );
                }
            },
            3 => {
                if (core.findAttachment(attachment_id)) |attachment| {
                    if (attachment.capabilities.read) {
                        const demand: ProjectionDemand = if (attachment.projectionDemand() == .active)
                            .paused
                        else
                            .active;
                        _ = try core.setProjectionDemand(connection, attachment_id, demand);
                    }
                }
            },
            4 => {
                if (core.findAttachment(attachment_id)) |attachment| {
                    if (attachment.capabilities.input) {
                        _ = try core.authorizeInput(connection, attachment_id);
                    }
                }
            },
            5 => {
                if (core.findAttachment(attachment_id)) |attachment| {
                    if (attachment.resize_lease) {
                        _ = try core.resize(connection, attachment_id, 80, 24);
                    }
                }
            },
            6 => _ = try core.detach(connection, attachment_id),
            7 => {
                _ = core.disconnect(connection);
                connection += 1;
                _ = try core.connect(connection, testId(ClientId, @intCast(30 + step % 100)));
            },
            8 => {
                try std.testing.expectError(
                    error.MissingConnection,
                    core.attach(
                        connection + 1,
                        attachment_id,
                        terminal_id,
                        capabilities,
                        false,
                    ),
                );
            },
            else => unreachable,
        }
        try expectCoreInvariants(&core);
    }

    _ = try core.childExited(terminal_id, .{ .exit_code = 0 });
    _ = try core.ptyDrained(terminal_id);
    _ = try core.disposeTerminal(connection, terminal_id);
    _ = core.disconnect(connection);
    try expectCoreInvariants(&core);
}

test "runtime revision synchronization rejects regression" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try core.createTerminal(connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = true,
    }, true);
    _ = try core.synchronizeRevision(terminal_id, 8);
    try std.testing.expectEqual(@as(u64, 8), core.findTerminal(terminal_id).?.observedRevision());
    try std.testing.expectError(error.InvalidPhase, core.synchronizeRevision(terminal_id, 7));
}

test "failed terminal revision accepts only an exact read coherence check" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try core.createTerminal(
        connection,
        testId(CreationId, 3),
        terminal_id,
        [_]u8{5} ** 32,
        80,
        24,
    );
    _ = try core.synchronizeRevision(terminal_id, 8);
    _ = try core.runtimeFailed(terminal_id, .read_output);
    _ = try core.failureContained(
        terminal_id,
        .{ .observed = .{ .exit_code = 1 } },
    );

    try std.testing.expectEqual(
        subject.TransitionOutcome.idempotent,
        try core.synchronizeRevision(terminal_id, 8),
    );
    try std.testing.expectError(error.InvalidPhase, core.synchronizeRevision(terminal_id, 7));
    try std.testing.expectError(error.InvalidPhase, core.synchronizeRevision(terminal_id, 9));
}

test "exited terminal revision accepts only its immutable final value" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try core.createTerminal(
        connection,
        testId(CreationId, 3),
        terminal_id,
        [_]u8{5} ** 32,
        80,
        24,
    );
    _ = try core.synchronizeRevision(terminal_id, 8);
    _ = try core.childExited(terminal_id, .{ .exit_code = 0 });
    _ = try core.ptyDrained(terminal_id);

    try std.testing.expectEqual(@as(u64, 8), core.findTerminal(terminal_id).?.settledFinalRevision().?);
    try std.testing.expectEqual(
        subject.TransitionOutcome.idempotent,
        try core.synchronizeRevision(terminal_id, 8),
    );
    try std.testing.expectError(error.InvalidPhase, core.synchronizeRevision(terminal_id, 7));
    try std.testing.expectError(error.InvalidPhase, core.synchronizeRevision(terminal_id, 9));
}

test "projection demand gates one in-flight frame and coalesces later revisions" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try core.createTerminal(connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
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
    _ = try core.createTerminal(
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

test "maximum frame sequence can never be acknowledged as a phantom retry" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try core.createTerminal(connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
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
    _ = try core.createTerminal(connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
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
    _ = try core.createTerminal(connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
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

test "recovery is exact and blocked projection requires explicit pause resume" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try core.createTerminal(connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
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

    try core.blockProjection(attachment_id, .capture_allocation_failed);
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
    _ = try core.createTerminal(
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

    try core.blockProjection(attachment_id, .projection_too_large);
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
    _ = try core.createTerminal(first_connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
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

// Stateful transition property suite for TH-006, TH-010, TH-011, and TH-012.
// It exercises demand, capture commit, acknowledgement, negative
// acknowledgement, block, revision advance, pause, and retry transitions.
test "projection transition sequences preserve core invariants after every action" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try core.createTerminal(
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
                    try core.blockProjection(attachment_id, .capture_allocation_failed);
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
