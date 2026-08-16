pub const std = @import("std");
pub const model_boundary = @import("terminal-host-model");
pub const subject = @import("terminal-host-core").state;
pub const terminal_configuration = model_boundary.configuration;
pub const terminal_geometry = model_boundary.geometry;

pub const Capabilities = subject.Capabilities;
pub const AttachmentId = subject.AttachmentId;
pub const ClientId = subject.ClientId;
pub const ConnectionId = subject.ConnectionId;
pub const CreationId = subject.CreationId;
pub const HostCore = subject.HostCore;
pub const HostEpoch = subject.HostEpoch;
pub const ProjectionBlock = subject.ProjectionBlock;
pub const ProjectionDemand = subject.ProjectionDemand;
pub const TerminalId = subject.TerminalId;
pub const TerminalPhase = subject.TerminalPhase;
pub const createModel = subject.testing.createTerminal;

pub fn testId(comptime Id: type, byte: u8) Id {
    return Id.fromBytes([_]u8{byte} ** 16);
}

pub fn pendingOutcomeToken(
    core: *HostCore,
    terminal_id: TerminalId,
) !subject.OutcomePublicationToken {
    const terminal = core.findTerminal(terminal_id) orelse return error.ExpectedTerminal;
    const pending = terminal.pendingOutcome() orelse return error.ExpectedPendingOutcome;
    return switch (pending) {
        .exited => |outcome| outcome.token,
        .failed => |outcome| outcome.token,
    };
}

pub fn publishPendingOutcome(core: *HostCore, terminal_id: TerminalId) !void {
    const token = try pendingOutcomeToken(core, terminal_id);
    try std.testing.expectEqual(
        subject.TransitionOutcome.applied,
        core.markOutcomePublished(token),
    );
    try std.testing.expectEqual(
        subject.TransitionOutcome.idempotent,
        core.markOutcomePublished(token),
    );
}

pub fn expectCoreInvariants(core: *HostCore) !void {
    try subject.testing.checkInvariants(core);
}

pub const ExpectedAttachment = struct {
    attachment_id: AttachmentId,
    capabilities: Capabilities,
    resize_lease: bool,
};

pub fn initAttachmentRollbackCore(
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
        try createModel(
            &core,
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

pub fn addExpectedAttachments(
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

pub fn expectAttachmentSnapshot(
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

pub fn expectAttachOutOfMemoryPreservesSnapshot(
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

// Stateful transition property suite for TH-001, TH-005, TH-008, TH-009,
// and TH-013. It exercises Create, graceful/force intent, both normal
// completion observations, failure containment, outcome publication, and
// Dispose while checking the Core invariants after every action.

// Stateful transition property suite for TH-002, TH-003, TH-004, TH-011,
// and TH-012. It exercises Attach, capability use, demand, acknowledgement,
// Detach, and broker replacement while checking both owner indexes.

pub fn expectActiveDemandAggregateExact(core: *HostCore) !void {
    for (core.terminalKeys()) |terminal_id| {
        var expected = false;
        for (core.attachmentValues()) |attachment| {
            if (!attachment.terminal_id.eql(terminal_id)) continue;
            if (attachment.capabilities.read and attachment.projectionDemand() == .active) {
                expected = true;
                break;
            }
        }
        try std.testing.expectEqual(
            expected,
            core.hasActiveProjectionDemand(terminal_id),
        );
    }
}

// THCR-026 regression: the O(1) per-terminal active-read-demand aggregate must
// remain exact across demand transitions, swap removals, and final removal.

// Stateful transition property suite for TH-006, TH-010, TH-011, and TH-012.
// It exercises demand, capture commit, acknowledgement, negative
// acknowledgement, block, revision advance, pause, and retry transitions.

// THCR-029 regression: an exact duplicate of an already-applied frame
// acknowledgement must be an idempotent no-op even while a newer frame is in
// flight, and it must never advance or clear the newer flight.
