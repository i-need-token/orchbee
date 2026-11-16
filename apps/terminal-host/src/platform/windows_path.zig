//! Win32 path spellings, read as text before anything opens them.
const std = @import("std");

/// Whether `path` names a file on a local drive by an absolute drive path: a drive letter, a colon,
/// a separator, and at least one component. A UNC or device path (`\\host\share`, `//host`,
/// `\\.\pipe`, `\\?\`, `\??\`), a rooted or relative path, a drive-relative path such as `X:name`,
/// a further colon, which names a stream or a device, a NUL byte, and any component that Win32 maps
/// to a device are refused, so opening an admitted path neither contacts another machine nor
/// reaches a device or pipe.
pub fn isLocalDrivePath(path: []const u8) bool {
    if (path.len < 4 or !std.ascii.isAlphabetic(path[0]) or path[1] != ':' or !isSeparator(path[2]))
        return false;
    if (std.mem.indexOfAny(u8, path[2..], ":\x00") != null) return false;
    var components = std.mem.tokenizeAny(u8, path[3..], "\\/");
    var named = false;
    while (components.next()) |component| {
        if (isDosDeviceName(component)) return false;
        named = true;
    }
    return named;
}

/// Win32 maps these names to devices in any directory, with or without an extension.
pub fn isDosDeviceName(component: []const u8) bool {
    const stem = std.mem.trimEnd(u8, component[0 .. std.mem.indexOfScalar(u8, component, '.') orelse component.len], " ");
    for ([_][]const u8{ "CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$" }) |name| {
        if (std.ascii.eqlIgnoreCase(stem, name)) return true;
    }
    if (stem.len < 4 or !(std.ascii.startsWithIgnoreCase(stem, "COM") or std.ascii.startsWithIgnoreCase(stem, "LPT")))
        return false;
    const suffix = stem[3..];
    return (suffix.len == 1 and std.ascii.isDigit(suffix[0])) or
        std.mem.eql(u8, suffix, "\u{00B9}") or std.mem.eql(u8, suffix, "\u{00B2}") or std.mem.eql(u8, suffix, "\u{00B3}");
}

fn isSeparator(byte: u8) bool {
    return byte == '\\' or byte == '/';
}
