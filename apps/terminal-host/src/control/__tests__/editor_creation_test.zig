const std = @import("std");
const control = @import("terminal-host-control");
const wire = control.testing.wire;
const allocator = std.testing.allocator;

fn editorPayload(source: anytype, immediate: bool) ![]u8 {
    const launch: wire.Launch = .{
        .arguments = &.{},
        .configuration = null,
        .currentDirectory = null,
        .environment = &.{},
        .executable = "editor.exe",
        .columns = if (immediate) 80 else null,
        .rows = if (immediate) 24 else null,
        .cellPixelWidth = if (immediate) 0 else null,
        .cellPixelHeight = if (immediate) 0 else null,
        .terminalProfile = .{
            .colorScheme = "dark",
            .defaultBackgroundRgb = 0,
            .defaultCursorRgb = null,
            .defaultForegroundRgb = 0xffffff,
            .deviceAttributes = .{
                .primaryConformanceLevel = 62,
                .primaryFeatures = &.{22},
                .secondaryDeviceType = 1,
                .secondaryFirmwareVersion = 0,
                .secondaryRomCartridge = 0,
                .tertiaryUnitId = 0,
            },
            .enquiryResponse = "",
            .palette = &[_]u32{0} ** 256,
            .versionResponse = "editor capture fixture",
        },
    };
    return std.json.Stringify.valueAlloc(allocator, .{
        .captureSource = source,
        .creationId = "33333333333333333333333333333333",
        .environmentOverlay = [_]wire.EnvironmentOverlay{},
        .launch = launch,
    }, .{});
}

fn sourceValue(generation: []const u8, revision: []const u8) wire.ScrollbackSource {
    return .{
        .attachmentId = "66666666666666666666666666666666",
        .terminalId = "44444444444444444444444444444444",
        .generation = generation,
        .revision = revision,
    };
}

fn expectRefused(expected: anyerror, payload: []const u8) !void {
    if (control.decode.decodeCreateTerminal(allocator, payload)) |accepted| {
        var decoded = accepted;
        defer decoded.deinit();
        return error.UnexpectedEditorCaptureAcceptance;
    } else |err| try std.testing.expectEqual(expected, err);
}

test "editor capture control decodes full uint64 fences and deferred launch without a document path" {
    for ([_][]const u8{ "0", "18446744073709551615" }) |revision| {
        const payload = try editorPayload(sourceValue("18446744073709551615", revision), false);
        defer allocator.free(payload);
        var decoded = try control.decode.decodeCreateTerminal(allocator, payload);
        defer decoded.deinit();
        try std.testing.expect(decoded.value.launch.launch.geometry == null);
        try std.testing.expectEqual(@as(u64, std.math.maxInt(u64)), decoded.value.capture_source.?.generation);
        try std.testing.expectEqual(try std.fmt.parseInt(u64, revision, 10), decoded.value.capture_source.?.revision);
        try std.testing.expectEqual(@as(usize, 0), decoded.value.launch.launch.arguments.len);
    }
}

test "editor capture control refuses malformed source null unknown fields and immediate geometry" {
    for ([_][]const u8{ "0", "01", "1e2", "18446744073709551616" }) |generation| {
        const payload = try editorPayload(sourceValue(generation, "0"), false);
        defer allocator.free(payload);
        try expectRefused(error.InvalidField, payload);
    }
    for ([_][]const u8{ "-1", "00", "1e2", "18446744073709551616" }) |revision| {
        const payload = try editorPayload(sourceValue("1", revision), false);
        defer allocator.free(payload);
        try expectRefused(error.InvalidField, payload);
    }
    const null_payload = try editorPayload(@as(?u8, null), false);
    defer allocator.free(null_payload);
    try expectRefused(error.InvalidField, null_payload);
    const unknown = try editorPayload(.{
        .attachmentId = "66666666666666666666666666666666",
        .terminalId = "44444444444444444444444444444444",
        .generation = "1",
        .revision = "0",
        .path = "C:\\foreign.txt",
    }, false);
    defer allocator.free(unknown);
    try expectRefused(error.InvalidField, unknown);
    const immediate = try editorPayload(sourceValue("1", "0"), true);
    defer allocator.free(immediate);
    try expectRefused(error.InvalidField, immediate);
    var invalid = sourceValue("1", "0");
    invalid.attachmentId = "00000000000000000000000000000000";
    const invalid_identity = try editorPayload(invalid, false);
    defer allocator.free(invalid_identity);
    try expectRefused(error.InvalidIdentifier, invalid_identity);
}
