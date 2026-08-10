const std = @import("std");
const builtin = @import("builtin");
const benchmark = @import("terminal-host-benchmark");

const buildStartupPlan = benchmark.testing.buildStartupPlanFn;
const display_interval_ns = benchmark.testing.display_interval_ns;
const nextDisplayDeadline = benchmark.testing.nextDisplayDeadlineFn;
const openRegularFixture = benchmark.testing.openRegularFixtureFn;
const validateCompletion = benchmark.testing.validateCompletionFn;
const Completion = benchmark.testing.Completion;
const Config = benchmark.testing.BenchmarkOptionsModule.Config;

test "benchmark startup plan keeps PTY launch and runtime cell pixels identical" {
    const default_plan = buildStartupPlan(.{}, "probe", &.{}, null);
    try std.testing.expectEqual(@as(u16, 9), default_plan.launch.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 9), default_plan.options.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 18), default_plan.launch.cell_pixel_height);
    try std.testing.expectEqual(@as(u16, 18), default_plan.options.cell_pixel_height);
    try std.testing.expectEqual(@as(u16, 120), default_plan.launch.columns);
    try std.testing.expectEqual(@as(u16, 40), default_plan.launch.rows);
    try std.testing.expectEqual(default_plan.launch.cell_pixel_width, default_plan.options.cell_pixel_width);
    try std.testing.expectEqual(default_plan.launch.cell_pixel_height, default_plan.options.cell_pixel_height);

    const explicit_config: Config = .{
        .columns = 200,
        .rows = 60,
        .cell_pixel_width = 12,
        .cell_pixel_height = 24,
    };
    const explicit_plan = buildStartupPlan(explicit_config, "probe", &.{}, null);
    try std.testing.expectEqual(@as(u16, 12), explicit_plan.launch.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 12), explicit_plan.options.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 24), explicit_plan.launch.cell_pixel_height);
    try std.testing.expectEqual(@as(u16, 24), explicit_plan.options.cell_pixel_height);
    try std.testing.expectEqual(@as(u16, 200), explicit_plan.launch.columns);
    try std.testing.expectEqual(@as(u16, 60), explicit_plan.launch.rows);
    try std.testing.expectEqual(explicit_plan.launch.cell_pixel_width, explicit_plan.options.cell_pixel_width);
    try std.testing.expectEqual(explicit_plan.launch.cell_pixel_height, explicit_plan.options.cell_pixel_height);
}

test "benchmark rejects non-regular and empty fixtures before launch" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    try std.testing.expectError(error.InvalidFileFixture, openRegularFixture("/tmp"));
    try std.testing.expectError(error.InvalidFileFixture, openRegularFixture("/dev/zero"));

    var temporary = std.testing.tmpDir(.{});
    defer temporary.cleanup();
    const empty_file = try temporary.dir.createFile("empty", .{});
    empty_file.close();
    const directory_path = try temporary.parent_dir.realpathAlloc(
        std.testing.allocator,
        &temporary.sub_path,
    );
    defer std.testing.allocator.free(directory_path);
    const empty_path = try std.fs.path.join(std.testing.allocator, &.{ directory_path, "empty" });
    defer std.testing.allocator.free(empty_path);
    try std.testing.expectError(error.EmptyFileFixture, openRegularFixture(empty_path));
}

test "benchmark display deadlines do not accumulate capture time" {
    try std.testing.expectEqual(display_interval_ns, nextDisplayDeadline(0, 0));
    try std.testing.expectEqual(display_interval_ns, nextDisplayDeadline(0, display_interval_ns));
    try std.testing.expectEqual(
        2 * display_interval_ns,
        nextDisplayDeadline(0, display_interval_ns + 1),
    );
    try std.testing.expectEqual(
        2 * display_interval_ns,
        nextDisplayDeadline(display_interval_ns, 2 * display_interval_ns),
    );
    try std.testing.expectEqual(
        4 * display_interval_ns,
        nextDisplayDeadline(display_interval_ns, 3 * display_interval_ns + 1),
    );
}

test "benchmark requires a successful child completion" {
    var completion: Completion = .{
        .child_exited = true,
        .classification = .normal,
        .exit_result = .{ .exit_code = 0 },
        .final_revision = 0,
        .pty_drained = true,
        .writer_stopped = true,
    };
    try validateCompletion(completion);

    completion.classification = .{ .failed = .process_observation };
    try std.testing.expectError(error.ProcessObservationFailed, validateCompletion(completion));
    completion.classification = .{ .failed = .pty_read };
    try std.testing.expectError(error.PtyReadFailed, validateCompletion(completion));
    completion.classification = .{ .failed = .pty_write };
    try std.testing.expectError(error.PtyWriteFailed, validateCompletion(completion));
    completion.classification = .{ .failed = .resize };
    try std.testing.expectError(error.TerminalResizeFailed, validateCompletion(completion));
    completion.classification = .{ .failed = .terminal_reply };
    try std.testing.expectError(error.TerminalReplyDropped, validateCompletion(completion));
    completion.classification = .{ .failed = .worker_integrity };
    try std.testing.expectError(error.WorkerIntegrityFailed, validateCompletion(completion));
    completion.classification = .pending;
    try std.testing.expectError(error.RuntimeCompletionPending, validateCompletion(completion));
    completion.classification = .normal;
    completion.exit_result = .{ .exit_code = 1 };
    try std.testing.expectError(error.ChildExitedNonZero, validateCompletion(completion));
    completion.exit_result = .{ .signal = 9 };
    try std.testing.expectError(error.ChildTerminatedBySignal, validateCompletion(completion));
    completion.exit_result = .{ .unknown = 0 };
    try std.testing.expectError(error.ChildExitUnknown, validateCompletion(completion));
    completion.exit_result = null;
    try std.testing.expectError(error.MissingExitResult, validateCompletion(completion));
}
