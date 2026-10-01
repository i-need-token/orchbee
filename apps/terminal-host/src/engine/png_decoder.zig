const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");
const thread_sync = @import("terminal-host-platform").thread_sync;
const wuffs = @import("wuffs");

const png_signature = "\x89PNG\r\n\x1a\n";
pub const acquisition_limit_bytes = 400 * 1024 * 1024;
var install_mutex: thread_sync.Mutex = .{};
var installed = false;

/// Installs the Host-owned PNG decoder required by Ghostty VT library builds.
///
/// Ghostty deliberately leaves this system hook unset for embedders. The
/// process-wide, thread-safe installation happens before terminal creation so
/// direct Kitty PNG transmissions have the same support as raw pixel payloads.
pub fn install() void {
    install_mutex.lock();
    defer install_mutex.unlock();
    if (installed) return;
    installDecoder();
    installed = true;
}

fn installDecoder() void {
    ghostty_vt.sys.decode_png = decodePng;
}

pub const decodeGraphicsPng = decodePng;

fn decodePng(
    allocator: std.mem.Allocator,
    data: []const u8,
) ghostty_vt.sys.DecodeError!ghostty_vt.sys.Image {
    if (data.len < 24 or
        data.len > acquisition_limit_bytes or
        !std.mem.eql(u8, data[0..8], png_signature) or
        std.mem.readInt(u32, data[8..12], .big) != 13 or
        !std.mem.eql(u8, data[12..16], "IHDR"))
    {
        return error.InvalidData;
    }
    const declared_width = std.mem.readInt(u32, data[16..20], .big);
    const declared_height = std.mem.readInt(u32, data[20..24], .big);
    const pixel_count = std.math.mul(
        usize,
        declared_width,
        declared_height,
    ) catch return error.InvalidData;
    const decoded_bytes = std.math.mul(usize, pixel_count, 4) catch
        return error.InvalidData;
    if (declared_width == 0 or
        declared_height == 0 or
        decoded_bytes > protocol.Limits.terminal_graphics_resource_bytes)
    {
        return error.InvalidData;
    }

    const decoded = wuffs.png.decode(allocator, data) catch |err| switch (err) {
        error.WuffsError => return error.InvalidData,
        error.OutOfMemory => return error.OutOfMemory,
        error.Overflow => return error.InvalidData,
    };
    errdefer allocator.free(decoded.data);
    if (decoded.width != declared_width or
        decoded.height != declared_height or
        decoded.data.len != decoded_bytes)
    {
        return error.InvalidData;
    }
    return .{
        .width = decoded.width,
        .height = decoded.height,
        .data = decoded.data,
    };
}
