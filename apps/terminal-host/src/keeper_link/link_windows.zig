//! A Keeper link's Windows side (`link.zig`): it connects over the Keeper's pipe, holds each
//! terminal and background program under duplicates of its handles, and waits on the wake event
//! and the Keeper process. A hold's duplicates stay open until the Keeper answered, and an
//! unanswered hold's until the Keeper ends, since the Keeper may still pull them; a refused or
//! unanswered release leaves the duplicates with the Keeper, which then keeps the console open
//! until it ends.

const std = @import("std");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");
const core = @import("link.zig");
const connect = @import("connect_windows.zig");
const move_requests = @import("moves.zig");
const holds = @import("hold_windows.zig");

const message = keeper_protocol.message;
const Gate = platform.keeper.Gate;
const Process = platform.process.Process;
const Retained = holds.Retained;

const windows = std.os.windows;
const HANDLE = windows.HANDLE;
const WAIT_OBJECT_0: windows.DWORD = 0;
const INFINITE: windows.DWORD = 0xFFFF_FFFF;

extern "kernel32" fn CreateEventW(attributes: ?*anyopaque, manual_reset: windows.BOOL, initial: windows.BOOL, name: ?[*:0]const u16) callconv(.winapi) ?HANDLE;
extern "kernel32" fn SetEvent(event: HANDLE) callconv(.winapi) windows.BOOL;
extern "kernel32" fn WaitForMultipleObjects(count: windows.DWORD, handles: [*]const HANDLE, wait_all: windows.BOOL, milliseconds: windows.DWORD) callconv(.winapi) windows.DWORD;

pub const Link = core.Link(@This());
pub const Phase = core.Phase;
pub const Queued = holds.Queued;
pub const Program = holds.Program;

/// Auto-reset: the dispatch thread, a gate's `pass`, or teardown sets it.
pub const Wake = HANDLE;
pub const Connection = connect.Connection;
pub const Watch = connect.Watch;
/// The Keeper's pipe endpoint id.
pub const Endpoint = [32]u8;
/// A request sends nothing beside it: the Keeper pulls what a hold names.
pub const Sent = void;
pub const nothing_sent: Sent = {};

pub const Own = struct {
    endpoint_id: [32]u8,
    /// The Keeper negotiated `programs`, so it takes background programs. Guarded by `mutex`.
    programs: bool = false,
    /// Duplicates an unanswered `hold` named, kept until the Keeper ends. Owned by the link thread.
    retained: std.ArrayList(Retained) = .empty,

    pub fn init(_: std.mem.Allocator, endpoint: Endpoint) !Own {
        return .{ .endpoint_id = endpoint };
    }

    pub fn deinit(self: *Own, allocator: std.mem.Allocator) void {
        self.retained.deinit(allocator);
    }
};

pub fn createWake() ?Wake {
    return CreateEventW(null, .FALSE, .FALSE, null);
}

pub fn signal(wake: Wake) void {
    _ = SetEvent(wake);
}

pub fn closeWake(wake: Wake) void {
    windows.CloseHandle(wake);
}

pub fn queuedTerminal(process: *Process, terminal: message.Terminal) Queued {
    return .{ .gate = undefined, .holding = undefined, .subject = .{ .terminal = .{ .process = process, .terminal = terminal } } };
}

/// The Worker hears of no background program left unheld.
pub fn unheldTerminal(item: Queued) ?[16]u8 {
    return switch (item.subject) {
        .terminal => |terminal| terminal.terminal.terminal_id,
        .program => null,
    };
}

/// Queues a background program's hold unless this attachment takes no holds, its Keeper takes
/// no programs, or it already tried this one.
pub fn offerProgram(link: *Link, gate: *Gate, program: Program) void {
    link.mutex.lock();
    defer link.mutex.unlock();
    if (!link.accepting or !link.own.programs) return;
    link.queueLocked(gate, .{ .gate = undefined, .holding = undefined, .subject = .{ .program = program } });
}

pub fn open(link: *Link) bool {
    link.connection = connect.open(link.keeper, &link.own.endpoint_id, core.hello_id, link.watch(), link.allocator) catch return false;
    link.mutex.lock();
    defer link.mutex.unlock();
    link.own.programs = link.connection.?.programs;
    return true;
}

pub fn serve(link: *Link, detaching: bool) void {
    link.serveHolds(detaching);
    if (!link.keeperEnded()) move_requests.serve(link);
}

/// Waits for work; false when the Keeper process ended.
pub fn idle(link: *Link) bool {
    const handles = [_]HANDLE{ link.connection.?.keeper_process, link.wake };
    return WaitForMultipleObjects(handles.len, &handles, .FALSE, INFINITE) != WAIT_OBJECT_0;
}

/// Holds what `item` names under duplicates this link owns, so the values the request names stay
/// valid until the Keeper answered.
pub fn hold(link: *Link, item: Queued, detaching: bool) void {
    var prepared = holds.prepare(item) catch |err| {
        item.gate.settle(link.index);
        return link.postUnheld(item, switch (err) {
            error.SystemConsole => "system-console",
            error.HoldFailed => "hold-failed",
        });
    };
    const outcome = link.call(prepared.body);
    if (outcome == .unanswered) {
        // The Keeper may still pull these values, so they stay open until it ends.
        link.own.retained.append(link.allocator, prepared.retained) catch prepared.retained.close();
    } else prepared.retained.close();
    link.settleHold(item, outcome, detaching);
}

pub fn exchange(connection: *Connection, arena: std.mem.Allocator, request: message.Request, _: Sent, watch: Watch) core.Exchanged {
    const reply = connection.request(arena, request, watch) catch |err| return switch (err) {
        error.KeeperEnded => .keeper_ended,
        else => .broken,
    };
    return .{ .reply = reply };
}

pub fn breakOff(connection: *Connection) void {
    connection.closePipe();
}

/// After a detach, an unanswered `hold` named values the Keeper may still pull; they stay open
/// until it ends or the Host stops.
pub fn afterDetach(link: *Link) void {
    while (link.own.retained.items.len > 0 and !link.keeperEnded()) {
        if (!idle(link) or link.requests().stop) break;
    }
    dropKept(link);
}

/// Closes the duplicates unanswered holds kept.
pub fn dropKept(link: *Link) void {
    for (link.own.retained.items) |*retained| retained.close();
    link.own.retained.clearRetainingCapacity();
}
