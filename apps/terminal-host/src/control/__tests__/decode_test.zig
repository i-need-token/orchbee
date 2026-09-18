const std = @import("std");
const control_boundary = @import("terminal-host-control");
const protocol = @import("terminal-protocol");

const subject = control_boundary.decode;
const wire = control_boundary.testing.wire;

const attachment_id = "11111111111111111111111111111111";

test "CreateTerminal preflights environment count before translation allocation" {
    const limit = protocol.Limits.launch_environment_entries;
    const entry: wire.Environment = .{ .name = "ORCHBEE", .value = null };
    const exact = [_]wire.Environment{entry} ** limit;
    const translated = try control_boundary.testing.decode_terminal.translateEnvironmentForTest(
        std.testing.allocator,
        &exact,
    );
    defer std.testing.allocator.free(translated);
    try std.testing.expectEqual(limit, translated.len);

    const excessive = [_]wire.Environment{entry} ** (limit + 1);
    var failing = std.testing.FailingAllocator.init(
        std.testing.allocator,
        .{ .fail_index = 0 },
    );
    try std.testing.expectError(
        error.InvalidField,
        control_boundary.testing.decode_terminal.translateEnvironmentForTest(
            failing.allocator(),
            &excessive,
        ),
    );
    try std.testing.expect(!failing.has_induced_failure);
}

test "launch geometry decode admits paired-zero pixels and rejects mixed pairs" {
    const subject_testing = control_boundary.testing.decode_terminal;
    const profile: wire.TerminalProfile = .{
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
        .versionResponse = "test",
    };
    const base: wire.Launch = .{
        .arguments = &.{},
        .cellPixelHeight = 0,
        .cellPixelWidth = 0,
        .columns = 80,
        .configuration = null,
        .currentDirectory = null,
        .environment = &.{},
        .executable = "probe",
        .rows = 24,
        .terminalProfile = profile,
    };
    const paired_zero = try subject_testing.optionalLaunchGeometryForTest(base);
    try std.testing.expectEqual(@as(u16, 0), paired_zero.?.cell_pixel_height);
    try std.testing.expectEqual(@as(u16, 0), paired_zero.?.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 80), paired_zero.?.columns);
    try std.testing.expectEqual(@as(u16, 24), paired_zero.?.rows);

    var mixed = base;
    mixed.cellPixelWidth = 9;
    try std.testing.expectError(error.InvalidField, subject_testing.optionalLaunchGeometryForTest(mixed));
    var mixed_height = base;
    mixed_height.cellPixelHeight = 18;
    try std.testing.expectError(error.InvalidField, subject_testing.optionalLaunchGeometryForTest(mixed_height));
    var zero_columns = base;
    zero_columns.columns = 0;
    try std.testing.expectError(error.InvalidField, subject_testing.optionalLaunchGeometryForTest(zero_columns));
    // An entirely absent geometry stays absent rather than materializing one.
    var absent_launch = base;
    absent_launch.cellPixelHeight = null;
    absent_launch.cellPixelWidth = null;
    absent_launch.columns = null;
    absent_launch.rows = null;
    const absent = try subject_testing.optionalLaunchGeometryForTest(absent_launch);
    try std.testing.expect(absent == null);
}

test "Hello optional feature names use a bounded UTF-8 byte length" {
    const ascii = [_][]const u8{"a"};
    var hello: wire.Hello = .{
        .clientId = "11111111111111111111111111111111",
        .frameBytes = 1024,
        .inputBytes = 1024,
        .protocol = .{ .major = 0, .minor = 0, .patch = 7 },
        .rowWindowBytes = 1024,
        .supportedOptionalFeatures = &ascii,
    };
    try std.testing.expect(subject.validHello(hello));
    const empty = [_][]const u8{""};
    hello.supportedOptionalFeatures = &empty;
    try std.testing.expect(!subject.validHello(hello));
    const combining_21 = "e\u{0301}" ** 21;
    const combining_feature = [_][]const u8{combining_21[0..]};
    hello.supportedOptionalFeatures = &combining_feature;
    try std.testing.expect(subject.validHello(hello));
    const sixteen_emojis = "😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀";
    const seventeen_emojis = "😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀";
    const features = [_][]const u8{sixteen_emojis};
    hello.supportedOptionalFeatures = &features;
    try std.testing.expect(subject.validHello(hello));
    const oversized_features = [_][]const u8{seventeen_emojis};
    hello.supportedOptionalFeatures = &oversized_features;
    try std.testing.expect(!subject.validHello(hello));
    var many_features: [65][]const u8 = undefined;
    @memset(&many_features, "x");
    hello.supportedOptionalFeatures = &many_features;
    try std.testing.expect(!subject.validHello(hello));
}

test "typed control decoder distinguishes identifiers from field failures" {
    try std.testing.expectError(
        error.InvalidIdentifier,
        subject.decodeFrameApplied(
            std.testing.allocator,
            "{\"attachmentId\":\"00000000000000000000000000000000\",\"generation\":\"1\",\"revision\":\"2\",\"sequence\":\"3\"}",
        ),
    );
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeFrameApplied(
            std.testing.allocator,
            "{\"attachmentId\":\"11111111111111111111111111111111\",\"generation\":\"01\",\"revision\":\"2\",\"sequence\":\"3\"}",
        ),
    );
}

test "canonical uint64 text rejects alternate spellings and overflow" {
    try std.testing.expectEqual(@as(u64, 0), try subject.parseCanonicalU64("0"));
    try std.testing.expectEqual(
        std.math.maxInt(u64),
        try subject.parseCanonicalU64("18446744073709551615"),
    );
    try std.testing.expectError(error.InvalidField, subject.parseCanonicalU64("00"));
    try std.testing.expectError(error.InvalidField, subject.parseCanonicalU64("+1"));
    try std.testing.expectError(
        error.InvalidField,
        subject.parseCanonicalU64("18446744073709551616"),
    );
}

test "typed decoder rejects zero generation and attachment-local flight sequences" {
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeFrameApplied(
            std.testing.allocator,
            "{\"attachmentId\":\"11111111111111111111111111111111\",\"generation\":\"0\",\"revision\":\"0\",\"sequence\":\"1\"}",
        ),
    );
    const zero_row_sequence = std.fmt.comptimePrint(
        "{{\"attachmentId\":\"{s}\",\"detail\":\"text\",\"generation\":\"1\",\"maximumBytes\":1024,\"origin\":{{\"kind\":\"tail\",\"rowsBefore\":0}},\"purpose\":\"viewport\",\"requestSequence\":\"0\",\"revision\":\"0\",\"revisionPolicy\":\"current-view\",\"rowCount\":1}}",
        .{attachment_id},
    );
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeReadRows(std.testing.allocator, zero_row_sequence),
    );
    const zero_export_sequence = std.fmt.comptimePrint(
        "{{\"attachmentId\":\"{s}\",\"exportSequence\":\"0\",\"format\":\"plain\"}}",
        .{attachment_id},
    );
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeSelectionExport(std.testing.allocator, zero_export_sequence),
    );
}

test "selection export decoder admits only the plain product format" {
    const plain_payload = std.fmt.comptimePrint(
        "{{\"attachmentId\":\"{s}\",\"exportSequence\":\"1\",\"format\":\"plain\"}}",
        .{attachment_id},
    );
    var plain = try subject.decodeSelectionExport(std.testing.allocator, plain_payload);
    defer plain.deinit();
    try std.testing.expectEqual(.plain, plain.value.format);

    inline for (.{ "html", "vt" }) |format| {
        const unsupported_payload = try std.fmt.allocPrint(
            std.testing.allocator,
            "{{\"attachmentId\":\"{s}\",\"exportSequence\":\"1\",\"format\":\"{s}\"}}",
            .{ attachment_id, format },
        );
        defer std.testing.allocator.free(unsupported_payload);
        try std.testing.expectError(
            error.InvalidField,
            subject.decodeSelectionExport(std.testing.allocator, unsupported_payload),
        );
    }
}

test "typed decoder enforces discriminated row origins" {
    const valid_payload = std.fmt.comptimePrint(
        "{{\"attachmentId\":\"{s}\",\"detail\":\"text\",\"generation\":\"1\",\"maximumBytes\":1024,\"origin\":{{\"kind\":\"tail\",\"rowsBefore\":4}},\"purpose\":\"viewport\",\"requestSequence\":\"2\",\"revision\":\"3\",\"revisionPolicy\":\"current-view\",\"rowCount\":12}}",
        .{attachment_id},
    );
    var decoded = try subject.decodeReadRows(std.testing.allocator, valid_payload);
    defer decoded.deinit();
    try std.testing.expectEqual(@as(u16, 12), decoded.value.row_count);
    try std.testing.expectEqual(@as(u32, 4), decoded.value.origin.tail);
    try std.testing.expectEqual(.current_view, decoded.value.revision_policy);

    const extra_variant_field = std.fmt.comptimePrint(
        "{{\"attachmentId\":\"{s}\",\"detail\":\"text\",\"generation\":\"1\",\"maximumBytes\":1024,\"origin\":{{\"kind\":\"tail\",\"row\":\"4\",\"rowsBefore\":4}},\"purpose\":\"viewport\",\"requestSequence\":\"2\",\"revision\":\"3\",\"revisionPolicy\":\"current-view\",\"rowCount\":12}}",
        .{attachment_id},
    );
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeReadRows(std.testing.allocator, extra_variant_field),
    );

    const current_view_absolute = std.fmt.comptimePrint(
        "{{\"attachmentId\":\"{s}\",\"detail\":\"text\",\"generation\":\"1\",\"maximumBytes\":1024,\"origin\":{{\"kind\":\"absolute\",\"row\":\"0\"}},\"purpose\":\"viewport\",\"requestSequence\":\"2\",\"revision\":\"3\",\"revisionPolicy\":\"current-view\",\"rowCount\":1}}",
        .{attachment_id},
    );
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeReadRows(std.testing.allocator, current_view_absolute),
    );

    const exact_absolute = std.fmt.comptimePrint(
        "{{\"attachmentId\":\"{s}\",\"detail\":\"text\",\"generation\":\"1\",\"maximumBytes\":1024,\"origin\":{{\"kind\":\"absolute\",\"row\":\"0\"}},\"purpose\":\"search\",\"requestSequence\":\"2\",\"revision\":\"3\",\"revisionPolicy\":\"exact\",\"rowCount\":1}}",
        .{attachment_id},
    );
    var exact = try subject.decodeReadRows(std.testing.allocator, exact_absolute);
    defer exact.deinit();
    try std.testing.expectEqual(.exact, exact.value.revision_policy);

    const excessive_delta: wire.RowOrigin = .{
        .delta = .{ .value = 4097 },
        .kind = "committed",
    };
    try std.testing.expectError(error.InvalidField, subject.parseRowOrigin(excessive_delta));
}

test "typed decoder closes native validation gaps from the TypeScript schema" {
    const zero_rows = std.fmt.comptimePrint(
        "{{\"attachmentId\":\"{s}\",\"detail\":\"text\",\"generation\":\"1\",\"maximumBytes\":1024,\"origin\":{{\"kind\":\"tail\",\"rowsBefore\":0}},\"purpose\":\"viewport\",\"requestSequence\":\"2\",\"revision\":\"3\",\"revisionPolicy\":\"current-view\",\"rowCount\":0}}",
        .{attachment_id},
    );
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeReadRows(std.testing.allocator, zero_rows),
    );

    const zero_clicks = std.fmt.comptimePrint(
        "{{\"action\":\"start\",\"attachmentId\":\"{s}\",\"clickCount\":0,\"generation\":\"1\",\"point\":{{\"column\":0,\"row\":0}},\"position\":{{\"x\":1,\"y\":1}},\"selectOutput\":false}}",
        .{attachment_id},
    );
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeSelectionGesture(std.testing.allocator, zero_clicks),
    );

    const wide_point = std.fmt.comptimePrint(
        "{{\"action\":\"start\",\"attachmentId\":\"{s}\",\"clickCount\":1,\"generation\":\"1\",\"point\":{{\"column\":4096,\"row\":0}},\"position\":{{\"x\":1,\"y\":1}},\"selectOutput\":false}}",
        .{attachment_id},
    );
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeSelectionGesture(std.testing.allocator, wide_point),
    );

    const start_with_update_fields = std.fmt.comptimePrint(
        "{{\"action\":\"start\",\"attachmentId\":\"{s}\",\"clickCount\":1,\"generation\":\"1\",\"geometry\":{{\"cellWidth\":8,\"columns\":80,\"paddingLeft\":0,\"screenHeight\":320}},\"point\":{{\"column\":0,\"row\":0}},\"position\":{{\"x\":1,\"y\":1}},\"selectOutput\":false}}",
        .{attachment_id},
    );
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeSelectionGesture(std.testing.allocator, start_with_update_fields),
    );

    const cancel_with_point = std.fmt.comptimePrint(
        "{{\"action\":\"cancel\",\"attachmentId\":\"{s}\",\"generation\":\"1\",\"point\":{{\"column\":0,\"row\":0}}}}",
        .{attachment_id},
    );
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeSelectionGesture(std.testing.allocator, cancel_with_point),
    );

    const empty_capabilities = std.fmt.comptimePrint(
        "{{\"attachmentId\":\"{s}\",\"capabilities\":{{\"input\":false,\"read\":false,\"resize\":false}},\"requestResizeLease\":false,\"terminalId\":\"22222222222222222222222222222222\"}}",
        .{attachment_id},
    );
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeAttachTerminal(std.testing.allocator, empty_capabilities),
    );

    const lease_without_resize = std.fmt.comptimePrint(
        "{{\"attachmentId\":\"{s}\",\"capabilities\":{{\"input\":true,\"read\":false,\"resize\":false}},\"requestResizeLease\":true,\"terminalId\":\"22222222222222222222222222222222\"}}",
        .{attachment_id},
    );
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeAttachTerminal(std.testing.allocator, lease_without_resize),
    );
}

test "selection command decoder rejects illegal union combinations" {
    const null_clear = std.fmt.comptimePrint(
        "{{\"attachmentId\":\"{s}\",\"command\":\"clear\",\"generation\":\"1\",\"point\":null,\"revision\":\"1\"}}",
        .{attachment_id},
    );
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeSelectionCommand(std.testing.allocator, null_clear),
    );

    const missing_select_output = std.fmt.comptimePrint(
        "{{\"attachmentId\":\"{s}\",\"command\":\"select-output\",\"generation\":\"1\",\"revision\":\"1\"}}",
        .{attachment_id},
    );
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeSelectionCommand(std.testing.allocator, missing_select_output),
    );

    const missing_range_end = std.fmt.comptimePrint(
        "{{\"attachmentId\":\"{s}\",\"command\":\"set-range\",\"generation\":\"1\",\"rectangle\":false,\"revision\":\"1\",\"start\":{{\"column\":0,\"row\":\"0\"}}}}",
        .{attachment_id},
    );
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeSelectionCommand(std.testing.allocator, missing_range_end),
    );

    const range_with_point = std.fmt.comptimePrint(
        "{{\"attachmentId\":\"{s}\",\"command\":\"set-range\",\"end\":{{\"column\":2,\"row\":\"0\"}},\"generation\":\"1\",\"point\":{{\"column\":1,\"row\":\"0\"}},\"rectangle\":false,\"revision\":\"1\",\"start\":{{\"column\":0,\"row\":\"0\"}}}}",
        .{attachment_id},
    );
    try std.testing.expectError(
        error.InvalidField,
        subject.decodeSelectionCommand(std.testing.allocator, range_with_point),
    );
}

test "selection command decoder accepts one explicit range shape" {
    const range = std.fmt.comptimePrint(
        "{{\"attachmentId\":\"{s}\",\"command\":\"set-range\",\"end\":{{\"column\":4,\"row\":\"12\"}},\"generation\":\"3\",\"rectangle\":true,\"revision\":\"9\",\"start\":{{\"column\":2,\"row\":\"10\"}}}}",
        .{attachment_id},
    );
    var decoded = try subject.decodeSelectionCommand(std.testing.allocator, range);
    defer decoded.deinit();
    try std.testing.expectEqual(@as(u16, 2), decoded.value.command.set_range.start.column);
    try std.testing.expectEqual(@as(u64, 10), decoded.value.command.set_range.start.row);
    try std.testing.expectEqual(@as(u16, 4), decoded.value.command.set_range.end.column);
    try std.testing.expectEqual(@as(u64, 12), decoded.value.command.set_range.end.row);
    try std.testing.expect(decoded.value.command.set_range.rectangle);
}
