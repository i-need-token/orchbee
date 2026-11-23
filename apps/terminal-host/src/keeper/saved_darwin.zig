//! The state a macOS Keeper hands the image it executes in its place (`schemas/keeper/README.md`,
//! Re-execution): format 1, one JSON object that names every holding, registered Host, and admitted
//! connection with the descriptors that carry them across `execve`. Every later Keeper reads each
//! format an earlier release wrote; a state these rules refuse sends the update back to the old
//! image.

const std = @import("std");
const handshake = @import("terminal-host-handshake");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");
const state = @import("state.zig");

const message = keeper_protocol.message;
const pty = message.pty;
const posix = std.posix;

pub const format: u32 = 1;
/// The most bytes a state may take.
pub const maximum_bytes = 16 * 1024 * 1024;

pub const Process = struct { pid: u32, created: []const u8 };
pub const Terminal = struct { creationId: []const u8, hostEpoch: []const u8, terminalId: []const u8 };
pub const Facts = struct { terminal: Terminal, leader: Process, slave: []const u8, launch: []const u8 };
pub const ProgramFacts = struct { process: Process, programId: []const u8, label: []const u8 };
/// A background program's facts and the read end of each stream its Host captures.
pub const Pipes = struct { facts: ProgramFacts, stdout: ?i32 = null, stderr: ?i32 = null };
pub const Exit = struct { code: ?u8 = null, signal: ?u8 = null };
pub const Claimed = struct { from: Process, mark: []const u8 };

pub const Holding = struct {
    holding: []const u8,
    owner: Process,
    claimed: ?Claimed = null,
    /// A terminal's PTY master and facts, or else a background program's pipes.
    master: ?i32 = null,
    facts: ?Facts = null,
    pipes: ?Pipes = null,
    parented: bool,
    ended: ?Exit = null,
    reaped: bool,
    exitSent: bool,
};

pub const Host = struct { process: Process, ended: bool };

pub const Connection = struct {
    socket: i32,
    role: []const u8,
    pid: u32,
    host: ?Process = null,
    capabilities: []const []const u8,
    usedIds: []const [2]u32,
    pending: ?u32 = null,
};

pub const State = struct {
    format: u32,
    previous: []const u8,
    listener: i32,
    lock: i32,
    hosts: []const Host,
    holdings: []const Holding,
    released: []const u32,
    connections: []const Connection,
};

/// The capability names a connection may carry, as the `keeper` registry declares them.
pub const capability_names = handshake.registry.keeper.declared_capabilities;

pub fn encode(arena: std.mem.Allocator, value: State) error{OutOfMemory}![]u8 {
    return std.json.Stringify.valueAlloc(arena, value, .{ .emit_null_optional_fields = false });
}

/// Parses `bytes` and checks every rule but whether its descriptors are open.
pub fn decode(arena: std.mem.Allocator, bytes: []const u8) error{InvalidState}!State {
    if (bytes.len > maximum_bytes or !std.unicode.utf8ValidateSlice(bytes)) return error.InvalidState;
    const value = std.json.parseFromSliceLeaky(State, arena, bytes, .{ .allocate = .alloc_always }) catch
        return error.InvalidState;
    if (!valid(value)) return error.InvalidState;
    return value;
}

/// Whether every descriptor `value` names is open in this process.
pub fn descriptorsOpen(value: State) bool {
    if (!open(value.listener) or !open(value.lock)) return false;
    for (value.holdings) |holding| for (descriptorsOf(holding)) |named| if (named) |descriptor| if (!open(descriptor)) return false;
    for (value.connections) |connection| if (!open(connection.socket)) return false;
    return true;
}

/// The descriptors a holding names: its master, or each captured stream's read end.
pub fn descriptorsOf(holding: Holding) [2]?i32 {
    if (holding.pipes) |pipes| return .{ pipes.stdout, pipes.stderr };
    return .{ holding.master, null };
}

fn open(descriptor: i32) bool {
    _ = platform.posix_compat.fcntl(descriptor, posix.F.GETFD, 0) catch return false;
    return true;
}

fn valid(value: State) bool {
    if (value.format != format or !message.validProgram(value.previous)) return false;
    if (value.hosts.len > message.maximum_hosts or value.holdings.len > message.maximum_holdings) return false;
    if (value.released.len > 2 * message.maximum_holdings) return false;
    if (value.connections.len > state.maximum_host_connections + 1) return false;
    for (value.hosts, 0..) |host, index| {
        const process = toProcess(host.process) catch return false;
        for (value.hosts[0..index]) |earlier| if (earlier.process.pid == process.pid) return false;
    }
    for (value.holdings, 0..) |holding, index| {
        const id = toId(holding.holding) catch return false;
        if (index > 0) {
            const before = toId(value.holdings[index - 1].holding) catch return false;
            if (!std.mem.lessThan(u8, &before, &id)) return false;
        }
        if (!listed(value.hosts, holding.owner)) return false;
        if (holding.claimed) |claimed| {
            _ = toProcess(claimed.from) catch return false;
            _ = toId(claimed.mark) catch return false;
        }
        if ((holding.master == null) != (holding.facts == null)) return false;
        if ((holding.master == null) == (holding.pipes == null)) return false;
        if (holding.facts) |facts| _ = toFacts(facts) catch return false;
        if (holding.pipes) |pipes| _ = toProgramFacts(pipes.facts) catch return false;
        if (holding.ended) |ended| _ = toExit(ended) catch return false;
        if (holding.ended != null and !holding.parented) return false;
        if (holding.reaped and holding.ended == null) return false;
    }
    for (value.released) |leader| if (leader == 0) return false;
    var workers: usize = 0;
    var pending: usize = 0;
    for (value.connections) |connection| {
        if (std.mem.eql(u8, connection.role, "worker")) {
            workers += 1;
            if (connection.host != null) return false;
        } else if (std.mem.eql(u8, connection.role, "host")) {
            const host = connection.host orelse return false;
            if (!listed(value.hosts, host) or host.pid != connection.pid) return false;
            if (connection.pending != null) return false;
        } else return false;
        for (connection.capabilities) |name| if (!known(name)) return false;
        if (!ascending(connection.usedIds)) return false;
        if (connection.pending) |id| {
            pending += 1;
            if (!usedId(connection.usedIds, id)) return false;
        }
    }
    if (workers > 1 or pending > 1) return false;
    return distinctDescriptors(value);
}

fn listed(hosts: []const Host, process: Process) bool {
    const wanted = toProcess(process) catch return false;
    for (hosts) |host| {
        const named = toProcess(host.process) catch return false;
        if (named.eql(wanted)) return true;
    }
    return false;
}

fn known(name: []const u8) bool {
    for (capability_names) |capability| if (std.mem.eql(u8, name, capability)) return true;
    return false;
}

fn ascending(ranges: []const [2]u32) bool {
    for (ranges, 0..) |range, index| {
        if (range[0] == 0 or range[0] > range[1]) return false;
        if (index > 0 and range[0] <= ranges[index - 1][1]) return false;
    }
    return true;
}

fn usedId(ranges: []const [2]u32, id: u32) bool {
    for (ranges) |range| if (range[0] <= id and id <= range[1]) return true;
    return false;
}

/// No descriptor is named twice, and none is a standard stream.
fn distinctDescriptors(value: State) bool {
    var seen: std.AutoHashMapUnmanaged(i32, void) = .empty;
    var buffer: [64 * 1024]u8 = undefined;
    var fixed = std.heap.FixedBufferAllocator.init(&buffer);
    const allocator = fixed.allocator();
    const add = struct {
        fn add(set: *std.AutoHashMapUnmanaged(i32, void), gpa: std.mem.Allocator, descriptor: i32) bool {
            if (descriptor <= 2) return false;
            const entry = set.getOrPut(gpa, descriptor) catch return false;
            return !entry.found_existing;
        }
    }.add;
    if (!add(&seen, allocator, value.listener) or !add(&seen, allocator, value.lock)) return false;
    for (value.holdings) |holding| for (descriptorsOf(holding)) |named| if (named) |descriptor| if (!add(&seen, allocator, descriptor)) return false;
    for (value.connections) |connection| if (!add(&seen, allocator, connection.socket)) return false;
    return true;
}

// The protocol's value grammars, as text.

pub fn processText(arena: std.mem.Allocator, process: message.Process) error{OutOfMemory}!Process {
    return .{ .pid = process.pid, .created = try std.fmt.allocPrint(arena, "{x:0>16}", .{process.created}) };
}

pub fn toProcess(text: Process) error{InvalidState}!message.Process {
    if (text.pid == 0 or text.created.len != 16 or !lowercaseHex(text.created)) return error.InvalidState;
    const created = std.fmt.parseInt(u64, text.created, 16) catch return error.InvalidState;
    if (created == 0) return error.InvalidState;
    return .{ .pid = text.pid, .created = created };
}

pub fn idText(arena: std.mem.Allocator, id: [16]u8) error{OutOfMemory}![]const u8 {
    return std.fmt.allocPrint(arena, "{x}", .{&id});
}

pub fn toId(text: []const u8) error{InvalidState}![16]u8 {
    if (text.len != 32 or !lowercaseHex(text)) return error.InvalidState;
    var id: [16]u8 = undefined;
    _ = std.fmt.hexToBytes(&id, text) catch return error.InvalidState;
    if (message.isZero(&id)) return error.InvalidState;
    return id;
}

pub fn factsText(arena: std.mem.Allocator, facts: pty.PtyFacts) error{OutOfMemory}!Facts {
    return .{
        .terminal = .{
            .creationId = try idText(arena, facts.terminal.creation_id),
            .hostEpoch = try idText(arena, facts.terminal.host_epoch),
            .terminalId = try idText(arena, facts.terminal.terminal_id),
        },
        .leader = try processText(arena, facts.leader),
        .slave = facts.slave,
        .launch = facts.launch,
    };
}

pub fn toFacts(text: Facts) error{InvalidState}!pty.PtyFacts {
    if (!pty.validSlave(text.slave) or !handshake.message.isName(text.launch)) return error.InvalidState;
    return .{
        .terminal = .{
            .creation_id = try toId(text.terminal.creationId),
            .host_epoch = try toId(text.terminal.hostEpoch),
            .terminal_id = try toId(text.terminal.terminalId),
        },
        .leader = try toProcess(text.leader),
        .slave = text.slave,
        .launch = text.launch,
    };
}

pub fn programFactsText(arena: std.mem.Allocator, facts: message.ProgramFacts) error{OutOfMemory}!ProgramFacts {
    return .{ .process = try processText(arena, facts.process), .programId = try idText(arena, facts.program_id), .label = facts.label };
}

pub fn toProgramFacts(text: ProgramFacts) error{InvalidState}!message.ProgramFacts {
    const facts: message.ProgramFacts = .{ .process = try toProcess(text.process), .program_id = try toId(text.programId), .label = text.label };
    if (!facts.valid()) return error.InvalidState;
    return facts;
}

pub fn exitText(status: pty.ExitStatus) Exit {
    return switch (status) {
        .code => |code| .{ .code = code },
        .signal => |number| .{ .signal = number },
    };
}

pub fn toExit(text: Exit) error{InvalidState}!pty.ExitStatus {
    if (text.code) |code| {
        if (text.signal != null) return error.InvalidState;
        return .{ .code = code };
    }
    const number = text.signal orelse return error.InvalidState;
    if (number == 0) return error.InvalidState;
    return .{ .signal = number };
}

fn lowercaseHex(text: []const u8) bool {
    for (text) |byte| switch (byte) {
        '0'...'9', 'a'...'f' => {},
        else => return false,
    };
    return true;
}
