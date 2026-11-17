//! The handover record, generation 1: one terminal's complete state as one Host exports it and
//! another imports it. docs/handover-record.md is the format's reference.
const std = @import("std");
pub const envelope = @import("envelope.zig");
pub const host_sections = @import("host_sections.zig");
pub const engine_section = @import("engine_section.zig");
pub const screen_pages = @import("screen_pages.zig");

pub const generation = envelope.generation;
pub const max_record_bytes = envelope.max_record_bytes;
pub const max_page_memory_bytes = screen_pages.max_page_memory_bytes;
pub const TerminalInputs = host_sections.TerminalInputs;
pub const TerminalFacts = host_sections.TerminalFacts;
pub const ConfigurationFacts = host_sections.ConfigurationFacts;
pub const ConsoleFacts = host_sections.ConsoleFacts;
pub const EngineState = engine_section.EngineState;

pub const Content = struct {
    product_version: []const u8,
    pin: []const u8,
    terminal: TerminalFacts,
    configuration: ConfigurationFacts,
    /// Ghostty's binary snapshot, version 1, with its continuation record.
    screen: []const u8,
    engine: EngineState,
    /// Present on Windows.
    console: ?ConsoleFacts,
};

pub const EncodeError = error{ InvalidRecord, RecordTooLarge } || std.mem.Allocator.Error;

pub fn encode(allocator: std.mem.Allocator, content: Content) EncodeError![]u8 {
    try checkPages(content.screen);
    var terminal: std.ArrayList(u8) = .empty;
    defer terminal.deinit(allocator);
    try host_sections.encodeTerminal(.{ .allocator = allocator, .list = &terminal }, content.terminal);
    var configuration: std.ArrayList(u8) = .empty;
    defer configuration.deinit(allocator);
    try host_sections.encodeConfiguration(.{ .allocator = allocator, .list = &configuration }, content.configuration);
    var engine: std.ArrayList(u8) = .empty;
    defer engine.deinit(allocator);
    try engine_section.encode(.{ .allocator = allocator, .list = &engine }, content.engine);
    var console: std.ArrayList(u8) = .empty;
    defer console.deinit(allocator);
    if (content.console) |facts| {
        try host_sections.encodeConsole(.{ .allocator = allocator, .list = &console }, facts);
    }

    var sections: [5]envelope.Section = undefined;
    var count: usize = 0;
    for ([_]struct { envelope.Tag, []const u8 }{
        .{ .terminal, terminal.items },
        .{ .configuration, configuration.items },
        .{ .screen, content.screen },
        .{ .engine, engine.items },
    }) |entry| {
        sections[count] = .{ .tag = entry[0], .required = true, .payload = entry[1] };
        count += 1;
    }
    if (content.console != null) {
        sections[count] = .{ .tag = .console, .required = true, .payload = console.items };
        count += 1;
    }
    return envelope.encode(allocator, .{
        .product_version = content.product_version,
        .pin = content.pin,
    }, sections[0..count]);
}

pub const DecodeError = envelope.DecodeError || std.mem.Allocator.Error || error{
    /// The Host's scanners were inside a sequence; H1 tracks no continuation.
    ContinuationPending,
    /// ConPTY signals were queued; H1 moves a terminal only once its signal sender is idle.
    SignalPending,
};

/// A validated record. Its screen and console borrow the record's bytes; the engine state lives
/// in its own arena.
pub const Decoded = struct {
    arena: std.heap.ArenaAllocator,
    product_version: []const u8,
    terminal: TerminalFacts,
    configuration: ConfigurationFacts,
    screen: []const u8,
    engine: EngineState,
    console: ?ConsoleFacts,

    pub fn deinit(self: *Decoded) void {
        self.arena.deinit();
        self.* = undefined;
    }
};

/// Decodes untrusted bytes. Every refusal leaves nothing allocated.
pub fn decode(allocator: std.mem.Allocator, bytes: []const u8, expected_pin: []const u8) DecodeError!Decoded {
    const framed = try envelope.decode(bytes, expected_pin);
    const terminal = framed.find(.terminal) orelse return error.InvalidRecord;
    const configuration = framed.find(.configuration) orelse return error.InvalidRecord;
    const screen = framed.find(.screen) orelse return error.InvalidRecord;
    const engine = framed.find(.engine) orelse return error.InvalidRecord;
    try checkPages(screen);

    var arena: std.heap.ArenaAllocator = .init(allocator);
    errdefer arena.deinit();
    const engine_state = engine_section.decode(arena.allocator(), engine) catch |err| return switch (err) {
        error.InvalidRecord => error.InvalidRecord,
        error.OutOfMemory => error.OutOfMemory,
    };
    if (engine_state.scanner_partial_state.len != 0) return error.ContinuationPending;
    if (!engine_state.conpty.empty()) return error.SignalPending;
    return .{
        .arena = arena,
        .product_version = framed.header.product_version,
        .terminal = try host_sections.decodeTerminal(terminal),
        .configuration = try host_sections.decodeConfiguration(configuration),
        .screen = screen,
        .engine = engine_state,
        .console = if (framed.find(.console)) |console| try host_sections.decodeConsole(console) else null,
    };
}

/// Refuses a screen section whose pages would take more memory than a quiesce admits, before
/// Ghostty allocates any of it.
fn checkPages(screen: []const u8) error{ InvalidRecord, RecordTooLarge }!void {
    _ = screen_pages.pageMemory(screen) catch |err| return switch (err) {
        error.InvalidRecord => error.InvalidRecord,
        error.TooLarge => error.RecordTooLarge,
    };
}
