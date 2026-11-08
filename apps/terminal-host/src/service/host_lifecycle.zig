//! The Host's lifecycle under the `drain` and `exit` control requests. `ready` admits new
//! terminals; `draining` admits none and ends the Host once it retains no terminal; `stopping`
//! ends the Host now, with every terminal. A request only moves the state forward, so a repeated,
//! late, or reordered request never undoes another.

const message = @import("terminal-host-handshake").message;

pub const State = enum(u8) {
    ready,
    draining,
    stopping,

    pub fn processState(self: State) message.ProcessState {
        return switch (self) {
            .ready => .ready,
            .draining => .draining,
            .stopping => .stopping,
        };
    }
};

pub const Request = enum {
    drain,
    exit,
};

/// Whether a `CreateTerminal` may add a retained terminal. A replay of a retained creation adds
/// none, so it is answered in every state.
pub const TerminalAdmission = enum {
    open,
    closed,
};

pub fn transition(state: State, request: Request) State {
    return switch (request) {
        .drain => switch (state) {
            .ready => .draining,
            .draining, .stopping => state,
        },
        .exit => .stopping,
    };
}

/// Whether `request` is already in effect in `state`, so its requester can be answered.
pub fn applied(state: State, request: Request) bool {
    return transition(state, request) == state;
}

pub fn terminalAdmission(state: State) TerminalAdmission {
    return switch (state) {
        .ready => .open,
        .draining, .stopping => .closed,
    };
}

/// Whether a control request ends the Host now, given how many terminals it retains. The idle
/// exit of a `ready` Host stays with the serving loop.
pub fn endsNow(state: State, retained_terminals: usize) bool {
    return switch (state) {
        .ready => false,
        .draining => retained_terminals == 0,
        .stopping => true,
    };
}
