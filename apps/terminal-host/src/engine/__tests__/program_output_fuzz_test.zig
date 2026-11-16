//! Seeded program-output fuzzing through the reader-batch commit a real terminal uses.
//!
//! Program output is untrusted and reaches Ghostty's parser and the Host's effect handlers
//! (replies, mode effects, terminal-driven resizes, Kitty graphics, OSC and APC hooks). In a
//! safety-checked Host build any assertion that output reaches ends the process and every
//! terminal on it. This harness feeds generated output, attachment resizes and projection
//! captures to one terminal with a real PTY, and requires that each batch either commits or
//! fences only that terminal.
//!
//! The Debug gate runs a short fixed-seed pass under Ghostty's slow integrity checks. Volume
//! belongs to `zig build test-output-fuzz -Doptimize=ReleaseSafe`, the release build mode, with
//! `-Doutput-fuzz-seed` and `-Doutput-fuzz-batches`; README "Development" names the command.
const builtin = @import("builtin");
const harness = @import("runtime_harness.zig");
const std = harness.std;
const ghostty_vt = harness.ghostty_vt;
const protocol = harness.protocol;
const options = @import("output_fuzz_options");

const AttachmentId = harness.AttachmentId;
const HostEpoch = harness.HostEpoch;
const ReaderBatchOutcome = harness.ReaderBatchOutcome;
const RuntimeTestingHooks = harness.RuntimeTestingHooks;
const TerminalRuntime = harness.TerminalRuntime;
const commitReaderBatch = harness.commitReaderBatch;
const createRuntimeWithHooks = harness.createRuntimeWithHooks;

const read_batch_bytes = protocol.Limits.pty_read_batch_bytes;
const gate_seed: u64 = 0x6f72_6368_6265_6501;
/// Reader batches per seed without `-Doutput-fuzz-batches`. A Debug batch takes seconds, so the
/// gate's pass stays a few batches long.
const default_batches: u64 = if (builtin.mode == .Debug) 2 else 300;
/// A valid 1x1 RGBA PNG, so the decoded-image placement paths are reached too.
const tiny_png_base64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

/// Discards everything the real reader reads, so only this harness commits output and the run
/// stays seeded. The reader still drains the PTY: the child echoes the replies the terminal
/// writes and ConPTY repaints on each resize, and an undrained pipe stops the console.
const DrainingProbe = struct {
    ready: harness.TestGate = .{},
    runtime: *TerminalRuntime = undefined,

    pub fn hooks(self: *DrainingProbe) RuntimeTestingHooks {
        return .{ .context = self, .read_pty = drain };
    }

    fn drain(context: *anyopaque, buffer: []u8) anyerror!usize {
        const self: *DrainingProbe = @ptrCast(@alignCast(context));
        self.ready.pause();
        while (true) {
            if (try harness.runtimeState(self.runtime).process.read(buffer) == 0) return 0;
        }
    }
};

const Totals = struct {
    bytes: u64 = 0,
    batches: u64 = 0,
    fenced_terminals: u64 = 0,
    projections: u64 = 0,
    resizes: u64 = 0,
    terminals: u64 = 0,
};

/// Ghostty logs an empty `CSI t` at error level, and the Zig test runner fails a test that logged
/// an error. A shadow parser finds that final byte and swaps it for `u`, which leaves the parser
/// in the same state; every other byte reaches the terminal unchanged.
const EmptyWindowOpScrubber = struct {
    parser: ghostty_vt.Parser,

    fn init() EmptyWindowOpScrubber {
        return .{ .parser = .init() };
    }

    fn deinit(self: *EmptyWindowOpScrubber) void {
        self.parser.deinit();
    }

    fn scrub(self: *EmptyWindowOpScrubber, output: []u8) void {
        for (output) |*byte| {
            for (self.parser.next(byte.*)) |action| {
                const dispatch = action orelse continue;
                if (dispatch != .csi_dispatch) continue;
                const csi = dispatch.csi_dispatch;
                if (csi.final == 't' and csi.intermediates.len == 0 and csi.params.len == 0) byte.* = 'u';
            }
        }
    }
};

const Generator = struct {
    allocator: std.mem.Allocator,
    random: std.Random,
    output: std.ArrayList(u8) = .empty,

    fn deinit(self: *Generator) void {
        self.output.deinit(self.allocator);
    }

    fn int(self: *Generator, comptime T: type, at_most: T) T {
        return self.random.intRangeAtMost(T, 0, at_most);
    }

    fn chance(self: *Generator, percent: u8) bool {
        return self.random.uintLessThan(u8, 100) < percent;
    }

    fn bytes(self: *Generator, literal: []const u8) !void {
        try self.output.appendSlice(self.allocator, literal);
    }

    fn print(self: *Generator, comptime format: []const u8, arguments: anytype) !void {
        try self.output.print(self.allocator, format, arguments);
    }

    fn pick(self: *Generator, comptime T: type, items: []const T) T {
        return items[self.random.uintLessThan(usize, items.len)];
    }

    /// A parameter value biased toward boundaries: zero, one, small, u16 and u32 edges.
    fn parameter(self: *Generator) u64 {
        return switch (self.int(u8, 9)) {
            0 => 0,
            1 => 1,
            2 => self.int(u64, 8),
            3 => self.int(u64, 300),
            4 => 255,
            5 => 65535,
            6 => 65536,
            7 => std.math.maxInt(u32),
            8 => std.math.maxInt(u64),
            else => self.int(u64, 5000),
        };
    }

    fn parameters(self: *Generator, separator: u8) !void {
        const count = switch (self.int(u8, 9)) {
            0 => 0,
            9 => self.int(usize, 40),
            else => self.int(usize, 4),
        };
        for (0..count) |index| {
            if (index != 0) try self.output.append(self.allocator, if (self.chance(15)) ':' else separator);
            if (self.chance(12)) continue;
            try self.print("{d}", .{self.parameter()});
        }
    }

    fn randomBytes(self: *Generator, length: usize) !void {
        const slice = try self.output.addManyAsSlice(self.allocator, length);
        self.random.bytes(slice);
    }

    fn invalidUtf8(self: *Generator) !void {
        const pieces = [_][]const u8{
            "\x80",             "\xbf",                 "\xc0\xaf",                 "\xe0\x80\xaf", "\xed\xa0\x80",
            "\xf4\x90\x80\x80", "\xf8\x88\x80\x80\x80", "\xff\xfe",                 "\xc3",         "\xe2\x82",
            "\xf0\x9f\x98",     "\xef\xbf\xbf",         "\xcc\x81\xcc\x81\xcc\x81", "\xe2\x80\x8d", "\xf0\x9f\x91\xa8\xe2\x80\x8d",
        };
        for (0..1 + self.int(usize, 12)) |_| try self.bytes(self.pick([]const u8, &pieces));
    }

    fn text(self: *Generator) !void {
        const pieces = [_][]const u8{
            "hello ",    "world",                      "\r\n",     "\n",       "\r",       "\t",       "\x08", "\x07", "\x0b", "\x0c", "\x0e", "\x0f",
            "漢字",
            "🙂",
            "e\xcc\x81", "\u{1f468}\u{200d}\u{1f469}", "\u{fe0f}",
            "ﷺ",
            "\u{0}",     "\x7f",                       "\xc2\x85", "\xc2\x9b", "\xc2\x9d", "\xc2\x90",
        };
        for (0..1 + self.int(usize, 20)) |_| try self.bytes(self.pick([]const u8, &pieces));
    }

    fn csi(self: *Generator) !void {
        const finals = "@ABCDEFGHIJKLMPSTXZ`abcdefghijlmnpqrstuxz{|}~";
        const privates = [_][]const u8{ "", "", "", "?", ">", "=", "<" };
        const intermediates = [_][]const u8{ "", "", "", " ", "$", "\"", "!", "'", "*", "#", "+", "&" };
        try self.bytes(if (self.chance(5)) "\xc2\x9b" else "\x1b[");
        try self.bytes(self.pick([]const u8, &privates));
        try self.parameters(';');
        try self.bytes(self.pick([]const u8, &intermediates));
        try self.output.append(self.allocator, self.pick(u8, finals));
    }

    /// Modes, resizes and screen switches that drive the Host's own effect handlers.
    fn modes(self: *Generator) !void {
        const modes_list = [_]u16{
            1,    3,    5,    6,    7,    12,   25,   40,   45,   47,   66,   67,   69,   80,
            1000, 1002, 1003, 1004, 1005, 1006, 1007, 1015, 1016, 1035, 1036, 1039, 1042, 1047,
            1048, 1049, 2004, 2026, 2027, 2031, 2048, 9001,
        };
        const enable = self.boolean();
        try self.print("\x1b[{s}{d}{s}", .{
            if (self.chance(90)) "?" else "",
            self.pick(u16, &modes_list),
            if (enable) "h" else "l",
        });
        if (self.chance(20)) try self.bytes(if (self.boolean()) "\x1b[?40h\x1b[?3h" else "\x1b[?40h\x1b[?3l");
        if (self.chance(10)) try self.bytes("\x1b[?3$p\x1b[?40$p");
        if (self.chance(10)) try self.print("\x1b[?{d}s\x1b[?{d}r", .{ self.pick(u16, &modes_list), self.pick(u16, &modes_list) });
    }

    fn boolean(self: *Generator) bool {
        return self.random.boolean();
    }

    fn escape(self: *Generator) !void {
        const sequences = [_][]const u8{
            "\x1bc",    "\x1b7",   "\x1b8",  "\x1bD",  "\x1bE",  "\x1bH",   "\x1bM",   "\x1bZ",
            "\x1b=",    "\x1b>",   "\x1b#8", "\x1b#3", "\x1b#6", "\x1b(0",  "\x1b(B",  "\x1b)0",
            "\x1b*A",   "\x1b+%5", "\x1bn",  "\x1bo",  "\x1b|",  "\x1b}",   "\x1b~",   "\x1b%G",
            "\x1b%@",   "\x1b N",  "\x1b F", "\x1b\\", "\x1b",   "\x1b[",   "\x1b]",   "\x1bP",
            "\x1b_",    "\x1b^",   "\x1bX",  "\x18",   "\x1a",   "\x1b[!p", "\x1b[0J", "\x1b[3J",
            "\x1b[22J",
        };
        try self.bytes(self.pick([]const u8, &sequences));
    }

    fn sgr(self: *Generator) !void {
        try self.bytes("\x1b[");
        const count = 1 + self.int(usize, 12);
        for (0..count) |index| {
            if (index != 0) try self.output.append(self.allocator, ';');
            switch (self.int(u8, 5)) {
                0 => try self.print("38:2::{d}:{d}:{d}", .{ self.parameter(), self.parameter(), self.parameter() }),
                1 => try self.print("48;5;{d}", .{self.parameter()}),
                2 => try self.print("58:5:{d}", .{self.parameter()}),
                3 => try self.print("4:{d}", .{self.parameter()}),
                4 => try self.print("38;2;{d};{d}", .{ self.parameter(), self.parameter() }),
                else => try self.print("{d}", .{self.int(u16, 110)}),
            }
        }
        try self.output.append(self.allocator, 'm');
    }

    fn terminator(self: *Generator) !void {
        switch (self.int(u8, 9)) {
            0 => {},
            1 => try self.bytes("\x1b"),
            2 => try self.bytes("\xc2\x9c"),
            3, 4, 5 => try self.bytes("\x07"),
            else => try self.bytes("\x1b\\"),
        }
    }

    fn payload(self: *Generator) !void {
        switch (self.int(u8, 6)) {
            0 => try self.randomBytes(self.int(usize, 64)),
            1 => try self.randomBytes(self.int(usize, 9000)),
            2 => try self.output.appendNTimes(self.allocator, 'A', self.int(usize, 70_000)),
            3 => try self.print("{d};{d}", .{ self.parameter(), self.parameter() }),
            4 => try self.bytes("?"),
            else => try self.text(),
        }
    }

    fn osc(self: *Generator) !void {
        const commands = [_][]const u8{
            "0",  "1",  "2",  "4",   "5",   "7",   "8",   "9",   "10",  "11",  "12",  "13",   "14",   "17",   "19",
            "21", "22", "52", "104", "105", "110", "111", "112", "133", "633", "777", "1337", "3008", "7501", "66",
            "99", "",   "x",
        };
        try self.bytes(if (self.chance(5)) "\xc2\x9d" else "\x1b]");
        const command = self.pick([]const u8, &commands);
        try self.bytes(command);
        if (self.chance(90)) try self.output.append(self.allocator, ';');
        if (std.mem.eql(u8, command, "4") or std.mem.eql(u8, command, "104")) {
            for (0..1 + self.int(usize, 6)) |_| try self.print("{d};{s};", .{
                self.parameter(),
                self.pick([]const u8, &.{ "?", "rgb:ff/00/7f", "#123456", "rgbi:1/0.5/0", "bogus", "" }),
            });
        } else if (std.mem.eql(u8, command, "8")) {
            try self.print("id={d};https://example.invalid/{d}", .{ self.parameter(), self.parameter() });
        } else if (std.mem.eql(u8, command, "52")) {
            try self.print("{s};{s}", .{
                self.pick([]const u8, &.{ "c", "p", "s", "", "cpq" }),
                self.pick([]const u8, &.{ "?", "aGVsbG8=", "!!!", "aGVsbG8", "" }),
            });
        } else if (std.mem.eql(u8, command, "133") or std.mem.eql(u8, command, "633")) {
            try self.print("{s};{s}", .{
                self.pick([]const u8, &.{ "A", "B", "C", "D", "P", "L", "N", "I", "E", "X" }),
                self.pick([]const u8, &.{ "", "redraw=1", "k=s", "aid=1", "0", "4294967296", "cl=m", "click_events=1" }),
            });
        } else if (std.mem.eql(u8, command, "9")) {
            try self.print("4;{d};{d}", .{ self.parameter(), self.parameter() });
        } else if (std.mem.eql(u8, command, "7501")) {
            try self.bytes(self.pick([]const u8, &.{ "?", "app=t:state=working", "state=clear", "id=1:app=x", "state=done:exit=1" }));
        } else {
            try self.payload();
        }
        try self.terminator();
    }

    fn dcs(self: *Generator) !void {
        const heads = [_][]const u8{ "$q", "+q", "1000p", "q", "0;1q", "$t", "+p", "", "2$u", "=1s" };
        try self.bytes(if (self.chance(5)) "\xc2\x90" else "\x1bP");
        try self.bytes(self.pick([]const u8, &heads));
        switch (self.int(u8, 4)) {
            0 => try self.bytes(self.pick([]const u8, &.{ "m", " q", "r", "s", "\"q", "\"p", "t", "544e", "436f", "6b62" })),
            1 => try self.randomBytes(self.int(usize, 200)),
            2 => try self.output.appendNTimes(self.allocator, '#', self.int(usize, 9000)),
            else => {},
        }
        // An unterminated DCS that a fresh hook replaces exercises the overlapping-hook recovery.
        if (self.chance(20)) try self.bytes("\x1bP$q");
        try self.terminator();
    }

    fn apc(self: *Generator) !void {
        try self.bytes("\x1b_");
        switch (self.int(u8, 4)) {
            0 => {
                try self.bytes("orchbee.agent.v1;");
                try self.bytes(self.pick([]const u8, &.{ "", "eyJ4IjoxfQ", "!!!", "AAAA", "eyJ" }));
            },
            1 => try self.bytes(self.pick([]const u8, &.{ "orchbee.command.v1;start;", "orchbee.command.v1;finish;00000000000000000000000000000001;0", "orchbee.command.v1;start;0123456789abcdef0123456789abcdef" })),
            2 => try self.payload(),
            else => {},
        }
        try self.terminator();
    }

    fn kittyKey(self: *Generator) !void {
        const keys = "afotsvSOiIpmxyXYwhcrCUzdHVPQq";
        const key = self.pick(u8, keys);
        try self.output.append(self.allocator, key);
        try self.output.append(self.allocator, '=');
        switch (key) {
            'a' => try self.output.append(self.allocator, self.pick(u8, "tTqpdfac")),
            't' => try self.output.append(self.allocator, self.pick(u8, "dfts")),
            'o' => try self.output.append(self.allocator, 'z'),
            'd' => try self.output.append(self.allocator, self.pick(u8, "aAiIpPcCnNqQrRxXyYzZfF")),
            'f' => try self.print("{d}", .{self.pick(u16, &.{ 24, 32, 100, 0, 7 })}),
            else => try self.print("{d}", .{if (self.chance(70)) self.int(u64, 8) else self.parameter()}),
        }
    }

    fn kittyPayload(self: *Generator, medium: u8) !void {
        switch (medium) {
            'f', 't', 's' => {
                // Only paths that cannot exist: the harness must never read a real file.
                var name: [80]u8 = undefined;
                const path = std.fmt.bufPrint(
                    &name,
                    "orchbee-output-fuzz-missing-tty-graphics-protocol-{d}",
                    .{self.int(u32, 1_000_000)},
                ) catch unreachable;
                const encoder = std.base64.standard.Encoder;
                const encoded = try self.output.addManyAsSlice(self.allocator, encoder.calcSize(path.len));
                _ = encoder.encode(encoded, path);
            },
            else => switch (self.int(u8, 5)) {
                0 => try self.bytes(tiny_png_base64),
                1 => try self.bytes("/wAA/wAA/wAA/wAA"),
                2 => try self.output.appendNTimes(self.allocator, 'A', 4 * self.int(usize, 4000)),
                3 => try self.randomBytes(self.int(usize, 40)),
                else => {},
            },
        }
    }

    fn kitty(self: *Generator) !void {
        try self.bytes("\x1b_G");
        const medium = self.pick(u8, "ddddddft s");
        const action = self.pick(u8, "tTTTqppdfffaac");
        try self.print("a={c},t={c},f={d},i={d},s={d},v={d}", .{
            action,
            if (medium == ' ') @as(u8, 'd') else medium,
            self.pick(u16, &.{ 24, 32, 100, 32, 24 }),
            1 + self.int(u32, 6),
            if (self.chance(85)) 1 + self.int(u32, 3) else @as(u32, @truncate(self.parameter())),
            if (self.chance(85)) 1 + self.int(u32, 3) else @as(u32, @truncate(self.parameter())),
        });
        for (0..self.int(usize, 8)) |_| {
            try self.output.append(self.allocator, ',');
            try self.kittyKey();
        }
        if (self.chance(85)) {
            try self.output.append(self.allocator, ';');
            try self.kittyPayload(medium);
        }
        try self.terminator();
        // Chunked transmissions continue with m=1 records and end with m=0.
        if (self.chance(15)) {
            for (0..self.int(usize, 4)) |_| {
                try self.bytes("\x1b_Gm=1;AAAA\x1b\\");
            }
            try self.bytes("\x1b_Gm=0;AAAA\x1b\\");
        }
    }

    /// A short sequence that starts an animation, so the animation clock runs during the pass.
    fn kittyAnimation(self: *Generator) !void {
        const id = 1 + self.int(u32, 3);
        try self.print("\x1b_Ga=T,t=d,f=24,i={d},s=1,v=1,q=2;/wAA\x1b\\", .{id});
        for (0..1 + self.int(usize, 3)) |_| {
            try self.print("\x1b_Ga=f,i={d},z={d},s=1,v=1,f=24,q=2;AP8A\x1b\\", .{ id, 1 + self.int(u32, 40) });
        }
        try self.print("\x1b_Ga=a,i={d},s={d},v={d},q=2\x1b\\", .{ id, 1 + self.int(u8, 2), self.int(u32, 3) });
    }

    fn structure(self: *Generator) !void {
        switch (self.int(u8, 7)) {
            0 => try self.print("\x1b[{d};{d}r", .{ self.parameter(), self.parameter() }),
            1 => try self.print("\x1b[?69h\x1b[{d};{d}s", .{ self.parameter(), self.parameter() }),
            2 => try self.print("\x1b[{d}b", .{self.parameter()}),
            3 => try self.print("\x1b[{d}L\x1b[{d}M\x1b[{d}@\x1b[{d}P", .{ self.parameter(), self.parameter(), self.parameter(), self.parameter() }),
            4 => try self.print("\x1b[{d};{d}H", .{ self.parameter(), self.parameter() }),
            5 => try self.print("\x1b[{d};{d};{d}t", .{ self.parameter(), self.parameter(), self.parameter() }),
            6 => try self.print("\x1b[{d}S\x1b[{d}T", .{ self.parameter(), self.parameter() }),
            else => try self.print("\x1b[>{d}u\x1b[={d};{d}u\x1b[?u\x1b[<{d}u", .{ self.parameter(), self.parameter(), self.parameter(), self.parameter() }),
        }
    }

    fn queries(self: *Generator) !void {
        const sequences = [_][]const u8{
            "\x1b[c",   "\x1b[>c",       "\x1b[=c",          "\x1b[5n", "\x1b[6n", "\x1b[?6n",     "\x1b[?996n",
            "\x1b[14t", "\x1b[16t",      "\x1b[18t",         "\x1b[>q", "\x05",    "\x1b[?2026$p", "\x1b[2$p",
            "\x1b[?u",  "\x1b]11;?\x07", "\x1b]4;1;?\x1b\\",
        };
        for (0..1 + self.int(usize, 40)) |_| try self.bytes(self.pick([]const u8, &sequences));
    }

    /// One generated program-output chunk; it may be fed in several reader batches.
    fn chunk(self: *Generator) ![]u8 {
        self.output.clearRetainingCapacity();
        const pieces = 1 + self.int(usize, 24);
        for (0..pieces) |_| {
            switch (self.random.weightedIndex(u8, &.{ 6, 3, 3, 10, 6, 4, 4, 6, 5, 3, 7, 1, 4, 3, 1 })) {
                0 => try self.text(),
                1 => try self.invalidUtf8(),
                2 => try self.randomBytes(self.int(usize, 512)),
                3 => try self.csi(),
                4 => try self.modes(),
                5 => try self.escape(),
                6 => try self.sgr(),
                7 => try self.osc(),
                8 => try self.dcs(),
                9 => try self.apc(),
                10 => try self.kitty(),
                11 => try self.kittyAnimation(),
                12 => try self.structure(),
                13 => try self.queries(),
                else => try self.randomBytes(self.int(usize, read_batch_bytes * 2)),
            }
        }
        return self.output.items;
    }
};

const Terminal = struct {
    probe: DrainingProbe = .{},
    runtime: *TerminalRuntime = undefined,
    projection_sequence: u64 = 0,

    fn start(self: *Terminal) !void {
        self.runtime = try createRuntimeWithHooks(self.probe.hooks());
        self.probe.runtime = self.runtime;
        self.probe.ready.release();
    }

    fn stop(self: *Terminal) void {
        self.runtime.destroy();
    }

    fn project(self: *Terminal) !void {
        self.projection_sequence += 1;
        const outcome = self.runtime.encodeProjectionFrameOwned(std.testing.allocator, 0, .{
            .attachment_id = AttachmentId.fromBytes([_]u8{0x6f} ** 16),
            .base_generation = null,
            .base_revision = null,
            .force_snapshot = true,
            .host_epoch = HostEpoch.fromBytes([_]u8{0x01} ** 16),
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = self.projection_sequence,
        }) catch |err| switch (err) {
            // Each is a refusal the projection owner retries: a deferred capture during
            // synchronized output, a resize this terminal could not apply, or a fenced runtime.
            error.SynchronizedOutput, error.TerminalResizeFailed, error.TerminalRuntimeUnavailable => return,
        };
        switch (outcome) {
            .frame => |frame| {
                defer std.testing.allocator.free(frame.allocation);
                // Every viewer decodes the frame strictly; one it rejects is asked for again.
                _ = try protocol.validateTerminalFrame(std.testing.allocator, frame.bytes);
            },
            .blocked => {},
        }
    }
};

fn resize(random: std.Random, runtime: *TerminalRuntime) void {
    const columns = if (random.uintLessThan(u8, 4) == 0) random.intRangeAtMost(u16, 1, 600) else random.intRangeAtMost(u16, 1, 200);
    const rows = if (random.uintLessThan(u8, 4) == 0) random.intRangeAtMost(u16, 1, 400) else random.intRangeAtMost(u16, 1, 80);
    const pixels = random.boolean();
    const width: u16 = if (pixels) random.intRangeAtMost(u16, 1, 32) else 0;
    const height: u16 = if (pixels) random.intRangeAtMost(u16, 1, 64) else 0;
    // Invalid geometry is refused before any effect; a refused resize is a valid outcome here.
    _ = runtime.resizeGeometry(columns, rows, width, height) catch {};
}

fn run(seed: u64, batch_budget: u64, totals: *Totals) !void {
    var prng: std.Random.DefaultPrng = .init(seed);
    const random = prng.random();
    var generator: Generator = .{ .allocator = std.testing.allocator, .random = random };
    defer generator.deinit();

    var terminal: Terminal = .{};
    try terminal.start();
    totals.terminals += 1;
    var running = true;
    defer if (running) terminal.stop();
    var scrubber: EmptyWindowOpScrubber = .init();
    defer scrubber.deinit();

    var batches: u64 = 0;
    errdefer std.debug.print(
        "program output fuzz failed after {d} batches; reproduce with -Doutput-fuzz-seed={d} -Doutput-fuzz-batches={d}\n",
        .{ batches, seed, batches },
    );
    while (batches < batch_budget) {
        const output = try generator.chunk();
        var rest = output;
        while (rest.len != 0 and batches < batch_budget) {
            const length = @min(rest.len, 1 + random.uintLessThan(usize, if (random.boolean()) 64 else read_batch_bytes));
            scrubber.scrub(rest[0..length]);
            const outcome = commitReaderBatch(terminal.runtime, rest[0..length]);
            rest = rest[length..];
            batches += 1;
            totals.batches += 1;
            totals.bytes += length;
            switch (outcome) {
                .committed => {},
                .runtime_failed => {
                    // Containment, never a Host panic: this terminal is fenced and classified.
                    try std.testing.expect(terminal.runtime.primaryFailure() != null);
                    totals.fenced_terminals += 1;
                    terminal.stop();
                    running = false;
                    terminal = .{};
                    try terminal.start();
                    running = true;
                    totals.terminals += 1;
                    scrubber.deinit();
                    scrubber = .init();
                    break;
                },
                .failure_fenced, .output_aborted => return error.UnexpectedReaderBatchOutcome,
            }
        }
        if (random.uintLessThan(u8, 5) == 0) {
            resize(random, terminal.runtime);
            totals.resizes += 1;
            // A viewer asks for a fresh frame after each resize.
            try terminal.project();
            totals.projections += 1;
        }
        if (random.uintLessThan(u8, 6) == 0) {
            try terminal.project();
            totals.projections += 1;
        }
    }
    try harness.subject.testing.checkInvariants(terminal.runtime);
}

test "program output fuzz commits or fences one terminal and never ends the Host" {
    const seeds: []const u64 = if (options.seed) |seed| &.{seed} else &.{ gate_seed, gate_seed +% 1, gate_seed +% 2 };
    // Ghostty warns on every malformed sequence; printing them would dominate the run time.
    const log_level = std.testing.log_level;
    std.testing.log_level = .err;
    defer std.testing.log_level = log_level;
    var totals: Totals = .{};
    var timer = try harness.platform_boundary.monotonic.Timer.start();
    for (seeds) |seed| try run(seed, options.batches orelse default_batches, &totals);
    std.debug.print(
        "program output fuzz: seeds {d} from {x}, {d} batches, {d} bytes, {d} resizes, {d} projections, {d} terminals, {d} fenced, {d} ms\n",
        .{ seeds.len, seeds[0], totals.batches, totals.bytes, totals.resizes, totals.projections, totals.terminals, totals.fenced_terminals, timer.read() / std.time.ns_per_ms },
    );
}

// Found by seed 1: Ghostty's reflow assumes one capacity increase always fits another hyperlinked
// cell, and a page's hyperlink capacity saturates near 26,000 cells. Reaching it links tens of
// thousands of cells, and Ghostty's Debug integrity check scans the page after each one, which
// outlasts the gate's per-test bound; the release-mode `test-output-fuzz` step runs it.
test "program output fuzz: a reflow after a long hyperlinked listing keeps the terminal and its newest links" {
    if (builtin.mode == .Debug) return error.SkipZigTest;
    var terminal: Terminal = .{};
    try terminal.start();
    defer terminal.stop();
    const lines = 1000;
    var line: [64]u8 = undefined;
    for (0..lines) |number| {
        var writer: std.Io.Writer = .fixed(&line);
        try writer.print("\x1b]8;;f:{d}\x1b\\", .{number});
        try writer.splatByteAll('f', 40);
        try writer.writeAll("\x1b]8;;\x1b\\\r\n");
        try std.testing.expectEqual(.committed, commitReaderBatch(terminal.runtime, writer.buffered()));
    }

    _ = try terminal.runtime.resizeGeometry(60, 24, 0, 0);
    try std.testing.expect(terminal.runtime.primaryFailure() == null);
    try terminal.project();
    {
        const state = harness.runtimeState(terminal.runtime);
        state.terminal_mutex.lock();
        defer state.terminal_mutex.unlock();
        const screen = state.terminal.screens.active;
        const newest = screen.pages.pin(.{ .active = .{ .x = 0, .y = screen.cursor.y - 1 } }).?;
        try std.testing.expect(newest.rowAndCell().cell.hyperlink);
    }
    try harness.subject.testing.checkInvariants(terminal.runtime);
}

// Found by seeds 1 and 2: a scroll that crosses a page boundary copies a hyperlinked row into the
// page above, and Ghostty panics when that page's link map is already saturated. Linked rows fill
// the first page, the active area straddles its end, and a scroll region pulls the next page's
// rows up into it. Like the reflow case, it links tens of thousands of cells.
test "program output fuzz: a region scroll across a page filled with links keeps the terminal and later links" {
    if (builtin.mode == .Debug) return error.SkipZigTest;
    var terminal: Terminal = .{};
    try terminal.start();
    defer terminal.stop();
    const page_rows = page_rows: {
        const state = harness.runtimeState(terminal.runtime);
        state.terminal_mutex.lock();
        defer state.terminal_mutex.unlock();
        break :page_rows state.terminal.screens.active.pages.pages.first.?.capacity().rows;
    };
    var line: [128]u8 = undefined;
    // Each line feeds one row; the active area's top 12 rows end the first page.
    for (0..page_rows + 11) |number| {
        var writer: std.Io.Writer = .fixed(&line);
        try writer.print("\x1b]8;;f:{d}\x1b\\", .{number});
        try writer.splatByteAll('f', 80);
        try writer.writeAll("\x1b]8;;\x1b\\\r\n");
        try std.testing.expectEqual(.committed, commitReaderBatch(terminal.runtime, writer.buffered()));
    }

    try std.testing.expectEqual(.committed, commitReaderBatch(terminal.runtime, "\x1b[2;24r\x1b[24;1H\n\n\n\n\x1b[r"));
    try std.testing.expect(terminal.runtime.primaryFailure() == null);
    // Once the full first page leaves the active area, new links fit again.
    for (0..60) |number| {
        var writer: std.Io.Writer = .fixed(&line);
        try writer.print("\r\n\x1b]8;;g:{d}\x1b\\g\x1b]8;;\x1b\\", .{number});
        try std.testing.expectEqual(.committed, commitReaderBatch(terminal.runtime, writer.buffered()));
    }
    try terminal.project();
    {
        const state = harness.runtimeState(terminal.runtime);
        state.terminal_mutex.lock();
        defer state.terminal_mutex.unlock();
        const screen = state.terminal.screens.active;
        const newest = screen.pages.pin(.{ .active = .{ .x = 0, .y = screen.cursor.y } }).?;
        try std.testing.expect(newest.rowAndCell().cell.hyperlink);
    }
    try harness.subject.testing.checkInvariants(terminal.runtime);
}
