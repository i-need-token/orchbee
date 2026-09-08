const harness = @import("runtime_harness.zig");
const std = harness.std;
const target_builtin = harness.target_builtin;
const ghostty_vt = harness.ghostty_vt;
const engine_boundary = harness.engine_boundary;
const model_boundary = harness.model_boundary;
const platform_boundary = harness.platform_boundary;
const posix_compat = platform_boundary.posix_compat;
const protocol = harness.protocol;
const semantic_wait = harness.semantic_wait;
const subject = harness.subject;
const terminal_launch = harness.terminal_launch;
const pty_process = harness.pty_process;
const applyScrollbackBytes = harness.applyScrollbackBytes;
const cancelInput = harness.cancelInput;
const clearTestingHooks = harness.clearTestingHooks;
const closeClientInput = harness.closeClientInput;
const commitReaderBatch = harness.commitReaderBatch;
const completionClassificationLoad = harness.completionClassificationLoad;
const createWithOptionsUsingSpawner = harness.createWithOptionsUsingSpawner;
const deinitOwnedProfile = harness.deinitOwnedProfile;
const FirstContainmentAttempt = harness.FirstContainmentAttempt;
const InputReservation = harness.InputReservation;
const InputSource = harness.InputSource;
const initOwnedProfile = harness.initOwnedProfile;
const inputQueueSnapshot = harness.inputQueueSnapshot;
const modifiers = harness.modifiers;
const mouseEncodeEvent = harness.mouseEncodeEvent;
const mouseHasPressedButton = harness.mouseHasPressedButton;
const markNormalCompletionReady = harness.markNormalCompletionReady;
const ProjectionEncodeRequest = harness.ProjectionEncodeRequest;
const AttachmentId = harness.AttachmentId;
const HostEpoch = harness.HostEpoch;
const reserveInput = harness.reserveInput;
const TerminalRuntime = harness.TerminalRuntime;
const runtimeState = harness.runtimeState;
const max_pending_input_bytes = harness.max_pending_input_bytes;
const max_pending_input_entries = harness.max_pending_input_entries;
const max_pending_terminal_reply_bytes = harness.max_pending_terminal_reply_bytes;
const max_writer_slice_bytes = harness.max_writer_slice_bytes;
const terminal_reply_burst_bound = harness.terminal_reply_burst_bound;
const publishInput = harness.publishInput;
const recordReaderFailure = harness.recordReaderFailure;
const recordTerminalReplyFailure = harness.recordTerminalReplyFailure;
const recordWorkerIntegrityFailure = harness.recordWorkerIntegrityFailure;
const recordWriterFailure = harness.recordWriterFailure;
const releaseOutputForShutdown = harness.releaseOutputForShutdown;
const ReaderBatchOutcome = harness.ReaderBatchOutcome;
const resizePtyWithRollback = harness.resizePtyWithRollback;
const RuntimeTestingHooks = harness.RuntimeTestingHooks;
const Geometry = harness.Geometry;
const requireProjectionFrame = harness.requireProjectionFrame;
const TestGate = harness.TestGate;
const TestSignal = harness.TestSignal;
const exerciseOwnedProfileAllocations = harness.exerciseOwnedProfileAllocations;
const FailingThreadSpawner = harness.FailingThreadSpawner;
const CountingThreadSpawner = harness.CountingThreadSpawner;
const HookedThreadSpawner = harness.HookedThreadSpawner;
const createRuntimeWithHooks = harness.createRuntimeWithHooks;
const createRuntimeWithHooksAndCommand = harness.createRuntimeWithHooksAndCommand;
const waitForWriterStop = harness.waitForWriterStop;
const createIdleRuntime = harness.createIdleRuntime;
const createIdleRuntimeWithGeometry = harness.createIdleRuntimeWithGeometry;
const parseTerminalInput = harness.parseTerminalInput;
const WriterPause = harness.WriterPause;
const WriterProbe = harness.WriterProbe;
const RepeatingGate = harness.RepeatingGate;
const BurstProbe = harness.BurstProbe;
const DescendantProbe = harness.DescendantProbe;
const ReaderProbe = harness.ReaderProbe;
const CanonicalSnapshot = harness.CanonicalSnapshot;
const canonicalSnapshot = harness.canonicalSnapshot;
const expectCanonicalSnapshotEqual = harness.expectCanonicalSnapshotEqual;
const readFrameInt = harness.readFrameInt;
const projectionFrameRowOffset = harness.projectionFrameRowOffset;
const expectIncludedProjectionRows = harness.expectIncludedProjectionRows;
const FrameHyperlinkObservation = harness.FrameHyperlinkObservation;
const observeFrameHyperlinks = harness.observeFrameHyperlinks;
const writeProjectionOutput = harness.writeProjectionOutput;
const expectProjectionRowRevisions = harness.expectProjectionRowRevisions;
const expectProjectionLinkTargets = harness.expectProjectionLinkTargets;
const expectInputQueuesCleared = harness.expectInputQueuesCleared;
const ContainmentProbe = harness.ContainmentProbe;
const FailureReporter = harness.FailureReporter;
const ShutdownProbe = harness.ShutdownProbe;
const OutputCleanupProbe = harness.OutputCleanupProbe;
const ResizeEffectOutcome = harness.ResizeEffectOutcome;
const ResizeProbe = harness.ResizeProbe;
const ResizeObservation = harness.ResizeObservation;
const TerminalDrivenResizeProbe = harness.TerminalDrivenResizeProbe;
const TerminalResizeSerializationProbe = harness.TerminalResizeSerializationProbe;
const ResizeCall = harness.ResizeCall;
const ReaderBatchCall = harness.ReaderBatchCall;
const expectRealPtyQueryReplies = harness.expectRealPtyQueryReplies;
const driveReplyBurstUntilClient = harness.driveReplyBurstUntilClient;
const parseDescendantPid = harness.parseDescendantPid;
const procCommEquals = harness.procCommEquals;
const processAlive = harness.processAlive;

test "raw runtime creation rejects invalid launches before any thread or process effect" {
    const many_arguments = try std.testing.allocator.alloc([]const u8, 4097);
    defer std.testing.allocator.free(many_arguments);
    @memset(many_arguments, "");
    var many_environment: [1025]pty_process.EnvironmentDelta = undefined;
    @memset(&many_environment, .{ .name = "ORCHBEE_BOUND", .value = null });

    const duplicate_names = if (target_builtin.os.tag == .windows)
        [_]pty_process.EnvironmentDelta{
            .{ .name = "Path", .value = "first" },
            .{ .name = "PATH", .value = "second" },
        }
    else
        [_]pty_process.EnvironmentDelta{
            .{ .name = "ORCHBEE_DUPLICATE", .value = "first" },
            .{ .name = "ORCHBEE_DUPLICATE", .value = "second" },
        };

    const InvalidCase = struct {
        expected_error: anyerror,
        launch: pty_process.Launch,
        options: subject.Options,
    };
    const cases = [_]InvalidCase{
        .{ // NUL in the executable: the base runtime accepted and spawned it
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = null,
                .environment = &.{},
                .executable = "probe\x00name",
                .rows = 24,
            },
            .options = .{},
        },
        .{ // columns exceed the protocol frame bound
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 4097,
                .current_directory = null,
                .environment = &.{},
                .executable = "probe",
                .rows = 1,
            },
            .options = .{},
        },
        .{ // cells exceed the protocol frame bound
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 4096,
                .current_directory = null,
                .environment = &.{},
                .executable = "probe",
                .rows = 65,
            },
            .options = .{},
        },
        .{ // pixel extents exceed the unsigned 16-bit PTY limit
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 4096,
                .current_directory = null,
                .environment = &.{},
                .executable = "probe",
                .rows = 1,
            },
            .options = .{ .cell_pixel_width = 16 },
        },
        .{ // scrollback exceeds the configuration maximum
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = null,
                .environment = &.{},
                .executable = "probe",
                .rows = 24,
            },
            .options = .{ .scrollback_bytes = 1_000_000_001 },
        },
        .{ // NUL in an argument
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{"argument\x00with-nul"},
                .columns = 80,
                .current_directory = null,
                .environment = &.{},
                .executable = "probe",
                .rows = 24,
            },
            .options = .{},
        },
        .{ // NUL in the current directory
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = "/tmp/orchbee\x00fixture",
                .environment = &.{},
                .executable = "probe",
                .rows = 24,
            },
            .options = .{},
        },
        .{ // NUL in an environment name
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = null,
                .environment = &.{.{
                    .name = "ORCHBEE\x00NAME",
                    .value = null,
                }},
                .executable = "probe",
                .rows = 24,
            },
            .options = .{},
        },
        .{ // NUL in an environment value
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = null,
                .environment = &.{.{
                    .name = "ORCHBEE_VALUE",
                    .value = "bad\x00value",
                }},
                .executable = "probe",
                .rows = 24,
            },
            .options = .{},
        },
        .{ // argument count exceeds the protocol launch bound
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = many_arguments,
                .columns = 80,
                .current_directory = null,
                .environment = &.{},
                .executable = "probe",
                .rows = 24,
            },
            .options = .{},
        },
        .{ // environment entry count exceeds the protocol launch bound
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = null,
                .environment = &many_environment,
                .executable = "probe",
                .rows = 24,
            },
            .options = .{},
        },
        .{ // duplicate environment names under the target operating system identity
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = null,
                .environment = &duplicate_names,
                .executable = "probe",
                .rows = 24,
            },
            .options = .{},
        },
        .{ // launch and options disagree on the cell-pixel width authority
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .cell_pixel_width = 9,
                .cell_pixel_height = 18,
                .columns = 80,
                .current_directory = null,
                .environment = &.{},
                .executable = "probe",
                .rows = 24,
            },
            .options = .{ .cell_pixel_width = 10 },
        },
    };

    for (cases) |case| {
        var spawner: CountingThreadSpawner = .{};
        try std.testing.expectError(
            case.expected_error,
            createWithOptionsUsingSpawner(
                std.testing.allocator,
                case.launch,
                case.options,
                &spawner,
            ),
        );
        try std.testing.expectEqual(@as(usize, 0), spawner.attempts);
    }
}
test "raw runtime creation accepts the exact canonical maxima and reaches the process spawn seam" {
    var spawner: FailingThreadSpawner = .{ .fail_at = 4 };
    // Maximum grid columns, exactly 262144 cells, and a cell-pixel geometry
    // whose total extents stay within the unsigned 16-bit PTY limit.
    try std.testing.expectError(
        error.SpawnFailed,
        createWithOptionsUsingSpawner(std.testing.allocator, .{
            .arguments = &.{},
            .cell_pixel_width = 1,
            .cell_pixel_height = 1,
            .columns = 4096,
            .current_directory = null,
            .environment = &.{},
            .executable = "max-geometry-must-reach-process-spawn",
            .rows = 64,
        }, .{}, &spawner),
    );
    try std.testing.expectEqual(@as(usize, 4), spawner.attempts);
}
test "POSIX PTY child reads TIOCGWINSZ matching the canonical launch geometry" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    const environment = [_]terminal_launch.EnvironmentEntry{};
    const features = [_]u16{22};
    const palette = [_]u32{0} ** 256;
    const canonical_launch: terminal_launch.Launch = .{
        .arguments = &.{},
        .configuration = null,
        .current_directory = null,
        .environment = &environment,
        .executable = "probe",
        .geometry = .{
            .cell_pixel_height = 18,
            .cell_pixel_width = 9,
            .columns = 120,
            .rows = 40,
        },
        .profile = .{
            .color_scheme = .dark,
            .default_background_rgb = 0,
            .default_cursor_rgb = null,
            .default_foreground_rgb = 0xffffff,
            .device_primary_conformance_level = 62,
            .device_primary_features = &features,
            .device_secondary_type = 1,
            .device_secondary_firmware_version = 0,
            .device_secondary_rom_cartridge = 0,
            .device_tertiary_unit_id = 0,
            .enquiry_response = "",
            .palette = &palette,
            .version_response = "Orchbee test",
        },
    };
    const validated = try terminal_launch.inspect(std.testing.allocator, canonical_launch);
    var prepared = try terminal_launch.prepareRuntime(validated, std.testing.allocator, .{});
    defer prepared.deinit(std.testing.allocator);
    const plan = prepared.processLaunch(validated.launch.geometry.?);

    const posix = std.posix;
    var pty = try platform_boundary.testing.pty.Pty.open(.{
        .columns = plan.columns,
        .pixel_height = plan.rows * plan.cell_pixel_height,
        .pixel_width = plan.columns * plan.cell_pixel_width,
        .rows = plan.rows,
    });
    defer pty.deinit();
    try pty.setSize(.{
        .columns = plan.columns,
        .pixel_height = plan.rows * plan.cell_pixel_height,
        .pixel_width = plan.columns * plan.cell_pixel_width,
        .rows = plan.rows,
    });

    const TIOCGWINSZ: c_int = comptime if (target_builtin.os.tag == .windows)
        0
    else switch (target_builtin.os.tag) {
        .macos, .freebsd => 0x40087468,
        .linux => 0x5413,
        else => @compileError("TIOCGWINSZ is not defined for this POSIX target"),
    };
    const report = try posix_compat.pipe2(.{ .CLOEXEC = true });
    defer posix_compat.close(report[0]);
    const child_id = try posix_compat.fork();
    if (child_id == 0) {
        posix_compat.close(report[0]);
        pty.childSetup() catch {
            _ = posix_compat.write(report[1], &.{0}) catch 0;
            posix_compat.exit(1);
        };
        var winsize: posix.winsize = undefined;
        if (std.c.ioctl(posix.STDIN_FILENO, TIOCGWINSZ, @intFromPtr(&winsize)) < 0) {
            _ = posix_compat.write(report[1], &.{0}) catch 0;
            posix_compat.exit(1);
        }
        var bytes: [8]u8 = undefined;
        std.mem.writeInt(u16, bytes[0..2], winsize.row, .little);
        std.mem.writeInt(u16, bytes[2..4], winsize.col, .little);
        std.mem.writeInt(u16, bytes[4..6], winsize.xpixel, .little);
        std.mem.writeInt(u16, bytes[6..8], winsize.ypixel, .little);
        _ = posix_compat.write(report[1], &bytes) catch 0;
        posix_compat.exit(0);
    }
    posix_compat.close(report[1]);
    var result: [8]u8 = undefined;
    var received: usize = 0;
    while (received < result.len) {
        const length = try posix.read(report[0], result[received..]);
        if (length == 0) return error.ChildClosedReportPipe;
        received += length;
    }
    _ = posix_compat.waitpid(child_id, 0);
    const row = std.mem.readInt(u16, result[0..2], .little);
    const column = std.mem.readInt(u16, result[2..4], .little);
    const xpixel = std.mem.readInt(u16, result[4..6], .little);
    const ypixel = std.mem.readInt(u16, result[6..8], .little);
    try std.testing.expectEqual(plan.rows, row);
    try std.testing.expectEqual(plan.columns, column);
    try std.testing.expectEqual(@as(u16, plan.columns * plan.cell_pixel_width), xpixel);
    try std.testing.expectEqual(@as(u16, plan.rows * plan.cell_pixel_height), ypixel);
}
test "Linux session descendant with a forged newline comm stays contained until shutdown kills it (THCR-028)" {
    const builtin = @import("builtin");
    if (builtin.os.tag != .linux) return error.SkipZigTest;

    // The leader forks one background subshell that closes its PTY descriptors,
    // writes the forged "worker) Z\nX" main-thread comm (the embedded newline is
    // not trailing, so /proc/self/comm keeps it), and blocks forever reading
    // /dev/zero; the leader echoes the descendant PID and exits. Ignore HUP
    // before forking so the descendant survives the session leader's exit.
    // On the pre-fix parser the single fgets line reads as a non-live 'Z'.
    const script =
        "trap '' HUP; " ++
        "( printf 'worker) Z\\nX' >/proc/self/comm; printf 'ORCHBEE-FORGED\\n'; exec 0</dev/zero 1>&- 2>&-; read x ) & " ++
        "printf 'ORCHBEE-DESC %s\\n' \"$!\"; exit 0";
    const arguments = [_][]const u8{ "-c", script };
    const runtime = try TerminalRuntime.createWithOptions(std.testing.allocator, .{
        .arguments = &arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = "/bin/sh",
        .rows = 24,
    }, .{});
    defer runtime.destroy();

    const completion = try runtime.waitForCompletion();
    try std.testing.expect(completion.pty_drained);

    var snapshot = try canonicalSnapshot(runtime);
    defer snapshot.deinit();
    const descendant_pid = parseDescendantPid(snapshot.text) orelse
        return error.DescendantPidMissing;
    // Best-effort cleanup: on the pre-fix parser containment is spoof-confirmed,
    // so a failed shutdown would leave the descendant running. Ensure it dies.
    defer {
        if (processAlive(descendant_pid)) {
            std.posix.kill(descendant_pid, std.posix.SIG.KILL) catch {};
        }
    }

    // The descendant emits this marker only after writing the forged comm and
    // before closing its PTY descriptors. PTY drain therefore proves the
    // external setup completed without polling /proc scheduling.
    try std.testing.expect(std.mem.indexOf(u8, snapshot.text, "ORCHBEE-FORGED") != null);
    try std.testing.expect(procCommEquals(descendant_pid, "worker) Z\nX"));

    // The descendant is a live, uncontained session member: containment must
    // not be confirmed and the leader identity must remain retained.
    try std.testing.expect(processAlive(descendant_pid));
    try std.testing.expect(!runtimeState(runtime).containment_confirmed.load(.acquire));
    try std.testing.expect(!try runtimeState(runtime).process.processTreeContained());
    try std.testing.expect(!runtimeState(runtime).process.identityReleased());

    // Shutdown force-terminates the session, confirms containment, and only
    // then releases the retained leader identity.
    try runtime.shutdown();
    try std.testing.expect(runtimeState(runtime).containment_confirmed.load(.acquire));
    try std.testing.expect(runtimeState(runtime).process.identityReleased());
    try std.testing.expect(try runtimeState(runtime).process.processTreeContained());

    // processTreeContained is the owned semantic check: a merely stopped
    // descendant remains live and would keep the result false. Host tests do
    // not wait for the unrelated timing of init reaping an exited descendant.
}
