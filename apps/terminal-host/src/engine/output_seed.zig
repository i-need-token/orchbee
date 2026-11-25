//! Earlier output that a new terminal shows before its child starts.
//!
//! A seed is admitted only when it is UTF-8 text, CR, LF and SGR sequences (`ESC [` digits,
//! `;` and `:`, then `m`). That grammar has no query, reply, mode, clipboard, title, directory
//! or application-signal effect, so applying it can never write to the PTY. The runtime feeds
//! it to the canonical screen before any worker or child exists. Where the child can start below
//! output already on the screen (`process.startsBelowOutput`), the seed stays in view with the
//! child's first output directly below it. Elsewhere the child's console assumes a blank screen
//! with the cursor home, so the seed moves into history above one.
const std = @import("std");

/// The largest seed, including the caller's divider line.
pub const max_bytes: usize = 1024 * 1024;

/// Whether `bytes` is a non-empty seed within the grammar and bound above.
pub fn admit(bytes: []const u8) bool {
    if (bytes.len == 0 or bytes.len > max_bytes) return false;
    if (!std.unicode.utf8ValidateSlice(bytes)) return false;
    var index: usize = 0;
    while (index < bytes.len) {
        const byte = bytes[index];
        if (byte == 0x1b) {
            index += 1;
            if (index >= bytes.len or bytes[index] != '[') return false;
            index += 1;
            while (index < bytes.len and (std.ascii.isDigit(bytes[index]) or
                bytes[index] == ';' or bytes[index] == ':')) index += 1;
            if (index >= bytes.len or bytes[index] != 'm') return false;
            index += 1;
            continue;
        }
        if (byte == '\r' or byte == '\n') {
            index += 1;
            continue;
        }
        if (byte < 0x20 or byte == 0x7f) return false;
        if (byte < 0x80) {
            index += 1;
            continue;
        }
        const length = std.unicode.utf8ByteSequenceLength(byte) catch return false;
        const codepoint = std.unicode.utf8Decode(bytes[index..][0..length]) catch return false;
        // C1 controls are terminal controls too.
        if (codepoint >= 0x80 and codepoint <= 0x9f) return false;
        index += length;
    }
    return true;
}

/// Where an applied seed leaves the child's first output.
pub const Placement = enum {
    /// Directly below the seed, which stays in view; rows past the bottom scroll into history as
    /// ordinary output does.
    in_view,
    /// At the top of a blank screen, with the whole seed in history above it.
    history,
};

/// Draws an admitted seed on a fresh stream and places the cursor for the child.
pub fn apply(stream: anytype, seed: []const u8, placement: Placement) void {
    std.debug.assert(admit(seed));
    stream.nextSlice(seed);
    // Reset attributes and end the seed's last line.
    stream.nextSlice("\x1b[0m\r\n");
    switch (placement) {
        .in_view => {},
        // Push every drawn row into scrollback (ED 22) and home the cursor.
        .history => stream.nextSlice("\x1b[22J\x1b[H"),
    }
}
