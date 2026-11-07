//! Earlier output that a new terminal shows before its child starts.
//!
//! A seed is admitted only when it is UTF-8 text, CR, LF and SGR sequences (`ESC [` digits,
//! `;` and `:`, then `m`). That grammar has no query, reply, mode, clipboard, title, directory
//! or application-signal effect, so applying it can never write to the PTY. The runtime feeds
//! it to the canonical screen before any worker or child exists, then moves everything it drew
//! into scrollback and homes the cursor: the pseudoconsole assumes it starts on a blank screen
//! at the origin, so the child's first output lands directly below the seed.
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

/// Draws an admitted seed on a fresh stream, then leaves a blank screen with the cursor home.
pub fn apply(stream: anytype, seed: []const u8) void {
    std.debug.assert(admit(seed));
    stream.nextSlice(seed);
    // Reset attributes, end any partial line, push every drawn row into scrollback (ED 22)
    // and home the cursor for the child.
    stream.nextSlice("\x1b[0m\r\n\x1b[22J\x1b[H");
}
