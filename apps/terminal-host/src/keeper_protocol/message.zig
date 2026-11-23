//! The `OBKP` messages of generation 1 of `keeper`, as `schemas/keeper/README.md` defines them.
//! Ids, times, and handles are numbers here and hexadecimal text on the wire.

const std = @import("std");
pub const pty = @import("message_pty.zig");
const programs = @import("message_program.zig");

const handshake = @import("terminal-host-handshake");

pub const Extensions = handshake.message.Extensions;

/// At most this many holdings in one Keeper, and Hosts registered with it.
pub const maximum_holdings = 1024;
pub const maximum_hosts = 256;
/// At most this many bytes in the program a `reexec` names.
pub const maximum_program_bytes = 1024;
/// At most this many holdings, and Hosts, in one `status-result`.
pub const maximum_status_entries = 256;
/// A reply is due this long after its request.
pub const reply_deadline_ns: u64 = 2 * std.time.ns_per_s;

/// A holding id or a mark id: 128 random bits, never zero.
pub const RandomId = [16]u8;
/// One of the three 128-bit identities of a Terminal Host terminal.
pub const Identity = [16]u8;

pub const Process = struct {
    pid: u32,
    /// The process's creation time as a `FILETIME`, never zero.
    created: u64,

    pub fn eql(self: Process, other: Process) bool {
        return self.pid == other.pid and self.created == other.created;
    }
};

/// Handle values in the process that holds them: nonzero and below 2^32.
pub const Handles = struct {
    job: u32,
    program: u32,
    console: u32,
    signal: u32,
    reference: u32,
    input: u32,
    output: u32,

    pub const roles = [_][]const u8{ "job", "program", "console", "signal", "reference", "input", "output" };
};

pub const Terminal = struct {
    creation_id: Identity,
    host_epoch: Identity,
    terminal_id: Identity,
};

pub const ProgramHandles = programs.ProgramHandles;
pub const maximum_label_bytes = programs.maximum_label_bytes;
pub const ProgramFacts = programs.ProgramFacts;
pub const validLabel = programs.validLabel;

pub const Facts = struct {
    terminal: Terminal,
    program: Process,
    console: Process,
    /// OpenConsole's SHA-256.
    console_build: [32]u8,
    signal_format: u16,
    /// A name, such as `managed-powershell`.
    launch: []const u8,
};

/// A requester's role; any other name reads as `unknown`.
pub const Role = enum {
    worker,
    host,
    unknown,

    pub fn name(self: Role) []const u8 {
        return wireName(self);
    }

    pub fn fromName(text: []const u8) Role {
        return fromWireName(Role, text);
    }
};

/// Why the Keeper refused a request; any other name reads as `unknown`.
pub const Reason = enum {
    not_registered,
    wrong_role,
    not_owner,
    not_target,
    unknown_holding,
    holding_exists,
    marked,
    stale_mark,
    identity_mismatch,
    capacity,
    draining,
    handle_failed,
    /// The holding's owner still runs; only `recovery`'s requests answer it.
    not_orphaned,
    /// Under `parent`: the Keeper could not start the program a `spawn` or `spawn-program` named.
    spawn_failed,
    /// Under `parent`: a `reap` of a parented leader that has not ended.
    running,
    /// Under `reexec`: a holding's owner is no live registered Host.
    orphaned,
    /// Under `reexec`: a `spawn` is starting a leader, another update is under way, or the
    /// connections did not stop in time.
    busy,
    /// Under `reexec`: the new program's check of the state failed.
    check_failed,
    /// Under `reexec`: the new program could not be executed or could not read the state, and the
    /// old program came back.
    exec_failed,
    unknown,

    pub fn name(self: Reason) []const u8 {
        return wireName(self);
    }

    pub fn fromName(text: []const u8) Reason {
        return fromWireName(Reason, text);
    }
};

/// A holding's state in a `status-result`; any other name reads as `unknown`.
pub const HoldingState = enum {
    held,
    quiesced,
    orphaned,
    unknown,

    pub fn name(self: HoldingState) []const u8 {
        return wireName(self);
    }

    pub fn fromName(text: []const u8) HoldingState {
        return fromWireName(HoldingState, text);
    }
};

/// The answer of `resume` or `decline`; any other name reads as `unknown`.
pub const Outcome = enum {
    resumed,
    declined,
    claimed,
    unknown,

    pub fn name(self: Outcome) []const u8 {
        return wireName(self);
    }

    pub fn fromName(text: []const u8) Outcome {
        return fromWireName(Outcome, text);
    }
};

pub const RequestKind = enum {
    hello,
    register_host,
    unregister_host,
    status,
    hold,
    mark,
    @"resume",
    inspect,
    claim,
    decline,
    release,
    /// The Worker's requests under `recovery`.
    assign,
    discard,
    /// A Host's hold of a background program, under `programs`.
    hold_program,
    /// A Host's hold of a terminal's PTY master, under `pty`.
    hold_pty,
    /// A Host's request that the Keeper start and parent a terminal's leader, and its later
    /// request that the Keeper reap it, under `parent`.
    spawn,
    reap,
    /// The Worker's request, under `reexec`, that the Keeper execute another program in place.
    reexec,
    /// A Host's hold of a background program's pipes, and its request that the Keeper start and
    /// parent one, under `programs` (`spawn-program` also under `parent`).
    hold_pipes,
    spawn_program,

    pub fn name(self: RequestKind) []const u8 {
        return wireName(self);
    }

    pub fn fromName(text: []const u8) ?RequestKind {
        inline for (@typeInfo(RequestKind).@"enum".fields) |field| {
            const kind: RequestKind = @enumFromInt(field.value);
            if (std.mem.eql(u8, kind.name(), text)) return kind;
        }
        return null;
    }

    /// The `type` of this request's result.
    pub fn resultName(self: RequestKind) []const u8 {
        return switch (self) {
            inline else => |kind| comptime wireName(kind) ++ "-result",
        };
    }

    pub fn fromResultName(text: []const u8) ?RequestKind {
        inline for (@typeInfo(RequestKind).@"enum".fields) |field| {
            const kind: RequestKind = @enumFromInt(field.value);
            if (std.mem.eql(u8, kind.resultName(), text)) return kind;
        }
        return null;
    }
};

pub const Hold = struct {
    holding: RandomId,
    handles: Handles,
    facts: Facts,
};

pub const HoldProgram = programs.HoldProgram;

pub const Mark = struct {
    holding: RandomId,
    target: Process,
    mark: RandomId,
};

/// A request that names a holding and one of its marks.
pub const Marked = struct {
    holding: RandomId,
    mark: RandomId,
};

pub const Claim = struct {
    holding: RandomId,
    mark: RandomId,
    program: Process,
    /// The claimant's identity for a terminal; absent for a background program.
    terminal: ?Terminal = null,
};

pub const Request = struct {
    id: u32,
    body: Body,
    extensions: Extensions = &.{},

    pub const Body = union(RequestKind) {
        hello: Role,
        register_host: Process,
        unregister_host: Process,
        /// The holding id the page starts after, absent for the first page.
        status: ?RandomId,
        hold: Hold,
        mark: Mark,
        @"resume": Marked,
        inspect: Marked,
        claim: Claim,
        decline: Marked,
        release: RandomId,
        /// An orphaned holding's claim by `target` under `mark`, which the Worker mints.
        assign: Mark,
        discard: RandomId,
        hold_program: HoldProgram,
        hold_pty: pty.HoldPty,
        spawn: pty.Spawn,
        reap: RandomId,
        /// The Terminal Host program to re-execute from: an absolute path.
        reexec: []const u8,
        hold_pipes: pty.HoldPipes,
        spawn_program: pty.SpawnProgram,
    };

    /// Whether only a connection that negotiated `recovery` sends this request.
    pub fn needsRecovery(kind: RequestKind) bool {
        return kind == .assign or kind == .discard;
    }

    /// Whether only a connection that negotiated `programs` sends this request.
    pub fn needsPrograms(kind: RequestKind) bool {
        return kind == .hold_program or kind == .hold_pipes or kind == .spawn_program;
    }

    /// Whether only a connection that negotiated `pty` sends this request.
    pub fn needsPty(kind: RequestKind) bool {
        return kind == .hold_pty;
    }

    /// Whether only a connection that negotiated `parent` sends this request.
    pub fn needsParent(kind: RequestKind) bool {
        return kind == .spawn or kind == .reap or kind == .spawn_program;
    }

    /// Whether only a connection that negotiated `reexec` sends this request.
    pub fn needsReexec(kind: RequestKind) bool {
        return kind == .reexec;
    }

    /// How many descriptors arrive with this request by `SCM_RIGHTS`.
    pub fn descriptorCount(body: Body) u8 {
        return switch (body) {
            .hold_pty => 1,
            .spawn => |spawn| spawn.descriptorCount(),
            .hold_pipes => |hold| hold.capture.count(),
            .spawn_program => |spawn| 2 * spawn.capture.count(),
            else => 0,
        };
    }
};

pub const InitiatorMessage = union(enum) {
    request: Request,
    /// A request type this build does not know; its other members are unread.
    unknown_request: struct { request: []const u8, id: u32 },
};

/// What a holding holds: a terminal, or under `programs` a background program.
pub const HoldingFacts = union(enum) {
    terminal: Facts,
    program: ProgramFacts,
    /// Under `pty`: a terminal whose PTY master the Keeper holds.
    pty: pty.PtyHolding,
};

pub const HoldingEntry = struct {
    holding: RandomId,
    owner: Process,
    state: HoldingState,
    /// The current mark's target, present exactly while a mark is current.
    target: ?Process = null,
    facts: HoldingFacts,
};

pub const HostEntry = struct {
    process: Process,
    ended: bool,
};

pub const StatusPage = struct {
    holdings: []const HoldingEntry,
    /// Every registered Host, on the first page only.
    hosts: ?[]const HostEntry = null,
    /// The last listed holding id, present exactly when more holdings follow.
    next: ?RandomId = null,

    /// Holding ids ascend strictly, a `held` entry names no mark target, and a `quiesced` one does.
    pub fn valid(self: StatusPage) bool {
        if (self.holdings.len > maximum_status_entries) return false;
        if (self.hosts) |hosts| if (hosts.len > maximum_status_entries) return false;
        for (self.holdings, 0..) |entry, index| {
            if (index > 0 and !std.mem.lessThan(u8, &self.holdings[index - 1].holding, &entry.holding)) return false;
            switch (entry.state) {
                .held => if (entry.target != null) return false,
                .quiesced => if (entry.target == null) return false,
                .orphaned, .unknown => {},
            }
        }
        return true;
    }
};

pub const Inspection = struct {
    handles: Handles,
    facts: Facts,
};

pub const ProgramInspection = programs.ProgramInspection;

pub const Reply = struct {
    id: u32,
    body: Body,
    extensions: Extensions = &.{},

    pub const Body = union(enum) {
        hello,
        register_host,
        unregister_host,
        hold,
        mark,
        claim,
        release,
        assign,
        discard,
        hold_program,
        hold_pty,
        reap,
        reexec,
        hold_pipes,
        /// `spawn-result`: the leader the Keeper started.
        spawn: Process,
        /// `spawn-program-result`: the program the Keeper started.
        spawn_program: Process,
        status: StatusPage,
        @"resume": Outcome,
        decline: Outcome,
        inspect: Inspection,
        /// `inspect-result` for a program holding.
        inspect_program: ProgramInspection,
        /// `inspect-result` for a PTY holding, sent with its master.
        inspect_pty: pty.PtyHolding,
        /// `inspect-result` for a pipes holding, sent with each captured stream's read end.
        inspect_pipes: pty.PipesHolding,
        refused: Reason,
        /// The requested type, which the Keeper does not know.
        unsupported: []const u8,

        /// Whether this body may answer a request of `kind`.
        pub fn answers(self: Body, kind: RequestKind) bool {
            return switch (self) {
                .refused, .unsupported => true,
                .hello => kind == .hello,
                .register_host => kind == .register_host,
                .unregister_host => kind == .unregister_host,
                .hold => kind == .hold,
                .mark => kind == .mark,
                .claim => kind == .claim,
                .release => kind == .release,
                .assign => kind == .assign,
                .discard => kind == .discard,
                .hold_program => kind == .hold_program,
                .hold_pty => kind == .hold_pty,
                .reap => kind == .reap,
                .reexec => kind == .reexec,
                .spawn => kind == .spawn,
                .hold_pipes => kind == .hold_pipes,
                .spawn_program => kind == .spawn_program,
                .status => kind == .status,
                .@"resume" => kind == .@"resume",
                .decline => kind == .decline,
                .inspect, .inspect_program, .inspect_pty, .inspect_pipes => kind == .inspect,
            };
        }

        /// How many descriptors leave with this reply by `SCM_RIGHTS`.
        pub fn descriptorCount(self: Body) u8 {
            return switch (self) {
                .inspect_pty => 1,
                .inspect_pipes => |holding| holding.capture.count(),
                else => 0,
            };
        }
    };
};

pub const AcceptorMessage = union(enum) {
    reply: Reply,
    /// Under `parent`: the event `exited`.
    exited: pty.Exited,
    /// Another frame without `id`, which a receiver ignores; its other members are unread.
    event: []const u8,
};

/// Whether `program` can name the program a `reexec` executes: an absolute path of at most
/// `maximum_program_bytes` bytes without a NUL.
pub fn validProgram(program: []const u8) bool {
    if (program.len == 0 or program.len > maximum_program_bytes or program[0] != '/') return false;
    return std.mem.indexOfScalar(u8, program, 0) == null;
}

pub fn isZero(bytes: []const u8) bool {
    return std.mem.allEqual(u8, bytes, 0);
}

fn wireName(value: anytype) []const u8 {
    return switch (value) {
        inline else => |tag| comptime blk: {
            const text = @tagName(tag);
            var spelled: [text.len]u8 = undefined;
            for (text, &spelled) |byte, *out| out.* = if (byte == '_') '-' else byte;
            const final = spelled;
            break :blk &final;
        },
    };
}

fn fromWireName(comptime Enum: type, text: []const u8) Enum {
    inline for (@typeInfo(Enum).@"enum".fields) |field| {
        const value: Enum = @enumFromInt(field.value);
        if (value != .unknown and std.mem.eql(u8, wireName(value), text)) return value;
    }
    return .unknown;
}
