//! Geometry changes that program output asks for during a reader batch: DECCOLM's column mode and
//! the viewport geometry restored when mode 3 is enabled. Each resizes the PTY without the
//! terminal lock, then the terminal under it, and fences the terminal when either step fails.
const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const terminal_geometry = @import("terminal-host-model").geometry;
const terminal_stream = @import("../terminal_stream.zig");
const state = @import("state.zig");
const mode_effects = @import("mode_effects.zig");
const output_ops = @import("output.zig");
const reflow_hyperlinks = @import("reflow_hyperlinks.zig");
const resize_ops = @import("resize.zig");

pub fn columnModeEffect(handler: *terminal_stream.Handler, enabled: bool) bool {
    const self = output_ops.runtimeFromHostHandler(handler);
    std.debug.assert(self.reader_commit_in_progress);
    const terminal = handler.terminal_handler.terminal;
    const target_columns: u16 = if (enabled) 132 else 80;
    const next_geometry: terminal_geometry.Geometry = .{
        .cell_pixel_height = self.cell_pixel_height,
        .cell_pixel_width = self.cell_pixel_width,
        .columns = target_columns,
        .rows = terminal.rows,
    };
    return terminalDrivenGeometryEffect(
        self,
        terminal,
        next_geometry,
        .{ .column_mode = enabled },
    );
}

pub fn restoreViewportGeometryEffect(handler: *terminal_stream.Handler) bool {
    const self = output_ops.runtimeFromHostHandler(handler);
    std.debug.assert(self.reader_commit_in_progress);
    return terminalDrivenGeometryEffect(
        self,
        handler.terminal_handler.terminal,
        self.viewport_geometry,
        .viewport,
    );
}

const TerminalGeometryMutation = union(enum) {
    column_mode: bool,
    viewport,
};

fn terminalDrivenGeometryEffect(
    self: *state.State,
    terminal: *ghostty_vt.Terminal,
    next_geometry: terminal_geometry.Geometry,
    mutation: TerminalGeometryMutation,
) bool {
    const previous_geometry: terminal_geometry.Geometry = .{
        .cell_pixel_height = self.cell_pixel_height,
        .cell_pixel_width = self.cell_pixel_width,
        .columns = terminal.cols,
        .rows = terminal.rows,
    };
    const geometry_changes = !terminal_geometry.eql(previous_geometry, next_geometry);

    if (geometry_changes) {
        std.debug.assert(!self.resize_effect_in_flight);
        self.resize_effect_in_flight = true;
        self.terminal_mutex.unlock();
        const resize_result = resize_ops.resizePtyEffect(self, previous_geometry, next_geometry);
        resize_ops.notifyResizeEffectReturned(self);
        self.terminal_mutex.lock();
        resize_result catch {
            self.resize_fenced.store(true, .release);
            self.terminal_resize_failure_pending = true;
            resize_ops.finishResizeEffectLocked(self);
            return false;
        };
    }

    applyTerminalGeometryMutation(self, terminal, next_geometry, mutation) catch {
        self.resize_fenced.store(true, .release);
        if (geometry_changes) {
            self.terminal_mutex.unlock();
            resize_ops.restorePtyGeometry(self, next_geometry, previous_geometry) catch {};
            self.terminal_mutex.lock();
            resize_ops.finishResizeEffectLocked(self);
        }
        self.terminal_resize_failure_pending = true;
        return false;
    };

    self.terminal_driven_resize_target = next_geometry;
    if (geometry_changes) {
        mode_effects.geometryCommittedLocked(self);
        resize_ops.finishResizeEffectLocked(self);
    }
    return true;
}

fn applyTerminalGeometryMutation(
    self: *state.State,
    terminal: *ghostty_vt.Terminal,
    next_geometry: terminal_geometry.Geometry,
    mutation: TerminalGeometryMutation,
) !void {
    switch (mutation) {
        .column_mode => |enabled| {
            if (terminal.modes.get(.enable_mode_3)) reflow_hyperlinks.fitReflow(terminal, if (enabled) 132 else 80);
            try terminal.deccolm(
                terminal.screens.active.alloc,
                if (enabled) .@"132_cols" else .@"80_cols",
            );
        },
        .viewport => {
            reflow_hyperlinks.fitReflow(terminal, next_geometry.columns);
            try terminal.resize(
                self.allocator,
                .{
                    .cell_size_px = .{
                        .height = next_geometry.cell_pixel_height,
                        .width = next_geometry.cell_pixel_width,
                    },
                    .cols = next_geometry.columns,
                    .rows = next_geometry.rows,
                },
            );
            self.cell_pixel_width = next_geometry.cell_pixel_width;
            self.cell_pixel_height = next_geometry.cell_pixel_height;
        },
    }
    self.stream.handler.resized();
}
