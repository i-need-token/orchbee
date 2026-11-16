//! Orchbee's managed startup for an interactive zsh or bash on POSIX. The scripts live in the
//! `shell-integration` directory installed beside the Host executable: zsh gets `ZDOTDIR` pointed
//! at its directory, with the person's own value kept in `ORCHBEE_ZSH_ZDOTDIR`, and bash gets
//! `--rcfile` naming its script in place of a login flag, with `ORCHBEE_BASH_LOGIN` set when a
//! login was asked for. The scripts read the person's own startup files and mark the prompt and
//! its commands. Only a shell given nothing but plain interactive or login flags is managed; any
//! other launch, and every launch whose script is missing, runs as given, and the variables only a
//! managed launch reads are removed from it.
const std = @import("std");
const process_types = @import("process_types.zig");
const thread_sync = @import("thread_sync.zig");

/// The variable through which a launch asks a managed shell to run an Agent as its first command.
pub const agent_start_variable = "ORCHBEE_AGENT_START";
const zsh_directory_variable = "ORCHBEE_ZSH_ZDOTDIR";
const bash_login_variable = "ORCHBEE_BASH_LOGIN";

pub const Prepared = struct {
    arguments: []const []const u8,
    launch_kind: process_types.LaunchKind,
};

pub const Shell = enum { zsh, bash };

/// The managed shell `executable` names, when every argument is a plain interactive or login flag.
pub fn managedShell(executable: []const u8, arguments: []const []const u8) ?Shell {
    const name = std.fs.path.basenamePosix(executable);
    const shell: Shell = if (std.mem.eql(u8, name, "zsh"))
        .zsh
    else if (std.mem.eql(u8, name, "bash"))
        .bash
    else
        return null;
    for (arguments) |argument| {
        if (!oneOf(argument, &.{ "-i", "-l", "-il", "-li", "--login" })) return null;
    }
    return shell;
}

/// The arguments and environment that give `executable` the managed startup, completing
/// `environment` in place.
pub fn prepare(
    arena: std.mem.Allocator,
    environment: *std.process.Environ.Map,
    executable: []const u8,
    arguments: []const []const u8,
) !Prepared {
    const managed: ?Prepared = if (managedShell(executable, arguments)) |shell| switch (shell) {
        .zsh => try zsh(arena, environment, arguments, integrationDirectory(arena)),
        .bash => try bash(arena, environment, arguments, integrationDirectory(arena)),
    } else null;
    const prepared = managed orelse unmanaged(arguments);
    if (prepared.launch_kind != .managed_zsh) _ = environment.orderedRemove(zsh_directory_variable);
    if (prepared.launch_kind != .managed_bash) _ = environment.orderedRemove(bash_login_variable);
    if (prepared.launch_kind == .other) _ = environment.orderedRemove(agent_start_variable);
    return prepared;
}

fn unmanaged(arguments: []const []const u8) Prepared {
    return .{ .arguments = arguments, .launch_kind = .other };
}

fn zsh(
    arena: std.mem.Allocator,
    environment: *std.process.Environ.Map,
    arguments: []const []const u8,
    root: ?[]const u8,
) !?Prepared {
    const directory = try std.fs.path.join(arena, &.{ root orelse return null, "zsh" });
    if (!readable(arena, directory, ".zshenv") or !readable(arena, directory, "orchbee.zsh")) return null;
    if (environment.get("ZDOTDIR")) |original| {
        try environment.put(zsh_directory_variable, original);
    } else {
        _ = environment.orderedRemove(zsh_directory_variable);
    }
    try environment.put("ZDOTDIR", directory);
    return .{ .arguments = arguments, .launch_kind = .managed_zsh };
}

/// bash reads `--rcfile` only when it is not a login shell, so a login flag becomes the script's
/// login emulation, and `--rcfile` comes first, as bash takes long options only there.
fn bash(
    arena: std.mem.Allocator,
    environment: *std.process.Environ.Map,
    arguments: []const []const u8,
    root: ?[]const u8,
) !?Prepared {
    const directory = try std.fs.path.join(arena, &.{ root orelse return null, "bash" });
    if (!readable(arena, directory, "orchbee.bash")) return null;
    var prepared: std.ArrayList([]const u8) = .empty;
    try prepared.appendSlice(arena, &.{ "--rcfile", try std.fs.path.join(arena, &.{ directory, "orchbee.bash" }) });
    var login = false;
    for (arguments) |argument| {
        if (oneOf(argument, &.{ "-l", "--login" })) {
            login = true;
        } else if (oneOf(argument, &.{ "-il", "-li" })) {
            login = true;
            try prepared.append(arena, "-i");
        } else {
            try prepared.append(arena, argument);
        }
    }
    if (login) {
        try environment.put(bash_login_variable, "1");
    } else {
        _ = environment.orderedRemove(bash_login_variable);
    }
    return .{ .arguments = prepared.items, .launch_kind = .managed_bash };
}

fn integrationDirectory(arena: std.mem.Allocator) ?[]const u8 {
    const executable_directory = std.process.executableDirPathAlloc(thread_sync.io(), arena) catch return null;
    return std.fs.path.join(arena, &.{ executable_directory, "shell-integration" }) catch null;
}

fn readable(arena: std.mem.Allocator, directory: []const u8, name: []const u8) bool {
    const path = std.fs.path.join(arena, &.{ directory, name }) catch return false;
    std.Io.Dir.accessAbsolute(thread_sync.io(), path, .{ .read = true }) catch return false;
    return true;
}

fn oneOf(value: []const u8, choices: []const []const u8) bool {
    for (choices) |choice| if (std.mem.eql(u8, value, choice)) return true;
    return false;
}
