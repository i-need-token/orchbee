const std = @import("std");

const ghostty_vt = @import("ghostty-vt");

const log = std.log.scoped(.terminal_host_stream);

/// Ghostty requires an allocator for OSC color operations and for a few OSC
/// payloads that may exceed its fixed parser buffer. Keep those child-driven
/// allocations bounded per terminal while admitting the allocation-backed
/// color-command parsing that Ghostty requires.
pub const osc_allocation_limit_bytes = ghostty_vt.osc.Parser.MAX_BUF * 32;

pub const ParserAllocator = struct {
    parent: std.mem.Allocator,
    active_bytes: usize = 0,

    pub fn init(parent: std.mem.Allocator) ParserAllocator {
        return .{ .parent = parent };
    }

    pub fn deinit(self: *ParserAllocator) void {
        std.debug.assert(self.active_bytes == 0);
        self.* = undefined;
    }

    pub fn allocator(self: *ParserAllocator) std.mem.Allocator {
        return .{
            .ptr = self,
            .vtable = &.{
                .alloc = alloc,
                .resize = resize,
                .remap = remap,
                .free = free,
            },
        };
    }

    fn alloc(
        context: *anyopaque,
        length: usize,
        alignment: std.mem.Alignment,
        return_address: usize,
    ) ?[*]u8 {
        const self: *ParserAllocator = @ptrCast(@alignCast(context));
        if (length > osc_allocation_limit_bytes -| self.active_bytes) return null;
        const result = self.parent.rawAlloc(length, alignment, return_address) orelse return null;
        self.active_bytes += length;
        return result;
    }

    fn resize(
        context: *anyopaque,
        memory: []u8,
        alignment: std.mem.Alignment,
        new_length: usize,
        return_address: usize,
    ) bool {
        const self: *ParserAllocator = @ptrCast(@alignCast(context));
        std.debug.assert(self.active_bytes >= memory.len);
        if (new_length > memory.len and
            new_length - memory.len > osc_allocation_limit_bytes -| self.active_bytes) return false;
        if (!self.parent.rawResize(memory, alignment, new_length, return_address)) return false;
        self.active_bytes = self.active_bytes - memory.len + new_length;
        return true;
    }

    fn remap(
        context: *anyopaque,
        memory: []u8,
        alignment: std.mem.Alignment,
        new_length: usize,
        return_address: usize,
    ) ?[*]u8 {
        const self: *ParserAllocator = @ptrCast(@alignCast(context));
        std.debug.assert(self.active_bytes >= memory.len);
        if (new_length > memory.len and
            new_length - memory.len > osc_allocation_limit_bytes -| self.active_bytes) return null;
        const result = self.parent.rawRemap(
            memory,
            alignment,
            new_length,
            return_address,
        ) orelse return null;
        self.active_bytes = self.active_bytes - memory.len + new_length;
        return result;
    }

    fn free(
        context: *anyopaque,
        memory: []u8,
        alignment: std.mem.Alignment,
        return_address: usize,
    ) void {
        const self: *ParserAllocator = @ptrCast(@alignCast(context));
        std.debug.assert(self.active_bytes >= memory.len);
        self.parent.rawFree(memory, alignment, return_address);
        self.active_bytes -= memory.len;
    }
};

/// Host-owned adapter around Ghostty's terminal-state stream.
///
/// The pinned `ghostty_vt.TerminalStream` deliberately ignores DCS actions.
/// This adapter restores the bounded query families that the Host owns while
/// delegating every terminal mutation and existing effect to Ghostty's
/// canonical terminal handler.
pub const Handler = struct {
    terminal_handler: ghostty_vt.TerminalStream.Handler,
    dcs_handler: ghostty_vt.dcs.Handler = .{},
    dcs_active: DcsKind = .none,
    column_mode: ?*const fn (*Handler, bool) bool = null,
    mode_changed: ?*const fn (*Handler, ghostty_vt.Mode, bool) void = null,
    full_reset: ?*const fn (*Handler) void = null,
    clipboard_contents: ?*const fn (*Handler, ghostty_vt.StreamAction.ClipboardContents) void = null,
    progress_report: ?*const fn (*Handler, ghostty_vt.osc.Command.ProgressReport) void = null,
    semantic_prompt: ?*const fn (*Handler, ghostty_vt.StreamAction.SemanticPrompt) void = null,
    metadata_reset: ?*const fn (*Handler) void = null,
    discard_actions: bool = false,

    pub fn init(terminal: *ghostty_vt.Terminal) Handler {
        var result: Handler = .{
            .terminal_handler = .init(terminal),
        };
        result.dcs_handler.max_bytes = xtgettcap_request_limit_bytes;
        result.terminal_handler.apc_handler.enable(.glyph, false);
        return result;
    }

    pub fn deinit(self: *Handler) void {
        self.dcs_handler.deinit();
        self.terminal_handler.deinit();
    }

    pub fn beginBatch(self: *Handler) void {
        self.discard_actions = false;
    }

    pub fn vt(
        self: *Handler,
        comptime action: ghostty_vt.StreamAction.Tag,
        value: ghostty_vt.StreamAction.Value(action),
    ) void {
        if (self.discard_actions) return;
        switch (action) {
            .dcs_hook => self.dcsHook(value),
            .dcs_put => self.dcsPut(value),
            .dcs_unhook => self.dcsUnhook(),
            .set_mode => {
                if (self.handleColumnMode(value, true)) return;
                self.terminal_handler.vt(action, value);
                if (self.mode_changed) |apply| apply(self, value.mode, true);
                return;
            },
            .reset_mode => {
                if (self.handleColumnMode(value, false)) return;
                self.terminal_handler.vt(action, value);
                if (self.mode_changed) |apply| apply(self, value.mode, false);
                return;
            },
            .restore_mode => {
                if (self.restoreColumnMode(value)) return;
                self.terminal_handler.vt(action, value);
                if (self.mode_changed) |apply| {
                    apply(
                        self,
                        value.mode,
                        self.terminal_handler.terminal.modes.get(value.mode),
                    );
                }
                return;
            },
            .clipboard_contents => {
                if (self.clipboard_contents) |apply| apply(self, value);
                return;
            },
            .progress_report => {
                if (self.progress_report) |apply| apply(self, value);
                return;
            },
            .semantic_prompt => {
                if (self.semantic_prompt) |apply| apply(self, value);
            },
            .full_reset => {
                const had_metadata = self.terminal_handler.terminal.getTitle() != null or
                    self.terminal_handler.terminal.getPwd() != null;
                self.terminal_handler.vt(action, value);
                restoreColorDefaults(self.terminal_handler.terminal);
                if (self.full_reset) |apply| apply(self);
                if (had_metadata) if (self.metadata_reset) |apply| apply(self);
                return;
            },
            else => {},
        }
        self.terminal_handler.vt(action, value);
    }

    fn handleColumnMode(
        self: *Handler,
        mode: ghostty_vt.StreamAction.Mode,
        enabled: bool,
    ) bool {
        if (mode.mode != .@"132_column" or
            !self.terminal_handler.terminal.modes.get(.enable_mode_3)) return false;
        const apply = self.column_mode orelse return false;
        if (!apply(self, enabled)) self.discard_actions = true;
        return true;
    }

    fn restoreColumnMode(
        self: *Handler,
        mode: ghostty_vt.StreamAction.Mode,
    ) bool {
        if (mode.mode != .@"132_column" or
            !self.terminal_handler.terminal.modes.get(.enable_mode_3)) return false;
        const terminal = self.terminal_handler.terminal;
        const previous = terminal.modes.get(mode.mode);
        const restored = terminal.modes.restore(mode.mode);
        terminal.modes.set(mode.mode, previous);
        return self.handleColumnMode(mode, restored);
    }

    fn dcsHook(self: *Handler, dcs: ghostty_vt.DCS) void {
        // A new hook supersedes any unterminated external sequence. Keep the
        // adapter recoverable even if malformed PTY output bypasses a normal
        // unhook transition.
        if (self.dcs_active != .none) {
            self.dcs_handler.deinit();
            self.dcs_active = .none;
        }
        if (dcs.intermediates.len != 1 or dcs.final != 'q') return;
        self.dcs_active = switch (dcs.intermediates[0]) {
            '$' => .decrqss,
            '+' => .xtgettcap,
            else => return,
        };

        const command_optional = self.dcs_handler.hook(self.terminal_handler.terminal.gpa(), dcs);
        var command = command_optional orelse return;
        defer command.deinit();
        self.handleDcsCommand(&command);
    }

    fn dcsPut(self: *Handler, byte: u8) void {
        if (self.dcs_active == .none) return;
        var command = self.dcs_handler.put(byte) orelse return;
        defer command.deinit();
        self.handleDcsCommand(&command);
    }

    fn dcsUnhook(self: *Handler) void {
        const active = self.dcs_active;
        if (active == .none) return;
        self.dcs_active = .none;
        var command = self.dcs_handler.unhook() orelse {
            if (active == .xtgettcap) self.reportInvalidTermcap();
            return;
        };
        defer command.deinit();
        self.handleDcsCommand(&command);
    }

    fn handleDcsCommand(self: *Handler, command: *ghostty_vt.dcs.Command) void {
        switch (command.*) {
            .decrqss => |request| self.reportStatusString(request) catch |err| {
                log.warn("could not encode DECRQSS response: {}", .{err});
            },
            .xtgettcap => |*request| self.reportTermcap(request),
            .tmux => unreachable,
        }
    }

    fn reportTermcap(
        self: *Handler,
        request: *ghostty_vt.dcs.Command.XTGETTCAP,
    ) void {
        var response: [xtgettcap_response_limit_bytes + 1]u8 = undefined;
        var stream = std.io.fixedBufferStream(response[0..xtgettcap_response_limit_bytes]);
        const writer = stream.writer();
        writer.writeAll("\x1bP1+r") catch {
            self.reportInvalidTermcap();
            return;
        };

        var count: usize = 0;
        while (request.next()) |name| {
            const value = termcapValue(name) orelse {
                if (count == 0) {
                    self.reportInvalidTermcap();
                    return;
                }
                break;
            };
            if (count > 0) writer.writeByte(';') catch {
                self.reportInvalidTermcap();
                return;
            };
            writer.writeAll(name) catch {
                self.reportInvalidTermcap();
                return;
            };
            writer.writeByte('=') catch {
                self.reportInvalidTermcap();
                return;
            };
            writer.writeAll(value) catch {
                self.reportInvalidTermcap();
                return;
            };
            count += 1;
        }
        if (count == 0) {
            self.reportInvalidTermcap();
            return;
        }
        writer.writeAll("\x1b\\") catch {
            self.reportInvalidTermcap();
            return;
        };
        self.writeReply(response[0..stream.pos]);
    }

    fn reportInvalidTermcap(self: *Handler) void {
        var response = "\x1bP0+r\x1b\\".*;
        self.writeReply(response[0..]);
    }

    fn writeReply(self: *Handler, bytes: []u8) void {
        const write_pty = self.terminal_handler.effects.write_pty orelse return;
        std.debug.assert(bytes.len <= xtgettcap_response_limit_bytes);
        var terminated: [xtgettcap_response_limit_bytes + 1]u8 = undefined;
        @memcpy(terminated[0..bytes.len], bytes);
        terminated[bytes.len] = 0;
        write_pty(&self.terminal_handler, terminated[0..bytes.len :0]);
    }

    fn reportStatusString(
        self: *Handler,
        request: ghostty_vt.dcs.Command.DECRQSS,
    ) !void {
        var response: [129]u8 = undefined;
        var stream = std.io.fixedBufferStream(response[0 .. response.len - 1]);
        const writer = stream.writer();

        const prefix_format = "\x1bP{d}$r";
        const prefix_length = std.fmt.comptimePrint(prefix_format, .{0}).len;
        stream.pos = prefix_length;

        const terminal = self.terminal_handler.terminal;
        switch (request) {
            .none => {},
            .sgr => {
                const attributes = try terminal.printAttributes(stream.buffer[stream.pos..]);
                stream.pos += attributes.len;
                try writer.writeByte('m');
            },
            .decscusr => {
                const blink = terminal.modes.get(.cursor_blinking);
                const style: u8 = switch (terminal.screens.active.cursor.cursor_style) {
                    .block, .block_hollow => if (blink) 1 else 2,
                    .underline => if (blink) 3 else 4,
                    .bar => if (blink) 5 else 6,
                };
                try writer.print("{d} q", .{style});
            },
            .decstbm => try writer.print("{d};{d}r", .{
                terminal.scrolling_region.top + 1,
                terminal.scrolling_region.bottom + 1,
            }),
            .decslrm => if (terminal.modes.get(.enable_left_and_right_margin)) {
                try writer.print("{d};{d}s", .{
                    terminal.scrolling_region.left + 1,
                    terminal.scrolling_region.right + 1,
                });
            },
        }

        const valid = stream.pos > prefix_length;
        try writer.writeAll("\x1b\\");
        _ = try std.fmt.bufPrint(
            response[0..prefix_length],
            prefix_format,
            .{@intFromBool(valid)},
        );
        response[stream.pos] = 0;

        const write_pty = self.terminal_handler.effects.write_pty orelse return;
        write_pty(&self.terminal_handler, response[0..stream.pos :0]);
    }
};

const DcsKind = enum {
    none,
    decrqss,
    xtgettcap,
};

pub const xtgettcap_request_limit_bytes = 4096;
const xtgettcap_response_limit_bytes = 4096;

const Termcap = struct {
    name: []const u8,
    value: []const u8,
};

// Names and values are hexadecimal so the reply can remain allocation-free
// and preserve the query spelling after Ghostty normalizes it to uppercase.
const termcaps = [_]Termcap{
    .{ .name = "544E", .value = "787465726D2D323536636F6C6F72" }, // TN=xterm-256color
    .{ .name = "436F", .value = "323536" }, // Co=256
    .{ .name = "636F6C6F7273", .value = "323536" }, // colors=256
    .{ .name = "524742", .value = "38" }, // RGB=8
    .{ .name = "6B6273", .value = "08" }, // kbs
    .{ .name = "6B64636831", .value = "1B5B337E" }, // kdch1
    .{ .name = "6B63756431", .value = "1B4F42" }, // kcud1
    .{ .name = "6B656E64", .value = "1B4F46" }, // kend
    .{ .name = "6B656E74", .value = "1B4F4D" }, // kent
    .{ .name = "6B6631", .value = "1B4F50" }, // kf1
    .{ .name = "6B6632", .value = "1B4F51" }, // kf2
    .{ .name = "6B6633", .value = "1B4F52" }, // kf3
    .{ .name = "6B6634", .value = "1B4F53" }, // kf4
    .{ .name = "6B6635", .value = "1B5B31357E" }, // kf5
    .{ .name = "6B6636", .value = "1B5B31377E" }, // kf6
    .{ .name = "6B6637", .value = "1B5B31387E" }, // kf7
    .{ .name = "6B6638", .value = "1B5B31397E" }, // kf8
    .{ .name = "6B6639", .value = "1B5B32307E" }, // kf9
    .{ .name = "6B663130", .value = "1B5B32317E" }, // kf10
    .{ .name = "6B663131", .value = "1B5B32337E" }, // kf11
    .{ .name = "6B663132", .value = "1B5B32347E" }, // kf12
    .{ .name = "6B686F6D65", .value = "1B4F48" }, // khome
    .{ .name = "6B63756231", .value = "1B4F44" }, // kcub1
    .{ .name = "6B6E70", .value = "1B5B367E" }, // knp
    .{ .name = "6B7070", .value = "1B5B357E" }, // kpp
    .{ .name = "6B63756631", .value = "1B4F43" }, // kcuf1
    .{ .name = "6B63757531", .value = "1B4F41" }, // kcuu1
    .{ .name = "6B69636831", .value = "1B5B327E" }, // kich1
};

fn termcapValue(name: []const u8) ?[]const u8 {
    for (termcaps) |entry| {
        if (std.mem.eql(u8, name, entry.name)) return entry.value;
    }
    return null;
}

/// RIS and Host-owned application reset both return dynamic colors to the
/// latest runtime theme, not to the launch-time profile or prior OSC values.
pub fn restoreColorDefaults(terminal: *ghostty_vt.Terminal) void {
    terminal.colors.background.override = null;
    terminal.colors.foreground.override = null;
    terminal.colors.cursor.override = null;
    terminal.colors.palette.resetAll();
    terminal.flags.dirty.palette = true;
}

pub const Stream = ghostty_vt.Stream(Handler);
