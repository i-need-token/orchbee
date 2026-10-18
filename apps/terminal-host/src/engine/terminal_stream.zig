const std = @import("std");
const builtin = @import("builtin");

const ghostty_vt = @import("ghostty-vt");

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
/// The pinned `ghostty_vt.TerminalStream` owns DCS query semantics. This
/// adapter bounds its parser and restores recovery when a malformed producer
/// starts a new DCS hook before terminating the previous one.
pub const Handler = struct {
    terminal_handler: ghostty_vt.TerminalStream.Handler,
    column_mode: ?*const fn (*Handler, bool) bool = null,
    mode_changed: ?*const fn (*Handler, ghostty_vt.Mode, bool) bool = null,
    full_reset: ?*const fn (*Handler) void = null,
    clipboard_contents: ?*const fn (*Handler, ghostty_vt.StreamAction.ClipboardContents) void = null,
    progress_report: ?*const fn (*Handler, ghostty_vt.osc.Command.ProgressReport) void = null,
    semantic_prompt: ?*const fn (*Handler, ghostty_vt.StreamAction.SemanticPrompt) void = null,
    metadata_reset: ?*const fn (*Handler) void = null,
    discard_actions: bool = false,

    pub fn init(terminal: *ghostty_vt.Terminal) Handler {
        restorePromptRedrawDefault(terminal);
        var result: Handler = .{
            .terminal_handler = .init(terminal),
        };
        result.terminal_handler.dcs_handler.max_bytes = xtgettcap_request_limit_bytes;
        result.terminal_handler.terminfo_name = "xterm-256color";
        result.terminal_handler.apc_handler.enable(.glyph, false);
        return result;
    }

    pub fn deinit(self: *Handler) void {
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
            .set_mode => {
                if (self.handleColumnMode(value, true)) return;
                self.terminal_handler.vt(action, value);
                if (self.mode_changed) |apply| {
                    if (!apply(self, value.mode, true)) self.discard_actions = true;
                }
                return;
            },
            .reset_mode => {
                if (self.handleColumnMode(value, false)) return;
                self.terminal_handler.vt(action, value);
                if (self.mode_changed) |apply| {
                    if (!apply(self, value.mode, false)) self.discard_actions = true;
                }
                return;
            },
            .restore_mode => {
                if (self.restoreColumnMode(value)) return;
                self.terminal_handler.vt(action, value);
                if (self.mode_changed) |apply| {
                    if (!apply(
                        self,
                        value.mode,
                        self.terminal_handler.terminal.modes.get(value.mode),
                    )) self.discard_actions = true;
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
                restoreResetDefaults(self.terminal_handler.terminal);
                if (self.full_reset) |apply| apply(self);
                if (had_metadata) if (self.metadata_reset) |apply| apply(self);
                return;
            },
            .dcs_hook => {
                // Ghostty asserts that hooks never overlap. Real PTYs can
                // produce a malformed unterminated DCS before a fresh hook;
                // discard only that partial parser state and let Ghostty own
                // every command and reply after recovery.
                self.terminal_handler.dcs_handler.deinit();
                self.terminal_handler.vt(action, value);
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
};

pub const xtgettcap_request_limit_bytes = 4096;

/// RIS and Host-owned reset restore platform prompt handling and the latest runtime colors.
pub fn restoreResetDefaults(terminal: *ghostty_vt.Terminal) void {
    restorePromptRedrawDefault(terminal);
    terminal.colors.background.override = null;
    terminal.colors.foreground.override = null;
    terminal.colors.cursor.override = null;
    terminal.colors.palette.resetAll();
    terminal.flags.dirty.palette = true;
}

fn restorePromptRedrawDefault(terminal: *ghostty_vt.Terminal) void {
    // ConPTY retains the console contents; PSReadLine does not redraw the prompt on every resize.
    // Preserve it unless the child explicitly opts into OSC 133 prompt redrawing.
    if (builtin.os.tag == .windows) terminal.flags.shell_redraws_prompt = .false;
}

pub const Stream = ghostty_vt.Stream(Handler);
