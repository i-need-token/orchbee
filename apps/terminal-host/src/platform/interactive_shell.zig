//! Conservative launch classification for process-based idle evidence. Commands,
//! scripts and unknown executables never inherit the shell's single-process fallback.
const std = @import("std");

pub fn isInteractive(executable: []const u8, arguments: []const []const u8) bool {
    const start = if (std.mem.lastIndexOfAny(u8, executable, "\\/")) |index| index + 1 else 0;
    const filename = executable[start..];
    const name = if (std.ascii.endsWithIgnoreCase(filename, ".exe")) filename[0 .. filename.len - 4] else filename;
    const powershell = std.ascii.eqlIgnoreCase(name, "powershell") or std.ascii.eqlIgnoreCase(name, "pwsh");
    const cmd = std.ascii.eqlIgnoreCase(name, "cmd");
    const posix = for ([_][]const u8{ "sh", "bash", "zsh", "fish", "dash", "ksh", "csh", "tcsh" }) |shell| {
        if (std.mem.eql(u8, name, shell)) break true;
    } else false;
    if (!powershell and !cmd and !posix) return false;
    for (arguments) |argument| {
        if (powershell) {
            if (!oneOfIgnoreCase(argument, &.{ "-NoLogo", "-NoProfile", "-Login", "-l" })) return false;
        } else if (cmd) {
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
