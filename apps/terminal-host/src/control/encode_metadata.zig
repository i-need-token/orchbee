const std = @import("std");

const common = @import("encode_common.zig");

const identifierText = common.identifierText;

pub fn terminalMetadataChanged(
    allocator: std.mem.Allocator,
    host_epoch: anytype,
    creation_id: anytype,
    terminal_id: anytype,
    event: anytype,
) ![]u8 {
    var creation_buffer: [32]u8 = undefined;
    var epoch_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writer.writer.writeAll("{\"currentDirectory\":");
    try std.json.Stringify.value(event.current_directory, .{}, &writer.writer);
    try writer.writer.print(
        ",\"identity\":{{\"creationId\":\"{s}\",\"hostEpoch\":\"{s}\",\"terminalId\":\"{s}\"}},\"revision\":\"{d}\",\"title\":",
        .{
            identifierText(creation_id, &creation_buffer),
            identifierText(host_epoch, &epoch_buffer),
            identifierText(terminal_id, &terminal_buffer),
            event.revision,
        },
    );
    try std.json.Stringify.value(event.title, .{}, &writer.writer);
    try writer.writer.writeAll("}");
    return writer.toOwnedSlice();
}
