const harness = @import("runtime_harness.zig");
const std = harness.std;
const builtin = harness.target_builtin;
const platform = harness.platform_boundary;
const pty_process = harness.pty_process;
const TerminalRuntime = harness.TerminalRuntime;
const runtimeState = harness.runtimeState;

const capture_limit_bytes = 256 * 1024;
const marker_timeout_ns = 20 * std.time.ns_per_s;
const child_name = "orchbee-conpty-pointer-child.exe";

const PointerMode = enum {
    button,
    any,
    released,
};

fn captureWhenPhaseCommitted(
    runtime: *TerminalRuntime,
    marker: []const u8,
    bell_sequence: u64,
) !harness.subject.ScrollbackTextCapture {
    var timer = try platform.monotonic.Timer.start();
    while (true) {
        if (runtime.bellSequence() >= bell_sequence) {
            var capture = try runtime.captureScrollbackText(
                std.testing.allocator,
                capture_limit_bytes,
            );
            if (std.mem.indexOf(u8, capture.text(), marker) != null) return capture;
            capture.deinit();
        }
        if (runtime.primaryFailure() != null) return error.TerminalRuntimeFailed;
        if (timer.read() >= marker_timeout_ns) return error.TerminalMarkerTimedOut;
        platform.monotonic.sleep(10 * std.time.ns_per_ms);
    }
}

fn expectPointerMode(runtime: *TerminalRuntime, expected: PointerMode) !void {
    const state = runtimeState(runtime);
    state.terminal_mutex.lock();
    defer state.terminal_mutex.unlock();

    const button = expected == .button;
    const any = expected == .any;
    const sgr = expected != .released;
    const expected_event: @TypeOf(state.terminal.flags.mouse_event) = switch (expected) {
        .button => .button,
        .any => .any,
        .released => .none,
    };
    const expected_format: @TypeOf(state.terminal.flags.mouse_format) = if (sgr) .sgr else .x10;
    try std.testing.expectEqual(button, state.terminal.modes.get(.mouse_event_button));
    try std.testing.expectEqual(any, state.terminal.modes.get(.mouse_event_any));
    try std.testing.expectEqual(sgr, state.terminal.modes.get(.mouse_format_sgr));
    try std.testing.expectEqual(expected_event, state.terminal.flags.mouse_event);
    try std.testing.expectEqual(expected_format, state.terminal.flags.mouse_format);
}

fn hoverInput() harness.subject.MouseInput {
    return .{
        .action = .{ .motion = .hover },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = 0,
        .y = 0,
    };
}

fn expectSilentHover(runtime: *TerminalRuntime) !void {
    const before = runtime.pendingInputBytes();
    try std.testing.expect(!try runtime.mouseInputWouldEmit(hoverInput()));
    try std.testing.expectEqual(@as(usize, 0), try runtime.enqueueMouseInput(hoverInput()));
    try std.testing.expectEqual(before, runtime.pendingInputBytes());
}

test "bundled Windows ConPTY carries admitted Orchbee pointer motion" {
    if (builtin.os.tag != .windows or builtin.cpu.arch != .x86_64) {
        return error.SkipZigTest;
    }
    try std.testing.expect(try platform.testing.conpty.initializedFromBundle());

    const self_directory = try std.process.executableDirPathAlloc(
        std.testing.io,
        std.testing.allocator,
    );
    defer std.testing.allocator.free(self_directory);
    const child = try std.fs.path.join(std.testing.allocator, &.{ self_directory, child_name });
    defer std.testing.allocator.free(child);

    const arguments = [_][]const u8{
        "--ignored",
        "--exact",
        "tests::windows_conpty_pointer::inherited_terminal_pointer_admission",
        "--test-threads=1",
        "--nocapture",
    };
    const environment = [_]pty_process.EnvironmentDelta{.{
        .name = "ORCHBEE_CONPTY_POINTER_WITNESS_CHILD",
        .value = "1",
    }};
    const runtime = try TerminalRuntime.createWithOptions(std.testing.allocator, .{
        .arguments = &arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &environment,
        .executable = child,
        .rows = 24,
    }, .{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
        .scrollback_bytes = 512 * 1024,
    });
    var destroy_required = true;
    defer if (destroy_required) runtime.destroy();

    var button = try captureWhenPhaseCommitted(runtime, "OBPW:BUTTON|", 1);
    defer button.deinit();
    try std.testing.expectEqual(.alternate, button.screen);
    try expectPointerMode(runtime, .button);
    try expectSilentHover(runtime);
    try std.testing.expectEqual(@as(usize, 1), try runtime.enqueueTextInput("b"));

    var any = try captureWhenPhaseCommitted(runtime, "OBPW:ANY|", 2);
    defer any.deinit();
    try std.testing.expectEqual(.alternate, any.screen);
    try expectPointerMode(runtime, .any);
    try std.testing.expect(try runtime.mouseInputWouldEmit(hoverInput()));
    try std.testing.expect((try runtime.enqueueMouseInput(hoverInput())) > 0);

    var released = try captureWhenPhaseCommitted(runtime, "OBPW:RELEASED|", 3);
    defer released.deinit();
    try std.testing.expectEqual(.alternate, released.screen);
    try expectPointerMode(runtime, .released);
    try expectSilentHover(runtime);
    try std.testing.expectEqual(@as(usize, 1), try runtime.enqueueTextInput("q"));

    var restored = try captureWhenPhaseCommitted(runtime, "OBPW:RESTORED|", 4);
    defer restored.deinit();
    try std.testing.expectEqual(.primary, restored.screen);
    try expectPointerMode(runtime, .released);
    {
        const state = runtimeState(runtime);
        state.terminal_mutex.lock();
        defer state.terminal_mutex.unlock();
        try std.testing.expect(state.terminal.modes.get(.cursor_visible));
    }

    const completion = try runtime.waitForCompletion();
    try std.testing.expectEqual(
        pty_process.ExitResult{ .exit_code = 0 },
        completion.exit_result.?,
    );
    try std.testing.expect(completion.child_exited);
    try std.testing.expect(completion.pty_drained);
    try std.testing.expect(completion.writer_stopped);
    try runtime.shutdown();
    runtime.destroyAfterShutdown();
    destroy_required = false;
}
