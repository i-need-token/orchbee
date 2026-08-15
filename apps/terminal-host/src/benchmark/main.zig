const builtin = @import("builtin");
const std = @import("std");

const benchmark_options = @import("options.zig");
const identifiers = @import("terminal-host-model").identifiers;
const terminal_runtime = @import("terminal-host-engine").runtime;

const Config = benchmark_options.Config;
const display_interval_nanoseconds: u64 = 16 * std.time.ns_per_ms;

const FixtureInput = struct {
    byte_count: u64,
    file: std.fs.File,
};

const Usage = struct {
    cpu_microseconds: ?u64,
    peak_rss_bytes: ?u64,
};

pub fn main() !void {
    const allocator = std.heap.smp_allocator;
    const arguments = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, arguments);
    if (arguments.len == 0) return error.InvalidArguments;
    switch (try benchmark_options.parse(allocator, arguments[1..])) {
        .emit => |byte_count| try emitGeneratedFixture(byte_count),
        .benchmark => |config| try runBenchmark(allocator, config),
    }
}

fn emitGeneratedFixture(byte_count: u64) !void {
    var output_buffer: [64 * 1024]u8 = undefined;
    for (&output_buffer, 0..) |*byte, index| {
        byte.* = if (index % 80 == 79) '\n' else @as(u8, 'a') + @as(u8, @intCast(index % 26));
    }
    var writer_buffer: [64 * 1024]u8 = undefined;
    var file_writer = std.fs.File.stdout().writerStreaming(&writer_buffer);
    var remaining = byte_count;
    while (remaining > 0) {
        const length: usize = @intCast(@min(remaining, output_buffer.len));
        try file_writer.interface.writeAll(output_buffer[0..length]);
        remaining -= length;
    }
    try file_writer.interface.flush();
}

pub const StartupPlan = struct {
    launch: terminal_runtime.ProcessLaunch,
    options: terminal_runtime.Options,
};

/// The benchmark carries one canonical geometry authority. The PTY launch and
/// the runtime options both receive the requested cell-pixel dimensions so the
/// reported geometry and the initial PTY winsize agree.
fn buildStartupPlan(
    config: Config,
    executable: []const u8,
    arguments: []const []const u8,
    stdin_fd: ?isize,
) StartupPlan {
    return .{
        .launch = .{
            .arguments = arguments,
            .cell_pixel_height = config.cell_pixel_height,
            .cell_pixel_width = config.cell_pixel_width,
            .columns = config.columns,
            .current_directory = null,
            .environment = &.{},
            .executable = executable,
            .rows = config.rows,
            .stdin_fd = stdin_fd,
        },
        .options = .{
            .cell_pixel_height = config.cell_pixel_height,
            .cell_pixel_width = config.cell_pixel_width,
        },
    };
}

fn runBenchmark(allocator: std.mem.Allocator, config: Config) !void {
    if (config.file_path != null and builtin.os.tag == .windows) {
        return error.FileFixtureUnsupported;
    }
    if (config.file_path != null) {
        try requireFileFixtureUtilities(allocator);
    }
    const executable = try std.fs.selfExePathAlloc(allocator);
    defer allocator.free(executable);
    var fixture_input: ?std.fs.File = null;
    defer if (fixture_input) |input| input.close();
    const byte_count = if (config.file_path) |file_path| block: {
        const fixture = try openRegularFixture(file_path);
        fixture_input = fixture.file;
        break :block fixture.byte_count;
    } else config.bytes;
    const byte_text = try std.fmt.allocPrint(allocator, "{d}", .{byte_count});
    defer allocator.free(byte_text);
    const generated_arguments = [_][]const u8{ "--emit", byte_text };
    const file_arguments = [_][]const u8{ "-p", "cat", "-n" };
    const child_executable = if (config.file_path == null) executable else "time";
    const child_arguments: []const []const u8 = if (config.file_path == null)
        &generated_arguments
    else
        &file_arguments;
    const usage_before = processUsage();
    var timer = try std.time.Timer.start();
    const stdin_fd: ?isize = if (builtin.os.tag == .windows)
        null
    else if (fixture_input) |input|
        @intCast(input.handle)
    else
        null;
    const plan = buildStartupPlan(config, child_executable, child_arguments, stdin_fd);
    const runtime = try terminal_runtime.TerminalRuntime.createWithOptions(allocator, plan.launch, plan.options);
    defer runtime.destroy();

    var frames: u64 = 0;
    if (config.mode == .no_demand) {
        _ = try runtime.waitForCompletion();
        try runtime.shutdown();
    } else {
        var acknowledged_generation: ?u64 = null;
        var acknowledged_revision: ?u64 = null;
        var next_display_deadline = timer.read();
        while (true) {
            const completion = runtime.completion();
            try validateRuntimeHealth(completion);
            const geometry = runtime.geometry();
            const generation_changed = acknowledged_generation != geometry.generation;
            if (acknowledged_revision == null or generation_changed or
                acknowledged_revision.? < geometry.revision)
            {
                const frame = try runtime.encodeProjectionFrame(.{
                    .attachment_id = identifiers.AttachmentId.fromBytes([_]u8{0x22} ** 16),
                    .base_generation = acknowledged_generation,
                    .base_revision = acknowledged_revision,
                    .force_snapshot = acknowledged_revision == null or generation_changed,
                    .host_epoch = identifiers.HostEpoch.fromBytes([_]u8{0x11} ** 16),
                    .maximum_bytes = 8 * 1024 * 1024,
                    .sequence = frames + 1,
                });
                acknowledged_generation = frame.generation;
                acknowledged_revision = frame.revision;
                frames += 1;
                allocator.free(frame.allocation);
            }
            if (activePollDone(completion, acknowledged_revision)) break;
            next_display_deadline = nextDisplayDeadline(
                next_display_deadline,
                timer.read(),
            );
            sleepUntil(&timer, next_display_deadline);
        }
        try runtime.shutdown();
    }

    const elapsed = timer.read();
    const completion = runtime.completion();
    try validateCompletion(completion);
    const counters = runtime.projectionCounters();
    const output_batches = runtime.takeOutputBatches();
    const usage_after = processUsage();
    const seconds = @as(f64, @floatFromInt(elapsed)) / std.time.ns_per_s;
    const mib_per_second = @as(f64, @floatFromInt(byte_count)) / (1024 * 1024) / seconds;
    const cpu_delta = optionalDelta(usage_after.cpu_microseconds, usage_before.cpu_microseconds);
    const rss_delta = optionalDelta(usage_after.peak_rss_bytes, usage_before.peak_rss_bytes);

    var output_buffer: [4096]u8 = undefined;
    var file_writer = std.fs.File.stdout().writerStreaming(&output_buffer);
    try std.json.Stringify.value(.{
        .bytes = byte_count,
        .captures = counters.captures,
        .cellPixelHeight = config.cell_pixel_height,
        .cellPixelWidth = config.cell_pixel_width,
        .columns = config.columns,
        .cpuMicroseconds = cpu_delta,
        .encodes = counters.encodes,
        .finalRevision = completion.final_revision,
        .frames = frames,
        .fixture = if (config.file_path == null) "generated" else "cat-numbered-file",
        .mibPerSecond = mib_per_second,
        .mode = if (config.mode == .no_demand) "no-demand" else "active",
        .outputBatches = output_batches,
        .peakRssDeltaBytes = rss_delta,
        .renderStateCreations = counters.render_state_creations,
        .rows = config.rows,
        .wallNanoseconds = elapsed,
    }, .{}, &file_writer.interface);
    try file_writer.interface.writeByte('\n');
    try file_writer.interface.flush();
}

fn nextDisplayDeadline(previous_deadline: u64, now: u64) u64 {
    const candidate = previous_deadline +| display_interval_nanoseconds;
    if (candidate >= now) return candidate;
    const skipped_intervals = (now - candidate) / display_interval_nanoseconds + 1;
    return candidate +| skipped_intervals *| display_interval_nanoseconds;
}

fn sleepUntil(timer: *std.time.Timer, deadline: u64) void {
    while (true) {
        const now = timer.read();
        if (now >= deadline) return;
        std.Thread.sleep(deadline - now);
    }
}

fn openRegularFixture(path: []const u8) !FixtureInput {
    if (builtin.os.tag == .windows) return error.FileFixtureUnsupported;
    const fd = try std.posix.open(path, .{
        .ACCMODE = .RDONLY,
        .CLOEXEC = true,
        .NONBLOCK = true,
    }, 0);
    var input: std.fs.File = .{ .handle = fd };
    errdefer input.close();
    const status = try input.stat();
    if (status.kind != .file) return error.InvalidFileFixture;
    if (status.size == 0) return error.EmptyFileFixture;
    return .{ .byte_count = status.size, .file = input };
}

/// POSIX fallback search path when the benchmark's own environment carries no
/// PATH. Mirrors the runtime platform's POSIX default so the pre-launch check
/// agrees with the child's actual executable resolution.
const default_utility_search_path = "/usr/local/bin:/bin/:/usr/bin";

/// True when `utility` resolves to an executable on the given PATH. Empty
/// PATH components resolve against the current directory, matching the runtime
/// platform's effective-PATH semantics.
fn executableOnPath(path: []const u8, utility: []const u8) bool {
    var iterator = std.mem.splitScalar(u8, path, ':');
    while (iterator.next()) |directory| {
        const prefix = if (directory.len == 0) "." else directory;
        const separator = if (std.mem.endsWith(u8, prefix, "/")) "" else "/";
        var buffer: [std.posix.PATH_MAX]u8 = undefined;
        const candidate = std.fmt.bufPrint(&buffer, "{s}{s}{s}", .{ prefix, separator, utility }) catch continue;
        if (std.posix.access(candidate, std.posix.X_OK)) |_| {
            return true;
        } else |_| {}
    }
    return false;
}

/// The file-fixture workload launches `time -p cat -n` resolved through the
/// child's effective PATH rather than fixed FHS paths. Both utilities must be
/// reachable on that PATH before any process or PTY effect so an unavailable
/// utility fails with a clear, deterministic error.
fn requireFileFixtureUtilitiesOnPath(path: []const u8) !void {
    if (!executableOnPath(path, "time")) return error.BenchmarkUtilityUnavailable;
    if (!executableOnPath(path, "cat")) return error.BenchmarkUtilityUnavailable;
}

fn requireFileFixtureUtilities(allocator: std.mem.Allocator) !void {
    if (builtin.os.tag == .windows) return;
    var environment = try std.process.getEnvMap(allocator);
    defer environment.deinit();
    const path = environment.get("PATH") orelse default_utility_search_path;
    try requireFileFixtureUtilitiesOnPath(path);
}

/// Live runtime health while the active loop is still polling. `.pending` is
/// the runtime's ordinary in-progress state (gated workers and the child are
/// still settling) and continues polling; only a classified failure ends the
/// benchmark.
fn validateRuntimeHealth(completion: terminal_runtime.Completion) !void {
    return switch (completion.classification) {
        .pending, .normal => {},
        .failed => |failure| switch (failure) {
            .process_observation => error.ProcessObservationFailed,
            .pty_read => error.PtyReadFailed,
            .pty_write => error.PtyWriteFailed,
            .resize => error.TerminalResizeFailed,
            .terminal_reply => error.TerminalReplyDropped,
            .worker_integrity => error.WorkerIntegrityFailed,
        },
    };
}

/// The active loop is complete only once the runtime has classified normal
/// (child exit, output drain, and writer quiescence are all guaranteed) and
/// the acknowledged projection revision has caught up to the final revision.
/// Pending is the ordinary live state and continues; failed is surfaced by
/// live health validation before this decision is reached.
fn activePollDone(completion: terminal_runtime.Completion, acknowledged_revision: ?u64) bool {
    return switch (completion.classification) {
        .pending, .failed => false,
        .normal => acknowledged_revision != null and acknowledged_revision.? >= completion.final_revision,
    };
}

/// Final completion validation after the benchmark loop has settled. Only a
/// normal classification with a successful child exit is accepted; pending is
/// rejected here because the loop has already decided the runtime is done.
fn validateCompletion(completion: terminal_runtime.Completion) !void {
    try validateRuntimeHealth(completion);
    switch (completion.classification) {
        .pending => return error.RuntimeCompletionPending,
        .normal => {},
        // validateRuntimeHealth already surfaced every failed classification.
        .failed => unreachable,
    }
    const exit_result = completion.exit_result orelse return error.MissingExitResult;
    switch (exit_result) {
        .exit_code => |code| if (code != 0) return error.ChildExitedNonZero,
        .signal => return error.ChildTerminatedBySignal,
        .unknown => return error.ChildExitUnknown,
    }
}

fn processUsage() Usage {
    if (builtin.os.tag != .linux and builtin.os.tag != .macos) {
        return .{ .cpu_microseconds = null, .peak_rss_bytes = null };
    }
    const usage = std.posix.getrusage(std.posix.rusage.SELF);
    const user_microseconds = @as(u64, @intCast(usage.utime.sec)) * std.time.us_per_s +
        @as(u64, @intCast(usage.utime.usec));
    const system_microseconds = @as(u64, @intCast(usage.stime.sec)) * std.time.us_per_s +
        @as(u64, @intCast(usage.stime.usec));
    const peak_rss: u64 = @intCast(usage.maxrss);
    return .{
        .cpu_microseconds = user_microseconds + system_microseconds,
        .peak_rss_bytes = if (builtin.os.tag == .linux) peak_rss * 1024 else peak_rss,
    };
}

fn optionalDelta(after: ?u64, before: ?u64) ?u64 {
    if (after == null or before == null) return null;
    return after.? -| before.?;
}

pub const testing = if (builtin.is_test) struct {
    pub const activePollDoneFn = activePollDone;
    pub const BenchmarkOptionsModule = benchmark_options;
    pub const buildStartupPlanFn = buildStartupPlan;
    pub const Completion = terminal_runtime.Completion;
    pub const display_interval_ns = display_interval_nanoseconds;
    pub const executableOnPathFn = executableOnPath;
    pub const nextDisplayDeadlineFn = nextDisplayDeadline;
    pub const openRegularFixtureFn = openRegularFixture;
    pub const requireFileFixtureUtilitiesOnPathFn = requireFileFixtureUtilitiesOnPath;
    pub const validateCompletionFn = validateCompletion;
    pub const validateRuntimeHealthFn = validateRuntimeHealth;
} else struct {};
