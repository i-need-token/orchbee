//! One key typed as a press and its release in the terminal's current keyboard
//! encoding, so a release-reporting keyboard mode sees no held key.
const std = @import("std");
const terminal_input = @import("terminal-input");
const terminal_input_encoding = @import("../input_encoding.zig");
const contracts = @import("contracts.zig");
const state = @import("state.zig");

/// Appends the tap to `line` and returns how many bytes the press took; zero
/// means this terminal encodes the key as nothing. Caller holds terminal_mutex.
pub fn append(self: *state.State, line: *std.ArrayList(u8), name: []const u8) !usize {
    const key = try terminal_input_encoding.parseKey(name);
    var press_bytes: usize = 0;
    for ([_]terminal_input.KeyAction{ .press, .release }) |action| {
        var buffer: [contracts.max_key_encoded_bytes]u8 = undefined;
        const encoded = try terminal_input_encoding.encodeKey(&buffer, &self.terminal, key, .{
            .action = action,
            .composing = false,
            .consumed_modifiers = .{},
            .key = name,
            .modifiers = .{},
            .text = "",
            .unshifted_codepoint = 0,
        });
        if (action == .press) press_bytes = encoded.len;
        try line.appendSlice(self.allocator, encoded);
    }
    return press_bytes;
}
