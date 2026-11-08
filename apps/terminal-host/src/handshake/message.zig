//! The closed `OBHS` messages, their value grammars, and the growable enums, which map an unknown
//! name to `unknown`. Text values are borrowed and already satisfy their grammar.

const std = @import("std");

const json = @import("json.zig");

pub const release_environment = "orchbee";
pub const maximum_name_bytes = 64;
pub const maximum_product_version_bytes = 64;
pub const maximum_capabilities = 256;
pub const maximum_query_processes = 258;
pub const maximum_process_connections = 64;
pub const commit_hex_digits = 40;
pub const cohort_hex_digits = 64;
pub const fingerprint_hex_digits = 64;

/// `extensions` members in bytewise name order; their names satisfy the name grammar.
pub const Extensions = []const json.Member;

pub const GenerationRange = struct {
    lowest: u16,
    highest: u16,

    pub fn valid(self: GenerationRange) bool {
        return self.lowest >= 1 and self.lowest <= self.highest;
    }

    pub fn highestCommon(self: GenerationRange, other: GenerationRange) ?u16 {
        const highest = @min(self.highest, other.highest);
        return if (highest >= @max(self.lowest, other.lowest)) highest else null;
    }
};

pub const Build = struct {
    commit: ?[]const u8 = null,
    cohort: ?[]const u8 = null,
};

pub const RejectionReason = enum {
    malformed,
    connection_unsupported,
    environment_mismatch,
    initiator_too_old,
    initiator_too_new,
    fingerprint_mismatch,
    capacity,
    stopping,
    timeout,
    unknown,

    pub fn name(self: RejectionReason) []const u8 {
        return wireName(self);
    }

    pub fn fromName(text: []const u8) RejectionReason {
        return fromWireName(RejectionReason, text);
    }
};

pub const ProcessRole = enum {
    server,
    worker,
    terminal_host,
    unknown,

    pub fn name(self: ProcessRole) []const u8 {
        return wireName(self);
    }

    pub fn fromName(text: []const u8) ProcessRole {
        return fromWireName(ProcessRole, text);
    }
};

pub const ProcessState = enum {
    ready,
    draining,
    stopping,
    unknown,

    pub fn name(self: ProcessState) []const u8 {
        return wireName(self);
    }

    pub fn fromName(text: []const u8) ProcessState {
        return fromWireName(ProcessState, text);
    }
};

pub const RequestKind = enum {
    query,
    drain,
    exit,

    pub fn name(self: RequestKind) []const u8 {
        return @tagName(self);
    }
};

pub const Offer = struct {
    connection: []const u8,
    generations: GenerationRange,
    capabilities: []const []const u8,
    product_version: []const u8,
    build: Build,
    environment: []const u8,
    /// Present exactly when `environment` is not `orchbee`.
    fingerprint: ?[]const u8,
    extensions: Extensions = &.{},
};

pub const ControlRequest = struct {
    kind: RequestKind,
    environment: []const u8,
    extensions: Extensions = &.{},
};

/// A request whose type is a name but no known message type; its other members are unread.
pub const UnknownRequest = struct {
    request: []const u8,
    environment: []const u8,
};

pub const InitiatorMessage = union(enum) {
    offer: Offer,
    request: ControlRequest,
    unknown_request: UnknownRequest,
};

pub const Answer = struct {
    generation: u16,
    capabilities: []const []const u8,
    product_version: []const u8,
    build: Build,
    extensions: Extensions = &.{},
};

pub const Rejection = struct {
    reason: RejectionReason,
    generations: GenerationRange,
    product_version: []const u8,
    environment: []const u8,
    extensions: Extensions = &.{},
};

pub const ConnectionEntry = struct {
    kind: []const u8,
    generations: GenerationRange,
    /// Present exactly in the answering process's entries.
    capabilities: ?[]const []const u8,
};

pub const ProcessEntry = struct {
    role: ProcessRole,
    product_version: []const u8,
    build: Build,
    environment: []const u8,
    pid: u32,
    state: ProcessState,
    terminals: u32,
    connections: []const ConnectionEntry,
};

pub const QueryResult = struct {
    processes: []const ProcessEntry,
    /// Present exactly when the first process is a Server.
    restart_keeps_terminals: ?bool,
    extensions: Extensions = &.{},

    /// The cross-member rules the decoder and the encoder share.
    pub fn valid(self: QueryResult) bool {
        if (self.processes.len < 1 or self.processes.len > maximum_query_processes) return false;
        if ((self.restart_keeps_terminals != null) != (self.processes[0].role == .server)) return false;
        for (self.processes, 0..) |process, index| {
            if (process.pid == 0) return false;
            if (process.connections.len > maximum_process_connections) return false;
            for (process.connections, 0..) |connection, position| {
                if ((connection.capabilities != null) != (index == 0)) return false;
                if (position > 0 and
                    !std.mem.lessThan(u8, process.connections[position - 1].kind, connection.kind))
                {
                    return false;
                }
            }
        }
        return true;
    }
};

pub const DrainResult = struct {
    state: ProcessState,
    extensions: Extensions = &.{},
};

pub const ExitResult = struct {
    extensions: Extensions = &.{},
};

pub const Unsupported = struct {
    request: []const u8,
    extensions: Extensions = &.{},
};

pub const AcceptorMessage = union(enum) {
    answer: Answer,
    rejection: Rejection,
    query_result: QueryResult,
    drain_result: DrainResult,
    exit_result: ExitResult,
    unsupported: Unsupported,
};

/// Every `type` either direction defines.
pub const MessageType = enum {
    offer,
    query,
    drain,
    exit,
    answer,
    rejection,
    query_result,
    drain_result,
    exit_result,
    unsupported,

    pub fn fromName(text: []const u8) ?MessageType {
        inline for (@typeInfo(MessageType).@"enum".fields) |field| {
            if (std.mem.eql(u8, text, comptime dashed(field.name))) return @enumFromInt(field.value);
        }
        return null;
    }

    pub fn name(self: MessageType) []const u8 {
        return wireName(self);
    }
};

pub fn isName(text: []const u8) bool {
    if (text.len < 1 or text.len > maximum_name_bytes) return false;
    for (text) |byte| switch (byte) {
        'a'...'z', '0'...'9', '-' => {},
        else => return false,
    };
    return true;
}

pub fn isProductVersion(text: []const u8) bool {
    if (text.len < 1 or text.len > maximum_product_version_bytes) return false;
    for (text) |byte| if (byte < 0x21 or byte > 0x7e) return false;
    return true;
}

pub fn isLowercaseHex(text: []const u8, digits: usize) bool {
    if (text.len != digits) return false;
    for (text) |byte| switch (byte) {
        '0'...'9', 'a'...'f' => {},
        else => return false,
    };
    return true;
}

/// At most 256 names in strictly ascending bytewise order.
pub fn isCapabilityList(names: []const []const u8) bool {
    if (names.len > maximum_capabilities) return false;
    for (names, 0..) |candidate, index| {
        if (!isName(candidate)) return false;
        if (index > 0 and !std.mem.lessThan(u8, names[index - 1], candidate)) return false;
    }
    return true;
}

pub fn isBuild(build: Build) bool {
    if (build.commit) |commit| if (!isLowercaseHex(commit, commit_hex_digits)) return false;
    if (build.cohort) |cohort| if (!isLowercaseHex(cohort, cohort_hex_digits)) return false;
    return true;
}

/// The environment and fingerprint pairing: a fingerprint exactly outside `orchbee`.
pub fn isEnvironment(environment: []const u8, fingerprint: ?[]const u8) bool {
    if (!isName(environment)) return false;
    const release = std.mem.eql(u8, environment, release_environment);
    if (fingerprint) |digest| return !release and isLowercaseHex(digest, fingerprint_hex_digits);
    return release;
}

fn wireName(value: anytype) []const u8 {
    return switch (value) {
        inline else => |tag| comptime dashed(@tagName(tag)),
    };
}

fn fromWireName(comptime Enum: type, text: []const u8) Enum {
    inline for (@typeInfo(Enum).@"enum".fields) |field| {
        if (std.mem.eql(u8, text, comptime dashed(field.name))) return @enumFromInt(field.value);
    }
    return .unknown;
}

fn dashed(comptime identifier: []const u8) []const u8 {
    comptime {
        var text: [identifier.len]u8 = undefined;
        for (identifier, 0..) |byte, index| text[index] = if (byte == '_') '-' else byte;
        const final = text;
        return &final;
    }
}
