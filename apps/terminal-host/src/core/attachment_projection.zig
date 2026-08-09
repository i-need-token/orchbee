const std = @import("std");

const identifiers = @import("terminal-host-model").identifiers;

pub const Demand = enum {
    active,
    paused,
};

pub const Block = enum {
    capture_allocation_failed,
    projection_too_large,
};

pub const FrameKind = enum {
    snapshot,
    delta,
};

pub const InFlightFrame = struct {
    generation: u64,
    revision: u64,
    sequence: u64,
};

pub const FrameBase = struct {
    generation: u64,
    revision: u64,
    sequence: u64,
};

pub const Ready = union(enum) {
    snapshot: ?FrameBase,
    delta: FrameBase,

    pub fn base(self: Ready) ?FrameBase {
        return switch (self) {
            .snapshot => |base_frame| base_frame,
            .delta => |base_frame| base_frame,
        };
    }

    pub fn forceSnapshot(self: Ready) bool {
        return switch (self) {
            .snapshot => true,
            .delta => false,
        };
    }
};

pub const Flight = struct {
    base_before: ?FrameBase,
    dirty_after_capture: bool = false,
    frame: InFlightFrame,
};

pub const Blocked = struct {
    base: ?FrameBase,
    observed_revision: u64,
    reason: Block,
};

pub const Delivery = union(enum) {
    paused,
    ready: Ready,
    in_flight: Flight,
    blocked: Blocked,

    pub fn demand(self: Delivery) Demand {
        return switch (self) {
            .paused => .paused,
            .ready, .in_flight, .blocked => .active,
        };
    }
};

pub const FrameCommit = struct {
    attachment_id: identifiers.AttachmentId,
    flight: InFlightFrame,
    ready_before: Ready,
};

pub const BlockCommit = struct {
    attachment_id: identifiers.AttachmentId,
    blocked: Blocked,
    ready_before: Ready,
};

pub const Transition = enum {
    applied,
    idempotent,
};

pub const Error = error{
    InvalidFrameAcknowledgement,
    InvalidPhase,
    SequenceOverflow,
};

pub const State = struct {
    delivery: Delivery = .paused,
    last_abandoned_sequence: u64 = 0,
    next_sequence: u64 = 1,

    pub fn demand(self: State) Demand {
        return self.delivery.demand();
    }

    pub fn ready(self: State) ?Ready {
        return switch (self.delivery) {
            .ready => |value| value,
            .paused, .in_flight, .blocked => null,
        };
    }

    pub fn inFlight(self: State) ?Flight {
        return switch (self.delivery) {
            .in_flight => |value| value,
            .paused, .ready, .blocked => null,
        };
    }

    pub fn blocked(self: State) ?Blocked {
        return switch (self.delivery) {
            .blocked => |value| value,
            .paused, .ready, .in_flight => null,
        };
    }

    pub fn activate(self: *State) void {
        std.debug.assert(self.demand() == .paused);
        self.delivery = .{ .ready = .{ .snapshot = null } };
    }

    pub fn pause(self: *State) void {
        std.debug.assert(self.demand() == .active);
        if (self.inFlight()) |flight| {
            self.last_abandoned_sequence = @max(
                self.last_abandoned_sequence,
                flight.frame.sequence,
            );
        }
        self.delivery = .paused;
    }

    pub fn commitFrame(
        self: *State,
        attachment_id: identifiers.AttachmentId,
        base_revision: ?u64,
        revision: u64,
        generation: u64,
        kind: FrameKind,
    ) Error!FrameCommit {
        const ready_before = self.ready() orelse return error.InvalidPhase;
        if (generation == 0) return error.InvalidPhase;
        if (kind == .snapshot) {
            if (base_revision != null) return error.InvalidPhase;
        } else {
            const base = switch (ready_before) {
                .delta => |value| value,
                .snapshot => return error.InvalidPhase,
            };
            if (base_revision != base.revision or base.generation != generation or
                revision < base.revision) return error.InvalidPhase;
        }
        const flight: InFlightFrame = .{
            .generation = generation,
            .revision = revision,
            .sequence = self.next_sequence,
        };
        const commit: FrameCommit = .{
            .attachment_id = attachment_id,
            .flight = flight,
            .ready_before = ready_before,
        };
        self.next_sequence = std.math.add(u64, self.next_sequence, 1) catch
            return error.SequenceOverflow;
        self.delivery = .{ .in_flight = .{
            .base_before = ready_before.base(),
            .frame = flight,
        } };
        return commit;
    }

    pub fn rollbackFrame(self: *State, commit: FrameCommit) Error!void {
        const flight = self.inFlight() orelse return error.InvalidFrameAcknowledgement;
        const expected_next_sequence = std.math.add(u64, commit.flight.sequence, 1) catch
            return error.InvalidFrameAcknowledgement;
        if (!sameInFlightFrame(flight.frame, commit.flight) or
            self.next_sequence != expected_next_sequence)
        {
            return error.InvalidFrameAcknowledgement;
        }
        self.next_sequence = commit.flight.sequence;
        self.delivery = .{ .ready = commit.ready_before };
    }

    pub fn frameApplied(
        self: *State,
        sequence: u64,
        revision: u64,
        generation: u64,
        geometry_changed_at_revision: u64,
    ) Error!Transition {
        if (sequence != 0 and sequence <= self.last_abandoned_sequence) return .idempotent;
        if (self.inFlight()) |flight| {
            if (flight.frame.sequence != sequence or flight.frame.revision != revision or
                flight.frame.generation != generation)
            {
                return error.InvalidFrameAcknowledgement;
            }
            const base: FrameBase = .{
                .generation = generation,
                .revision = revision,
                .sequence = sequence,
            };
            self.delivery = .{ .ready = if (geometry_changed_at_revision > revision)
                .{ .snapshot = base }
            else
                .{ .delta = base } };
            return .applied;
        }
        const base = switch (self.delivery) {
            .ready => |ready_state| ready_state.base(),
            .blocked => |blocked_state| blocked_state.base,
            .paused, .in_flight => null,
        };
        if (base) |applied| {
            if (applied.sequence == sequence and applied.revision == revision and
                applied.generation == generation)
            {
                return .idempotent;
            }
        }
        return error.InvalidFrameAcknowledgement;
    }

    pub fn needSnapshot(self: *State, sequence: u64) Error!Transition {
        if (sequence != 0 and sequence <= self.last_abandoned_sequence) return .idempotent;
        const flight = self.inFlight() orelse return error.InvalidFrameAcknowledgement;
        if (flight.frame.sequence != sequence) return error.InvalidFrameAcknowledgement;
        self.last_abandoned_sequence = @max(self.last_abandoned_sequence, sequence);
        self.delivery = .{ .ready = .{ .snapshot = flight.base_before } };
        return .applied;
    }

    pub fn block(
        self: *State,
        attachment_id: identifiers.AttachmentId,
        reason: Block,
        observed_revision: u64,
    ) Error!BlockCommit {
        const ready_before = self.ready() orelse return error.InvalidPhase;
        const blocked_state: Blocked = .{
            .base = ready_before.base(),
            .observed_revision = observed_revision,
            .reason = reason,
        };
        const commit: BlockCommit = .{
            .attachment_id = attachment_id,
            .blocked = blocked_state,
            .ready_before = ready_before,
        };
        self.delivery = .{ .blocked = blocked_state };
        return commit;
    }

    pub fn rollbackBlock(self: *State, commit: BlockCommit) Error!void {
        const blocked_state = self.blocked() orelse return error.InvalidPhase;
        if (!sameBlock(blocked_state, commit.blocked)) return error.InvalidPhase;
        self.delivery = .{ .ready = commit.ready_before };
    }

    pub fn requireCaughtUp(self: State, observed_revision: u64) Error!void {
        const ready_state = self.ready() orelse return error.InvalidPhase;
        const base = switch (ready_state) {
            .delta => |value| value,
            .snapshot => return error.InvalidPhase,
        };
        if (base.revision < observed_revision) return error.InvalidPhase;
    }

    pub fn refresh(
        self: *State,
        observed_revision: u64,
        geometry_changed_at_revision: u64,
    ) void {
        switch (self.delivery) {
            .paused => {},
            .ready => |*ready_state| switch (ready_state.*) {
                .snapshot => {},
                .delta => |base| {
                    if (geometry_changed_at_revision > base.revision) {
                        ready_state.* = .{ .snapshot = base };
                    }
                },
            },
            .in_flight => |*flight| {
                if (observed_revision > flight.frame.revision) {
                    flight.dirty_after_capture = true;
                }
            },
            .blocked => |blocked_state| {
                if (observed_revision > blocked_state.observed_revision) {
                    self.delivery = .{ .ready = .{ .snapshot = blocked_state.base } };
                }
            },
        }
    }
};

fn sameInFlightFrame(left: InFlightFrame, right: InFlightFrame) bool {
    return left.generation == right.generation and
        left.revision == right.revision and
        left.sequence == right.sequence;
}

fn sameBlock(left: Blocked, right: Blocked) bool {
    return left.reason == right.reason and
        left.observed_revision == right.observed_revision and
        if (left.base) |left_base|
            if (right.base) |right_base|
                left_base.generation == right_base.generation and
                    left_base.revision == right_base.revision and
                    left_base.sequence == right_base.sequence
            else
                false
        else
            right.base == null;
}
