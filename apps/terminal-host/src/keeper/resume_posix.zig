//! The image a macOS or Linux Keeper executes in its own place (`reexec_posix.zig`): it reads the
//! state the old image left, takes over its endpoint, table, and connections, records each parented
//! leader that ended meanwhile, and answers the pending `reexec`. An image that cannot read the
//! state executes the old program with it and `--returned`, and that image answers `exec-failed`.

const std = @import("std");
const handshake = @import("terminal-host-handshake");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");
const build_options = @import("build-options");
const connection = @import("connection.zig");
const service = @import("service.zig");
const state = @import("state.zig");
const saved = @import("saved_posix.zig");
const reexec = @import("reexec_posix.zig");

const message = keeper_protocol.message;
const handles = state.handles;
const content = state.content;
const posix = std.posix;
const c = std.c;
const posix_compat = platform.posix_compat;
const native = platform.keeper.native;

/// Outside release builds, a state is refused once while this names a file, which is removed.
const refuse_state_variable = "ORCHBEE_TEST_KEEPER_REFUSE_STATE";

pub const Handover = struct {
    state: posix.fd_t,
    previous: []const u8,
    returned: bool,
};

/// Takes the state over and serves; never returns.
pub fn run(allocator: std.mem.Allocator, options: service.Options, handover: Handover) noreturn {
    // The state's text lives as long as the process; connections copy what they keep.
    var arena_state = std.heap.ArenaAllocator.init(allocator);
    const arena = arena_state.allocator();
    const value = read(arena, handover.state) catch goBack(arena, options, handover);
    var server = service.Server.adopt(
        allocator,
        options.runtime_directory,
        options.endpoint_directory orelse options.runtime_directory,
        options.endpoint_id,
        value.listener,
        value.lock,
    ) catch goBack(arena, options, handover);

    // Committed: every descriptor the state names is this image's own from here on.
    for (named(arena, value) catch std.process.exit(1)) |descriptor| reexec.inherit(descriptor, false);
    posix_compat.close(handover.state);
    posix_compat.closeInheritedDescriptors(platform.thread_sync.io()) catch {};
    service.armProcess(options.runtime_directory, &server);

    const table = allocator.create(state.Table) catch std.process.exit(1);
    table.init() catch std.process.exit(1);
    state.activate(table);
    table.startWatch() catch std.process.exit(1);
    table.lock();
    rebuild(table, value) catch std.process.exit(1);
    content.pollLeaders(table);
    table.unlock();
    service.armUpdate(allocator, options, &server);

    const answer: message.Reply.Body = if (handover.returned) .{ .refused = .exec_failed } else .reexec;
    var slot: usize = 0;
    for (value.connections) |entry| {
        defer slot += 1;
        const resumed: connection.Resumed = .{
            .peer = peerOf(entry),
            .capabilities = capabilitiesOf(entry.capabilities),
            .used = entry.usedIds,
            .pending = if (entry.pending) |id| .{ .id = id, .body = answer } else null,
        };
        if (!service.resumeConnection(allocator, table, native.Stream.init(entry.socket), slot, resumed)) {
            table.lock();
            table.releaseConnection(slot);
            table.unlock();
            posix_compat.close(entry.socket);
        }
    }
    service.serve(allocator, &server, table);
}

fn read(arena: std.mem.Allocator, descriptor: posix.fd_t) error{InvalidState}!saved.State {
    if (refusedForTest()) return error.InvalidState;
    const bytes = arena.alloc(u8, saved.maximum_bytes + 1) catch return error.InvalidState;
    var filled: usize = 0;
    while (filled < bytes.len) {
        const count = c.pread(descriptor, bytes[filled..].ptr, bytes.len - filled, @intCast(filled));
        if (count == 0) break;
        if (count < 0) {
            if (posix.errno(count) == .INTR) continue;
            return error.InvalidState;
        }
        filled += @intCast(count);
    }
    const value = try saved.decode(arena, bytes[0..filled]);
    if (!saved.descriptorsOpen(value)) return error.InvalidState;
    return value;
}

fn refusedForTest() bool {
    if (std.mem.eql(u8, build_options.environment, handshake.message.release_environment)) return false;
    const flag = c.getenv(refuse_state_variable) orelse return false;
    return c.unlink(flag) == 0;
}

/// Executes the previous program with the same state, which then answers `exec-failed`; an image
/// that is itself the way back ends instead.
fn goBack(arena: std.mem.Allocator, options: service.Options, handover: Handover) noreturn {
    if (handover.returned) std.process.exit(1);
    const program = arena.dupeZ(u8, handover.previous) catch std.process.exit(1);
    const argv = reexec.arguments(arena, .{
        .program = program,
        .endpoint = .{
            .runtime_directory = options.runtime_directory,
            .endpoint_id = options.endpoint_id,
            .endpoint_directory = options.endpoint_directory,
            .listener = -1,
            .lock = -1,
            .program = handover.previous,
        },
        .state = handover.state,
        .previous = handover.previous,
        .returned = true,
    }) catch std.process.exit(1);
    _ = c.execve(program.ptr, argv, @ptrCast(c.environ));
    std.process.exit(1);
}

fn named(arena: std.mem.Allocator, value: saved.State) error{OutOfMemory}![]posix.fd_t {
    var list: std.ArrayList(posix.fd_t) = .empty;
    try list.appendSlice(arena, &.{ value.listener, value.lock });
    for (value.holdings) |holding| for (saved.descriptorsOf(holding)) |kept| if (kept) |descriptor| try list.append(arena, descriptor);
    for (value.connections) |entry| try list.append(arena, entry.socket);
    return list.items;
}

/// Restores the table from a state `saved.decode` accepted. Called with the lock held.
fn rebuild(table: *state.Table, value: saved.State) error{InvalidState}!void {
    for (value.hosts, 0..) |entry, index| {
        const process = try saved.toProcess(entry.process);
        const context = (@as(usize, table.host_generations[index]) << 16) | index;
        const wait = if (entry.ended) null else if (handles.openHost(process)) |handle|
            handles.watchEnd(handle, state.hostEnded, context)
        else |_|
            null;
        table.hosts[index] = .{ .process = process, .handle = process.pid, .wait = wait };
        table.registered_hosts += 1;
    }
    for (value.holdings, 0..) |entry, index| {
        const owner = try saved.toProcess(entry.owner);
        if (table.findHost(owner) == null) return error.InvalidState;
        const parent: content.Parent = .{
            .parented = entry.parented,
            .ended = if (entry.ended) |ended| try saved.toExit(ended) else null,
            .reaped = entry.reaped,
            .exit_sent = entry.exitSent,
        };
        table.holdings[index] = .{
            .id = try saved.toId(entry.holding),
            .owner = owner,
            .content = if (entry.pipes) |pipes| .{ .pipes = .{
                .streams = .{ pipes.stdout, pipes.stderr },
                .facts = .store(try saved.toProgramFacts(pipes.facts)),
                .parent = parent,
            } } else .{ .pty = .{
                .master = entry.master.?,
                .facts = content.StoredPtyFacts.store(try saved.toFacts(entry.facts.?)) orelse return error.InvalidState,
                .parent = parent,
            } },
            .claimed = if (entry.claimed) |claimed| .{ .from = try saved.toProcess(claimed.from), .mark = try saved.toId(claimed.mark) } else null,
        };
        if (entry.parented and entry.ended == null) handles.watchLeader(table.holdings[index].content.leader().pid);
    }
    table.holding_count = value.holdings.len;
    for (value.released, 0..) |leader, index| {
        table.extra.released[index] = leader;
        handles.watchLeader(leader);
    }
    table.extra.released_count = value.released.len;
    // A Host that ended, or whose process id another process took, while no image watched ends
    // as its end callback would have ended it.
    for (value.hosts, 0..) |_, index| {
        if (table.hosts[index].?.wait != null) continue;
        if (table.hostEndedLocked(index, table.host_generations[index])) |host| host.release(false);
    }
    for (value.connections, 0..) |entry, slot| {
        const role: state.ConnectionKind = if (entry.host != null) .host else .worker;
        table.connections[slot] = .{ .in_use = true, .kind = role, .pid = entry.pid, .wake = table.connections[slot].wake };
        switch (role) {
            .worker => table.worker_connected = true,
            .host => table.host_connections += 1,
            .pending => unreachable,
        }
        // A Host connection whose Host ended unnamed ends at once, as its unregistration would end it.
        if (entry.host) |host| if (table.findHost(try saved.toProcess(host)) == null) {
            table.connections[slot].closing = true;
        };
    }
    table.settleIdle();
}

fn peerOf(entry: saved.Connection) state.Peer {
    if (entry.host) |host| return .{ .host = saved.toProcess(host) catch unreachable };
    const capabilities = capabilitiesOf(entry.capabilities);
    return .{ .worker = .{ .programs = capabilities.programs, .pty = capabilities.pty } };
}

fn capabilitiesOf(names: []const []const u8) reexec.Capabilities {
    var capabilities: reexec.Capabilities = .{};
    inline for (@typeInfo(reexec.Capabilities).@"struct".fields) |field| {
        for (names) |name| if (std.mem.eql(u8, name, field.name)) {
            @field(capabilities, field.name) = true;
        };
    }
    return capabilities;
}
