const harness = @import("runtime_harness.zig");
const std = harness.std;
const target_builtin = harness.target_builtin;
const ghostty_vt = harness.ghostty_vt;
const engine_boundary = harness.engine_boundary;
const model_boundary = harness.model_boundary;
const platform_boundary = harness.platform_boundary;
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
const createImmediateExitRuntime = harness.createImmediateExitRuntime;
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

fn expectAttachmentPreparationUnavailable(
    runtime: *TerminalRuntime,
    attachment_id: AttachmentId,
    capabilities: subject.Capabilities,
) !void {
    var preparation = runtime.prepareAttachment(attachment_id, capabilities) catch |err| {
        try std.testing.expectEqual(error.TerminalRuntimeUnavailable, err);
        return;
    };
    defer preparation.deinit();
    return error.ExpectedAttachmentPreparationFailure;
}

test "platform resize errors restore committed geometry or report an unsafe rollback" {
    const FakePty = struct {
        call_count: usize = 0,
        calls: [2]Geometry = undefined,
        rollback_fails: bool,

        pub fn resizePixels(
            self: *@This(),
            columns: u16,
            rows: u16,
            cell_pixel_width: u16,
            cell_pixel_height: u16,
        ) !void {
            self.calls[self.call_count] = .{
                .cell_pixel_height = cell_pixel_height,
                .cell_pixel_width = cell_pixel_width,
                .columns = columns,
                .rows = rows,
            };
            self.call_count += 1;
            if (self.call_count == 1) return error.PlatformResizeFailed;
            if (self.rollback_fails) return error.PlatformRollbackFailed;
        }
    };
    const previous: Geometry = .{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
        .columns = 80,
        .rows = 24,
    };
    const next: Geometry = .{
        .cell_pixel_height = 18,
        .cell_pixel_width = 9,
        .columns = 120,
        .rows = 40,
    };

    var recovered: FakePty = .{ .rollback_fails = false };
    try std.testing.expectError(
        error.PlatformResizeFailed,
        resizePtyWithRollback(&recovered, previous, next),
    );
    try std.testing.expectEqual(@as(usize, 2), recovered.call_count);
    try std.testing.expectEqualDeep(next, recovered.calls[0]);
    try std.testing.expectEqualDeep(previous, recovered.calls[1]);

    var unsafe: FakePty = .{ .rollback_fails = true };
    try std.testing.expectError(
        error.ResizeRollbackFailed,
        resizePtyWithRollback(&unsafe, previous, next),
    );
    try std.testing.expectEqual(@as(usize, 2), unsafe.call_count);
    try std.testing.expectEqualDeep(next, unsafe.calls[0]);
    try std.testing.expectEqualDeep(previous, unsafe.calls[1]);
}
test "semantic modifiers map every named field into Ghostty" {
    const mapped = modifiers(.{
        .shift = true,
        .control = true,
        .alt = true,
        .meta = true,
        .caps_lock = true,
        .num_lock = true,
        .shift_right = true,
        .control_right = true,
        .alt_right = true,
        .meta_right = true,
    });

    try std.testing.expect(mapped.shift);
    try std.testing.expect(mapped.ctrl);
    try std.testing.expect(mapped.alt);
    try std.testing.expect(mapped.super);
    try std.testing.expect(mapped.caps_lock);
    try std.testing.expect(mapped.num_lock);
    try std.testing.expectEqual(ghostty_vt.input.KeyMods.Side{
        .shift = .right,
        .ctrl = .right,
        .alt = .right,
        .super = .right,
    }, mapped.sides);
}
test "mouse drag preserves its active encoder button" {
    const input: subject.MouseInput = .{
        .action = .{ .motion = .{ .drag = .left } },
        .modifiers = .{},
        .pressed_buttons = .{ .left = true },
        .x = 10,
        .y = 20,
    };

    const encoded = mouseEncodeEvent(input);
    try std.testing.expectEqual(ghostty_vt.input.MouseAction.motion, encoded.action);
    try std.testing.expectEqual(ghostty_vt.input.MouseButton.left, encoded.button.?);
    try std.testing.expect(mouseHasPressedButton(input));
}
test "mouse hover encodes without an active button" {
    const input: subject.MouseInput = .{
        .action = .{ .motion = .hover },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = 10,
        .y = 20,
    };

    const encoded = mouseEncodeEvent(input);
    try std.testing.expectEqual(ghostty_vt.input.MouseAction.motion, encoded.action);
    try std.testing.expectEqual(@as(?ghostty_vt.input.MouseButton, null), encoded.button);
    try std.testing.expect(!mouseHasPressedButton(input));
}
test "mouse press counts as pressed for encoder options before persistent state changes" {
    const input: subject.MouseInput = .{
        .action = .{ .press = .four },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = 10,
        .y = 20,
    };

    try std.testing.expect(mouseHasPressedButton(input));
}
test "dynamic scrollback configuration evicts history and can retain history again" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .max_scrollback = 1_000_000,
        .rows = 3,
    });
    defer terminal.deinit(std.testing.allocator);
    for (0..100) |_| try terminal.printString("line\n");
    try std.testing.expect(terminal.screens.get(.primary).?.pages.total_rows > terminal.rows);

    const evicted = applyScrollbackBytes(&terminal, 0);
    try std.testing.expect(evicted > 0);
    try std.testing.expectEqual(
        @as(usize, terminal.rows),
        terminal.screens.get(.primary).?.pages.total_rows,
    );

    _ = applyScrollbackBytes(&terminal, 1_000_000);
    for (0..100) |_| try terminal.printString("line\n");
    try std.testing.expect(terminal.screens.get(.primary).?.pages.total_rows > terminal.rows);
}
test "owned terminal profile releases every partial allocation" {
    try std.testing.checkAllAllocationFailures(
        std.testing.allocator,
        exerciseOwnedProfileAllocations,
        .{},
    );
}
test "runtime rejects an incomplete palette before starting a process" {
    try std.testing.expectError(
        error.InvalidTerminalProfile,
        TerminalRuntime.createWithOptions(std.testing.allocator, .{
            .arguments = &.{},
            .columns = 80,
            .current_directory = null,
            .environment = &.{},
            .executable = "not-started",
            .rows = 24,
        }, .{ .profile = .{
            .color_scheme = .dark,
            .default_background_rgb = 0,
            .default_cursor_rgb = null,
            .default_foreground_rgb = 0xffffff,
            .device_primary_conformance_level = 62,
            .device_primary_features = &.{22},
            .device_secondary_type = 1,
            .device_secondary_firmware_version = 0,
            .device_secondary_rom_cartridge = 0,
            .device_tertiary_unit_id = 0,
            .enquiry_response = "",
            .palette = &.{},
            .version_response = "Orchbee",
        } }),
    );
}
test "thread startup failure releases transferred runtime resources" {
    for (0..3) |failure_stage| {
        var spawner: FailingThreadSpawner = .{ .fail_at = failure_stage };
        try std.testing.expectError(
            error.InjectedThreadSpawnFailure,
            createWithOptionsUsingSpawner(std.testing.allocator, .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = null,
                .environment = &.{},
                // The injected thread error must win because no process launch
                // is attempted until all three workers exist.
                .executable = "thread-startup-must-precede-process-spawn",
                .rows = 24,
            }, .{}, &spawner),
        );
    }
}
test "process spawn failure releases workers waiting at the startup gate" {
    var spawner: FailingThreadSpawner = .{ .fail_at = 3 };
    try std.testing.expectError(
        error.SpawnFailed,
        createWithOptionsUsingSpawner(std.testing.allocator, .{
            .arguments = &.{},
            .columns = 80,
            .current_directory = null,
            .environment = &.{},
            .executable = "runtime-process-spawn-must-fail",
            .rows = 24,
        }, .{}, &spawner),
    );
    try std.testing.expectEqual(@as(usize, 3), spawner.attempts);
}
test "unrecoverable worker failures contain the process tree without a connected broker" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const FailureCase = struct {
        expected_error: anyerror,
        expected_kind: subject.RuntimeFailureKind,
        inject: *const fn (*TerminalRuntime) void,
    };
    const cases = [_]FailureCase{
        .{
            .expected_error = error.PtyReadFailed,
            .expected_kind = .pty_read,
            .inject = recordReaderFailure,
        },
        .{
            .expected_error = error.PtyWriteFailed,
            .expected_kind = .pty_write,
            .inject = recordWriterFailure,
        },
        .{
            .expected_error = error.TerminalReplyDropped,
            .expected_kind = .terminal_reply,
            .inject = recordTerminalReplyFailure,
        },
        .{
            .expected_error = error.WorkerIntegrityFailed,
            .expected_kind = .worker_integrity,
            .inject = recordWorkerIntegrityFailure,
        },
    };

    for (cases) |case| {
        const runtime = try TerminalRuntime.create(std.testing.allocator, .{
            .arguments = arguments,
            .columns = 80,
            .current_directory = null,
            .environment = &.{},
            .executable = executable,
            .rows = 24,
        });
        defer runtime.destroy();

        case.inject(runtime);

        try std.testing.expect(runtime.containment_confirmed.load(.acquire));
        try std.testing.expectEqual(case.expected_kind, runtime.primaryFailure().?);
        try std.testing.expectError(case.expected_error, runtime.waitForCompletion());
        try runtime.shutdown();
    }
}
test "runtime parses PTY output before exposing canonical terminal state" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{
        "-c",
        "printf '\\033[31mhost\\033[0m:'; read value; printf '%s' \"$value\"",
    };
    const windows_arguments = [_][]const u8{
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "set /p value=& <nul set /p =host:!value!",
    };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    const client_input = if (builtin.os.tag == .windows) "orchbee\r\n" else "orchbee\n";
    try runtime.enqueueInput(client_input);
    const completion = try runtime.waitForCompletion();
    try runtime.shutdown();
    try std.testing.expect(completion.child_exited);
    try std.testing.expect(completion.pty_drained);
    switch (completion.classification) {
        .normal => {},
        .pending, .failed => return error.ExpectedNormalCompletion,
    }
    try std.testing.expect(completion.final_revision > 0);
    try std.testing.expectEqual(pty_process.ExitResult{ .exit_code = 0 }, completion.exit_result.?);
    try std.testing.expectEqual(client_input.len, runtime.takeWrittenBytes());

    const plain = try runtime.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain);
    try std.testing.expect(std.mem.indexOf(u8, plain, "host:orchbee") != null);
}
test "Ghostty BEL effects advance one terminal bell sequence per control byte" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();

    try std.testing.expectEqual(@as(u64, 0), runtime.bellSequence());
    parseTerminalInput(runtime, "\x07\x07");
    try std.testing.expectEqual(@as(u64, 2), runtime.bellSequence());
    parseTerminalInput(runtime, "text\x07");
    try std.testing.expectEqual(@as(u64, 3), runtime.bellSequence());
    try runtime.forceTerminate();
}
test "Windows child exiting 259 completes normally with exit code 259 (THCR-041)" {
    const builtin = @import("builtin");
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const executable = try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec");
    defer std.testing.allocator.free(executable);
    const arguments = [_][]const u8{ "/D", "/S", "/C", "exit 259" };
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = &arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    const completion = try runtime.waitForCompletion();
    try runtime.shutdown();
    try std.testing.expect(completion.child_exited);
    try std.testing.expect(completion.pty_drained);
    try std.testing.expect(completion.writer_stopped);
    switch (completion.classification) {
        .normal => {},
        .pending, .failed => return error.ExpectedNormalCompletion,
    }
    try std.testing.expectEqual(
        @as(?subject.RuntimeFailureKind, null),
        runtime.primaryFailure(),
    );
    try std.testing.expectEqual(
        pty_process.ExitResult{ .exit_code = 259 },
        completion.exit_result.?,
    );
}
test "normal completion freezes revision generation geometry and configuration" {
    const builtin = @import("builtin");
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    const arguments = [_][]const u8{
        "-c",
        "i=0; while [ $i -lt 100 ]; do printf 'line\\n'; i=$((i+1)); done",
    };
    const runtime = try TerminalRuntime.createWithOptions(std.testing.allocator, .{
        .arguments = &arguments,
        .columns = 8,
        .current_directory = null,
        .environment = &.{},
        .executable = "/bin/sh",
        .rows = 3,
    }, .{ .scrollback_bytes = 1_000_000 });
    defer runtime.destroy();

    const settled = try runtime.waitForCompletion();
    const settled_geometry = runtime.geometry();
    const settled_configuration = runtime.configuration();
    try std.testing.expect(settled.pty_drained);
    try std.testing.expectEqual(settled.final_revision, settled_geometry.revision);

    try std.testing.expectError(
        error.TerminalRuntimeUnavailable,
        runtime.updateConfiguration(.{ .scrollback_bytes = 0 }),
    );
    try std.testing.expectError(
        error.TerminalRuntimeUnavailable,
        runtime.resizeGeometry(9, 4, 2, 2),
    );
    try std.testing.expectEqualDeep(settled_geometry, runtime.geometry());
    try std.testing.expectEqualDeep(settled_configuration, runtime.configuration());
    try std.testing.expectEqual(settled.final_revision, runtime.completion().final_revision);
    try runtime.shutdown();
}
test "attachment preparation applies the live gate to every resize capability" {
    const runtime = try createImmediateExitRuntime();
    defer runtime.destroy();
    _ = try runtime.waitForCompletion();

    const read_attachment = AttachmentId.fromBytes([_]u8{0x21} ** 16);
    {
        var preparation = try runtime.prepareAttachment(
            read_attachment,
            .{ .input = false, .read = true, .resize = false },
        );
        defer preparation.deinit();
    }
    try expectAttachmentPreparationUnavailable(
        runtime,
        AttachmentId.fromBytes([_]u8{0x22} ** 16),
        .{ .input = false, .read = false, .resize = true },
    );
    try expectAttachmentPreparationUnavailable(
        runtime,
        AttachmentId.fromBytes([_]u8{0x23} ** 16),
        .{ .input = false, .read = true, .resize = true },
    );
}
test "resize-only attachment preparation reserves no input ownership capacity" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    runtime.input_mutex.lock();
    const capacity_before = runtime.input_ownership.attachments.capacity();
    runtime.input_mutex.unlock();

    var preparation = try runtime.prepareAttachment(
        AttachmentId.fromBytes([_]u8{0x24} ** 16),
        .{ .input = false, .read = false, .resize = true },
    );
    defer preparation.deinit();

    runtime.input_mutex.lock();
    const capacity_after = runtime.input_ownership.attachments.capacity();
    runtime.input_mutex.unlock();
    try std.testing.expectEqual(capacity_before, capacity_after);
}
test "attachment preparation derives reservations from canonical capabilities" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    const cases = [_]subject.Capabilities{
        .{ .input = true, .read = false, .resize = false },
        .{ .input = false, .read = true, .resize = false },
        .{ .input = false, .read = false, .resize = true },
        .{ .input = true, .read = true, .resize = false },
        .{ .input = true, .read = false, .resize = true },
        .{ .input = false, .read = true, .resize = true },
        .{ .input = true, .read = true, .resize = true },
    };

    for (cases, 0..) |capabilities, index| {
        var bytes = [_]u8{0x30} ** 16;
        bytes[bytes.len - 1] = @intCast(index);
        const attachment_id = AttachmentId.fromBytes(bytes);
        var preparation = try runtime.prepareAttachment(attachment_id, capabilities);
        try std.testing.expectEqual(capabilities.read, preparation.history_reserved);
        try std.testing.expectEqual(capabilities.input, preparation.input_state_reserved);
        preparation.commit();
        try std.testing.expectEqual(
            capabilities.input,
            subject.testing.hasAttachmentInputOwnership(runtime, attachment_id),
        );
    }
}
test "runtime resets projection resources after encode failure" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    const failed_request: ProjectionEncodeRequest = .{
        .attachment_id = AttachmentId.fromBytes([_]u8{0x22} ** 16),
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = HostEpoch.fromBytes([_]u8{0x11} ** 16),
        .maximum_bytes = 0,
        .sequence = 1,
    };
    const attempted_revision = runtime.geometry().revision;
    const blocked = try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        failed_request,
    );
    switch (blocked) {
        .blocked => |failure| {
            try std.testing.expectEqual(attempted_revision, failure.attempted_revision);
            try std.testing.expectEqual(
                subject.ProjectionBlockReason.projection_too_large,
                failure.reason,
            );
        },
        .frame => |frame| {
            std.testing.allocator.free(frame.allocation);
            return error.ExpectedProjectionBlock;
        },
    }
    try std.testing.expect(!runtime.projectionResident());
    try std.testing.expectError(
        error.ProjectionTooLarge,
        runtime.encodeProjectionFrame(failed_request),
    );
    try std.testing.expect(!runtime.projectionResident());

    const frame = try runtime.encodeProjectionFrame(.{
        .attachment_id = failed_request.attachment_id,
        .base_generation = failed_request.base_generation,
        .base_revision = failed_request.base_revision,
        .force_snapshot = failed_request.force_snapshot,
        .host_epoch = failed_request.host_epoch,
        .maximum_bytes = 8 * 1024 * 1024,
        .sequence = 2,
    });
    defer std.testing.allocator.free(frame.allocation);
    try std.testing.expect(frame.bytes.len > 0);
}
test "reader alternate-screen batch publishes one coherent projection generation" {
    const runtime = try createIdleRuntimeWithGeometry(8, 6, .{});
    defer runtime.destroy();

    const primary_bytes = "\x1b]2;Orchbee primary\x1b\\" ++
        "\x1b]7;file:///workspace/primary\x1b\\" ++
        "\x1b[4 q\x1b[?25l\x1b[?1h\x1b=" ++
        "\x1b[?2004h\x1b[?1004h\x1b[?1002h\x1b[?1006h" ++
        "\x1b[>4;2m\x1b[=31;1u\x1b[HPRI";
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        primary_bytes,
    ));
    const primary_geometry = runtime.geometry();
    const attachment_id = AttachmentId.fromBytes([_]u8{0x62} ** 16);
    const host_epoch = HostEpoch.fromBytes([_]u8{0x71} ** 16);
    const primary = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = attachment_id,
            .base_generation = null,
            .base_revision = null,
            .force_snapshot = true,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 1,
        },
    ));
    defer std.testing.allocator.free(primary.allocation);
    const primary_summary = try protocol.validateTerminalFrame(
        std.testing.allocator,
        primary.bytes,
    );
    try std.testing.expectEqual(protocol.FrameKind.snapshot, primary_summary.kind);
    try std.testing.expectEqual(protocol.ActiveScreen.primary, primary_summary.active_screen);
    try std.testing.expectEqualStrings("Orchbee primary", primary_summary.title.?);
    try std.testing.expectEqualStrings(
        "file:///workspace/primary",
        primary_summary.current_directory.?,
    );
    try std.testing.expectEqual(protocol.CursorShape.underline, primary_summary.cursor_shape);
    try std.testing.expectEqual(@as(u8, 0), primary.bytes[97] & 0x03);
    const primary_interaction = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) |
        (1 << 6) | (1 << 9) | (1 << 12) | (1 << 13) | (1 << 14) |
        (1 << 15) | (1 << 16) | (1 << 17);
    try std.testing.expectEqual(
        @as(u32, primary_interaction),
        try readFrameInt(u32, primary.bytes, 100),
    );

    const alternate_bytes = "\x1b[?1049h" ++
        "\x1b]2;Orchbee alternate\x1b\\" ++
        "\x1b]7;file:///workspace/alternate\x1b\\" ++
        "\x1b[5 q\x1b[?25h\x1b[?1l\x1b>" ++
        "\x1b[?2004l\x1b[?1004l\x1b[?1002l\x1b[?1006l" ++
        "\x1b[>4;0m\x1b[?1003h\x1b[?1016h\x1b[=3;1u\x1b[HALT";
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        alternate_bytes,
    ));
    const alternate_geometry = runtime.geometry();
    try std.testing.expectEqual(primary_geometry.generation + 1, alternate_geometry.generation);
    try std.testing.expectEqual(primary_geometry.revision + 1, alternate_geometry.revision);

    const alternate = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = attachment_id,
            .base_generation = primary_summary.generation,
            .base_revision = primary_summary.revision,
            .force_snapshot = false,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 2,
        },
    ));
    defer std.testing.allocator.free(alternate.allocation);
    const alternate_summary = try protocol.validateTerminalFrame(
        std.testing.allocator,
        alternate.bytes,
    );
    try std.testing.expectEqual(protocol.FrameKind.snapshot, alternate_summary.kind);
    try std.testing.expectEqual(protocol.ActiveScreen.alternate, alternate_summary.active_screen);
    try std.testing.expectEqualStrings("Orchbee alternate", alternate_summary.title.?);
    try std.testing.expectEqualStrings(
        "file:///workspace/alternate",
        alternate_summary.current_directory.?,
    );
    try std.testing.expectEqual(protocol.CursorShape.bar, alternate_summary.cursor_shape);
    try std.testing.expectEqual(@as(u8, 0x03), alternate.bytes[97] & 0x03);
    const alternate_interaction = (1 << 7) | (1 << 11) | (1 << 13) | (1 << 14);
    try std.testing.expectEqual(
        @as(u32, alternate_interaction),
        try readFrameInt(u32, alternate.bytes, 100),
    );
    try std.testing.expectEqual(alternate_geometry.generation, alternate_summary.generation);
    try std.testing.expectEqual(alternate_geometry.revision, alternate_summary.revision);
}
test "application clear erases and homes the active screen without resetting metadata or modes" {
    const runtime = try createIdleRuntimeWithGeometry(8, 6, .{});
    defer runtime.destroy();

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b[?1049h\x1b]2;Clear metadata\x1b\\" ++
            "\x1b]7;file:///workspace/clear\x1b\\" ++
            "\x1b[?2004h\x1b[?1004h\x1b[?1002h\x1b[?1006h\x1b[3;4HCLEAR",
    ));
    const before = runtime.geometry();
    const attachment_id = AttachmentId.fromBytes([_]u8{0x63} ** 16);
    const host_epoch = HostEpoch.fromBytes([_]u8{0x72} ** 16);
    const baseline = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = attachment_id,
            .base_generation = null,
            .base_revision = null,
            .force_snapshot = true,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 1,
        },
    ));
    defer std.testing.allocator.free(baseline.allocation);
    const baseline_summary = try protocol.validateTerminalFrame(
        std.testing.allocator,
        baseline.bytes,
    );
    const baseline_interaction = try readFrameInt(u32, baseline.bytes, 100);

    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
    const update = try runtime.applyTerminalControl(.clear);
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
    const after = runtime.geometry();
    try std.testing.expectEqual(before.generation + 1, update.generation);
    try std.testing.expectEqual(before.revision + 1, update.revision);
    try std.testing.expectEqual(update.generation, after.generation);
    try std.testing.expectEqual(update.revision, after.revision);
    try std.testing.expectEqual(before.columns, after.columns);
    try std.testing.expectEqual(before.rows, after.rows);
    try std.testing.expectEqual(before.cell_pixel_width, after.cell_pixel_width);
    try std.testing.expectEqual(before.cell_pixel_height, after.cell_pixel_height);

    const plain = try runtime.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain);
    try std.testing.expectEqual(@as(usize, 0), std.mem.trim(u8, plain, " \r\n\t").len);

    const frame = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = attachment_id,
            .base_generation = before.generation,
            .base_revision = before.revision,
            .force_snapshot = false,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 2,
        },
    ));
    defer std.testing.allocator.free(frame.allocation);
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, frame.bytes);
    try std.testing.expectEqual(protocol.FrameKind.snapshot, summary.kind);
    try std.testing.expectEqual(protocol.ActiveScreen.alternate, summary.active_screen);
    try std.testing.expectEqualStrings("Clear metadata", summary.title.?);
    try std.testing.expectEqualStrings("file:///workspace/clear", summary.current_directory.?);
    try std.testing.expectEqual(@as(u16, 0), summary.cursor_column);
    try std.testing.expectEqual(@as(u16, 0), summary.cursor_row);
    try std.testing.expectEqual(baseline_summary.active_screen, summary.active_screen);
    try std.testing.expectEqual(baseline_interaction, try readFrameInt(u32, frame.bytes, 100));
}
test "application reset returns alternate state to a fresh primary terminal" {
    const runtime = try createIdleRuntimeWithGeometry(8, 6, .{});
    defer runtime.destroy();

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "PRIMARY\x1b[?1049h\x1b]2;Reset metadata\x1b\\" ++
            "\x1b]7;file:///workspace/reset\x1b\\" ++
            "\x1b[5 q\x1b[?25l\x1b[?1h\x1b=\x1b[?2004h" ++
            "\x1b[?1004h\x1b[?1003h\x1b[?1016h\x1b[4;5HRESET",
    ));
    const before = runtime.geometry();

    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
    const update = try runtime.applyTerminalControl(.reset);
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
    const after = runtime.geometry();
    try std.testing.expectEqual(before.generation + 1, update.generation);
    try std.testing.expectEqual(before.revision + 1, update.revision);
    try std.testing.expectEqualDeep(before.columns, after.columns);
    try std.testing.expectEqualDeep(before.rows, after.rows);
    try std.testing.expectEqualDeep(before.cell_pixel_width, after.cell_pixel_width);
    try std.testing.expectEqualDeep(before.cell_pixel_height, after.cell_pixel_height);
    try std.testing.expect(!runtime.focusInputWouldEmit());

    const plain = try runtime.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain);
    try std.testing.expectEqual(@as(usize, 0), std.mem.trim(u8, plain, " \r\n\t").len);

    const frame = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = AttachmentId.fromBytes([_]u8{0x64} ** 16),
            .base_generation = before.generation,
            .base_revision = before.revision,
            .force_snapshot = false,
            .host_epoch = HostEpoch.fromBytes([_]u8{0x73} ** 16),
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 1,
        },
    ));
    defer std.testing.allocator.free(frame.allocation);
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, frame.bytes);
    try std.testing.expectEqual(protocol.FrameKind.snapshot, summary.kind);
    try std.testing.expectEqual(protocol.ActiveScreen.primary, summary.active_screen);
    try std.testing.expectEqual(@as(?[]const u8, null), summary.title);
    try std.testing.expectEqual(@as(?[]const u8, null), summary.current_directory);
    try std.testing.expectEqual(protocol.CursorShape.block, summary.cursor_shape);
    try std.testing.expectEqual(@as(u16, 0), summary.cursor_column);
    try std.testing.expectEqual(@as(u16, 0), summary.cursor_row);
    try std.testing.expectEqual(@as(u32, 0), try readFrameInt(u32, frame.bytes, 100));
}
test "application terminal controls reject counter exhaustion before mutation" {
    const runtime = try createIdleRuntimeWithGeometry(8, 6, .{});
    defer runtime.destroy();
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "unchanged"),
    );

    runtime.terminal_mutex.lock();
    runtime.generation = std.math.maxInt(u64);
    runtime.terminal_mutex.unlock();
    var before = try canonicalSnapshot(runtime);
    defer before.deinit();
    try std.testing.expectError(error.RevisionOverflow, runtime.applyTerminalControl(.clear));
    var after = try canonicalSnapshot(runtime);
    defer after.deinit();
    try expectCanonicalSnapshotEqual(before, after);

    runtime.terminal_mutex.lock();
    runtime.generation = 1;
    runtime.revision.store(std.math.maxInt(u64), .monotonic);
    runtime.terminal_mutex.unlock();
    var revision_before = try canonicalSnapshot(runtime);
    defer revision_before.deinit();
    try std.testing.expectError(error.RevisionOverflow, runtime.applyTerminalControl(.reset));
    var revision_after = try canonicalSnapshot(runtime);
    defer revision_after.deinit();
    try expectCanonicalSnapshotEqual(revision_before, revision_after);
}
test "reader applies terminal-driven 80 and 132 column modes to the PTY before commit" {
    var resize_probe: TerminalDrivenResizeProbe = .{};
    const runtime = try createRuntimeWithHooks(resize_probe.hooks());
    defer runtime.destroy();

    const initial = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 80), initial.columns);
    try std.testing.expectEqual(@as(u16, 24), initial.rows);

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b[?40h\x1b[?3h",
    ));
    const expanded = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 132), expanded.columns);
    try std.testing.expectEqual(initial.rows, expanded.rows);
    try std.testing.expectEqual(initial.cell_pixel_width, expanded.cell_pixel_width);
    try std.testing.expectEqual(initial.cell_pixel_height, expanded.cell_pixel_height);
    try std.testing.expectEqual(initial.generation + 1, expanded.generation);
    try std.testing.expectEqual(initial.revision + 1, expanded.revision);

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b[?3l",
    ));
    const restored = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 80), restored.columns);
    try std.testing.expectEqual(expanded.rows, restored.rows);
    try std.testing.expectEqual(expanded.cell_pixel_width, restored.cell_pixel_width);
    try std.testing.expectEqual(expanded.cell_pixel_height, restored.cell_pixel_height);
    try std.testing.expectEqual(expanded.generation + 1, restored.generation);
    try std.testing.expectEqual(expanded.revision + 1, restored.revision);

    const calls = resize_probe.observedCalls();
    try std.testing.expectEqual(@as(usize, 2), calls.len);
    try std.testing.expectEqual(initial.columns, calls[0].previous.columns);
    try std.testing.expectEqual(expanded.columns, calls[0].next.columns);
    try std.testing.expectEqual(expanded.columns, calls[1].previous.columns);
    try std.testing.expectEqual(restored.columns, calls[1].next.columns);
    try std.testing.expectEqual(initial.rows, calls[0].next.rows);
    try std.testing.expectEqual(initial.cell_pixel_width, calls[0].next.cell_pixel_width);
    try std.testing.expectEqual(initial.cell_pixel_height, calls[0].next.cell_pixel_height);

    try runtime.forceTerminate();
}
test "reader ignores 132 column mode while the enabling private mode is disabled" {
    var resize_probe: TerminalDrivenResizeProbe = .{};
    const runtime = try createRuntimeWithHooks(resize_probe.hooks());
    defer runtime.destroy();

    const initial = runtime.geometry();
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?3h"),
    );

    const after = runtime.geometry();
    try std.testing.expectEqual(initial.columns, after.columns);
    try std.testing.expectEqual(initial.rows, after.rows);
    try std.testing.expectEqual(initial.generation, after.generation);
    try std.testing.expectEqual(initial.revision + 1, after.revision);
    try std.testing.expectEqual(@as(usize, 0), resize_probe.observedCalls().len);

    try runtime.forceTerminate();
}
test "reader preserves parser order for multiple terminal-driven resizes in one commit" {
    var resize_probe: TerminalDrivenResizeProbe = .{};
    const runtime = try createRuntimeWithHooks(resize_probe.hooks());
    defer runtime.destroy();

    const initial = runtime.geometry();
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?40h\x1b[?3h\x1b[?3l"),
    );

    const final = runtime.geometry();
    try std.testing.expectEqual(initial.columns, final.columns);
    try std.testing.expectEqual(initial.rows, final.rows);
    try std.testing.expectEqual(initial.generation + 1, final.generation);
    try std.testing.expectEqual(initial.revision + 1, final.revision);
    const calls = resize_probe.observedCalls();
    try std.testing.expectEqual(@as(usize, 2), calls.len);
    try std.testing.expectEqual(@as(u16, 80), calls[0].previous.columns);
    try std.testing.expectEqual(@as(u16, 132), calls[0].next.columns);
    try std.testing.expectEqual(@as(u16, 132), calls[1].previous.columns);
    try std.testing.expectEqual(@as(u16, 80), calls[1].next.columns);

    try runtime.forceTerminate();
}
test "terminal-driven PTY resize failure fences before Ghostty mutation" {
    var resize_probe: TerminalDrivenResizeProbe = .{ .fail = true };
    const runtime = try createRuntimeWithHooks(resize_probe.hooks());
    defer runtime.destroy();
    var before = try canonicalSnapshot(runtime);
    defer before.deinit();
    const initial = runtime.geometry();

    try std.testing.expectEqual(
        ReaderBatchOutcome.runtime_failed,
        commitReaderBatch(runtime, "\x1b[?40h\x1b[?3hdiscarded-after-failure"),
    );

    const after_geometry = runtime.geometry();
    try std.testing.expectEqualDeep(initial, after_geometry);
    try std.testing.expect(runtime.isResizeFenced());
    try std.testing.expect(subject.testing.failureFenced(runtime));
    try std.testing.expectEqual(subject.RuntimeFailureKind.resize, runtime.primaryFailure().?);
    const calls = resize_probe.observedCalls();
    try std.testing.expectEqual(@as(usize, 1), calls.len);
    try std.testing.expectEqual(@as(u16, 80), calls[0].previous.columns);
    try std.testing.expectEqual(@as(u16, 132), calls[0].next.columns);
    var after = try canonicalSnapshot(runtime);
    defer after.deinit();
    try expectCanonicalSnapshotEqual(before, after);
}
test "attachment resize waits for a terminal-driven reader transaction" {
    var resize_probe: TerminalResizeSerializationProbe = .{};
    const runtime = try createRuntimeWithHooks(resize_probe.hooks());
    defer runtime.destroy();

    var reader_call: ReaderBatchCall = .{ .runtime = runtime };
    const reader_thread = try std.Thread.spawn(.{}, ReaderBatchCall.run, .{&reader_call});
    defer reader_thread.join();
    defer resize_probe.first_effect_returned.release();
    resize_probe.first_effect_returned.wait();

    var resize_call: ResizeCall = .{ .runtime = runtime };
    const resize_thread = try std.Thread.spawn(.{}, ResizeCall.run, .{&resize_call});
    defer resize_thread.join();
    defer resize_probe.first_effect_returned.release();
    resize_probe.reader_commit_waiting.wait();
    try std.testing.expectEqual(@as(usize, 1), resize_probe.observedCalls().len);

    resize_probe.first_effect_returned.release();
    reader_call.done.wait();
    resize_call.done.wait();
    try std.testing.expectEqual(ReaderBatchOutcome.committed, reader_call.observedOutcome().?);
    switch (resize_call.observedResult()) {
        .resized => |revision| try std.testing.expectEqual(@as(u64, 2), revision),
        .failed => |err| return err,
    }
    const calls = resize_probe.observedCalls();
    try std.testing.expectEqual(@as(usize, 2), calls.len);
    try std.testing.expectEqual(@as(u16, 80), calls[0].previous.columns);
    try std.testing.expectEqual(@as(u16, 132), calls[0].next.columns);
    try std.testing.expectEqual(@as(u16, 132), calls[1].previous.columns);
    try std.testing.expectEqual(@as(u16, 100), calls[1].next.columns);
    const final = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 100), final.columns);
    try std.testing.expectEqual(@as(u16, 30), final.rows);

    try runtime.forceTerminate();
}
test "runtime projection preserves resize and hyperlink transitions" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 4,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 2,
    });
    defer runtime.destroy();

    const attachment_id = AttachmentId.fromBytes([_]u8{0x52} ** 16);
    const host_epoch = HostEpoch.fromBytes([_]u8{0x37} ** 16);
    const hyperlink_target = "https://projection.example/linked";
    const initial_revision = try writeProjectionOutput(runtime, "\x1b[HAA");
    const initial_geometry = runtime.geometry();
    try std.testing.expectEqual(initial_revision, initial_geometry.revision);

    const initial = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        32,
        .{
            .attachment_id = attachment_id,
            .base_generation = null,
            .base_revision = null,
            .force_snapshot = true,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 1,
        },
    ));
    defer std.testing.allocator.free(initial.allocation);
    const initial_summary = try protocol.validateTerminalFrame(std.testing.allocator, initial.bytes);
    try std.testing.expectEqual(protocol.FrameKind.snapshot, initial_summary.kind);
    try std.testing.expectEqual(@as(u16, 4), initial_summary.columns);
    try std.testing.expectEqual(@as(u16, 2), initial_summary.rows);
    try std.testing.expectEqual(@as(u16, 2), initial_summary.row_count);
    try std.testing.expectEqual(initial_revision, initial_summary.revision);
    try expectIncludedProjectionRows(initial.bytes, &.{ 0, 1 });
    try expectProjectionRowRevisions(runtime, &.{ initial_revision, initial_revision });
    const initial_links = try observeFrameHyperlinks(initial.bytes, hyperlink_target);
    try std.testing.expect(!initial_links.any);
    try std.testing.expect(!initial_links.target);

    const linked_revision = try writeProjectionOutput(
        runtime,
        "\x1b[H\x1b]8;;" ++ hyperlink_target ++ "\x07BB\x1b]8;;\x07",
    );
    const delta = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        32,
        .{
            .attachment_id = attachment_id,
            .base_generation = initial_summary.generation,
            .base_revision = initial_revision,
            .force_snapshot = false,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 2,
        },
    ));
    defer std.testing.allocator.free(delta.allocation);
    const delta_summary = try protocol.validateTerminalFrame(std.testing.allocator, delta.bytes);
    try std.testing.expectEqual(protocol.FrameKind.delta, delta_summary.kind);
    try std.testing.expectEqual(@as(u16, 4), delta_summary.columns);
    try std.testing.expectEqual(@as(u16, 2), delta_summary.rows);
    try std.testing.expectEqual(@as(u16, 1), delta_summary.row_count);
    try std.testing.expectEqual(initial_revision, delta_summary.base_revision.?);
    try std.testing.expectEqual(linked_revision, delta_summary.revision);
    try expectIncludedProjectionRows(delta.bytes, &.{0});
    try expectProjectionRowRevisions(runtime, &.{ linked_revision, initial_revision });
    try expectProjectionLinkTargets(runtime, &.{hyperlink_target});
    const delta_links = try observeFrameHyperlinks(delta.bytes, hyperlink_target);
    try std.testing.expect(delta_links.any);
    try std.testing.expect(delta_links.target);

    const unlinked_revision = try writeProjectionOutput(runtime, "\x1b[HCC");
    const resized_revision = try runtime.resizeGeometry(5, 3, 8, 16);
    try std.testing.expectEqual(unlinked_revision + 1, resized_revision);
    const resized = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        32,
        .{
            .attachment_id = attachment_id,
            .base_generation = initial_summary.generation,
            .base_revision = linked_revision,
            .force_snapshot = false,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 3,
        },
    ));
    defer std.testing.allocator.free(resized.allocation);
    const resized_summary = try protocol.validateTerminalFrame(std.testing.allocator, resized.bytes);
    try std.testing.expectEqual(protocol.FrameKind.snapshot, resized_summary.kind);
    try std.testing.expectEqual(@as(u16, 5), resized_summary.columns);
    try std.testing.expectEqual(@as(u16, 3), resized_summary.rows);
    try std.testing.expectEqual(@as(u16, 3), resized_summary.row_count);
    try std.testing.expectEqual(initial_summary.generation + 1, resized_summary.generation);
    try std.testing.expectEqual(resized_revision, resized_summary.revision);
    try expectIncludedProjectionRows(resized.bytes, &.{ 0, 1, 2 });
    try expectProjectionRowRevisions(
        runtime,
        &.{ resized_revision, resized_revision, resized_revision },
    );
    try expectProjectionLinkTargets(runtime, &.{});
    const resized_links = try observeFrameHyperlinks(resized.bytes, hyperlink_target);
    try std.testing.expect(!resized_links.any);
    try std.testing.expect(!resized_links.target);
}
