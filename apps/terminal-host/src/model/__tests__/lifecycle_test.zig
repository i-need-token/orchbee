const std = @import("std");
const lifecycle_model = @import("terminal-host-model").lifecycle;

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
