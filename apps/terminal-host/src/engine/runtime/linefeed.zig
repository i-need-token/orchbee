const std = @import("std");

pub fn expandedLength(bytes: []const u8, enabled: bool) !usize {
    if (!enabled) return bytes.len;
    return std.math.add(
        usize,
        bytes.len,
        std.mem.count(u8, bytes, "\r"),
    );
}

pub fn duplicateExpanded(
    allocator: std.mem.Allocator,
    bytes: []const u8,
    enabled: bool,
    expanded_length: usize,
) ![]u8 {
    std.debug.assert(expanded_length == try expandedLength(bytes, enabled));
    const result = try allocator.alloc(u8, expanded_length);
    if (!enabled) {
        @memcpy(result, bytes);
        return result;
    }

    var output_index: usize = 0;
    for (bytes) |byte| {
        result[output_index] = byte;
        output_index += 1;
        if (byte == '\r') {
            result[output_index] = '\n';
            output_index += 1;
        }
    }
    std.debug.assert(output_index == result.len);
    return result;
}
