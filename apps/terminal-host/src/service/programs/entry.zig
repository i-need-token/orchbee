//! One background program the Host runs for its Worker: its process, Job, and captured streams,
//! the threads that read them and wait for its exit, and the outcome they publish. The dispatch
//! thread starts, cancels, reads, and ends it; the threads only fill its captures and its outcome.
//! Its release-first gate keeps every Keeper that holds it from holding past its end: the waiter
//! passes the gate before it ends the Job's members and publishes the exit.

const std = @import("std");
const builtin = @import("builtin");
const platform = @import("terminal-host-platform");
const control = @import("terminal-host-control");

const program = platform.program;
const is_windows = builtin.os.tag == .windows;
const Gate = platform.keeper.Gate;
const thread_sync = platform.thread_sync;
const Summary = control.encode.program.Summary;
const Exit = control.encode.program.Exit;

/// Each captured stream keeps this many bytes and discards the rest.
pub const capture_bytes = 64 * 1024;
const thread_stack_bytes = 256 * 1024;

const Capture = struct {
    data: []u8,
    len: u32 = 0,
    truncated: bool = false,
    thread: ?std.Thread = null,
    stream: program.Handle,
};

pub const Entry = struct {
    allocator: std.mem.Allocator,
    id: [16]u8,
    label: []u8,
    spawned: program.Spawned,
    captures: [2]?Capture,
    waiter: ?std.Thread = null,
    mutex: thread_sync.Mutex = .{},
    /// Set by the waiter once the root exited, the Job is empty, and every capture ended.
    exit: ?Exit = null,
    cancelled: bool = false,
    /// Whether a session has received this program's `ProgramExited`.
    announced: bool = false,
    /// Each attached Keeper's slot; a hold keeps the program's handles and label alive.
    gate: Gate = if (Gate == void) {} else .{},

    /// Starts `launch` and the threads that follow it. On failure nothing keeps running.
    pub fn start(allocator: std.mem.Allocator, id: [16]u8, label: []const u8, launch: program.Launch) !*Entry {
        const entry = try allocator.create(Entry);
        const owned_label = allocator.dupe(u8, label) catch |err| {
            allocator.destroy(entry);
            return err;
        };
        var buffers: [2]?[]u8 = .{ null, null };
        const spawned = spawned: {
            for (launch.capture, 0..) |wanted, index| {
                if (wanted) buffers[index] = allocator.alloc(u8, capture_bytes) catch break :spawned error.OutOfMemory;
            }
            break :spawned program.spawn(allocator, launch);
        } catch |err| {
            for (buffers) |buffer| if (buffer) |bytes| allocator.free(bytes);
            allocator.free(owned_label);
            allocator.destroy(entry);
            return err;
        };
        entry.* = .{
            .allocator = allocator,
            .id = id,
            .label = owned_label,
            .spawned = spawned,
            .captures = .{ null, null },
        };
        for (spawned.streams, 0..) |stream, index| {
            if (stream) |handle| entry.captures[index] = .{ .data = buffers[index].?, .stream = handle };
        }
        buffers = .{ null, null };
        entry.follow() catch |err| {
            entry.abandon();
            return err;
        };
        return entry;
    }

    /// A program a Keeper kept after its Host ended, from this Host's duplicates of its handles,
    /// which it then owns. Its captures restart at zero and report truncated, since what the lost
    /// Host read is gone. Its threads start with `follow` once its holding is claimed.
    pub fn recovered(
        allocator: std.mem.Allocator,
        id: [16]u8,
        label: []const u8,
        pid: u32,
        created: u64,
        handles: Handles,
    ) error{OutOfMemory}!*Entry {
        if (comptime !is_windows) unreachable;
        const entry = try allocator.create(Entry);
        errdefer allocator.destroy(entry);
        const owned_label = try allocator.dupe(u8, label);
        errdefer allocator.free(owned_label);
        var buffers: [2]?[]u8 = .{ null, null };
        errdefer for (buffers) |buffer| if (buffer) |bytes| allocator.free(bytes);
        for (handles.streams, &buffers) |stream, *buffer| {
            if (stream != null) buffer.* = try allocator.alloc(u8, capture_bytes);
        }
        entry.* = .{
            .allocator = allocator,
            .id = id,
            .label = owned_label,
            .spawned = .{ .process = handles.process, .job = handles.job, .streams = handles.streams, .pid = pid, .created = created },
            .captures = .{ null, null },
        };
        for (handles.streams, 0..) |stream, index| if (stream) |handle| {
            entry.captures[index] = .{ .data = buffers[index].?, .stream = handle, .truncated = true };
        };
        return entry;
    }

    /// A recovered program's handles: its Job, its process, and each captured stream's read end.
    pub const Handles = struct {
        job: program.Handle,
        process: program.Handle,
        streams: [2]?program.Handle,
    };

    /// Spawns whichever reader and waiter has not started yet; a recovered program retries what
    /// failed. The waiter starts last, so it joins every reader.
    pub fn follow(self: *Entry) !void {
        for (&self.captures) |*slot| if (slot.*) |*capture| if (capture.thread == null) {
            capture.thread = try std.Thread.spawn(.{ .stack_size = thread_stack_bytes }, readCapture, .{capture});
        };
        if (self.waiter == null) self.waiter = try std.Thread.spawn(.{ .stack_size = thread_stack_bytes }, wait, .{self});
    }

    /// Frees a recovered program no thread follows and no Keeper slot holds: its claim did not
    /// apply, so the Keeper keeps it running.
    pub fn discard(self: *Entry) void {
        self.destroy();
    }

    /// Ends a program whose threads did not all start, joins those that did, and frees it.
    fn abandon(self: *Entry) void {
        _ = program.endMembers(self.spawned.job);
        _ = program.waitExit(self.spawned.process, std.math.maxInt(u32));
        for (&self.captures) |*slot| if (slot.*) |*capture| if (capture.thread) |thread| {
            thread.join();
            capture.thread = null;
        };
        self.destroy();
    }

    /// The program's outcome so far, read under its lock.
    pub fn outcome(self: *Entry) ?Exit {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.exit;
    }

    pub fn summary(self: *Entry) Summary {
        const exit = self.outcome();
        var streams: [2]?control.encode.program.StreamFacts = .{ null, null };
        // The waiter joined every reader before it published the exit, so the captures are final.
        if (exit != null) for (self.captures, 0..) |slot, index| if (slot) |capture| {
            streams[index] = .{ .bytes = capture.len, .truncated = capture.truncated };
        };
        return .{
            .program_id = self.id,
            .pid = self.spawned.pid,
            .created = self.spawned.created,
            .exit = exit,
            .streams = streams,
            .label = self.label,
        };
    }

    /// The captured bytes of an exited program's stream, or null for one sent to the null device.
    pub fn captured(self: *Entry, stream: program.Stream) ?[]const u8 {
        std.debug.assert(self.outcome() != null);
        const capture = self.captures[@intFromEnum(stream)] orelse return null;
        return capture.data[0..capture.len];
    }

    /// Ends the program's Job; the waiter then reports it as cancelled. False once it exited.
    pub fn cancel(self: *Entry) bool {
        self.mutex.lock();
        const running = self.exit == null;
        if (running) self.cancelled = true;
        self.mutex.unlock();
        if (running) _ = program.endMembers(self.spawned.job);
        return running;
    }

    /// Ends the program if it still runs, joins its threads, closes its handles, and frees it. A
    /// recovered program whose waiter has not started releases first here and joins its readers.
    pub fn end(self: *Entry) void {
        _ = program.endMembers(self.spawned.job);
        if (self.waiter) |thread| {
            thread.join();
        } else {
            if (comptime is_windows) self.gate.pass();
            for (&self.captures) |*slot| if (slot.*) |*capture| if (capture.thread) |thread| {
                thread.join();
                capture.thread = null;
            };
        }
        self.waiter = null;
        self.destroy();
    }

    fn destroy(self: *Entry) void {
        const allocator = self.allocator;
        for (self.captures) |slot| if (slot) |capture| {
            program.close(capture.stream);
            allocator.free(capture.data);
        };
        program.close(self.spawned.process);
        program.close(self.spawned.job);
        allocator.free(self.label);
        allocator.destroy(self);
    }
};

/// Keeps the first `capture_bytes` of a stream and reads the rest to its end, so the program never
/// blocks on a full pipe.
fn readCapture(capture: *Capture) void {
    var discard: [4096]u8 = undefined;
    while (true) {
        const kept: usize = capture.len;
        const target = if (kept < capture.data.len) capture.data[kept..] else discard[0..];
        const count = program.read(capture.stream, target);
        if (count == 0) return;
        if (kept < capture.data.len) {
            capture.len += @intCast(count);
        } else {
            capture.truncated = true;
        }
    }
}

/// Waits for the root's exit, releases the program from every Keeper, ends the Job's remaining
/// members as the Worker's own owner does, lets every capture reach its end, and publishes the
/// outcome, so no Keeper holds a program whose exit a Worker heard of.
fn wait(self: *Entry) void {
    _ = program.waitExit(self.spawned.process, std.math.maxInt(u32));
    const code = program.exitCode(self.spawned.process) orelse 1;
    if (comptime is_windows) self.gate.pass();
    _ = program.endMembers(self.spawned.job);
    for (&self.captures) |*slot| if (slot.*) |*capture| if (capture.thread) |thread| {
        thread.join();
        capture.thread = null;
    };
    self.mutex.lock();
    defer self.mutex.unlock();
    self.exit = if (self.cancelled) .cancelled else .{ .code = code };
}
