//! Orchbee-started interactive cmd marks its own prompt for the Host's prompt
//! gate. The child's PROMPT wraps its resolved value between OSC 133 `P` and
//! `B`, so the mark lives only in this process: no registry, AutoRun or file.
const std = @import("std");
const interactive_shell = @import("interactive_shell.zig");

/// `P` has no fresh-line action, so it never moves the cursor or the prompt
/// text. cmd has no preexec, so there is no `C` and no `D`.
const prompt_start = "$E]133;P$E\\";
const input_start = "$E]133;B$E\\";
/// cmd's own prompt when PROMPT is unset.
const default_prompt = "$P$G";

/// Sets the child's PROMPT when the launch is an interactive cmd.
pub fn markPrompt(
    allocator: std.mem.Allocator,
    environment: *std.process.Environ.Map,
    executable: []const u8,
    arguments: []const []const u8,
) !void {
    if (!isCmd(executable) or !interactive_shell.isInteractive(executable, arguments)) return;
    try environment.put("PROMPT", try prompt(allocator, environment.get("PROMPT")));
}

pub fn isCmd(executable: []const u8) bool {
    const start = if (std.mem.lastIndexOfAny(u8, executable, "\\/")) |index| index + 1 else 0;
    const filename = executable[start..];
    const name = if (std.ascii.endsWithIgnoreCase(filename, ".exe")) filename[0 .. filename.len - 4] else filename;
    return std.ascii.eqlIgnoreCase(name, "cmd");
}

/// Wraps the child's resolved PROMPT, or cmd's default when it has none. A
/// value already wrapped by an outer Orchbee cmd is kept as it is.
pub fn prompt(allocator: std.mem.Allocator, original: ?[]const u8) ![]const u8 {
    const value = if (original) |text| if (text.len == 0) default_prompt else text else default_prompt;
    if (value.len >= prompt_start.len + input_start.len and
        std.ascii.startsWithIgnoreCase(value, prompt_start) and
        std.ascii.endsWithIgnoreCase(value, input_start)) return allocator.dupe(u8, value);
    return std.mem.concat(allocator, u8, &.{ prompt_start, value, input_start });
}
