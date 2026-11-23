//! One background program the Host runs for its Worker: its process, Job, and captured streams,
//! the threads that read them and wait for its exit, and the outcome they publish. The dispatch
//! thread starts, cancels, reads, and ends it; the threads only fill its captures and its outcome.

const std = @import("std");
const platform = @import("terminal-host-platform");
const control = @import("terminal-host-control");

const program = platform.program;
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

    /// Spawns the readers and the waiter. A thread that cannot start ends the program first.
    fn follow(self: *Entry) !void {
        for (&self.captures) |*slot| if (slot.*) |*capture| {
            capture.thread = try std.Thread.spawn(.{ .stack_size = thread_stack_bytes }, readCapture, .{capture});
        };
        self.waiter = try std.Thread.spawn(.{ .stack_size = thread_stack_bytes }, wait, .{self});
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

    /// Ends the program if it still runs, joins its threads, closes its handles, and frees it.
    pub fn end(self: *Entry) void {
        _ = program.endMembers(self.spawned.job);
        if (self.waiter) |thread| thread.join();
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

/// Waits for the root's exit, ends the Job's remaining members as the Worker's own owner does, lets
/// every capture reach its end, and publishes the outcome.
fn wait(self: *Entry) void {
    _ = program.waitExit(self.spawned.process, std.math.maxInt(u32));
    const code = program.exitCode(self.spawned.process) orelse 1;
    _ = program.endMembers(self.spawned.job);
    for (&self.captures) |*slot| if (slot.*) |*capture| if (capture.thread) |thread| {
        thread.join();
        capture.thread = null;
    };
    self.mutex.lock();
    defer self.mutex.unlock();
    self.exit = if (self.cancelled) .cancelled else .{ .code = code };
}
