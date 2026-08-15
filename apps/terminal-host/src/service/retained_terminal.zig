const core_module = @import("terminal-host-core").state;
const terminal_runtime = @import("terminal-host-engine").runtime;

/// The one allocation that binds a retained terminal's semantic record to its
/// native resources. Core indexes the stable address of `session`; the Host
/// runtime aggregate owns this enclosing record and its complete lifetime.
pub const RetainedTerminal = struct {
    session: core_module.TerminalSession,
    runtime: *terminal_runtime.TerminalRuntime,

    pub fn fromSession(
        session: *const core_module.TerminalSession,
    ) *const RetainedTerminal {
        return @fieldParentPtr("session", session);
    }

    pub fn fromMutableSession(
        session: *core_module.TerminalSession,
    ) *RetainedTerminal {
        return @fieldParentPtr("session", session);
    }
};
