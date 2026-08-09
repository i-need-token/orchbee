const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const engine_boundary = @import("terminal-host-engine");
const model_boundary = @import("terminal-host-model");
const platform_boundary = @import("terminal-host-platform");
const subject = engine_boundary.runtime;
const pty_process = platform_boundary.process;

const applyScrollbackBytes = subject.testing.applyScrollbackBytesFn;
const cancelInput = subject.testing.cancelInputFn;
const closeClientInput = subject.testing.closeClientInputFn;
const CompletionClassification = subject.testing.CompletionClassificationType;
const completionClassificationLoad = subject.testing.completionClassificationLoadFn;
const createWithOptionsUsingSpawner = subject.testing.createWithOptionsUsingSpawnerFn;
const deinitOwnedProfile = subject.testing.deinitOwnedProfileFn;
const InputEntry = subject.testing.InputEntryType;
const InputReservation = subject.testing.InputReservationType;
const initOwnedProfile = subject.testing.initOwnedProfileFn;
const modifiers = subject.testing.modifiersFn;
const mouseEncodeEvent = subject.testing.mouseEncodeEventFn;
const mouseHasPressedButton = subject.testing.mouseHasPressedButtonFn;
const nextWritableInputEntry = subject.testing.nextWritableInputEntryFn;
const markCompletionFailure = subject.testing.markCompletionFailureFn;
const markCompletionNormal = subject.testing.markCompletionNormalFn;
const ProjectionEncodeRequest = subject.ProjectionEncodeRequest;
const AttachmentId = subject.AttachmentId;
const HostEpoch = subject.HostEpoch;
const reserveInput = subject.testing.reserveInputFn;
const TerminalRuntime = subject.TerminalRuntime;
const max_pending_input_bytes = subject.testing.pending_input_limit;
const max_pending_input_entries = subject.testing.pending_input_entry_limit;
const max_pending_terminal_reply_bytes = subject.testing.pending_terminal_reply_limit;
const publishInput = subject.testing.publishInputFn;
const recordReaderFailure = subject.testing.recordReaderFailureFn;
const recordTerminalReplyFailure = subject.testing.recordTerminalReplyFailure;
const recordWorkerIntegrityFailure = subject.testing.recordWorkerIntegrityFailureFn;
const recordWriterFailure = subject.testing.recordWriterFailureFn;
const resizePtyWithRollback = subject.testing.resizePtyWithRollbackFn;
const Geometry = model_boundary.geometry.Geometry;

test "runtime completion classification is terminal and preserves the first outcome" {
    var classification: CompletionClassification = .{};
    switch (completionClassificationLoad(&classification)) {
        .pending => {},
        .normal, .failed => return error.ExpectedPendingCompletion,
    }

    try std.testing.expect(markCompletionNormal(&classification));
    switch (completionClassificationLoad(&classification)) {
        .normal => {},
        .pending, .failed => return error.ExpectedNormalCompletion,
    }

    markCompletionFailure(&classification, .pty_read);
    switch (completionClassificationLoad(&classification)) {
        .normal => {},
        .pending, .failed => return error.ExpectedTerminalNormalCompletion,
    }
    try std.testing.expect(!markCompletionNormal(&classification));

    var failed_classification: CompletionClassification = .{};
    markCompletionFailure(&failed_classification, .pty_read);
    markCompletionFailure(&failed_classification, .pty_write);
    switch (completionClassificationLoad(&failed_classification)) {
        .failed => |failure| try std.testing.expectEqual(
            subject.RuntimeFailureKind.pty_read,
            failure,
        ),
        .pending, .normal => return error.ExpectedFirstFailure,
    }
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

fn exerciseOwnedProfileAllocations(allocator: std.mem.Allocator) !void {
    const features = [_]u16{ 1, 22, 52 };
    const palette = [_]u32{0} ** 256;
    var profile = try initOwnedProfile(allocator, .{
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
        .enquiry_response = "orchbee-enquiry",
        .palette = &palette,
        .version_response = "orchbee-version",
    });
    defer deinitOwnedProfile(&profile, allocator);
}

const FailingThreadSpawner = struct {
    fail_at: usize,
    attempts: usize = 0,

    pub fn spawn(
        self: *FailingThreadSpawner,
        comptime thread_function: fn (*TerminalRuntime) void,
        runtime: *TerminalRuntime,
    ) !std.Thread {
        const attempt = self.attempts;
        self.attempts += 1;
        if (attempt == self.fail_at) return error.InjectedThreadSpawnFailure;
        return std.Thread.spawn(.{}, thread_function, .{runtime});
    }
};

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

test "terminal generated replies are excluded from client input accounting" {
    const builtin = @import("builtin");
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    const arguments = [_][]const u8{
        "-c",
        "stty -echo -icanon min 9 time 20; printf '\\033[c'; " ++
            "response=$(dd bs=9 count=1 2>/dev/null); " ++
            "expected=$(printf '\\033[?62;22c'); [ \"$response\" = \"$expected\" ]",
    };
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = &arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = "/bin/sh",
        .rows = 24,
    });
    defer runtime.destroy();

    _ = try runtime.waitForCompletion();
    try runtime.shutdown();
    const completion = runtime.completion();
    try std.testing.expectEqual(pty_process.ExitResult{ .exit_code = 0 }, completion.exit_result.?);
    switch (completion.classification) {
        .normal => {},
        .pending, .failed => return error.ExpectedNormalCompletion,
    }
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
}

test "root child exit rejects new client input without stopping terminal replies" {
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

    closeClientInput(runtime);
    try std.testing.expectError(
        error.TerminalExited,
        reserveInput(runtime, 1, false, .client),
    );
    const reply = try reserveInput(runtime, 1, false, .terminal);
    try publishInput(runtime, reply, try std.testing.allocator.dupe(u8, "x"));
    runtime.input_mutex.lock();
    try std.testing.expect(!runtime.writer_stop);
    runtime.input_mutex.unlock();
    try runtime.forceTerminate();
}

test "runtime queue applies bounded backpressure before allocation" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 1" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 3 127.0.0.1 >nul" };
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

    const bytes = try std.testing.allocator.alloc(u8, max_pending_input_bytes + 1);
    defer std.testing.allocator.free(bytes);
    @memset(bytes, 'x');
    try std.testing.expectError(error.InputBackpressure, runtime.enqueueInput(bytes));
    try runtime.terminate();
}

test "terminal replies preempt client slices without reordering either source" {
    var first_client = [_]u8{'a'};
    var reply = [_]u8{'r'};
    var second_client = [_]u8{'b'};
    var entries = [_]InputEntry{
        .{
            .bytes = first_client[0..],
            .identifier = 1,
            .reserved_bytes = first_client.len,
            .source = .client,
        },
        .{
            .bytes = reply[0..],
            .identifier = 2,
            .reserved_bytes = reply.len,
            .source = .terminal,
        },
        .{
            .bytes = second_client[0..],
            .identifier = 3,
            .reserved_bytes = second_client.len,
            .source = .client,
        },
    };

    try std.testing.expectEqual(@as(?usize, 1), nextWritableInputEntry(&entries));

    entries[1].bytes = null;
    try std.testing.expectEqual(@as(?usize, null), nextWritableInputEntry(&entries));

    entries[1].source = .client;
    try std.testing.expectEqual(@as(?usize, 0), nextWritableInputEntry(&entries));

    entries[0].bytes = null;
    try std.testing.expectEqual(@as(?usize, null), nextWritableInputEntry(&entries));
}

test "discarding prepared input releases its ordered queue reservation" {
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

    var prepared = try runtime.prepareRawInput("abc");
    try std.testing.expectEqual(@as(usize, 3), prepared.byteCount());
    try std.testing.expectEqual(@as(usize, 3), runtime.pendingInputBytes());
    prepared.deinit();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}

test "zero-byte semantic encodings do not retain queue metadata" {
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

    for (0..max_pending_input_entries + 1) |_| {
        try std.testing.expectEqual(@as(usize, 0), try runtime.enqueueFocusInput(true));
    }
    runtime.input_mutex.lock();
    try std.testing.expectEqual(@as(usize, 0), runtime.input_entries.items.len);
    try std.testing.expectEqual(@as(usize, 0), runtime.client_input_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), runtime.terminal_reply_charge.bytes);
    runtime.input_mutex.unlock();

    var entry_reservations: [max_pending_input_entries]InputReservation = undefined;
    for (&entry_reservations) |*reservation| {
        reservation.* = try reserveInput(runtime, 1, false, .client);
    }
    try std.testing.expectError(
        error.InputBackpressure,
        reserveInput(runtime, 1, false, .client),
    );
    const entry_reply = try reserveInput(runtime, 1, false, .terminal);
    cancelInput(runtime, entry_reply.identifier);
    try std.testing.expectEqual(@as(usize, 0), try runtime.enqueueFocusInput(true));
    for (entry_reservations) |reservation| cancelInput(runtime, reservation.identifier);
    const byte_reservation = try reserveInput(runtime, max_pending_input_bytes, false, .client);
    try std.testing.expectError(
        error.InputBackpressure,
        reserveInput(runtime, 1, false, .client),
    );
    const byte_reply = try reserveInput(
        runtime,
        max_pending_terminal_reply_bytes,
        false,
        .terminal,
    );
    cancelInput(runtime, byte_reply.identifier);
    try std.testing.expectEqual(@as(usize, 0), try runtime.enqueueFocusInput(true));
    cancelInput(runtime, byte_reservation.identifier);
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}

test "semantic input limits apply to encoded terminal bytes" {
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

    try std.testing.expectEqual(@as(usize, 3), try runtime.enqueueTextInputWithLimit("abc", 3));
    try std.testing.expectError(
        error.InputLimitExceeded,
        runtime.enqueueTextInputWithLimit("abcd", 3),
    );
    try std.testing.expectEqual(@as(usize, 0), try runtime.enqueueFocusInputWithLimit(true, 0));
    try runtime.forceTerminate();
}

test "input queue bounds entry metadata independently of byte charge" {
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

    var reservations: [max_pending_input_entries]InputReservation = undefined;
    for (&reservations) |*reservation| {
        reservation.* = try reserveInput(runtime, 1, false, .client);
    }
    try std.testing.expectError(
        error.InputBackpressure,
        reserveInput(runtime, 1, false, .client),
    );
    for (reservations) |reservation| cancelInput(runtime, reservation.identifier);
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}

test "force termination releases every engine-owned client input reservation" {
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

    const input = try std.testing.allocator.alloc(u8, 512 * 1024);
    defer std.testing.allocator.free(input);
    @memset(input, 'x');
    try runtime.enqueueInput(input);
    try runtime.forceTerminate();
    try runtime.shutdown();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
}

test "stopped writer rejects reservations even when queue capacity remains" {
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

    runtime.input_mutex.lock();
    runtime.writer_stop = true;
    runtime.input_condition.broadcast();
    runtime.input_mutex.unlock();

    try std.testing.expectError(
        error.TerminalExited,
        reserveInput(runtime, 1, false, .client),
    );
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}

test "resize counter exhaustion leaves process and terminal geometry unchanged" {
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

    runtime.terminal_mutex.lock();
    runtime.generation = std.math.maxInt(u64);
    runtime.terminal_mutex.unlock();
    const generation_exhausted = runtime.geometry();
    try std.testing.expectError(
        error.RevisionOverflow,
        runtime.resizeGeometry(81, 25, 2, 3),
    );
    try std.testing.expectEqualDeep(generation_exhausted, runtime.geometry());

    runtime.terminal_mutex.lock();
    runtime.generation = 1;
    runtime.revision.store(std.math.maxInt(u64), .monotonic);
    runtime.terminal_mutex.unlock();
    const revision_exhausted = runtime.geometry();
    try std.testing.expectError(
        error.RevisionOverflow,
        runtime.resizeGeometry(81, 25, 2, 3),
    );
    try std.testing.expectEqualDeep(revision_exhausted, runtime.geometry());
    try runtime.forceTerminate();
}

test "runtime can force the retained session after the root child exits" {
    const builtin = @import("builtin");
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const executable = try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec");
    defer std.testing.allocator.free(executable);
    const arguments = [_][]const u8{
        "/D",
        "/S",
        "/C",
        "start \"\" /B ping -n 31 127.0.0.1 >nul & ping -n 2 127.0.0.1 >nul",
    };
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = &arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    var observed_finishing_order = false;
    for (0..600) |_| {
        const completion = runtime.completion();
        if (completion.child_exited) {
            observed_finishing_order = !completion.pty_drained;
            break;
        }
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expect(observed_finishing_order);

    try runtime.terminate();
    try runtime.forceTerminate();
    try runtime.shutdown();
    const completed = runtime.completion();
    try std.testing.expect(completed.child_exited);
}

test "ordered input reservations prevent overtaking and cancellation leaves no gap" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{
        "-c",
        "read value; printf '<%s>' \"$value\"",
    };
    const windows_arguments = [_][]const u8{
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "set /p value=& <nul set /p =^<!value!^>",
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

    const first = try reserveInput(runtime, 1, false, .client);
    const cancelled = try reserveInput(runtime, 1, false, .client);
    const third = try reserveInput(runtime, 2, false, .client);
    try publishInput(runtime, third, try std.testing.allocator.dupe(u8, if (builtin.os.tag == .windows) "C\r" else "C\n"));
    cancelInput(runtime, cancelled.identifier);
    std.Thread.sleep(10 * std.time.ns_per_ms);
    try std.testing.expectEqual(@as(usize, 3), runtime.pendingInputBytes());
    try publishInput(runtime, first, try std.testing.allocator.dupe(u8, "A"));

    _ = try runtime.waitForCompletion();
    try runtime.shutdown();
    const plain = try runtime.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain);
    try std.testing.expect(std.mem.indexOf(u8, plain, "<AC>") != null);
}
