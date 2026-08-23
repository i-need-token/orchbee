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
/// Orchbee's pre-Host terminal supported DECRQSS, so this adapter restores only
/// that reply-producing command while delegating every terminal mutation and
/// existing effect to Ghostty's canonical terminal handler.
pub const Handler = struct {
    terminal_handler: ghostty_vt.TerminalStream.Handler,
    dcs_handler: ghostty_vt.dcs.Handler = .{},
    decrqss_active: bool = false,
    column_mode: ?*const fn (*Handler, bool) bool = null,
    clipboard_contents: ?*const fn (*Handler, ghostty_vt.StreamAction.ClipboardContents) void = null,
    progress_report: ?*const fn (*Handler, ghostty_vt.osc.Command.ProgressReport) void = null,
    semantic_prompt: ?*const fn (*Handler, ghostty_vt.StreamAction.SemanticPrompt) void = null,
    metadata_reset: ?*const fn (*Handler) void = null,
    discard_actions: bool = false,

    pub fn init(terminal: *ghostty_vt.Terminal) Handler {
        return .{
            .terminal_handler = .init(terminal),
        };
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
            .set_mode => if (self.handleColumnMode(value, true)) return,
            .reset_mode => if (self.handleColumnMode(value, false)) return,
            .restore_mode => if (self.restoreColumnMode(value)) return,
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
        if (self.decrqss_active) {
            self.dcs_handler.deinit();
            self.decrqss_active = false;
        }
        if (dcs.intermediates.len != 1 or
            dcs.intermediates[0] != '$' or
            dcs.final != 'q') return;

        const command_optional = self.dcs_handler.hook(self.terminal_handler.terminal.gpa(), dcs);
        self.decrqss_active = true;
        var command = command_optional orelse return;
        defer command.deinit();
        self.handleDcsCommand(&command);
    }

    fn dcsPut(self: *Handler, byte: u8) void {
        if (!self.decrqss_active) return;
        var command = self.dcs_handler.put(byte) orelse return;
        defer command.deinit();
        self.handleDcsCommand(&command);
    }

    fn dcsUnhook(self: *Handler) void {
        if (!self.decrqss_active) return;
        self.decrqss_active = false;
        var command = self.dcs_handler.unhook() orelse return;
        defer command.deinit();
        self.handleDcsCommand(&command);
    }

    fn handleDcsCommand(self: *Handler, command: *ghostty_vt.dcs.Command) void {
        switch (command.*) {
            .decrqss => |request| self.reportStatusString(request) catch |err| {
                log.warn("could not encode DECRQSS response: {}", .{err});
            },
            .tmux, .xtgettcap => unreachable,
        }
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
