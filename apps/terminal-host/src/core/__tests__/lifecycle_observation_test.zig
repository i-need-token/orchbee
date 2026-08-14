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

test "input authority, resize, and reordered completion preserve bounds" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
    const attachment_id = testId(AttachmentId, 6);
    _ = try core.connect(connection, testId(ClientId, 2));
    _ = try createModel(&core, connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
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
    try publishPendingOutcome(&core, terminal_id);
    _ = try core.disposeTerminal(connection, terminal_id);
    try std.testing.expect(core.findTerminal(terminal_id) == null);
}
test "retained runtime capabilities separate live writes from retained reads" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
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
    _ = try createModel(
        &core,
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
        _ = try createModel(
            &core,
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
    _ = try createModel(
        &core,
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
    _ = try createModel(
        &core,
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
    _ = try createModel(
        &core,
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
test "lifecycle command sequences preserve invariants after every transition" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    _ = try core.connect(connection, testId(ClientId, 3));

    for (0..64) |cycle| {
        const terminal_id = testId(TerminalId, @intCast(cycle + 10));
        const attachment_id = testId(AttachmentId, @intCast(cycle + 80));
        _ = try createModel(
            &core,
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
test "attachment capability sequences preserve broker and terminal indexes" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    var connection: ConnectionId = 2;
    _ = try core.connect(connection, testId(ClientId, 3));
    const terminal_id = testId(TerminalId, 4);
    _ = try createModel(
        &core,
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
    try publishPendingOutcome(&core, terminal_id);
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
    _ = try createModel(&core, connection, testId(CreationId, 3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = true,
    }, true);
    _ = try core.synchronizeRevision(terminal_id, 8);
    try std.testing.expectEqual(@as(u64, 8), core.findTerminal(terminal_id).?.observedRevision());
    try std.testing.expectError(error.InvalidPhase, core.synchronizeRevision(terminal_id, 7));
}
test "finishing terminal accepts a newer terminal-driven geometry observation" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
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
    _ = try core.childExited(terminal_id, .{ .exit_code = 0 });
    try std.testing.expectEqual(
        TerminalPhase.finishing,
        core.findTerminal(terminal_id).?.phase(),
    );

    try core.synchronizeGeometry(terminal_id, 132, 24, 9, 18, 1);

    const terminal = core.findTerminal(terminal_id).?;
    try std.testing.expectEqual(@as(u16, 132), terminal.observedGeometry().columns);
    try std.testing.expectEqual(@as(u16, 24), terminal.observedGeometry().rows);
    try std.testing.expectEqual(@as(u16, 9), terminal.observedGeometry().cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 18), terminal.observedGeometry().cell_pixel_height);
    try std.testing.expectEqual(@as(u64, 1), terminal.observedRevision());
}
test "settled terminal geometry accepts only its exact immutable observation" {
    const SettledCase = enum { exited, failed };
    const cases = [_]SettledCase{ .exited, .failed };

    for (cases, 0..) |case, index| {
        var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
        defer core.deinit();
        const connection: ConnectionId = 2;
        const terminal_id = testId(TerminalId, @intCast(4 + index));
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
        try core.synchronizeGeometry(terminal_id, 132, 24, 9, 18, 1);
        switch (case) {
            .exited => {
                _ = try core.childExited(terminal_id, .{ .exit_code = 0 });
                _ = try core.ptyDrained(terminal_id);
            },
            .failed => {
                _ = try core.runtimeFailed(terminal_id, .read_output);
                _ = try core.failureContained(
                    terminal_id,
                    .{ .observed = .{ .exit_code = 1 } },
                );
            },
        }

        try core.synchronizeGeometry(terminal_id, 132, 24, 9, 18, 1);
        try std.testing.expectError(
            error.InvalidPhase,
            core.synchronizeGeometry(terminal_id, 80, 24, 9, 18, 1),
        );
        try std.testing.expectError(
            error.InvalidPhase,
            core.synchronizeGeometry(terminal_id, 132, 24, 9, 18, 2),
        );
    }
}
test "failed terminal revision accepts only an exact read coherence check" {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = testId(TerminalId, 4);
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
    _ = try createModel(
        &core,
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
