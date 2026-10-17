//! Adds process-local reader markers only to an ordinary interactive PowerShell
//! launch. Profiles run normally before this bootstrap; no persistent file is edited.
const std = @import("std");
const interactive_shell = @import("interactive_shell.zig");

pub fn arguments(allocator: std.mem.Allocator, executable: []const u8, original: []const []const u8) ![]const []const u8 {
    if (!interactive_shell.isInteractive(executable, original)) return original;
    const start = if (std.mem.lastIndexOfAny(u8, executable, "\\/")) |index| index + 1 else 0;
    const filename = executable[start..];
    const name = if (std.ascii.endsWithIgnoreCase(filename, ".exe")) filename[0 .. filename.len - 4] else filename;
    if (!std.ascii.eqlIgnoreCase(name, "pwsh") and !std.ascii.eqlIgnoreCase(name, "powershell")) return original;
    const result = try allocator.alloc([]const u8, original.len + 3);
    @memcpy(result[0..original.len], original);
    @memcpy(result[original.len..], &[_][]const u8{ "-NoExit", "-Command", @embedFile("powershell_prompt.ps1") });
    return result;
}
