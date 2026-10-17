//! Conservative launch classification for process-based idle evidence. Commands,
//! scripts and unknown executables never inherit the shell's single-process fallback.
const std = @import("std");
const powershell = @import("powershell_integration.zig");

pub fn isInteractive(executable: []const u8, arguments: []const []const u8) bool {
    const start = if (std.mem.lastIndexOfAny(u8, executable, "\\/")) |index| index + 1 else 0;
    const filename = executable[start..];
    const name = if (std.ascii.endsWithIgnoreCase(filename, ".exe")) filename[0 .. filename.len - 4] else filename;
    if (powershell.isPowerShell(executable)) return powershell.isInteractiveArguments(arguments);
    const cmd = std.ascii.eqlIgnoreCase(name, "cmd");
    const posix = for ([_][]const u8{ "sh", "bash", "zsh", "fish", "dash", "ksh", "csh", "tcsh" }) |shell| {
        if (std.mem.eql(u8, name, shell)) break true;
    } else false;
    if (!cmd and !posix) return false;
    for (arguments) |argument| {
        if (cmd) {
            if (!oneOfIgnoreCase(argument, &.{ "/D", "/Q" })) return false;
        } else {
            if (!oneOfIgnoreCase(argument, &.{ "-i", "-l", "-il", "-li", "--login", "--noprofile", "--norc" })) return false;
        }
    }
    return true;
}

fn oneOfIgnoreCase(value: []const u8, choices: []const []const u8) bool {
    for (choices) |choice| if (std.ascii.eqlIgnoreCase(value, choice)) return true;
    return false;
}
