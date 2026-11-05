const std = @import("std");
const builtin = @import("builtin");
const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");
const engine_testing = @import("terminal-host-engine").testing;
const png_decoder = engine_testing.png_decoder;
const subject = engine_testing.projection;

const FrameKind = subject.FrameKind;
const GraphicsState = subject.testing.GraphicsStateType;
const max_frame_bytes = subject.testing.frame_byte_limit;
const max_link_cache_bytes = subject.testing.link_cache_byte_limit;
const max_link_entries = subject.testing.link_entry_limit;
const max_link_value_bytes = subject.testing.link_value_byte_limit;
const capturedStyle = subject.testing.capturedStyleFn;
const LinkTable = subject.testing.LinkTableType;
const LinkStringMap = subject.testing.LinkStringMapType;
const SourceLinkCache = subject.testing.SourceLinkCacheType;
const SourceLinkKey = subject.testing.SourceLinkKeyType;
const linkTableDeinit = subject.testing.linkTableDeinitFn;
const linkTableInit = subject.testing.linkTableInitFn;
const linkTableIntern = subject.testing.linkTableInternFn;
const linkTableInternCallCount = subject.testing.linkTableInternCallCountFn;
const linkWorkObservation = subject.testing.linkWorkObservationFn;
const Projection = subject.Projection;
const StringTable = subject.testing.StringTableType;
const AttachmentId = subject.AttachmentId;
const HostEpoch = subject.HostEpoch;

test "palette cell tags preserve ANSI provenance and independent style attributes" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{ .cols = 16, .rows = 3 });
    defer terminal.deinit(std.testing.allocator);
    var stream = terminal.vtStream();
    defer stream.deinit();
    for (0..16) |index| {
        var buffer: [64]u8 = undefined;
        stream.nextSlice(try std.fmt.bufPrint(&buffer, "\x1b[38;5;{d};48;5;{d}mX", .{ index, index }));
    }
    stream.nextSlice("\r\n\x1b[0;1;2;7;31;44;4;58;5;2mI" ++
        "\x1b[0;38;5;16;48;5;255;4;58;5;16mE" ++
        "\x1b[0;38;2;0;0;0;48;2;1;2;3mR" ++
        "\x1b[0;44m\x1b[X");
    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    const state = &projection.render_state.?;
    const ansi = state.row_data.items(.cells)[0];
    for (0..16) |index| {
        const style = subject.testing.styleRecordFn(ansi.items(.raw)[index], capturedStyle(ansi, index), &state.colors, .{}, 0);
        try std.testing.expectEqual(protocol.color_palette | @as(u32, @intCast(index)), style.foreground);
        try std.testing.expectEqual(style.foreground, style.background);
    }
    const cells = state.row_data.items(.cells)[1];
    const inverted = subject.testing.styleRecordFn(cells.items(.raw)[0], capturedStyle(cells, 0), &state.colors, .{}, 0);
    try std.testing.expectEqual(protocol.color_palette | 1, inverted.foreground);
    try std.testing.expectEqual(protocol.color_palette | 4, inverted.background);
    try std.testing.expectEqual(protocol.color_palette | 2, inverted.underline);
    try std.testing.expectEqual(@as(u16, 1 | 4 | 16), inverted.flags);
    const extended = subject.testing.styleRecordFn(cells.items(.raw)[1], capturedStyle(cells, 1), &state.colors, .{}, 0);
    try std.testing.expectEqual(@as(u32, 0), extended.foreground);
    try std.testing.expectEqual(@as(u32, 0xeeeeee), extended.background);
    try std.testing.expectEqual(@as(u32, 0), extended.underline);
    const direct = subject.testing.styleRecordFn(cells.items(.raw)[2], capturedStyle(cells, 2), &state.colors, .{}, 0);
    try std.testing.expectEqual(@as(u32, 0), direct.foreground);
    try std.testing.expectEqual(@as(u32, 0x010203), direct.background);
    const erased = subject.testing.styleRecordFn(cells.items(.raw)[3], capturedStyle(cells, 3), &state.colors, .{}, 0);
    try std.testing.expectEqual(protocol.color_palette | 4, erased.background);
}

test "child OSC palette overrides resolve RGB until a slot or complete reset restores the tag" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{ .cols = 2, .rows = 1 });
    defer terminal.deinit(std.testing.allocator);
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice("\x1b[31;41;4;58;5;1mA");
    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    const steps = [_]struct { bytes: []const u8, color: u32 }{
        .{ .bytes = "", .color = protocol.color_palette | 1 },
        .{ .bytes = "\x1b]4;1;#123456\x1b\\", .color = 0x123456 },
        .{ .bytes = "\x1b]104;1\x1b\\", .color = protocol.color_palette | 1 },
        .{ .bytes = "\x1b]4;1;#abcdef\x1b\\", .color = 0xabcdef },
        .{ .bytes = "\x1b]104\x1b\\", .color = protocol.color_palette | 1 },
    };
    for (steps, 0..) |step, revision| {
        stream.nextSlice(step.bytes);
        try projection.beginCapture(&terminal, @intCast(revision + 1), 1, 8, 16);
        projection.endCapture();
        const state = &projection.render_state.?;
        const cells = state.row_data.items(.cells)[0];
        const style = subject.testing.styleRecordFn(cells.items(.raw)[0], capturedStyle(cells, 0), &state.colors, projection.captured_default_colors, 0);
        try std.testing.expectEqual(step.color, style.foreground);
        try std.testing.expectEqual(step.color, style.background);
        try std.testing.expectEqual(step.color, style.underline);
    }
}

test "no-demand projection owns no render state or frame allocation" {
    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try std.testing.expect(projection.render_state == null);
    try std.testing.expectEqual(@as(u64, 0), projection.counters.render_state_creations);
    try std.testing.expectEqual(@as(u64, 0), projection.counters.captures);
    try std.testing.expectEqual(@as(u64, 0), projection.counters.encodes);
}

test "projection captures Ghostty state into a valid OBTF 0.1.0 snapshot" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 4,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("A\x1b[31mB");

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    const frame = try projection.encode(.{
        .attachment_id = AttachmentId.fromBytes([_]u8{2} ** 16),
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = HostEpoch.fromBytes([_]u8{1} ** 16),
        .maximum_bytes = max_frame_bytes,
        .sequence = 1,
    });
    defer std.testing.allocator.free(frame.allocation);
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, frame.bytes);
    try std.testing.expectEqual(protocol.FrameKind.snapshot, summary.kind);
    try std.testing.expectEqual(@as(u64, 1), summary.revision);
    try std.testing.expectEqual(@as(u16, 4), summary.columns);
    try std.testing.expectEqual(@as(u16, 2), summary.rows);
    try std.testing.expectEqual(@as(u16, 2), summary.extension_count);
    try std.testing.expectEqual(protocol.TerminalProgressState.remove, summary.progress.state);
    try std.testing.expectEqual(@as(?u8, null), summary.progress.value);
    try std.testing.expectEqual(protocol.TerminalGraphicsUpdateKind.snapshot, summary.graphics.?.kind);
    try std.testing.expectEqual(@as(u16, 0), summary.graphics.?.image_count);
    try std.testing.expectEqual(@as(u16, 0), summary.graphics.?.placement_count);
}

test "projection snapshots Kitty resources and emits placement-only deltas" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 20,
        .kitty_image_loading_limits = .direct,
        .kitty_image_storage_limit = protocol.Limits.terminal_graphics_resource_bytes,
        .rows = 4,
    });
    defer terminal.deinit(std.testing.allocator);
    setTerminalCellSize(&terminal, 8, 16);
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice(
        "\x1b_Ga=T,t=d,f=24,i=1,p=1,s=1,v=2,c=10,r=1;////////\x1b\\",
    );

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    const snapshot = try projection.encode(.{
        .attachment_id = AttachmentId.fromBytes([_]u8{2} ** 16),
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = HostEpoch.fromBytes([_]u8{1} ** 16),
        .maximum_bytes = max_frame_bytes,
        .sequence = 1,
    });
    defer std.testing.allocator.free(snapshot.allocation);
    const snapshot_summary = try protocol.validateTerminalFrame(
        std.testing.allocator,
        snapshot.bytes,
    );
    try std.testing.expectEqual(protocol.FrameKind.snapshot, snapshot_summary.kind);
    try std.testing.expectEqual(protocol.TerminalGraphicsUpdateKind.snapshot, snapshot_summary.graphics.?.kind);
    try std.testing.expectEqual(@as(u16, 1), snapshot_summary.graphics.?.image_count);
    try std.testing.expectEqual(@as(u16, 1), snapshot_summary.graphics.?.placement_count);
    try std.testing.expectEqual(@as(u64, 1), snapshot_summary.graphics.?.resource_revision);
    try std.testing.expectEqual(@as(usize, 1), projection.graphics.images.items.len);
    try std.testing.expectEqualSlices(
        u8,
        &.{ 0xff, 0xff, 0xff, 0xff, 0xff, 0xff },
        projection.graphics.images.items[0].data,
    );
    try std.testing.expectEqual(@as(u32, 80), projection.graphics.placements.items[0].destination_width);
    try std.testing.expectEqual(@as(u32, 16), projection.graphics.placements.items[0].destination_height);

    stream.nextSlice("\x1b_Ga=p,i=1,p=2,c=5,r=1;\x1b\\");
    try projection.beginCapture(&terminal, 2, 1, 8, 16);
    projection.endCapture();
    const delta = try projection.encode(.{
        .attachment_id = AttachmentId.fromBytes([_]u8{2} ** 16),
        .base_generation = 1,
        .base_revision = 1,
        .force_snapshot = false,
        .host_epoch = HostEpoch.fromBytes([_]u8{1} ** 16),
        .maximum_bytes = max_frame_bytes,
        .sequence = 2,
    });
    defer std.testing.allocator.free(delta.allocation);
    const delta_summary = try protocol.validateTerminalFrame(std.testing.allocator, delta.bytes);
    try std.testing.expectEqual(protocol.FrameKind.delta, delta_summary.kind);
    try std.testing.expectEqual(protocol.TerminalGraphicsUpdateKind.placements, delta_summary.graphics.?.kind);
    try std.testing.expectEqual(@as(u16, 0), delta_summary.graphics.?.image_count);
    try std.testing.expectEqual(@as(u16, 2), delta_summary.graphics.?.placement_count);
    try std.testing.expectEqual(@as(u64, 1), delta_summary.graphics.?.resource_revision);

    stream.nextSlice("\x1b_Ga=T,t=d,f=24,i=1,p=1,s=1,v=2,c=10,r=1;AAAAAAAA\x1b\\");
    try projection.beginCapture(&terminal, 3, 1, 8, 16);
    projection.endCapture();
    const replacement = try projection.encode(.{
        .attachment_id = AttachmentId.fromBytes([_]u8{2} ** 16),
        .base_generation = 1,
        .base_revision = 2,
        .force_snapshot = false,
        .host_epoch = HostEpoch.fromBytes([_]u8{1} ** 16),
        .maximum_bytes = max_frame_bytes,
        .sequence = 3,
    });
    defer std.testing.allocator.free(replacement.allocation);
    const replacement_summary = try protocol.validateTerminalFrame(
        std.testing.allocator,
        replacement.bytes,
    );
    try std.testing.expectEqual(protocol.FrameKind.snapshot, replacement_summary.kind);
    try std.testing.expectEqual(@as(u64, 3), replacement_summary.graphics.?.resource_revision);
    try std.testing.expectEqualSlices(
        u8,
        &.{ 0, 0, 0, 0, 0, 0 },
        projection.graphics.images.items[0].data,
    );

    stream.nextSlice("\x1b_Ga=d,d=A;\x1b\\");
    try projection.beginCapture(&terminal, 4, 1, 8, 16);
    projection.endCapture();
    const deleted = try projection.encode(.{
        .attachment_id = AttachmentId.fromBytes([_]u8{2} ** 16),
        .base_generation = 1,
        .base_revision = 3,
        .force_snapshot = false,
        .host_epoch = HostEpoch.fromBytes([_]u8{1} ** 16),
        .maximum_bytes = max_frame_bytes,
        .sequence = 4,
    });
    defer std.testing.allocator.free(deleted.allocation);
    const deleted_summary = try protocol.validateTerminalFrame(std.testing.allocator, deleted.bytes);
    try std.testing.expectEqual(protocol.FrameKind.snapshot, deleted_summary.kind);
    try std.testing.expectEqual(@as(u16, 0), deleted_summary.graphics.?.image_count);
    try std.testing.expectEqual(@as(u16, 0), deleted_summary.graphics.?.placement_count);
    try std.testing.expectEqual(@as(u64, 4), deleted_summary.graphics.?.resource_revision);
}

test "Host-installed decoder projects direct Kitty PNG transmissions" {
    png_decoder.install();
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 4,
        .kitty_image_loading_limits = .direct,
        .kitty_image_storage_limit = protocol.Limits.terminal_graphics_resource_bytes,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    setTerminalCellSize(&terminal, 8, 16);
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice(
        "\x1b_Ga=T,t=d,f=100,i=1,p=1,c=1,r=1;" ++
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==" ++
            "\x1b\\",
    );

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();

    try std.testing.expectEqual(@as(usize, 1), projection.graphics.images.items.len);
    const image = projection.graphics.images.items[0];
    try std.testing.expectEqual(subject.testing.ImageFormat.rgba, image.format);
    try std.testing.expectEqual(@as(u32, 1), image.width);
    try std.testing.expectEqual(@as(u32, 1), image.height);
    try std.testing.expectEqualSlices(u8, &.{ 0xff, 0, 0, 0xff }, image.data);
}

test "Host-installed decoder rejects direct PNG dimensions beyond the resource budget" {
    png_decoder.install();
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 4,
        .kitty_image_loading_limits = .direct,
        .kitty_image_storage_limit = protocol.Limits.terminal_graphics_resource_bytes,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice(
        "\x1b_Ga=T,t=d,f=100,i=1,p=1,q=2;" ++
            "iVBORw0KGgoAAAANSUhEUgAABAEAAAQB" ++
            "\x1b\\",
    );

    try std.testing.expectEqual(@as(usize, 0), terminal.screens.active.kitty_images.images.count());
    try std.testing.expectEqual(@as(usize, 0), terminal.screens.active.kitty_images.placements.count());
}

test "Host-installed decoder admits a 1536 by 1360 PNG within the resource budget" {
    png_decoder.install();
    try std.testing.expectEqual(@as(usize, 400 * 1024 * 1024), png_decoder.acquisition_limit_bytes);
    const encoded = try storedRgbaPng(std.testing.allocator, 1536, 1360);
    defer std.testing.allocator.free(encoded);
    try std.testing.expect(encoded.len > 4 * 1024 * 1024);

    const decode = ghostty_vt.sys.decode_png orelse return error.TestUnexpectedResult;
    const decoded = try decode(std.testing.allocator, encoded);
    defer std.testing.allocator.free(decoded.data);

    try std.testing.expectEqual(@as(u32, 1536), decoded.width);
    try std.testing.expectEqual(@as(u32, 1360), decoded.height);
    try std.testing.expectEqual(@as(usize, 1536 * 1360 * 4), decoded.data.len);
}

test "projection never aliases equal Kitty generations across primary and alternate screens" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 4,
        .kitty_image_loading_limits = .direct,
        .kitty_image_storage_limit = protocol.Limits.terminal_graphics_resource_bytes,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    setTerminalCellSize(&terminal, 8, 16);
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice("\x1b_Ga=T,t=d,f=24,i=1,p=1,s=1,v=1;/wAA\x1b\\");

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    try std.testing.expectEqualSlices(u8, &.{ 0xff, 0, 0 }, projection.graphics.images.items[0].data);

    stream.nextSlice("\x1b[?1049h\x1b_Ga=T,t=d,f=24,i=1,p=1,s=1,v=1;AP8A\x1b\\");
    try projection.beginCapture(&terminal, 2, 2, 8, 16);
    projection.endCapture();
    try std.testing.expectEqual(@as(u64, 2), projection.graphics.resource_revision);
    try std.testing.expectEqualSlices(u8, &.{ 0, 0xff, 0 }, projection.graphics.images.items[0].data);
}

test "direct-only Kitty loading never reads file or shared-memory media" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 4,
        .kitty_image_loading_limits = .direct,
        .kitty_image_storage_limit = protocol.Limits.terminal_graphics_resource_bytes,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    var stream = terminal.vtStream();
    defer stream.deinit();

    inline for (.{ 'f', 't', 's' }) |medium| {
        var command_buffer: [128]u8 = undefined;
        const command = try std.fmt.bufPrint(
            &command_buffer,
            "\x1b_Ga=t,t={c},f=24,s=1,v=1,q=2;L2V0Yy9wYXNzd2Q=\x1b\\",
            .{medium},
        );
        stream.nextSlice(command);
    }
    try std.testing.expectEqual(@as(usize, 0), terminal.screens.active.kitty_images.images.count());
    try std.testing.expectEqual(@as(usize, 0), terminal.screens.active.kitty_images.placements.count());
}

test "projection owns decoded pixels loaded through file and temporary-file media" {
    var temporary = std.testing.tmpDir(.{});
    defer temporary.cleanup();
    try temporary.dir.writeFile(std.testing.io, .{
        .sub_path = "regular-rgb.data",
        .data = &.{ 0xff, 0, 0 },
    });
    try temporary.dir.writeFile(std.testing.io, .{
        .sub_path = "tty-graphics-protocol-temporary-rgb.data",
        .data = &.{ 0, 0xff, 0 },
    });

    var directory_buffer: [std.fs.max_path_bytes]u8 = undefined;
    const directory = directory_buffer[0..try temporary.dir.realPath(
        std.testing.io,
        &directory_buffer,
    )];
    var regular_path_buffer: [std.fs.max_path_bytes]u8 = undefined;
    const regular_path = regular_path_buffer[0..try temporary.dir.realPathFile(
        std.testing.io,
        "regular-rgb.data",
        &regular_path_buffer,
    )];
    var temporary_path_buffer: [std.fs.max_path_bytes]u8 = undefined;
    const temporary_path = temporary_path_buffer[0..try temporary.dir.realPathFile(
        std.testing.io,
        "tty-graphics-protocol-temporary-rgb.data",
        &temporary_path_buffer,
    )];

    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 4,
        .kitty_image_loading_limits = .allWithTempDir(directory),
        .kitty_image_storage_limit = protocol.Limits.terminal_graphics_resource_bytes,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    setTerminalCellSize(&terminal, 8, 16);
    var stream = terminal.vtStream();
    defer stream.deinit();
    try transmitPathGraphic(&stream, 'f', 1, regular_path);
    try transmitPathGraphic(&stream, 't', 2, temporary_path);

    try temporary.dir.access(std.testing.io, "regular-rgb.data", .{});
    try std.testing.expectError(
        error.FileNotFound,
        temporary.dir.access(
            std.testing.io,
            "tty-graphics-protocol-temporary-rgb.data",
            .{},
        ),
    );

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    try std.testing.expectEqual(@as(usize, 2), projection.graphics.images.items.len);
    try std.testing.expectEqualSlices(
        u8,
        &.{ 0xff, 0, 0 },
        projection.graphics.images.items[0].data,
    );
    try std.testing.expectEqualSlices(
        u8,
        &.{ 0, 0xff, 0 },
        projection.graphics.images.items[1].data,
    );
}

test "projection owns decoded pixels loaded through POSIX shared memory" {
    if (comptime @hasDecl(std.c, "shm_open") and builtin.os.tag != .windows) {
        var name_buffer: [128]u8 = undefined;
        const name = try std.fmt.bufPrintZ(
            &name_buffer,
            "/obkg-{x}",
            .{@intFromPtr(&name_buffer)},
        );
        const descriptor = std.c.shm_open(
            name,
            @as(c_int, @bitCast(std.c.O{
                .ACCMODE = .RDWR,
                .CREAT = true,
                .EXCL = true,
            })),
            @as(std.c.mode_t, 0o600),
        );
        try std.testing.expectEqual(std.posix.E.SUCCESS, std.posix.errno(descriptor));
        defer _ = std.c.shm_unlink(name);
        defer _ = std.c.close(descriptor);
        const pixels = [_]u8{ 0, 0, 0xff };
        try std.testing.expectEqual(
            @as(c_int, 0),
            std.c.ftruncate(descriptor, pixels.len),
        );
        const mapped = try std.posix.mmap(
            null,
            pixels.len,
            .{ .READ = true, .WRITE = true },
            std.c.MAP{ .TYPE = .SHARED },
            descriptor,
            0,
        );
        defer std.posix.munmap(mapped);
        @memcpy(mapped, &pixels);

        var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
            .cols = 4,
            .kitty_image_loading_limits = .{
                .file = false,
                .temporary_file = .disabled,
                .shared_memory = true,
            },
            .kitty_image_storage_limit = protocol.Limits.terminal_graphics_resource_bytes,
            .rows = 2,
        });
        defer terminal.deinit(std.testing.allocator);
        setTerminalCellSize(&terminal, 8, 16);
        var stream = terminal.vtStream();
        defer stream.deinit();
        try transmitPathGraphic(&stream, 's', 1, name);

        var projection = Projection.init(std.testing.allocator);
        defer projection.deinit();
        try projection.beginCapture(&terminal, 1, 1, 8, 16);
        projection.endCapture();
        try std.testing.expectEqual(@as(usize, 1), projection.graphics.images.items.len);
        try std.testing.expectEqualSlices(
            u8,
            &.{ 0, 0, 0xff },
            projection.graphics.images.items[0].data,
        );
    } else {
        return error.SkipZigTest;
    }
}

test "projection resolves relative Kitty placement chains to canonical coordinates" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 8,
        .kitty_image_loading_limits = .direct,
        .kitty_image_storage_limit = protocol.Limits.terminal_graphics_resource_bytes,
        .rows = 4,
    });
    defer terminal.deinit(std.testing.allocator);
    setTerminalCellSize(&terminal, 8, 16);
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice(
        "\x1b_Ga=t,t=d,f=24,s=1,v=1,i=1;/wAA\x1b\\" ++
            "\x1b_Ga=p,i=1,p=1,C=1,c=1,r=1\x1b\\" ++
            "\x1b_Ga=p,i=1,p=2,P=1,Q=1,H=3,V=2,c=1,r=1\x1b\\",
    );

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    try std.testing.expectEqual(@as(usize, 2), projection.graphics.placements.items.len);
    const parent = projection.graphics.placements.items[0];
    const child = projection.graphics.placements.items[1];
    try std.testing.expectEqual(@as(u32, 1), parent.placement_id);
    try std.testing.expectEqual(@as(i32, 0), parent.column);
    try std.testing.expectEqual(@as(i32, 0), parent.row);
    try std.testing.expectEqual(@as(u32, 2), child.placement_id);
    try std.testing.expectEqual(@as(i32, 3), child.column);
    try std.testing.expectEqual(@as(i32, 2), child.row);
}

test "projection resolves virtual Kitty placeholders into owned canonical placements" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 4,
        .kitty_image_loading_limits = .direct,
        .kitty_image_storage_limit = protocol.Limits.terminal_graphics_resource_bytes,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    setTerminalCellSize(&terminal, 8, 16);
    terminal.modes.set(.grapheme_cluster, true);
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice(
        "\x1b_Ga=T,t=d,f=24,s=1,v=1,i=1,U=1,c=1,r=1;/wAA\x1b\\" ++
            "\x1b[38;5;1m\u{10EEEE}\u{0305}\u{0305}\x1b[39m",
    );

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    try std.testing.expectEqual(@as(usize, 1), projection.graphics.images.items.len);
    try std.testing.expectEqual(@as(usize, 1), projection.graphics.placements.items.len);
    const placement = projection.graphics.placements.items[0];
    try std.testing.expectEqual(@as(u32, 1), placement.image_id);
    try std.testing.expectEqual(@as(i32, 0), placement.column);
    try std.testing.expectEqual(@as(i32, 0), placement.row);
    try std.testing.expectEqual(@as(i32, -1), placement.z);
}

test "projection ignores absent style data on background-only cells" {
    var cells: std.MultiArrayList(ghostty_vt.RenderState.Cell) = .{};
    defer cells.deinit(std.testing.allocator);
    try cells.append(std.testing.allocator, .{
        .raw = .{
            .content_tag = .bg_color_rgb,
            .content = .{ .color_rgb = .{ .r = 1, .g = 2, .b = 3 } },
        },
        .grapheme = undefined,
        .style = .{ .fg_color = .{ .palette = 7 } },
    });

    const captured = capturedStyle(cells.slice(), 0);
    try std.testing.expect(captured.default());
}

test "projection completes Ghostty render updates before exposing captured styles" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 2,
        .rows = 1,
    });
    defer terminal.deinit(std.testing.allocator);
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice("\x1b[1mA");

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();

    stream.nextSlice("\x1b[H\x1b[0;3mB");
    try projection.beginCapture(&terminal, 2, 1, 8, 16);

    const state = &projection.render_state.?;
    try std.testing.expectEqual(@as(usize, 0), state.pending_styles.items.len);
    const cell = state.row_data.items(.cells)[0].get(0);
    try std.testing.expectEqual(@as(u21, 'B'), cell.raw.codepoint());
    try std.testing.expect(!cell.style.flags.bold);
    try std.testing.expect(cell.style.flags.italic);

    projection.endCapture();
}

test "default color projection preserves OSC provenance across unchanged rows and capture" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 6,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    terminal.colors.foreground = .init(.{ .r = 20, .g = 20, .b = 20 });
    terminal.colors.background = .init(.{ .r = 252, .g = 252, .b = 252 });
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice(
        "\x1b]4;1;#141414;4;#fcfcfc\x1b\\D" ++
            "\x1b[38;2;20;20;20;48;2;252;252;252mE\x1b[0;31;44mP" ++
            "\x1b[0;7mI\x1b[0;4;58;2;7;8;9mU\x1b[0m" ++
            "\x1b[48;2;10;11;12m\x1b[K\x1b[0m",
    );

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    const marker: u32 = 0x01000000;
    const steps = [_]struct { bytes: []const u8, foreground: u32, background: u32 }{
        .{ .bytes = "", .foreground = marker, .background = marker },
        .{ .bytes = "\x1b]10;#141414\x1b\\", .foreground = 0x141414, .background = marker },
        .{ .bytes = "\x1b]11;#fcfcfc\x1b\\", .foreground = 0x141414, .background = 0xfcfcfc },
        .{ .bytes = "\x1b]110\x1b\\", .foreground = marker, .background = 0xfcfcfc },
        .{ .bytes = "\x1b]10;#010203\x1b\\", .foreground = 0x010203, .background = 0xfcfcfc },
        .{ .bytes = "\x1b]111\x1b\\", .foreground = 0x010203, .background = marker },
        .{ .bytes = "\x1b[?5h", .foreground = 0xfcfcfc, .background = 0x010203 },
        .{ .bytes = "\x1b[?5l", .foreground = 0x010203, .background = marker },
        .{ .bytes = "\x1b]110\x1b\\", .foreground = marker, .background = marker },
        .{ .bytes = "\x1b[?5h", .foreground = 0xfcfcfc, .background = 0x141414 },
        .{ .bytes = "\x1b[?5l", .foreground = marker, .background = marker },
    };
    for (steps, 0..) |step, index| {
        stream.nextSlice(step.bytes);
        const revision: u64 = @intCast(index + 1);
        try projection.beginCapture(&terminal, revision, 1, 8, 16);
        projection.endCapture();
        try expectDefaultColorFrame(
            &projection,
            if (index == 0) null else revision - 1,
            step.foreground,
            step.background,
        );
    }

    // Encoding runs after the terminal lock is released. Later OSC input must
    // not change either style-table pass of the already captured frame.
    const captured_revision = steps.len + 1;
    stream.nextSlice("\x1b]10;#102030\x1b\\");
    try projection.beginCapture(&terminal, captured_revision, 1, 8, 16);
    stream.nextSlice("\x1b]10;#405060\x1b\\");
    projection.endCapture();
    try expectDefaultColorFrame(&projection, captured_revision - 1, 0x102030, marker);
    try projection.beginCapture(&terminal, captured_revision + 1, 1, 8, 16);
    projection.endCapture();
    try expectDefaultColorFrame(&projection, captured_revision, 0x405060, marker);

    projection.resetAfterCaptureFailure();
    try std.testing.expectEqual(marker, projection.captured_default_colors.foreground);
    try std.testing.expectEqual(marker, projection.captured_default_colors.background);
    try projection.beginCapture(&terminal, captured_revision + 1, 1, 8, 16);
    projection.endCapture();
    try expectDefaultColorFrame(&projection, null, 0x405060, marker);
    projection.releaseResources();
    try std.testing.expectEqual(marker, projection.captured_default_colors.foreground);
    try std.testing.expectEqual(marker, projection.captured_default_colors.background);
}

test "projection capture deduplicates repeated hyperlink URIs" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 4,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.screens.active.startHyperlink("https://example.com", null);
    try terminal.printString("ABCD");
    terminal.screens.active.endHyperlink();

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    try std.testing.expectEqual(@as(usize, 1), projection.link_table.values.items.len);
    try std.testing.expectEqual(@as(usize, "https://example.com".len), projection.link_table.byte_count);
    try std.testing.expect(projection.link_table.ref_counts.items[0] >= 4);
}

test "projection capture resolves one shared near-limit source hyperlink once" {
    const columns = 64;
    const rows = 8;
    const linked_cell_count = columns * rows;
    var target = [_]u8{'u'} ** (max_link_value_bytes - 64);
    @memcpy(target[0.."https://".len], "https://");
    const text = try std.testing.allocator.alloc(u8, linked_cell_count);
    defer std.testing.allocator.free(text);
    @memset(text, 'X');

    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = columns,
        .rows = rows,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.screens.active.startHyperlink(&target, "shared");
    try terminal.printString(text);
    terminal.screens.active.endHyperlink();

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();

    try std.testing.expectEqual(@as(usize, 1), projection.link_table.values.items.len);
    try std.testing.expectEqual(
        @as(u32, linked_cell_count),
        projection.link_table.ref_counts.items[0],
    );
    try std.testing.expectEqualSlices(u8, &target, projection.link_table.values.items[0]);

    try std.testing.expectEqual(
        @as(usize, 1),
        linkWorkObservation(&projection).source_link_resolutions,
    );

    const frame = try projection.encode(.{
        .attachment_id = AttachmentId.fromBytes([_]u8{2} ** 16),
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = HostEpoch.fromBytes([_]u8{1} ** 16),
        .maximum_bytes = max_frame_bytes,
        .sequence = 1,
    });
    defer std.testing.allocator.free(frame.allocation);
    _ = try protocol.validateTerminalFrame(std.testing.allocator, frame.bytes);
    var decoded = try decodeFrameLinks(std.testing.allocator, frame.bytes);
    defer decoded.deinit();
    try std.testing.expectEqual(linked_cell_count, decoded.cell_links.len);
    for (decoded.cell_links) |link_index| {
        try std.testing.expectEqualStrings(&target, decoded.linkTarget(link_index).?);
    }
    try std.testing.expectEqual(
        @as(usize, 1),
        linkWorkObservation(&projection).frame_link_resolutions,
    );
}

test "projection frame resolves one captured LinkTable value across both passes" {
    const columns = 16;
    const rows = 4;
    var target = [_]u8{'v'} ** (max_link_value_bytes - 64);
    var text = [_]u8{'Y'} ** (columns * rows);

    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = columns,
        .rows = rows,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.screens.active.startHyperlink(&target, "shared");
    try terminal.printString(&text);
    terminal.screens.active.endHyperlink();

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    const frame = try projection.encode(.{
        .attachment_id = AttachmentId.fromBytes([_]u8{2} ** 16),
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = HostEpoch.fromBytes([_]u8{1} ** 16),
        .maximum_bytes = max_frame_bytes,
        .sequence = 1,
    });
    defer std.testing.allocator.free(frame.allocation);
    _ = try protocol.validateTerminalFrame(std.testing.allocator, frame.bytes);

    try std.testing.expectEqual(
        @as(usize, 1),
        linkWorkObservation(&projection).frame_link_resolutions,
    );
}

test "projection source hyperlink cache includes page identity in its key" {
    var first_terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 1,
        .rows = 1,
    });
    defer first_terminal.deinit(std.testing.allocator);
    var second_terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 1,
        .rows = 1,
    });
    defer second_terminal.deinit(std.testing.allocator);
    const first_page = first_terminal.screens.active.cursor.page_pin.node.page();
    const second_page = second_terminal.screens.active.cursor.page_pin.node.page();
    var resolver_calls: usize = 0;
    const resolver: KeyedSourceLinkResolver = .{
        .calls = &resolver_calls,
        .first_page = first_page,
    };
    const keys = [_]SourceLinkKey{
        .{ .page = first_page, .identifier = 1 },
        .{ .page = second_page, .identifier = 1 },
        .{ .page = second_page, .identifier = 2 },
    };
    var source_links: SourceLinkCache = .{};
    defer source_links.deinit(std.testing.allocator);
    var link_table: LinkTable = linkTableInit(std.testing.allocator);
    defer linkTableDeinit(&link_table);

    const first = try source_links.resolve(
        std.testing.allocator,
        keys[0],
        &link_table,
        resolver,
    );
    const second = try source_links.resolve(
        std.testing.allocator,
        keys[1],
        &link_table,
        resolver,
    );
    const deduplicated = try source_links.resolve(
        std.testing.allocator,
        keys[2],
        &link_table,
        resolver,
    );
    for (keys) |key| {
        _ = try source_links.resolve(std.testing.allocator, key, &link_table, resolver);
    }

    try std.testing.expectEqual(@as(u32, 0), first);
    try std.testing.expectEqual(@as(u32, 1), second);
    try std.testing.expectEqual(first, deduplicated);
    try std.testing.expectEqual(@as(usize, 3), resolver_calls);
    try std.testing.expectEqual(@as(usize, 3), source_links.testingResolutionCount());
    try std.testing.expectEqual(@as(usize, 3), linkTableInternCallCount(&link_table));
    try std.testing.expectEqual(@as(usize, 2), link_table.values.items.len);
}

test "projection source hyperlink cache remembers invalid UTF-8 omission" {
    var page_token: u8 align(@alignOf(ghostty_vt.Page)) = 0;
    const key: SourceLinkKey = .{
        .page = @ptrCast(&page_token),
        .identifier = 1,
    };
    const invalid = [_]u8{0xff};
    var resolver_calls: usize = 0;
    const resolver: FixedSourceLinkResolver = .{
        .calls = &resolver_calls,
        .value = &invalid,
    };
    var source_links: SourceLinkCache = .{};
    defer source_links.deinit(std.testing.allocator);
    var link_table: LinkTable = linkTableInit(std.testing.allocator);
    defer linkTableDeinit(&link_table);

    const first = try source_links.resolve(
        std.testing.allocator,
        key,
        &link_table,
        resolver,
    );
    const second = try source_links.resolve(
        std.testing.allocator,
        key,
        &link_table,
        resolver,
    );

    try std.testing.expectEqual(std.math.maxInt(u32), first);
    try std.testing.expectEqual(first, second);
    try std.testing.expectEqual(@as(usize, 1), resolver_calls);
    try std.testing.expectEqual(@as(usize, 1), source_links.testingResolutionCount());
    try std.testing.expectEqual(@as(usize, 1), linkTableInternCallCount(&link_table));
    try std.testing.expectEqual(@as(usize, 0), link_table.values.items.len);
}

test "projection link work releases every partial allocation" {
    try std.testing.checkAllAllocationFailures(
        std.testing.allocator,
        exerciseLinkWorkAllocations,
        .{},
    );
}

test "projection graphics capture releases every partial image allocation" {
    try std.testing.checkAllAllocationFailures(
        std.testing.allocator,
        exerciseGraphicsCaptureAllocations,
        .{},
    );
}

fn exerciseGraphicsCaptureAllocations(allocator: std.mem.Allocator) !void {
    // Keep the VT's source image alive independently of the failing projection
    // allocator, so every failure belongs to capture's ownership transfer.
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 20,
        .rows = 4,
        .kitty_image_loading_limits = .direct,
        .kitty_image_storage_limit = protocol.Limits.terminal_graphics_resource_bytes,
    });
    defer terminal.deinit(std.testing.allocator);
    setTerminalCellSize(&terminal, 8, 16);
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.nextSlice("\x1b_Ga=T,t=d,f=24,i=1,p=1,s=1,v=2,c=10,r=1;////////\x1b\\");

    var graphics = GraphicsState.init(allocator);
    defer graphics.deinit();
    try graphics.capture(&terminal, 1, 8, 16);
    try std.testing.expectEqual(@as(usize, 1), graphics.images.items.len);
}

test "projection capture compacts a dirty linked row" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 4,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.screens.active.startHyperlink("https://example.com", null);
    try terminal.printString("ABCD");
    terminal.screens.active.endHyperlink();

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    try std.testing.expectEqual(@as(usize, 1), projection.link_table.values.items.len);

    try terminal.printString("\x1b[H    ");
    try projection.beginCapture(&terminal, 2, 1, 8, 16);
    projection.endCapture();
    try std.testing.expectEqual(@as(usize, 0), projection.link_table.values.items.len);
    try std.testing.expectEqual(@as(usize, 0), projection.link_table.byte_count);
}

test "projection dirty recapture compacts links and preserves delta first-use order" {
    const target_a = "https://a.example";
    const target_b = "https://b.example";
    const target_c = "https://c.example";
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 5,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try writeLinked(&terminal, target_a, "a", "AAAA");
    terminal.setCursorPos(2, 1);
    try writeLinked(&terminal, target_b, "b", "BBBB");
    try terminal.screens.active.startHyperlink(target_c, "c");
    terminal.screens.active.endHyperlink();
    terminal.setCursorPos(1, 1);

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    try std.testing.expectEqual(@as(usize, 2), projection.link_table.values.items.len);
    try std.testing.expectEqualSlices(u8, target_a, projection.link_table.values.items[0]);
    try std.testing.expectEqualSlices(u8, target_b, projection.link_table.values.items[1]);

    try writeLinked(&terminal, target_c, "c", "C");
    try writeLinked(&terminal, target_b, "b", "B");
    try writeLinked(&terminal, target_c, "c", "C");
    try writeLinked(&terminal, target_b, "b", "B");
    try projection.beginCapture(&terminal, 2, 1, 8, 16);
    projection.endCapture();

    try std.testing.expectEqualSlices(u64, &.{ 2, 1 }, projection.row_revisions.items);
    try std.testing.expectEqual(@as(usize, 2), projection.link_table.values.items.len);
    try std.testing.expectEqualSlices(u8, target_b, projection.link_table.values.items[0]);
    try std.testing.expectEqualSlices(u8, target_c, projection.link_table.values.items[1]);
    try std.testing.expectEqualSlices(u32, &.{ 6, 2 }, projection.link_table.ref_counts.items);
    try std.testing.expectEqualSlices(
        u32,
        &.{ 1, 0, 1, 0, std.math.maxInt(u32) },
        projection.links.items[0].indices.items,
    );
    try std.testing.expectEqualSlices(
        u32,
        &.{ 0, 0, 0, 0, std.math.maxInt(u32) },
        projection.links.items[1].indices.items,
    );
    try std.testing.expectEqual(
        @as(usize, 4),
        linkWorkObservation(&projection).source_link_resolutions,
    );

    const delta = try projection.encode(.{
        .attachment_id = AttachmentId.fromBytes([_]u8{2} ** 16),
        .base_generation = 1,
        .base_revision = 1,
        .force_snapshot = false,
        .host_epoch = HostEpoch.fromBytes([_]u8{1} ** 16),
        .maximum_bytes = max_frame_bytes,
        .sequence = 2,
    });
    defer std.testing.allocator.free(delta.allocation);
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, delta.bytes);
    try std.testing.expectEqual(protocol.FrameKind.delta, summary.kind);
    try std.testing.expectEqual(@as(u16, 1), summary.row_count);
    var decoded = try decodeFrameLinks(std.testing.allocator, delta.bytes);
    defer decoded.deinit();
    try decoded.expectTargets(&.{ target_c, target_b, target_c, target_b }, 0);
    try std.testing.expect(decoded.linkTarget(decoded.cell_links[4]) == null);
    try std.testing.expect(decoded.stringIndex(target_c).? < decoded.stringIndex(target_b).?);
    try std.testing.expectEqual(
        @as(usize, 2),
        linkWorkObservation(&projection).frame_link_resolutions,
    );
}

test "projection resets all hyperlink state after an aggregate frame limit failure" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 4,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.screens.active.startHyperlink("https://example.com", null);
    try terminal.printString("ABCD");
    terminal.screens.active.endHyperlink();

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    try std.testing.expectError(error.ProjectionTooLarge, projection.encode(.{
        .attachment_id = AttachmentId.fromBytes([_]u8{2} ** 16),
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = HostEpoch.fromBytes([_]u8{1} ** 16),
        .maximum_bytes = 160,
        .sequence = 1,
    }));
    projection.resetAfterCaptureFailure();
    try std.testing.expect(!projection.isResident());
    try std.testing.expectEqual(@as(usize, 0), projection.link_table.values.items.len);
    try std.testing.expectEqual(@as(usize, 0), projection.links.capacity);
}

test "projection release frees resident capture resources and recreates lazily" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 4,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("resident");

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    try std.testing.expect(projection.isResident());
    try std.testing.expect(projection.row_revisions.capacity > 0);

    try terminal.screens.active.startHyperlink("https://example.com", null);
    try terminal.printString("link");
    terminal.screens.active.endHyperlink();
    try projection.beginCapture(&terminal, 2, 1, 8, 16);
    projection.endCapture();
    try std.testing.expect(projection.link_table.values.items.len > 0);

    projection.releaseResources();
    try std.testing.expect(!projection.isResident());
    try std.testing.expectEqual(@as(usize, 0), projection.row_revisions.capacity);
    try std.testing.expectEqual(@as(usize, 0), projection.links.capacity);
    try std.testing.expectEqual(@as(usize, 0), projection.link_table.values.capacity);
    try std.testing.expectEqual(@as(usize, 0), projection.link_table.values.items.len);
    try std.testing.expect(projection.captured_title == null);
    try std.testing.expect(projection.captured_current_directory == null);

    try projection.beginCapture(&terminal, 2, 1, 8, 16);
    projection.endCapture();
    try std.testing.expect(projection.isResident());
    try std.testing.expectEqual(@as(u64, 2), projection.counters.render_state_creations);
}

test "projection omits invalid UTF-8 optional strings" {
    var table = StringTable.init(std.testing.allocator);
    defer table.deinit();
    const invalid = [_]u8{0xff};
    try std.testing.expectEqual(std.math.maxInt(u32), try table.addOptional(&invalid));
    try std.testing.expectEqual(std.math.maxInt(u32), table.indexOfOptional(&invalid));
}

test "projection interns repeated hyperlink URIs" {
    try std.testing.expectEqual(@as(usize, 1), try repeatedLinkCount(std.testing.allocator));
}

test "projection link cache enforces aggregate and distinct-entry budgets" {
    try linkTableBoundary(std.testing.allocator);
}

test "production projection source excludes executable test scenarios" {
    const source = subject.testing.projection_source;
    inline for (.{ "std.testing", "linkTableBoundary", "repeatedLinkCount" }) |forbidden| {
        try std.testing.expect(std.mem.indexOf(u8, source, forbidden) == null);
    }
}

test "projection owns one frame-scoped grapheme scratch buffer across both cell passes" {
    const source = subject.testing.projection_source;
    try std.testing.expectEqual(
        @as(usize, 1),
        std.mem.count(u8, source, "var grapheme_buffer: [max_string_bytes]u8 = undefined;"),
    );
    try std.testing.expectEqual(
        @as(usize, 2),
        std.mem.count(u8, source, "&grapheme_buffer"),
    );
}

fn linkTableBoundary(allocator: std.mem.Allocator) !void {
    var aggregate: LinkTable = linkTableInit(allocator);
    defer linkTableDeinit(&aggregate);
    var value: [max_link_value_bytes]u8 = undefined;
    var aggregate_entry_count: usize = 0;
    while (true) : (aggregate_entry_count += 1) {
        @memset(&value, 'x');
        const prefix = try std.fmt.bufPrint(value[0..8], "{d:0>8}", .{aggregate_entry_count});
        std.debug.assert(prefix.len == 8);
        _ = linkTableIntern(&aggregate, allocator, &value) catch |err| switch (err) {
            error.ProjectionTooLarge => break,
            else => return err,
        };
    }
    try std.testing.expectEqual(aggregate_entry_count, aggregate.values.items.len);
    try std.testing.expect(aggregate.byte_count <= max_link_cache_bytes);
    try std.testing.expectEqual(max_link_cache_bytes, aggregate.byte_count);

    var distinct: LinkTable = linkTableInit(allocator);
    defer linkTableDeinit(&distinct);
    var small_value: [16]u8 = undefined;
    for (0..max_link_entries) |index| {
        @memset(&small_value, ' ');
        _ = try std.fmt.bufPrint(small_value[0..8], "{d:0>8}", .{index});
        _ = try linkTableIntern(&distinct, allocator, &small_value);
    }
    @memset(&small_value, ' ');
    _ = try std.fmt.bufPrint(small_value[0..8], "{d:0>8}", .{max_link_entries});
    try std.testing.expectError(
        error.ProjectionTooLarge,
        linkTableIntern(&distinct, allocator, &small_value),
    );
    try std.testing.expectEqual(max_link_entries, distinct.values.items.len);
}

fn repeatedLinkCount(allocator: std.mem.Allocator) !usize {
    var table: LinkTable = linkTableInit(allocator);
    defer linkTableDeinit(&table);
    _ = try linkTableIntern(&table, allocator, "https://example.com");
    _ = try linkTableIntern(&table, allocator, "https://example.com");
    return table.values.items.len;
}

const FixedSourceLinkResolver = struct {
    calls: *usize,
    value: []const u8,

    pub fn resolve(self: FixedSourceLinkResolver, _: SourceLinkKey) []const u8 {
        self.calls.* += 1;
        return self.value;
    }
};

const KeyedSourceLinkResolver = struct {
    calls: *usize,
    first_page: *const ghostty_vt.Page,

    pub fn resolve(self: KeyedSourceLinkResolver, key: SourceLinkKey) []const u8 {
        self.calls.* += 1;
        return if (key.page == self.first_page or key.identifier == 2)
            "https://shared.example"
        else
            "https://second-page.example";
    }
};

fn expectDefaultColorFrame(
    projection: *Projection,
    base_revision: ?u64,
    foreground: u32,
    background: u32,
) !void {
    const frame = try projection.encode(.{
        .attachment_id = AttachmentId.fromBytes([_]u8{2} ** 16),
        .base_generation = if (base_revision != null) 1 else null,
        .base_revision = base_revision,
        .force_snapshot = base_revision == null,
        .host_epoch = HostEpoch.fromBytes([_]u8{1} ** 16),
        .maximum_bytes = max_frame_bytes,
        .sequence = projection.captured_revision,
    });
    defer std.testing.allocator.free(frame.allocation);
    const bytes = frame.bytes;
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, bytes);
    try std.testing.expectEqual(if (base_revision == null) FrameKind.snapshot else FrameKind.delta, summary.kind);
    // An OSC-only change must replace even the untouched blank second row.
    try std.testing.expectEqual(@as(u16, 2), std.mem.readInt(u16, bytes[130..132], .little));
    var offset: usize = protocol.terminal_frame_header_bytes;
    for (0..std.mem.readInt(u32, bytes[124..128], .little)) |_| {
        const length = readFrameInt(u32, bytes, &offset);
        offset += length;
    }
    const styles_start = offset;
    offset += @as(usize, std.mem.readInt(u16, bytes[128..130], .little)) * 20;
    for (0..2) |row| {
        try std.testing.expectEqual(@as(u16, @intCast(row)), readFrameInt(u16, bytes, &offset));
        offset += 6;
        for (0..6) |column| {
            offset += 4;
            const style_index = readFrameInt(u16, bytes, &offset);
            offset += 2;
            const style = bytes[styles_start + @as(usize, style_index) * 20 ..][0..20];
            const explicit = row == 0 and (column == 1 or column == 2);
            try std.testing.expectEqual(if (explicit) 0x141414 else foreground, std.mem.readInt(u32, style[0..4], .little));
            const expected_background: u32 = if (explicit) 0xfcfcfc else if (row == 0 and column == 5) 0x0a0b0c else background;
            try std.testing.expectEqual(expected_background, std.mem.readInt(u32, style[4..8], .little));
            const underlined = row == 0 and column == 4;
            try std.testing.expectEqual(@as(u32, if (underlined) 0x070809 else 0x02000000), std.mem.readInt(u32, style[8..12], .little));
            try std.testing.expectEqual(@as(u16, if (row == 0 and column == 3) 1 << 4 else 0), std.mem.readInt(u16, style[16..18], .little));
            try std.testing.expectEqual(@as(u8, if (underlined) 1 else 0), style[18]);
        }
    }
}

const DecodedFrameLinks = struct {
    allocator: std.mem.Allocator,
    cell_links: []u32,
    strings: [][]const u8,

    fn deinit(self: *DecodedFrameLinks) void {
        self.allocator.free(self.cell_links);
        self.allocator.free(self.strings);
        self.* = undefined;
    }

    fn expectTargets(
        self: *const DecodedFrameLinks,
        expected: []const []const u8,
        first_cell: usize,
    ) !void {
        try std.testing.expect(first_cell + expected.len <= self.cell_links.len);
        for (self.cell_links[first_cell..][0..expected.len], expected) |link_index, target| {
            try std.testing.expectEqualStrings(target, self.linkTarget(link_index).?);
        }
    }

    fn linkTarget(self: *const DecodedFrameLinks, index: u32) ?[]const u8 {
        if (index == std.math.maxInt(u32)) return null;
        return self.strings[index];
    }

    fn stringIndex(self: *const DecodedFrameLinks, target: []const u8) ?usize {
        for (self.strings, 0..) |value, index| {
            if (std.mem.eql(u8, value, target)) return index;
        }
        return null;
    }
};

fn decodeFrameLinks(
    allocator: std.mem.Allocator,
    bytes: []const u8,
) !DecodedFrameLinks {
    const string_count: usize = @intCast(std.mem.readInt(u32, bytes[124..128], .little));
    const style_count: usize = @intCast(std.mem.readInt(u16, bytes[128..130], .little));
    const row_count: usize = @intCast(std.mem.readInt(u16, bytes[130..132], .little));
    const columns: usize = @intCast(std.mem.readInt(u16, bytes[88..90], .little));
    const strings = try allocator.alloc([]const u8, string_count);
    errdefer allocator.free(strings);
    const style_links = try allocator.alloc(u32, style_count);
    defer allocator.free(style_links);
    const cell_links = try allocator.alloc(u32, row_count * columns);
    errdefer allocator.free(cell_links);

    var offset: usize = protocol.terminal_frame_header_bytes;
    for (strings) |*value| {
        const length: usize = @intCast(readFrameInt(u32, bytes, &offset));
        value.* = bytes[offset..][0..length];
        offset += length;
    }
    for (style_links) |*link_index| {
        offset += 12;
        link_index.* = readFrameInt(u32, bytes, &offset);
        offset += 4;
    }
    var cell_offset: usize = 0;
    for (0..row_count) |_| {
        offset += 8;
        for (0..columns) |_| {
            offset += 4;
            const style_index = readFrameInt(u16, bytes, &offset);
            offset += 2;
            cell_links[cell_offset] = style_links[style_index];
            cell_offset += 1;
        }
    }
    return .{
        .allocator = allocator,
        .cell_links = cell_links,
        .strings = strings,
    };
}

fn transmitPathGraphic(
    stream: anytype,
    medium: u8,
    image_id: u32,
    path: []const u8,
) !void {
    var encoded_path_buffer: [std.base64.standard.Encoder.calcSize(std.fs.max_path_bytes)]u8 = undefined;
    const encoded_path = std.base64.standard.Encoder.encode(
        encoded_path_buffer[0..std.base64.standard.Encoder.calcSize(path.len)],
        path,
    );
    var command_buffer: [encoded_path_buffer.len + 128]u8 = undefined;
    const command = try std.fmt.bufPrint(
        &command_buffer,
        "\x1b_Ga=T,t={c},f=24,s=1,v=1,i={d},p=1,C=1;{s}\x1b\\",
        .{ medium, image_id, encoded_path },
    );
    stream.nextSlice(command);
}

fn storedRgbaPng(
    allocator: std.mem.Allocator,
    width: u32,
    height: u32,
) ![]u8 {
    const row_bytes = try std.math.add(usize, 1, try std.math.mul(usize, width, 4));
    const raw_bytes = try std.math.mul(usize, row_bytes, height);
    const raw = try allocator.alloc(u8, raw_bytes);
    defer allocator.free(raw);
    @memset(raw, 0);

    var idat: std.Io.Writer.Allocating = .init(allocator);
    defer idat.deinit();
    try idat.writer.writeAll(&.{ 0x78, 0x01 });
    var offset: usize = 0;
    while (offset < raw.len) {
        const block_length = @min(raw.len - offset, std.math.maxInt(u16));
        const encoded_length: u16 = @intCast(block_length);
        try idat.writer.writeByte(if (offset + block_length == raw.len) 1 else 0);
        try idat.writer.writeInt(u16, encoded_length, .little);
        try idat.writer.writeInt(u16, ~encoded_length, .little);
        try idat.writer.writeAll(raw[offset..][0..block_length]);
        offset += block_length;
    }
    try idat.writer.writeInt(u32, std.hash.Adler32.hash(raw), .big);
    const idat_bytes = try idat.toOwnedSlice();
    defer allocator.free(idat_bytes);

    var ihdr: [13]u8 = @splat(0);
    std.mem.writeInt(u32, ihdr[0..4], width, .big);
    std.mem.writeInt(u32, ihdr[4..8], height, .big);
    ihdr[8] = 8;
    ihdr[9] = 6;

    var png: std.Io.Writer.Allocating = .init(allocator);
    errdefer png.deinit();
    try png.writer.writeAll("\x89PNG\r\n\x1a\n");
    try appendPngChunk(&png.writer, "IHDR", &ihdr);
    try appendPngChunk(&png.writer, "IDAT", idat_bytes);
    try appendPngChunk(&png.writer, "IEND", &.{});
    return png.toOwnedSlice();
}

fn appendPngChunk(
    writer: *std.Io.Writer,
    kind: []const u8,
    data: []const u8,
) !void {
    std.debug.assert(kind.len == 4);
    try writer.writeInt(u32, @intCast(data.len), .big);
    try writer.writeAll(kind);
    try writer.writeAll(data);
    var crc: std.hash.Crc32 = .init();
    crc.update(kind);
    crc.update(data);
    try writer.writeInt(u32, crc.final(), .big);
}

fn setTerminalCellSize(
    terminal: *ghostty_vt.Terminal,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
) void {
    terminal.width_px = @as(u32, terminal.cols) * cell_pixel_width;
    terminal.height_px = @as(u32, terminal.rows) * cell_pixel_height;
}

fn readFrameInt(
    comptime T: type,
    bytes: []const u8,
    offset: *usize,
) T {
    const value = std.mem.readInt(T, bytes[offset.*..][0..@sizeOf(T)], .little);
    offset.* += @sizeOf(T);
    return value;
}

fn exerciseLinkWorkAllocations(allocator: std.mem.Allocator) !void {
    var page_token: u8 align(@alignOf(ghostty_vt.Page)) = 0;
    const key: SourceLinkKey = .{
        .page = @ptrCast(&page_token),
        .identifier = 1,
    };
    var resolver_calls: usize = 0;
    const resolver: FixedSourceLinkResolver = .{
        .calls = &resolver_calls,
        .value = "https://allocation.example",
    };
    var source_links: SourceLinkCache = .{};
    defer source_links.deinit(allocator);
    var link_table: LinkTable = linkTableInit(allocator);
    defer linkTableDeinit(&link_table);
    const link_table_index = try source_links.resolve(
        allocator,
        key,
        &link_table,
        resolver,
    );

    var strings = StringTable.init(allocator);
    defer strings.deinit();
    var frame_links = try LinkStringMap.init(allocator, link_table.values.items.len);
    defer frame_links.deinit();
    _ = try frame_links.resolve(link_table_index, link_table.values.items, &strings);
}

fn writeLinked(
    terminal: *ghostty_vt.Terminal,
    target: []const u8,
    identifier: []const u8,
    text: []const u8,
) !void {
    try terminal.screens.active.startHyperlink(target, identifier);
    defer terminal.screens.active.endHyperlink();
    try terminal.printString(text);
}
