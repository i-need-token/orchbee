//! Shared canonical PowerShell startup, and exact recognition of the Worker's
//! prepared argv. Profiles run before this bootstrap; no persistent file is edited.
const std = @import("std");
pub const script = @import("powershell-startup-options").script;

pub fn isPowerShell(executable: []const u8) bool {
    const start = if (std.mem.lastIndexOfAny(u8, executable, "\\/")) |index| index + 1 else 0;
    const filename = executable[start..];
    const name = if (std.ascii.endsWithIgnoreCase(filename, ".exe")) filename[0 .. filename.len - 4] else filename;
    return std.ascii.eqlIgnoreCase(name, "pwsh") or std.ascii.eqlIgnoreCase(name, "powershell");
}

pub fn isInteractiveArguments(original: []const []const u8) bool {
    return isCanonicalArguments(original) or plainFlags(original);
}

fn plainFlags(original: []const []const u8) bool {
    for (original) |flag| {
        const known = for ([_][]const u8{ "-NoLogo", "-NoProfile", "-NoExit", "-Login", "-l" }) |choice| {
            if (std.ascii.eqlIgnoreCase(flag, choice)) break true;
        } else false;
        if (!known) return false;
    }
    return true;
}

fn hasNoExit(original: []const []const u8) bool {
    for (original) |flag| if (std.ascii.eqlIgnoreCase(flag, "-NoExit")) return true;
    return false;
}

fn isCanonicalArguments(original: []const []const u8) bool {
    if (original.len < 3) return false;
    const prefix = original[0 .. original.len - 2];
    return hasNoExit(prefix) and plainFlags(prefix) and
        std.ascii.eqlIgnoreCase(original[original.len - 2], "-Command") and
        std.mem.eql(u8, original[original.len - 1], script);
}

pub fn arguments(allocator: std.mem.Allocator, executable: []const u8, original: []const []const u8) ![]const []const u8 {
    if (!isPowerShell(executable) or isCanonicalArguments(original) or !plainFlags(original)) return original;
    const extra: usize = if (hasNoExit(original)) 2 else 3;
    const result = try allocator.alloc([]const u8, original.len + extra);
    @memcpy(result[0..original.len], original);
    if (extra == 3) result[original.len] = "-NoExit";
    @memcpy(result[result.len - 2 ..], &[_][]const u8{ "-Command", script });
    return result;
}
