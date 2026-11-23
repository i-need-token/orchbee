//! The POSIX messages of `keeper`: a PTY holding under `pty`, the Keeper as the parent of a
//! terminal's leader under `parent`, and a background program's pipes holding under `programs`. A
//! PTY holding keeps the terminal's master descriptor, and a pipes holding the read end of each
//! stream its Host captures; each moves by `SCM_RIGHTS` with the message that names it, never as a
//! value in the payload.

const std = @import("std");
const message = @import("message.zig");

const Process = message.Process;
const RandomId = message.RandomId;
const Terminal = message.Terminal;

/// At most this many bytes in a PTY's slave name, such as `/dev/ttys004`.
pub const maximum_slave_bytes = 128;
/// At most this many arguments, environment entries, or executable candidates in one launch.
pub const maximum_launch_items = 4096;

/// What a PTY holding records: the owner's terminal identity, the leader's process, the slave's
/// name as `ptsname` reads it, and the launch kind a Host reports.
pub const PtyFacts = struct {
    terminal: Terminal,
    leader: Process,
    slave: []const u8,
    launch: []const u8,
};

/// What a `spawn` names before the leader exists: `PtyFacts` without the leader.
pub const SpawnFacts = struct {
    terminal: Terminal,
    slave: []const u8,
    launch: []const u8,
};

/// The program a `spawn` starts, as the Host prepared it: the executable as named, its arguments
/// after the first, the environment as `name=value` entries, the paths to try in order when the
/// executable names no folder, and the directory to start in.
pub const Launch = struct {
    executable: []const u8,
    arguments: []const []const u8,
    environment: []const []const u8,
    candidates: []const []const u8,
    directory: ?[]const u8 = null,
};

pub const HoldPty = struct {
    holding: RandomId,
    facts: PtyFacts,
};

/// A terminal the Keeper starts and parents. The message carries the master and the slave, and the
/// standard input to use in place of the slave's when `input` is set.
pub const Spawn = struct {
    holding: RandomId,
    input: bool,
    launch: Launch,
    facts: SpawnFacts,

    pub fn descriptorCount(self: Spawn) u8 {
        return if (self.input) 3 else 2;
    }
};

/// Which of a program's stdout and stderr reach a pipe its Host reads; the others go to the null
/// device. A message carries the descriptors of captured streams in this order.
pub const Capture = struct {
    stdout: bool,
    stderr: bool,

    pub fn count(self: Capture) u8 {
        return @as(u8, @intFromBool(self.stdout)) + @intFromBool(self.stderr);
    }
};

/// A Host's hold of a background program it started itself; the message carries the read end of
/// each captured stream.
pub const HoldPipes = struct {
    holding: RandomId,
    facts: message.ProgramFacts,
    capture: Capture,
};

/// What a `spawn-program` names before the program exists: its facts without the process.
pub const SpawnProgramFacts = struct {
    program_id: [16]u8,
    label: []const u8,
};

/// A background program the Keeper starts and parents, with the null device as standard input.
/// The message carries, for each captured stream, its read end and then its write end; the Keeper
/// keeps the read ends and gives the program the write ends.
pub const SpawnProgram = struct {
    holding: RandomId,
    launch: Launch,
    facts: SpawnProgramFacts,
    capture: Capture,
};

/// A pipes holding as `inspect` shows it, sent with each captured stream's read end.
pub const PipesHolding = struct {
    facts: message.ProgramFacts,
    capture: Capture,
    parented: bool,
    ended: ?ExitStatus = null,
};

/// How a parented leader ended, as `waitid` read it.
pub const ExitStatus = union(enum) {
    code: u8,
    signal: u8,
};

/// A PTY holding as `inspect` and `status` show it: whether the Keeper parents the leader, and how
/// the leader ended once the Keeper saw it end.
pub const PtyHolding = struct {
    facts: PtyFacts,
    parented: bool,
    ended: ?ExitStatus = null,
};

/// The event `exited`: the parented leader of `holding` ended with `status`, and the Keeper keeps
/// it unreaped until its owner's `reap`.
pub const Exited = struct {
    holding: RandomId,
    status: ExitStatus,
};

/// A slave name: an absolute device path without a NUL, at most 128 bytes.
pub fn validSlave(slave: []const u8) bool {
    return slave.len > "/dev/".len and slave.len <= maximum_slave_bytes and
        std.mem.startsWith(u8, slave, "/dev/") and std.mem.indexOfScalar(u8, slave, 0) == null;
}

/// A launch string: nonempty where `nonempty`, and without a NUL, since it becomes a C string.
pub fn validLaunchText(text: []const u8, nonempty: bool) bool {
    return (!nonempty or text.len > 0) and std.mem.indexOfScalar(u8, text, 0) == null;
}

pub fn validLaunch(launch: Launch) bool {
    if (!validLaunchText(launch.executable, true)) return false;
    if (launch.directory) |directory| if (!validLaunchText(directory, true)) return false;
    inline for (.{ "arguments", "environment", "candidates" }) |list_name| {
        const items = @field(launch, list_name);
        if (items.len > maximum_launch_items) return false;
        for (items) |item| if (!validLaunchText(item, !std.mem.eql(u8, list_name, "arguments"))) return false;
    }
    for (launch.environment) |entry| if (std.mem.indexOfScalar(u8, entry, '=') == null) return false;
    return true;
}
