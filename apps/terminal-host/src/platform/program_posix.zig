//! A background program's process on macOS (`program.zig`): started in a session of its own with
//! the null device as input and each output sent to the null device or to a pipe the caller reads,
//! by the attached Keeper, which then parents it, or by the Host when no Keeper starts it. Its
//! Keeper record holds its release-first gate and its parent, as a terminal's does, and its
//! remaining members end through its session while the session id is still its own.

const std = @import("std");
const fork_launch = @import("fork_launch_posix.zig");
const posix_compat = @import("posix_compat.zig");
const process_common = @import("process_common.zig");
const process_session = @import("process_session.zig");
const keeper_record = @import("process_keeper_posix.zig");
const program = @import("program.zig");

const posix = std.posix;
const Record = keeper_record.Record;
const Spawned = program.Spawned;
const SpawnError = program.SpawnError;

pub fn spawn(allocator: std.mem.Allocator, launch: program.Launch) SpawnError!Spawned {
    if (!std.fs.path.isAbsolutePosix(launch.executable) or !std.fs.path.isAbsolutePosix(launch.directory))
        return error.InvalidCommand;
    var arena_state = std.heap.ArenaAllocator.init(allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const executable = try text(arena, launch.executable);
    if (std.c.access(executable, 0) != 0) return error.NotFound;
    const extra = @intFromBool(launch.raw_argument != null);
    const argv = try arena.allocSentinel(?[*:0]const u8, launch.arguments.len + 1 + extra, null);
    argv[0] = executable;
    for (launch.arguments, 1..) |argument, index| argv[index] = try text(arena, argument);
    if (launch.raw_argument) |raw| argv[launch.arguments.len + 1] = try text(arena, raw);
    var environment = process_common.inheritedEnvironmentForLaunch(arena, launch.environment) catch |err| switch (err) {
        error.OutOfMemory => return error.OutOfMemory,
        else => return error.SpawnFailed,
    };
    const block = (environment.createPosixBlock(arena, .{}) catch return error.OutOfMemory).slice.ptr;

    const null_device = posix_compat.openNullDevice() catch return error.SpawnFailed;
    defer posix_compat.close(null_device);
    var pipes: [2]?[2]posix.fd_t = .{ null, null };
    defer for (pipes) |pair| if (pair) |ends| posix_compat.close(ends[1]);
    errdefer for (pipes) |pair| if (pair) |ends| posix_compat.close(ends[0]);
    for (launch.capture, &pipes) |captured, *pair| if (captured) {
        pair.* = posix_compat.pipe2(.{ .CLOEXEC = true }) catch return error.SpawnFailed;
    };
    const record = try allocator.create(Record);
    record.* = .{};
    errdefer allocator.destroy(record);

    const pid = started: {
        if (launch.keeper) |keeper| switch (keeper.spawn(keeper.context, &.{
            .record = record,
            .executable = launch.executable,
            .argv = argv,
            .environment = block,
            .directory = launch.directory,
            .pipes = pipes,
            .program_id = launch.program_id,
            .label = launch.label,
        })) {
            .started => |pid| break :started pid,
            .refused => {},
            .spawn_failed, .unanswered => return error.SpawnFailed,
        };
        const pid = fork_launch.start(.{
            .executable = launch.executable,
            .argv = argv,
            .environment = block,
            .candidates = &.{},
            .directory = launch.directory,
            .streams = .{ .pipes = .{
                .input = null_device,
                .output = if (pipes[0]) |ends| ends[1] else null_device,
                .errors = if (pipes[1]) |ends| ends[1] else null_device,
            } },
        }) catch return error.SpawnFailed;
        record.started(pid);
        break :started pid;
    };
    return .{
        .process = {},
        .job = {},
        .streams = .{ if (pipes[0]) |ends| ends[0] else null, if (pipes[1]) |ends| ends[0] else null },
        .pid = @intCast(pid),
        .created = record.leader_created,
        .record = record,
    };
}

/// A recovered program's record: the claim checked its process, and the Keeper parents it when
/// `parented`, until the link that claims its holding becomes its parent.
pub fn recoveredRecord(allocator: std.mem.Allocator, created: u64, parented: bool) error{OutOfMemory}!*Record {
    const record = try allocator.create(Record);
    record.* = .{ .leader_created = created, .claims_parent = parented };
    return record;
}

/// Reads what a captured stream has; 0 once every writer has closed it or the read failed.
pub fn read(stream: posix.fd_t, buffer: []u8) usize {
    while (true) {
        const count = std.c.read(stream, buffer.ptr, buffer.len);
        if (count >= 0) return @intCast(count);
        if (posix.errno(count) != .INTR) return 0;
    }
}

/// The program's exit, as its parent or the kernel reports it, or null once it leaves this Host. A
/// signal reads as 128 plus its number, and an end no parent reported as 1.
pub fn awaitExit(spawned: Spawned, adopted: bool) ?u32 {
    const result = spawned.record.awaitLeader(@intCast(spawned.pid), adopted) catch return 1;
    return switch (result orelse return null) {
        .exit_code => |code| code,
        .signal => |number| 128 + number,
        .unknown => 1,
    };
}

/// Ends every member of the program's session while its session id is still its own: this Host or
/// its Keeper keeps the ended root unreaped, or the root still runs with its start time.
pub fn endMembers(spawned: Spawned, adopted: bool) void {
    const record = spawned.record;
    const pid: posix.pid_t = @intCast(spawned.pid);
    if (!record.keepsLeader(adopted) and !record.leaderRuns(pid)) return;
    process_session.forceTerminateSession(pid, process_session.NativeSessionSignaler{}) catch {};
}

/// Reaps the root once it ended when this Host forked it; its Keeper or `launchd` reaps any other.
pub fn reapOwn(spawned: Spawned, adopted: bool) void {
    if (adopted or spawned.record.isParented()) return;
    _ = posix_compat.waitpid(@intCast(spawned.pid), 0);
}

fn text(arena: std.mem.Allocator, value: []const u8) SpawnError![*:0]const u8 {
    if (std.mem.indexOfScalar(u8, value, 0) != null) return error.InvalidCommand;
    return (try arena.dupeZ(u8, value)).ptr;
}
