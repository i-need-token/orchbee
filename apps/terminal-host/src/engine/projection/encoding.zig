const std = @import("std");
const target_builtin = @import("builtin");

const protocol = @import("terminal-protocol");
const render_encoding = @import("../render_encoding.zig");

const max_frame_bytes = protocol.Limits.frame_bytes;
const max_frame_strings = protocol.Limits.frame_strings;
const max_link_cache_bytes = max_frame_bytes;
const max_string_bytes = render_encoding.max_string_bytes;
const no_string_index = render_encoding.no_string_index;
const no_base_revision = render_encoding.no_base_revision;
const color_default = render_encoding.color_default;
const LimitedWriter = render_encoding.LimitedWriter;
const LinkStringMap = render_encoding.LinkStringMap;
const StringTable = render_encoding.StringTable;
const StyleTable = render_encoding.StyleTable;
const cellGrapheme = render_encoding.cellGrapheme;
const cellWidth = render_encoding.cellWidth;
const cursorShape = render_encoding.cursorShape;
const rgbCode = render_encoding.rgbCode;
const rowPromptClass = render_encoding.rowPromptClass;
const semanticContent = render_encoding.semanticContent;
const styleRecord = render_encoding.styleRecord;
const capturedStyle = render_encoding.capturedStyle;
const terminalInteraction = render_encoding.terminalInteraction;
const contracts = @import("contracts.zig");
const EncodeRequest = contracts.EncodeRequest;
const EncodedFrame = contracts.EncodedFrame;
const FrameKind = contracts.FrameKind;
const ProjectionError = contracts.ProjectionError;

pub fn encode(
    self: anytype,
    request: EncodeRequest,
) (std.mem.Allocator.Error || ProjectionError)!EncodedFrame {
    return encodeOwned(self, self.allocator, 0, request);
}

pub fn encodeOwned(
    self: anytype,
    output_allocator: std.mem.Allocator,
    output_prefix_bytes: usize,
    request: EncodeRequest,
) (std.mem.Allocator.Error || ProjectionError)!EncodedFrame {
    const state = if (self.render_state) |*value| value else return error.CaptureAllocationFailed;
    const snapshot = request.force_snapshot or request.base_revision == null or
        request.base_generation == null or
        request.base_generation.? != self.captured_generation or
        self.graphics.requiresSnapshot(request.base_revision);
    const kind: FrameKind = if (snapshot) .snapshot else .delta;
    const base_revision: ?u64 = if (snapshot) null else request.base_revision;
    const extension_count: u16 = 2 +
        @as(u16, @intFromBool(request.close_activity != .unknown)) +
        @as(u16, @intFromBool(request.command_completion != null));
    if (!snapshot and base_revision.? >= self.captured_revision) return error.ProjectionTooLarge;

    var strings = StringTable.init(self.allocator);
    defer strings.deinit();
    var frame_links = try LinkStringMap.init(
        self.allocator,
        self.link_table.values.items.len,
    );
    defer frame_links.deinit();
    defer {
        if (comptime target_builtin.is_test) {
            self.testing_state.frame_link_resolutions +=
                frame_links.testingResolutionCount();
        }
    }
    var styles = StyleTable.init(self.allocator);
    defer styles.deinit();
    const title_index = try strings.addOptional(self.captured_title);
    const current_directory_index = try strings.addOptional(self.captured_current_directory);

    // Keep this bounded scratch frame-scoped: ReleaseSafe initializes undefined
    // storage at declaration, so per-cell ownership turns the string limit into
    // grid-sized memory traffic in both passes below.
    var grapheme_buffer: [max_string_bytes]u8 = undefined;
    var included_rows: usize = 0;
    const row_data = state.row_data.slice();
    const row_cells = row_data.items(.cells);
    for (0..state.rows) |row_index| {
        if (!snapshot and self.row_revisions.items[row_index] <= base_revision.?) continue;
        included_rows += 1;
        const cells = row_cells[row_index].slice();
        for (0..state.cols) |column| {
            const grapheme = try cellGrapheme(
                cells.items(.raw)[column],
                cells.items(.grapheme)[column],
                &grapheme_buffer,
            );
            _ = try strings.add(grapheme);
            const hyperlink_index = try frame_links.resolve(
                self.links.items[row_index].linkIndex(column),
                self.link_table.values.items,
                &strings,
            );
            const style = styleRecord(
                cells.items(.raw)[column],
                capturedStyle(cells, column),
                &state.colors,
                hyperlink_index,
            );
            _ = try styles.add(style);
        }
    }

    const hard_limit = @min(request.maximum_bytes, max_frame_bytes);
    const allocation_limit = std.math.add(
        usize,
        output_prefix_bytes,
        hard_limit,
    ) catch return error.ProjectionTooLarge;
    var writer = LimitedWriter.init(output_allocator, allocation_limit);
    errdefer writer.deinit();
    try writer.appendZeros(output_prefix_bytes);
    const frame_offset = writer.bytes.items.len;
    try writer.appendSlice(protocol.terminal_frame_magic);
    try writer.appendInt(u8, protocol.terminal_frame_format_major);
    try writer.appendInt(u8, protocol.terminal_frame_format_minor);
    try writer.appendInt(u8, @intFromEnum(kind));
    try writer.appendInt(u8, 0);
    const total_length_offset = writer.bytes.items.len;
    try writer.appendInt(u32, 0);
    try writer.appendInt(u16, protocol.terminal_frame_header_bytes);
    try writer.appendInt(u16, 0);
    try writer.appendSlice(&request.host_epoch.bytes);
    try writer.appendSlice(&request.attachment_id.bytes);
    try writer.appendInt(u64, request.sequence);
    try writer.appendInt(u64, base_revision orelse no_base_revision);
    try writer.appendInt(u64, self.captured_revision);
    try writer.appendInt(u64, self.captured_generation);
    try writer.appendInt(u64, self.captured_scrollback_rows);
    try writer.appendInt(u16, state.cols);
    try writer.appendInt(u16, state.rows);
    const cursor = state.cursor.viewport;
    const cursor_column: u16 = if (cursor) |value| value.x else @min(state.cursor.active.x, state.cols - 1);
    const cursor_row: u16 = if (cursor) |value| value.y else @min(state.cursor.active.y, state.rows - 1);
    try writer.appendInt(u16, cursor_column);
    try writer.appendInt(u16, cursor_row);
    try writer.appendInt(u8, cursorShape(state.cursor.visual_style));
    try writer.appendInt(u8, (if (state.cursor.visible and cursor != null) @as(u8, 1) else 0) |
        (if (state.cursor.blinking) @as(u8, 2) else 0) |
        (if (cursor != null and cursor.?.wide_tail) @as(u8, 4) else 0));
    try writer.appendInt(u8, if (state.screen == .primary) 1 else 2);
    try writer.appendInt(u8, 0);
    try writer.appendInt(u32, self.captured_interaction);
    try writer.appendInt(u32, rgbCode(state.colors.foreground));
    try writer.appendInt(u32, rgbCode(state.colors.background));
    try writer.appendInt(u32, if (state.colors.cursor) |value| rgbCode(value) else color_default);
    try writer.appendInt(u32, title_index);
    try writer.appendInt(u32, current_directory_index);
    try writer.appendInt(u32, @intCast(strings.values.items.len));
    try writer.appendInt(u16, @intCast(styles.values.items.len));
    try writer.appendInt(u16, @intCast(included_rows));
    try writer.appendInt(u16, extension_count);
    try writer.appendInt(u16, 0);
    try writer.appendInt(u16, self.captured_cell_pixel_width);
    try writer.appendInt(u16, self.captured_cell_pixel_height);
    try writer.appendInt(u32, @intCast(included_rows * state.cols));
    try writer.appendInt(u8, protocol.terminal_frame_format_patch);
    try writer.appendInt(u64, request.theme_revision);
    try writer.appendZeros(7);
    std.debug.assert(
        writer.bytes.items.len - frame_offset == protocol.terminal_frame_header_bytes,
    );

    for (strings.values.items) |value| {
        try writer.appendInt(u32, @intCast(value.len));
        try writer.appendSlice(value);
    }
    for (styles.values.items) |style| {
        try writer.appendInt(u32, style.foreground);
        try writer.appendInt(u32, style.background);
        try writer.appendInt(u32, style.underline);
        try writer.appendInt(u32, style.hyperlink_index);
        try writer.appendInt(u16, style.flags);
        try writer.appendInt(u8, style.underline_kind);
        try writer.appendInt(u8, 0);
    }

    const row_raw = row_data.items(.raw);
    for (0..state.rows) |row_index| {
        if (!snapshot and self.row_revisions.items[row_index] <= base_revision.?) continue;
        const row = row_raw[row_index];
        try writer.appendInt(u16, @intCast(row_index));
        try writer.appendInt(u8, (if (row.wrap) @as(u8, 1) else 0) |
            (if (row.wrap_continuation) @as(u8, 2) else 0));
        try writer.appendInt(u8, rowPromptClass(row.semantic_prompt));
        try writer.appendInt(u16, state.cols);
        try writer.appendInt(u16, 0);
        const cells = row_cells[row_index].slice();
        for (0..state.cols) |column| {
            const grapheme = try cellGrapheme(
                cells.items(.raw)[column],
                cells.items(.grapheme)[column],
                &grapheme_buffer,
            );
            const grapheme_index = strings.indexOf(grapheme).?;
            const hyperlink_index = frame_links.mapped(
                self.links.items[row_index].linkIndex(column),
            );
            const style = styleRecord(
                cells.items(.raw)[column],
                capturedStyle(cells, column),
                &state.colors,
                hyperlink_index,
            );
            try writer.appendInt(u32, grapheme_index);
            try writer.appendInt(u16, styles.indexOf(style).?);
            const raw = cells.items(.raw)[column];
            try writer.appendInt(u8, cellWidth(raw.wide) |
                (semanticContent(raw.semantic_content) << 2) |
                (if (raw.protected) @as(u8, 0x10) else 0));
            try writer.appendInt(u8, 0);
        }
    }

    if (request.close_activity != .unknown) {
        try writer.appendInt(u16, protocol.terminal_frame_extension_close_activity);
        try writer.appendInt(u16, 0);
        try writer.appendInt(u32, 1);
        try writer.appendInt(u8, @intFromEnum(request.close_activity));
    }
    const graphics_length = try self.graphics.encodedLength(snapshot);
    try writer.appendInt(u16, protocol.terminal_frame_extension_kitty_graphics);
    try writer.appendInt(u16, 0);
    try writer.appendInt(u32, @intCast(graphics_length));
    try self.graphics.encode(&writer, snapshot);
    try writer.appendInt(u16, protocol.terminal_frame_extension_progress);
    try writer.appendInt(u16, 0);
    try writer.appendInt(u32, 2);
    try writer.appendInt(u8, @intFromEnum(request.progress.state));
    try writer.appendInt(u8, request.progress.value orelse std.math.maxInt(u8));
    if (request.command_completion) |completion| {
        try writer.appendInt(u16, protocol.terminal_frame_extension_command_completion);
        try writer.appendInt(u16, 0);
        try writer.appendInt(u32, 25);
        try writer.appendInt(u64, completion.sequence);
        try writer.appendInt(u64, completion.duration_milliseconds);
        try writer.appendInt(u64, completion.completed_at_milliseconds);
        try writer.appendInt(u8, completion.exit_code);
    }

    const frame_length = writer.bytes.items.len - frame_offset;
    if (frame_length > std.math.maxInt(u32)) return error.ProjectionTooLarge;
    std.mem.writeInt(
        u32,
        writer.bytes.items[total_length_offset..][0..4],
        @intCast(frame_length),
        .little,
    );
    self.counters.encodes += 1;
    const allocation = try writer.toOwnedSlice();
    return .{
        .allocation = allocation,
        .base_revision = base_revision,
        .bytes = allocation[output_prefix_bytes..],
        .cell_pixel_height = self.captured_cell_pixel_height,
        .cell_pixel_width = self.captured_cell_pixel_width,
        .columns = state.cols,
        .generation = self.captured_generation,
        .kind = kind,
        .revision = self.captured_revision,
        .rows = state.rows,
        .sequence = request.sequence,
        .theme_revision = request.theme_revision,
    };
}
