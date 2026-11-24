//! Kitty graphics file, temporary-file, and shared-memory media, read off the terminal lock.
//!
//! Ghostty reads and decodes such a medium inside its command handler, which runs under the
//! terminal lock. The reader instead takes the command at its APC end, stops feeding the batch
//! there, and commits what came before. A `terminal-media` thread reads and decodes the medium
//! through Ghostty's own loader, so its path checks stay, while the reader waits without any
//! terminal lock for at most `read_deadline_ns`. The next batch then hands Ghostty the decoded
//! pixels as a direct transmission of the same command, so Ghostty still applies its storage,
//! response, quiet setting, and identifiers, and the reader feeds the rest of the output after it.
//!
//! A read that fails, would exceed its bounds, or misses the deadline drops the image: Ghostty
//! then runs the command with no data, which fails it as a failed load would, and the log keeps
//! the reason. A read still blocked at the deadline is left to finish alone; it holds nothing of
//! the terminal, and until it finishes that terminal drops each further medium at once. A path the
//! Host does not admit is dropped before anything opens it, and a medium dropped unread is removed
//! as Ghostty's loader would have removed it (`graphics_media_files.zig`).
const std = @import("std");
const target_builtin = @import("builtin");
const ghostty_vt = @import("ghostty-vt");
const platform = @import("terminal-host-platform");
const fatal_record = platform.fatal_record;
const monotonic = platform.monotonic;
const thread_sync = platform.thread_sync;
const protocol = @import("terminal-protocol");
const terminal_stream = @import("../terminal_stream.zig");
const media_files = @import("graphics_media_files.zig");
const state = @import("state.zig");
const output_ops = @import("output.zig");
const program_status_feed = @import("program_status_feed.zig");

const graphics = ghostty_vt.kitty.graphics;
const Command = graphics.Command;
const Transmission = @FieldType(Command.Control, "transmit");

/// How long the reader waits for a medium before it drops the image and goes on.
pub const read_deadline_ns: u64 = 2 * std.time.ns_per_s;
/// The most memory one read may hold at once: the medium, its decoded pixels, and a copy.
pub const read_memory_limit: usize = 3 * protocol.Limits.terminal_graphics_resource_bytes;
const wait_slice_ns: u64 = 20 * std.time.ns_per_ms;

pub const State = struct {
    /// The command the current batch stopped at, its path data owned here.
    pending: ?Command = null,
    /// A read that missed its deadline and has not finished yet.
    stalled: ?*Read = null,
};

/// Why an image was dropped; the log keeps it.
const Reason = enum {
    @"path not admitted",
    @"earlier read still running",
    @"read thread unavailable",
    @"not a regular file",
    @"larger than 32 MiB",
    @"no bytes to read",
    @"memory bound exceeded",
    @"rejected by the loader",
    @"timed out",
};

const Failure = struct {
    reason: Reason,
    cause: ?anyerror = null,
};

const Media = struct {
    data: []u8,
    format: Transmission.Format,
    compression: Transmission.Compression,
    width: u32,
    height: u32,
};

const Result = union(enum) {
    loaded: Media,
    failed: Failure,
};

/// One medium read, shared by the reader and the read's thread; the last release frees it.
const Read = struct {
    allocator: std.mem.Allocator,
    budget: terminal_stream.BoundedAllocator,
    command: Command,
    limits: graphics.LoadingImage.Limits,
    temporary_directory: ?[]u8,
    starting: if (target_builtin.is_test) ?TestHook else void,
    mutex: thread_sync.Mutex = .{},
    condition: thread_sync.Condition = .{},
    done: bool = false,
    result: Result = undefined,
    references: std.atomic.Value(u8) = .init(1),

    fn finished(self: *Read) bool {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.done;
    }

    fn release(self: *Read) void {
        if (self.references.fetchSub(1, .acq_rel) != 1) return;
        if (self.done) switch (self.result) {
            .loaded => |media| self.budget.allocator().free(media.data),
            .failed => {},
        };
        self.command.deinit(self.allocator);
        if (self.temporary_directory) |path| self.allocator.free(path);
        const allocator = self.allocator;
        allocator.destroy(self);
    }
};

const TestHook = struct {
    run: *const fn (*anyopaque) void,
    context: *anyopaque,
};

/// What the next batch applies before it feeds the rest of the output.
pub const Loaded = struct {
    /// The command without its path data.
    command: Command,
    result: Result,
    read: ?*Read,

    pub fn deinit(self: *Loaded) void {
        if (self.read) |read| read.release();
        self.* = undefined;
    }
};

/// The terminal stream's Kitty graphics hook: takes a command whose medium Ghostty would read.
pub fn takeEffect(handler: *terminal_stream.Handler, command: *Command) bool {
    const self = output_ops.runtimeFromHostHandler(handler);
    if (!readsMedium(&self.terminal, command)) return false;
    // The handler frees the command's own data after this hook.
    var dropped = command.*;
    // Refused before anything opens it, in output replayed at creation too.
    if (!media_files.pathAdmitted(command.data)) {
        drop(self, &dropped, .{ .reason = .@"path not admitted" });
        return true;
    }
    // Only a reader batch can stop at the command; earlier output replayed at creation runs inline.
    if (!self.reader_commit_in_progress) return false;
    if (self.graphics_media.pending != null or stalledReadRunning(self)) {
        media_files.removeUnread(command, imageLimits(self));
        drop(self, &dropped, .{ .reason = .@"earlier read still running" });
        return true;
    }
    self.graphics_media.pending = .{
        .control = command.control,
        .quiet = command.quiet,
        .data = command.toOwnedData(),
    };
    return true;
}

/// Feeds `bytes` up to and including the end of an APC whose command was taken; returns how many.
pub fn feedLocked(self: *state.State, bytes: []const u8) usize {
    std.debug.assert(self.reader_commit_in_progress);
    var consumed: usize = 0;
    while (consumed < bytes.len and self.graphics_media.pending == null) {
        const end = consumed + sliceLength(&self.stream, bytes[consumed..]);
        program_status_feed.feedLocked(self, bytes[consumed..end]);
        consumed = end;
    }
    return consumed;
}

pub fn pending(self: *const state.State) bool {
    return self.graphics_media.pending != null;
}

/// Releases a taken command that a failed batch will not load, and removes its medium.
pub fn discardPending(self: *state.State) void {
    if (self.graphics_media.pending) |command| {
        media_files.removeUnread(&command, imageLimits(self));
        command.deinit(self.allocator);
    }
    self.graphics_media.pending = null;
}

/// Releases a read that missed its deadline once it has finished; each reader batch starts here,
/// so a finished read is released without waiting for a later medium.
pub fn releaseFinishedRead(self: *state.State) void {
    _ = stalledReadRunning(self);
}

/// Reads the taken command's medium on its own thread and waits for it, holding no terminal
/// lock. Returns null when output was aborted meanwhile.
pub fn load(self: *state.State) ?Loaded {
    const taken = self.graphics_media.pending.?;
    self.graphics_media.pending = null;
    const command: Command = .{ .control = taken.control, .quiet = taken.quiet };
    const started = startRead(self, taken) catch |err| return .{
        .command = command,
        .result = .{ .failed = .{ .reason = .@"read thread unavailable", .cause = err } },
        .read = null,
    };
    const read = started.read;
    var timer = monotonic.Timer.start() catch null;
    var waited_ns: u64 = 0;
    read.mutex.lock();
    while (!read.done) {
        if (self.output_abort_requested.load(.acquire)) {
            read.mutex.unlock();
            started.thread.detach();
            read.release();
            return null;
        }
        const elapsed_ns = if (timer) |*value| value.read() else waited_ns;
        if (elapsed_ns >= read_deadline_ns) {
            read.mutex.unlock();
            started.thread.detach();
            // The reader keeps its reference until the read finishes; `takeEffect` takes no
            // command while an earlier read is still running.
            std.debug.assert(self.graphics_media.stalled == null);
            self.graphics_media.stalled = read;
            return .{ .command = command, .result = .{ .failed = .{ .reason = .@"timed out" } }, .read = null };
        }
        const slice_ns = @min(wait_slice_ns, read_deadline_ns - elapsed_ns);
        read.condition.timedWait(&read.mutex, slice_ns) catch {};
        waited_ns += slice_ns;
    }
    const result = read.result;
    read.mutex.unlock();
    // A finished read's thread only has its release left.
    started.thread.join();
    return .{ .command = command, .result = result, .read = read };
}

/// Hands Ghostty the loaded command as a direct transmission, or drops it; consumes `loaded`.
pub fn applyLocked(self: *state.State, loaded: *Loaded) void {
    std.debug.assert(self.reader_commit_in_progress);
    defer loaded.deinit();
    var command = loaded.command;
    switch (loaded.result) {
        .failed => |failure| drop(self, &command, failure),
        .loaded => |media| {
            const transmission = transmissionOf(&command);
            transmission.medium = .direct;
            transmission.offset = 0;
            transmission.size = 0;
            transmission.format = media.format;
            transmission.compression = media.compression;
            transmission.width = media.width;
            transmission.height = media.height;
            // Ghostty copies a direct payload; the read keeps ownership of this one.
            command.data = media.data;
            self.stream.handler.runKittyGraphics(&command);
        },
    }
}

/// Releases what a runtime still holds of its reads, once its reader has stopped.
pub fn deinit(self: *state.State) void {
    discardPending(self);
    if (self.graphics_media.stalled) |read| read.release();
    self.graphics_media.stalled = null;
}

/// Runs the command without its medium, so Ghostty fails it as a failed load and stores nothing.
fn drop(self: *state.State, command: *Command, failure: Failure) void {
    const transmission = transmissionOf(command);
    std.log.warn("dropped a Kitty graphics image read from {s} media: {s}{s}{s}", .{
        @tagName(transmission.medium),
        @tagName(failure.reason),
        if (failure.cause != null) ", " else "",
        if (failure.cause) |cause| @errorName(cause) else "",
    });
    transmission.medium = .direct;
    transmission.offset = 0;
    transmission.size = 0;
    transmission.compression = .none;
    switch (command.control) {
        .query => {},
        else => transmission.more_chunks = false,
    }
    command.data = "";
    self.stream.handler.runKittyGraphics(command);
}

fn stalledReadRunning(self: *state.State) bool {
    const read = self.graphics_media.stalled orelse return false;
    if (!read.finished()) return true;
    read.release();
    self.graphics_media.stalled = null;
    return false;
}

/// Whether Ghostty would read this command's medium: the same checks it makes before reading.
fn readsMedium(terminal: *ghostty_vt.Terminal, command: *const Command) bool {
    const storage = &terminal.screens.active.kitty_images;
    if (!storage.enabled()) return false;
    const identifiers = command.control.identifiers();
    if (identifiers.image_id > 0 and identifiers.image_number > 0) return false;
    const transmission = command.transmission() orelse return false;
    switch (command.control) {
        .query => if (transmission.image_id == 0) return false,
        // A command that arrives while a load is in progress continues it with its own bytes.
        else => if (storage.loading != null) return false,
    }
    if (transmission.format_unknown) return false;
    const limits = storage.image_limits;
    return switch (transmission.medium) {
        .direct => false,
        .file => limits.file,
        .temporary_file => limits.temporary_file == .enabled,
        .shared_memory => limits.shared_memory,
    };
}

fn transmissionOf(command: *Command) *Transmission {
    return switch (command.control) {
        .query, .transmit => |*transmission| transmission,
        .transmit_and_display => |*value| &value.transmission,
        .transmit_animation_frame => |*frame| &frame.transmission,
        .display, .delete, .control_animation, .compose_animation => unreachable,
    };
}

/// The length of the next slice to feed. It ends just after a byte that may end an APC, so a
/// command taken at that end stops the feed before any later byte, and just after the byte that
/// opens one, so the next slice is measured with `in_apc` set. A C1 byte can also open one once
/// the parser has left ground; that rare form is fed in one slice and only loses ordering.
fn sliceLength(stream: *const terminal_stream.Stream, bytes: []const u8) usize {
    if (stream.handler.in_apc) return if (apcEndCandidate(bytes)) |index| index + 1 else bytes.len;
    if (stream.parser.state == .escape and opensString(bytes[0])) return 1;
    var start: usize = 0;
    while (std.mem.indexOfScalarPos(u8, bytes, start, 0x1b)) |escape| {
        if (escape + 1 == bytes.len) return bytes.len;
        if (opensString(bytes[escape + 1])) return escape + 2;
        start = escape + 1;
    }
    return bytes.len;
}

fn opensString(byte: u8) bool {
    return byte == '_' or byte == 'X' or byte == '^';
}

/// The first byte that can leave an APC string: CAN, SUB, ESC, or a byte at or above 0x80.
fn apcEndCandidate(bytes: []const u8) ?usize {
    var index: usize = 0;
    if (comptime std.simd.suggestVectorLength(u8)) |length| {
        const Vector = @Vector(length, u8);
        while (index + length <= bytes.len) : (index += length) {
            const chunk: Vector = bytes[index..][0..length].*;
            const candidates = (chunk == @as(Vector, @splat(0x18))) |
                (chunk == @as(Vector, @splat(0x1a))) |
                (chunk == @as(Vector, @splat(0x1b))) |
                (chunk >= @as(Vector, @splat(0x80)));
            if (@reduce(.Or, candidates)) break;
        }
    }
    while (index < bytes.len) : (index += 1) switch (bytes[index]) {
        0x18, 0x1a, 0x1b, 0x80...0xff => return index,
        else => {},
    };
    return null;
}

const StartedRead = struct {
    read: *Read,
    thread: std.Thread,
};

/// Starts the read on its own thread; the read owns `taken` from here, even when this fails, and
/// a read that cannot start removes its medium.
fn startRead(self: *state.State, taken: Command) !StartedRead {
    const read = createRead(self, taken) catch |err| {
        media_files.removeUnread(&taken, imageLimits(self));
        taken.deinit(self.allocator);
        return err;
    };
    // The read's thread takes the second reference.
    read.references.store(2, .release);
    const thread = std.Thread.spawn(platform.thread_stack.default_budget, readMedium, .{read}) catch |err| {
        media_files.removeUnread(&read.command, read.limits);
        read.references.store(1, .release);
        read.release();
        return err;
    };
    return .{ .read = read, .thread = thread };
}

fn imageLimits(self: *const state.State) graphics.LoadingImage.Limits {
    return self.terminal.screens.active.kitty_images.image_limits;
}

fn createRead(self: *state.State, taken: Command) !*Read {
    const limits = imageLimits(self);
    const temporary_directory: ?[]u8 = switch (limits.temporary_file) {
        .enabled => |value| try self.allocator.dupe(u8, value.directory),
        .disabled => null,
    };
    errdefer if (temporary_directory) |path| self.allocator.free(path);
    const read = try self.allocator.create(Read);
    read.* = .{
        .allocator = self.allocator,
        .budget = .init(self.allocator, read_memory_limit),
        .command = taken,
        .limits = .{
            .file = limits.file,
            .temporary_file = if (temporary_directory) |path|
                .{ .enabled = .{ .directory = path } }
            else
                .disabled,
            .shared_memory = limits.shared_memory,
        },
        .temporary_directory = temporary_directory,
        .starting = if (comptime target_builtin.is_test)
            if (self.testing_hooks.graphics_media_read_starting) |run|
                .{ .run = run, .context = self.testing_hooks.context.? }
            else
                null
        else {},
    };
    return read;
}

fn readMedium(read: *Read) void {
    fatal_record.markThread(.@"terminal-media");
    if (comptime target_builtin.is_test) if (read.starting) |hook| hook.run(hook.context);
    const result = readResult(read);
    read.mutex.lock();
    read.result = result;
    read.done = true;
    read.condition.broadcast();
    read.mutex.unlock();
    read.release();
}

fn readResult(read: *Read) Result {
    const io = thread_sync.io();
    var command = read.command;
    const transmission = transmissionOf(&command);
    switch (transmission.medium) {
        .file, .temporary_file => switch (media_files.precheck(io, &command, read.temporary_directory)) {
            .read => |size| transmission.size = size,
            .refused => |refused| return failed(switch (refused.refusal) {
                inline else => |refusal| @field(Reason, @tagName(refusal)),
            }, refused.cause),
        },
        .shared_memory => {},
        .direct => unreachable,
    }
    const allocator = read.budget.allocator();
    var loading = graphics.LoadingImage.init(io, allocator, &command, read.limits) catch |err|
        return loaderFailure(read, err);
    defer loading.deinit(allocator);
    // A frame or a first chunk completes later, under the lock, with the rest of its data.
    const completes_later = switch (command.control) {
        .query => false,
        .transmit_animation_frame => true,
        else => transmission.more_chunks,
    };
    if (completes_later) {
        const data = loading.data.toOwnedSlice(allocator) catch |err| return loaderFailure(read, err);
        return .{ .loaded = .{
            .data = data,
            .format = loading.image.format,
            .compression = loading.image.compression,
            .width = loading.image.width,
            .height = loading.image.height,
        } };
    }
    const image = loading.complete(allocator) catch |err| return loaderFailure(read, err);
    return .{ .loaded = .{
        .data = @constCast(image.data.complete),
        .format = image.format,
        .compression = .none,
        .width = image.width,
        .height = image.height,
    } };
}

fn failed(reason: Reason, cause: ?anyerror) Result {
    return .{ .failed = .{ .reason = reason, .cause = cause } };
}

fn loaderFailure(read: *Read, err: anyerror) Result {
    return failed(if (read.budget.exhausted) .@"memory bound exceeded" else .@"rejected by the loader", err);
}
