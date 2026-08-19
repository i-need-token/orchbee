const std = @import("std");
const lifecycle_model = @import("terminal-host-model").lifecycle;

test "awaiting-start owns no child lifecycle and starts exactly once" {
    var lifecycle: lifecycle_model.Lifecycle = .{ .awaiting_start = .{} };

    try std.testing.expectEqual(lifecycle_model.Phase.awaiting_start, lifecycle.phase());
    try std.testing.expect(!lifecycle.acceptsRuntimeWork());
    try std.testing.expectEqual(
        lifecycle_model.DisposableAdmission.dispose,
        try lifecycle.requireDisposable(),
    );
    try std.testing.expectEqual(
        lifecycle_model.TransitionOutcome.applied,
        try lifecycle.start(),
    );
    try std.testing.expectEqual(lifecycle_model.Phase.running, lifecycle.phase());
    try std.testing.expectEqual(
        lifecycle_model.TransitionOutcome.idempotent,
        try lifecycle.start(),
    );
}

test "exited and failed outcomes require matching publication before disposal" {
    const cases = [_]lifecycle_model.Lifecycle{
        .{ .exited = .{
            .final_revision = 7,
            .result = .{ .exit_code = 0 },
            .termination = .none,
        } },
        .{ .failed = .{
            .failure = .read_output,
            .final_revision = 9,
            .process = .{ .observed = .{ .exit_code = 1 } },
        } },
    };

    for (cases) |initial| {
        var lifecycle = initial;
        const kind: lifecycle_model.OutcomeKind = switch (lifecycle) {
            .exited => .exited,
            .failed => .failed,
            else => unreachable,
        };
        const mismatched_kind: lifecycle_model.OutcomeKind = switch (kind) {
            .exited => .failed,
            .failed => .exited,
        };

        try std.testing.expect(lifecycle.pendingOutcome() != null);
        try std.testing.expectError(
            error.OutcomePublicationPending,
            lifecycle.requireDisposable(),
        );
        try std.testing.expectEqual(
            lifecycle_model.TransitionOutcome.idempotent,
            lifecycle.markOutcomePublished(mismatched_kind),
        );
        try std.testing.expectError(
            error.OutcomePublicationPending,
            lifecycle.requireDisposable(),
        );
        try std.testing.expectEqual(
            lifecycle_model.TransitionOutcome.applied,
            lifecycle.markOutcomePublished(kind),
        );
        _ = try lifecycle.requireDisposable();
        try std.testing.expectEqual(
            lifecycle_model.TransitionOutcome.idempotent,
            lifecycle.markOutcomePublished(kind),
        );
        try std.testing.expect(lifecycle.pendingOutcome() == null);
    }
}

test "requireDisposable distinguishes stopping from blocked cleanup containment" {
    var lifecycle: lifecycle_model.Lifecycle = .{ .running = .{} };
    _ = lifecycle.runtimeFailed(.read_output);
    try std.testing.expectEqual(
        lifecycle_model.DisposableAdmission.containment_stopping,
        try lifecycle.requireDisposable(),
    );
    _ = lifecycle.containmentBlocked(.release_process_identity, 123) catch unreachable;
    switch (try lifecycle.requireDisposable()) {
        .cleanup_blocked => |failure| try std.testing.expectEqual(
            lifecycle_model.RuntimeCleanupFailure.release_process_identity,
            failure,
        ),
        else => return error.ExpectedCleanupBlockedAdmission,
    }
}

test "retained cleanup advances independently through blocked complete state" {
    var cleanup: lifecycle_model.RetainedCleanup = .inactive;

    try std.testing.expectEqual(
        lifecycle_model.TransitionOutcome.applied,
        try cleanup.block(.contain_process, 123),
    );
    try std.testing.expect(cleanup.pending());
    try std.testing.expectEqual(
        lifecycle_model.RuntimeCleanupFailure.contain_process,
        cleanup.failure().?,
    );
    try std.testing.expectEqual(@as(i128, 123), cleanup.retryAt().?);

    try std.testing.expectEqual(
        lifecycle_model.TransitionOutcome.applied,
        try cleanup.completeCleanup(),
    );
    try std.testing.expect(!cleanup.pending());
    try std.testing.expect(cleanup.failure() == null);
    try std.testing.expect(cleanup.retryAt() == null);
    try std.testing.expectEqual(
        lifecycle_model.TransitionOutcome.idempotent,
        try cleanup.completeCleanup(),
    );
    try std.testing.expectError(
        error.InvalidPhase,
        cleanup.block(.release_output, 456),
    );
}

test "retained cleanup exhaustion preserves exact failure without a retry deadline" {
    var cleanup: lifecycle_model.RetainedCleanup = .inactive;
    _ = try cleanup.block(.release_process_identity, 321);

    try std.testing.expectEqual(
        lifecycle_model.TransitionOutcome.applied,
        try cleanup.exhaust(),
    );
    try std.testing.expect(cleanup.pending());
    try std.testing.expectEqual(
        lifecycle_model.RuntimeCleanupFailure.release_process_identity,
        cleanup.failure().?,
    );
    try std.testing.expect(cleanup.retryAt() == null);
    try std.testing.expectEqual(
        lifecycle_model.TransitionOutcome.idempotent,
        try cleanup.exhaust(),
    );
    try std.testing.expectError(error.InvalidPhase, cleanup.completeCleanup());
}
