//! Delivers the Agent start of every Tab, split, Workspace or Worktree creation that carried one,
//! so no Client types it: one candidate per started terminal, which types at most one line and
//! publishes at most one `agentStartFailure` on its Presentation. It never starts an Agent in an
//! existing shell, reserves a name, or replays a line. The reference is
//! `apps/orchbee-server/docs/agent-start-delivery.md`.
//!
//! # Candidate
//!
//! States, by the Host's launch kind once a read shows it:
//! - `Unread`: no process sample has been read yet.
//! - `Offering`: a marked cmd waits for a confirmed prompt to take the line.
//! - `Sending`: the one line is at the Host's prompt gate.
//! - `Watching`: a managed shell, PowerShell, zsh or bash, runs the start as its first command, or
//!   the Host typed the line; the early check reads the terminal until the watch ends.
//! - `Overtime`: that command still runs past its watch with no process of the provider seen;
//!   slower reads wait for it to start the Agent or return to the prompt.
//! - `Ended`.
//!
//! Transitions, with the launch's acceptance as `accepted`:
//! - `Unread`, a sample with launch kind managed PowerShell, zsh or bash: `Watching` until
//!   `accepted` + 10 s, nothing typed. Marked cmd: `Offering`. Another program: ends
//!   `unsupported-shell`. No launch kind: ends `terminal-too-old`. A read without a sample reads
//!   again; at `accepted` + 30 s it ends with no fact.
//! - `Offering`, a read that shows Client input or a shell command: ends with no fact. A read at
//!   `accepted` + 30 s or later: ends `prompt-unconfirmed`. A read the readiness rule admits, no
//!   sooner than a refusal's pause: `Sending`. Anything else reads again in 250 ms.
//! - `Sending`, the Host typed it: `Watching` until 3 s from then. A refusal that typed nothing:
//!   `Offering` after 250 ms, or the prompt's quiet period when it was settling, unless that
//!   reaches `accepted` + 30 s, which ends `prompt-unconfirmed`. A Host without the gate: ends
//!   `terminal-too-old`. Any other result: ends with no fact, never sent again.
//! - `Watching`, the shell idle after a process of the provider was seen: ends `exited`. Idle at
//!   the end of the watch with none seen: ends `not-found`. Busy then with none seen: `Overtime`.
//!   The end of the watch otherwise, an unread sample included: ends with no fact. Client input
//!   and shell commands are not refutations here.
//! - `Overtime`, read every 1 s: a process of the provider: `Watching` until 3 s from then, with
//!   it seen. The shell back at its own prompt, by the readiness rule, so a moment between two of
//!   a script's children never counts: ends `not-found` at that read. A read at `accepted` + 3 min,
//!   or one without a sample 30 s after the last read that had one: ends with no fact. Client
//!   input and shell commands are not refutations here either; the watch only reads.
//! - Any state, the Presentation left or rebound, its terminal of the launch's generation stopped
//!   answering, or a guard refused the send before anything was typed: ends with no fact.
//!
//! The Server stopping drops every candidate with nothing typed and no fact.
mod driver;
mod model;
#[cfg(test)]
mod tests;

pub(super) use driver::AgentStartDelivery;
