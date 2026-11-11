const harness = @import("runtime_harness.zig");
const std = harness.std;
const protocol = harness.protocol;
const terminal_theme = harness.model_boundary.theme;
const AttachmentId = harness.AttachmentId;
const HostEpoch = harness.HostEpoch;
const ReaderBatchOutcome = harness.ReaderBatchOutcome;
const commitReaderBatch = harness.commitReaderBatch;
const cancelInput = harness.cancelInput;
const createIdleRuntime = harness.createIdleRuntime;
const createIdleRuntimeWithGeometryAndHooks = harness.createIdleRuntimeWithGeometryAndHooks;
const expectNextQueuedTerminalBytes = harness.expectNextQueuedTerminalBytes;
const expectNoQueuedTerminalBytes = harness.expectNoQueuedTerminalBytes;
const max_pending_terminal_reply_bytes = harness.max_pending_terminal_reply_bytes;
const reserveInput = harness.reserveInput;
const GatedPtyProbe = harness.GatedPtyProbe;
const WriterProbe = harness.WriterProbe;
const requireProjectionFrame = harness.requireProjectionFrame;
const runtimeState = harness.runtimeState;

const initial_palette = [_]u32{0} ** 256;
const replacement_palette = palette: {
    var palette: [256]u32 = undefined;
    for (&palette, 0..) |*color, index| color.* = @intCast(index * 0x01_01_01);
    break :palette palette;
};
const replacement: terminal_theme.Theme = .{
    .color_scheme = .light,
    .default_background_rgb = 0xee_ee_ee,
    .default_cursor_rgb = 0x12_34_56,
    .default_foreground_rgb = 0x11_11_11,
    .palette = replacement_palette,
};

test "runtime theme replacement is atomic and establishes the next dynamic-color defaults" {
    const runtime = try createIdleRuntime(.{ .profile = .{
        .color_scheme = .dark,
        .default_background_rgb = 0,
        .default_cursor_rgb = null,
        .default_foreground_rgb = 0xff_ff_ff,
        .device_primary_conformance_level = 62,
        .device_primary_features = &.{22},
        .device_secondary_type = 1,
        .device_secondary_firmware_version = 0,
        .device_secondary_rom_cartridge = 0,
        .device_tertiary_unit_id = 0,
        .enquiry_response = "",
        .palette = &initial_palette,
        .version_response = "Orchbee",
    } });
    defer runtime.destroy();

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(
            runtime,
            "\x1b]10;#010203\x1b\\\x1b]11;#040506\x1b\\" ++
                "\x1b]12;#070809\x1b\\\x1b]4;42;#0a0b0c\x1b\\",
        ),
    );
    const before = runtime.geometry();
    const update = try runtime.applyTheme(replacement);

    try std.testing.expectEqual(before.generation, update.generation);
    try std.testing.expectEqual(before.revision + 1, update.revision);
    try std.testing.expect(terminal_theme.eql(replacement, runtime.themeDefaults()));
    try expectRuntimeColors(runtime, replacement, true);

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(
            runtime,
            "\x1b]10;#a1a2a3\x1b\\\x1b]11;#b1b2b3\x1b\\" ++
                "\x1b]12;#c1c2c3\x1b\\\x1b]4;42;#d1d2d3\x1b\\",
        ),
    );
    try expectRuntimeColors(runtime, replacement, false);

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b]110\x1b\\\x1b]111\x1b\\\x1b]112\x1b\\\x1b]104\x1b\\"),
    );
    try expectRuntimeColors(runtime, replacement, true);

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b]10;#010101\x1b\\\x1b]4;42;#020202\x1b\\"),
    );
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1bc"),
    );
    try expectRuntimeColors(runtime, replacement, true);

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b]10;#030303\x1b\\\x1b]4;42;#040404\x1b\\"),
    );
    _ = try runtime.applyTerminalControl(.reset);
    try expectRuntimeColors(runtime, replacement, true);
}

test "failed runtime theme preflight preserves every prior default" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    const initial = runtime.themeDefaults();
    const state = runtimeState(runtime);
    state.revision.store(std.math.maxInt(u64), .release);

    try std.testing.expectError(error.RevisionOverflow, runtime.applyTheme(replacement));
    try std.testing.expect(terminal_theme.eql(initial, runtime.themeDefaults()));
}

test "default color projection same-theme application clears equal-RGB OSC overrides" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    _ = try runtime.applyTheme(replacement);
    const inputs = [_][]const u8{
        "\x1b]10;#111111\x1b\\",
        "\x1b]11;#eeeeee\x1b\\",
        "\x1b]12;#123456\x1b\\",
    };
    for (inputs) |input| {
        try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, input));
        const before = runtime.geometry();
        const update = try runtime.applyTheme(replacement);
        try std.testing.expectEqual(before.generation, update.generation);
        try std.testing.expectEqual(before.revision + 1, update.revision);
        try expectRuntimeColors(runtime, replacement, true);
        const unchanged = try runtime.applyTheme(replacement);
        try std.testing.expectEqual(update.revision, unchanged.revision);
    }

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, inputs[0]));
    const state = runtimeState(runtime);
    state.revision.store(std.math.maxInt(u64), .release);
    try std.testing.expectError(error.RevisionOverflow, runtime.applyTheme(replacement));
    state.terminal_mutex.lock();
    const override_preserved = state.terminal.colors.foreground.override != null;
    state.terminal_mutex.unlock();
    try std.testing.expect(override_preserved);
    try std.testing.expect(terminal_theme.eql(replacement, runtime.themeDefaults()));
}

test "one theme mutation is projected at the same revision to two attachments" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    const update = try runtime.applyTheme(replacement);

    const first = try projection(runtime, 0x11, 1);
    defer std.testing.allocator.free(first.allocation);
    const second = try projection(runtime, 0x22, 1);
    defer std.testing.allocator.free(second.allocation);

    try std.testing.expectEqual(update.revision, first.revision);
    try std.testing.expectEqual(first.revision, second.revision);
    try std.testing.expectEqual(first.generation, second.generation);
    try std.testing.expectEqual(replacement.default_foreground_rgb, readU32(first.bytes, 104));
    try std.testing.expectEqual(replacement.default_background_rgb, readU32(first.bytes, 108));
    try std.testing.expectEqual(replacement.default_cursor_rgb.?, readU32(first.bytes, 112));
    try std.testing.expectEqualSlices(u8, first.bytes[104..116], second.bytes[104..116]);
}

test "color-scheme change queues its report only for a program that enabled mode 2031" {
    var probe: GatedPtyProbe(WriterProbe) = .{ .probe = .{ .pause_at = .copied } };
    const runtime = try createIdleRuntimeWithGeometryAndHooks(80, 24, .{}, probe.hooks());
    probe.runtime = runtime;
    defer {
        probe.reader.release();
        probe.probe.copied.release();
        runtime.destroy();
    }
    probe.reader.wait();

    var dark = replacement;
    dark.color_scheme = .dark;
    try std.testing.expectEqual(terminal_theme.ColorScheme.dark, runtime.themeDefaults().color_scheme);

    // Without mode 2031 the program is not told about the appearance change.
    _ = try runtime.applyTheme(replacement);
    try expectNoQueuedTerminalBytes(runtime);

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?2031h"),
    );
    _ = try runtime.applyTheme(dark);
    cancelInput(runtime, try expectNextQueuedTerminalBytes(runtime, "\x1b[?997;1n"));
    try expectNoQueuedTerminalBytes(runtime);
}

test "mode 2031 reports only effective appearance changes, not palette-only edits" {
    var probe: GatedPtyProbe(WriterProbe) = .{ .probe = .{ .pause_at = .copied } };
    const runtime = try createIdleRuntimeWithGeometryAndHooks(80, 24, .{}, probe.hooks());
    probe.runtime = runtime;
    defer {
        probe.reader.release();
        probe.probe.copied.release();
        runtime.destroy();
    }
    probe.reader.wait();

    _ = try runtime.applyTheme(replacement);
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?2031h"),
    );

    // The appearance stays light, so only the palette changes and nothing is reported.
    var palette_only = replacement;
    palette_only.palette[42] = 0x01_02_03;
    _ = try runtime.applyTheme(palette_only);
    try expectNoQueuedTerminalBytes(runtime);

    var dark = palette_only;
    dark.color_scheme = .dark;
    _ = try runtime.applyTheme(dark);
    cancelInput(runtime, try expectNextQueuedTerminalBytes(runtime, "\x1b[?997;1n"));

    _ = try runtime.applyTheme(replacement);
    cancelInput(runtime, try expectNextQueuedTerminalBytes(runtime, "\x1b[?997;2n"));
}

test "mode 2031 ends at reset-mode, RIS, or a Host reset, and DECRQM reports the subscription" {
    var probe: GatedPtyProbe(WriterProbe) = .{ .probe = .{ .pause_at = .copied } };
    const runtime = try createIdleRuntimeWithGeometryAndHooks(80, 24, .{}, probe.hooks());
    probe.runtime = runtime;
    defer {
        probe.reader.release();
        probe.probe.copied.release();
        runtime.destroy();
    }
    probe.reader.wait();

    var dark = replacement;
    dark.color_scheme = .dark;
    const endings = [_][]const u8{ "\x1b[?2031l", "\x1bc", "" };
    var light = true;
    for (endings) |ending| {
        try std.testing.expectEqual(
            ReaderBatchOutcome.committed,
            commitReaderBatch(runtime, "\x1b[?2031h\x1b[?2031$p"),
        );
        cancelInput(runtime, try expectNextQueuedTerminalBytes(runtime, "\x1b[?2031;1$y"));

        // An empty ending stands for the Host-driven reset control.
        if (ending.len == 0) {
            _ = try runtime.applyTerminalControl(.reset);
        } else {
            try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, ending));
        }
        _ = try runtime.applyTheme(if (light) replacement else dark);
        light = !light;
        try expectNoQueuedTerminalBytes(runtime);

        try std.testing.expectEqual(
            ReaderBatchOutcome.committed,
            commitReaderBatch(runtime, "\x1b[?2031$p"),
        );
        cancelInput(runtime, try expectNextQueuedTerminalBytes(runtime, "\x1b[?2031;2$y"));
    }
}

test "an unqueued color-scheme notice is dropped and the theme still applies" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    try std.testing.expectEqual(terminal_theme.ColorScheme.dark, runtime.themeDefaults().color_scheme);
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?2031h"),
    );

    // An unpublished reservation takes the whole reply budget, so the notice cannot be queued.
    const budget = try reserveInput(runtime, max_pending_terminal_reply_bytes, false, .terminal);
    defer cancelInput(runtime, budget.identifier);

    const before = runtime.geometry();
    const update = try runtime.applyTheme(replacement);
    try std.testing.expectEqual(before.revision + 1, update.revision);
    try std.testing.expect(terminal_theme.eql(replacement, runtime.themeDefaults()));

    // The terminal still accepts runtime work: the same theme is a no-op at the new revision.
    const unchanged = try runtime.applyTheme(replacement);
    try std.testing.expectEqual(update.revision, unchanged.revision);
}

fn expectRuntimeColors(
    runtime: anytype,
    expected: terminal_theme.Theme,
    expect_defaults_active: bool,
) !void {
    const state = runtimeState(runtime);
    state.terminal_mutex.lock();
    defer state.terminal_mutex.unlock();
    try std.testing.expectEqual(expected.color_scheme, state.profile.color_scheme);
    try std.testing.expectEqual(expected.default_background_rgb, rgbCode(state.terminal.colors.background.default.?));
    try std.testing.expectEqual(expected.default_foreground_rgb, rgbCode(state.terminal.colors.foreground.default.?));
    try std.testing.expectEqual(expected.default_cursor_rgb.?, rgbCode(state.terminal.colors.cursor.default.?));
    try std.testing.expectEqual(expected.palette[42], rgbCode(state.terminal.colors.palette.original[42]));
    if (expect_defaults_active) {
        try std.testing.expect(state.terminal.colors.background.override == null);
        try std.testing.expect(state.terminal.colors.foreground.override == null);
        try std.testing.expect(state.terminal.colors.cursor.override == null);
        try std.testing.expectEqual(expected.default_background_rgb, rgbCode(state.terminal.colors.background.get().?));
        try std.testing.expectEqual(expected.default_foreground_rgb, rgbCode(state.terminal.colors.foreground.get().?));
        try std.testing.expectEqual(expected.default_cursor_rgb.?, rgbCode(state.terminal.colors.cursor.get().?));
        try std.testing.expectEqual(@as(usize, 0), state.terminal.colors.palette.mask.count());
        try std.testing.expectEqual(expected.palette[42], rgbCode(state.terminal.colors.palette.current[42]));
    } else {
        try std.testing.expect(state.terminal.colors.background.override != null);
        try std.testing.expect(state.terminal.colors.foreground.override != null);
        try std.testing.expect(state.terminal.colors.cursor.override != null);
        try std.testing.expect(state.terminal.colors.palette.mask.isSet(42));
    }
}

fn projection(runtime: anytype, attachment_byte: u8, sequence: u64) !harness.subject.ProjectionFrame {
    return requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = AttachmentId.fromBytes([_]u8{attachment_byte} ** 16),
            .base_generation = null,
            .base_revision = null,
            .force_snapshot = true,
            .host_epoch = HostEpoch.fromBytes([_]u8{0xaa} ** 16),
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = sequence,
        },
    ));
}

fn rgbCode(color: anytype) u32 {
    return (@as(u32, color.r) << 16) | (@as(u32, color.g) << 8) | color.b;
}

fn readU32(bytes: []const u8, offset: usize) u32 {
    return std.mem.readInt(u32, bytes[offset..][0..4], .little);
}
