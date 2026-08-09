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
        try lifecycle.requireDisposable();
        try std.testing.expectEqual(
            lifecycle_model.TransitionOutcome.idempotent,
            lifecycle.markOutcomePublished(kind),
        );
        try std.testing.expect(lifecycle.pendingOutcome() == null);
    }
}
