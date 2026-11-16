//! The one thread that writes a pseudoconsole's signals, and its close.
//!
//! ConPTY writes every resize and clear into a 4 KiB signal pipe, and OpenConsole reads it only
//! while it can take its console lock, which it cannot while its output is unread. A full pipe
//! blocks the writer until the reader drains the output. So no Host thread writes a signal: a
//! request only records itself here and returns, and this per-terminal sender writes it while it
//! holds no lock that the reader or any Host thread takes. Sizes coalesce to the latest, so a
//! stalled console holds at most what its pipe accepted plus one queued size and one clear. Close
//! cancels a blocked write, as `abortRead` cancels the reader, and makes no ConPTY call until the
//! sender has stopped.
const std = @import("std");
const windows = std.os.windows;
const fatal_record = @import("fatal_record.zig");
const thread_sync = @import("thread_sync.zig");
const windows_api = @import("windows_api.zig");
const windows_conpty = @import("conpty_windows.zig");

/// The sender only calls into ConPTY, which writes one pipe packet.
const sender_stack_bytes = 256 * 1024;

pub const Signals = struct {
    resize_console: @FieldType(windows_conpty.Api, "resize"),
    clear_console: @FieldType(windows_conpty.Api, "clear"),
    close_console: @FieldType(windows_conpty.Api, "close"),
    /// Owned from `start` until `close` closes it.
    pseudo_console: windows_api.HPCON,
    sender: std.Thread = undefined,
    /// Guards every field below. Held only for bookkeeping, never across a ConPTY call.
    mutex: thread_sync.Mutex = .{},
    /// Wakes the sender for queued work or close, and close or a second closer for progress.
    condition: thread_sync.Condition = .{},
    queue: Queue = .{},
    state: State = .open,
    /// The sender is inside a ConPTY call.
    sending: bool = false,
    /// A signal write failed while the console was open; later requests are refused.
    failed: bool = false,

    const State = enum { open, closing, closed };

    /// Signals in pipe order: a size, a clear, then a size requested after that clear.
    const Queue = struct {
        size: ?windows.COORD = null,
        clear: bool = false,
        size_after_clear: ?windows.COORD = null,

        fn take(self: *Queue) ?Signal {
            if (self.size) |size| {
                self.size = null;
                return .{ .resize = size };
            }
            if (!self.clear) return null;
            self.clear = false;
            self.size = self.size_after_clear;
            self.size_after_clear = null;
            return .clear;
        }
    };

    const Signal = union(enum) { resize: windows.COORD, clear };

    /// Takes ownership of `pseudo_console` and starts its sender; on failure the caller still owns
    /// the pseudoconsole. `api` must stay loaded until `close` returns.
    pub fn start(
        allocator: std.mem.Allocator,
        api: *const windows_conpty.Api,
        pseudo_console: windows_api.HPCON,
    ) !*Signals {
        const self = try allocator.create(Signals);
        errdefer allocator.destroy(self);
        self.* = .{
            .resize_console = api.resize,
            .clear_console = api.clear,
            .close_console = api.close,
            .pseudo_console = pseudo_console,
        };
        self.sender = try std.Thread.spawn(.{ .stack_size = sender_stack_bytes }, send, .{self});
        return self;
    }

    /// Frees a closed sender.
    pub fn destroy(self: *Signals, allocator: std.mem.Allocator) void {
        std.debug.assert(self.state == .closed);
        allocator.destroy(self);
    }

    /// Queues `size` and returns without waiting for the console. A queued size that the sender
    /// has not taken is replaced, since only the latest size matters.
    pub fn resize(self: *Signals, size: windows.COORD) error{ PtyClosed, ResizePtyFailed }!void {
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.state != .open) return error.PtyClosed;
        if (self.failed) return error.ResizePtyFailed;
        if (self.queue.clear) self.queue.size_after_clear = size else self.queue.size = size;
        self.condition.broadcast();
    }

    /// Queues ConPTY's buffer clear that keeps the cursor row, after any queued size, and returns
    /// without waiting for the console. False when nothing will be signalled: the system ConHost
    /// has no clear, the console is closed or failed, or a clear and a later size are already
    /// waiting for a stalled console. A clear already waiting with nothing after it is this one.
    pub fn clearKeepingCursorRow(self: *Signals) bool {
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.clear_console == null or self.state != .open or self.failed) return false;
        if (self.queue.clear) return self.queue.size_after_clear == null;
        self.queue.clear = true;
        self.condition.broadcast();
        return true;
    }

    /// Stops the sender, cancelling a write the console has not taken, and then closes the
    /// pseudoconsole. Every call returns once it is closed; only the first one closes it.
    pub fn close(self: *Signals) void {
        self.mutex.lock();
        switch (self.state) {
            .open => {},
            .closing => {
                while (self.state != .closed) self.condition.wait(&self.mutex);
                self.mutex.unlock();
                return;
            },
            .closed => {
                self.mutex.unlock();
                return;
            },
        }
        self.state = .closing;
        self.queue = .{};
        self.condition.broadcast();
        var cancellation_requested = false;
        while (self.sending) {
            if (cancellation_requested) {
                self.condition.wait(&self.mutex);
                continue;
            }
            self.mutex.unlock();
            if (windows_api.CancelSynchronousIo(self.sender.getHandle()).toBool()) {
                cancellation_requested = true;
            } else switch (windows.GetLastError()) {
                // The sender marks itself sending just before it calls ConPTY, and its write may
                // have just completed; check again until it is in the write or out of the call.
                .NOT_FOUND => std.Thread.yield() catch {},
                // Without cancellation the write ends once the reader drains the output.
                else => cancellation_requested = true,
            }
            self.mutex.lock();
        }
        self.mutex.unlock();
        self.sender.join();
        // The sender has returned, so this is the only ConPTY call on this pseudoconsole.
        self.close_console(self.pseudo_console);
        self.mutex.lock();
        self.state = .closed;
        self.condition.broadcast();
        self.mutex.unlock();
    }

    fn send(self: *Signals) void {
        fatal_record.markThread(.@"terminal-signals");
        self.mutex.lock();
        defer self.mutex.unlock();
        while (self.state == .open) {
            const signal = self.queue.take() orelse {
                self.condition.wait(&self.mutex);
                continue;
            };
            self.sending = true;
            self.mutex.unlock();
            const result = switch (signal) {
                .resize => |size| self.resize_console(self.pseudo_console, size),
                .clear => self.clear_console.?(self.pseudo_console, .TRUE),
            };
            self.mutex.lock();
            self.sending = false;
            // A write close cancelled is not a console failure.
            if (result != windows_api.S_OK and self.state == .open) {
                self.failed = true;
                self.queue = .{};
            }
            self.condition.broadcast();
        }
    }
};
