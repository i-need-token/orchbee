//! The `blockingProcess` member: a process an earlier start registered that is still running, which
//! keeps a start from recovering the Session until it ends. The `recovery-required` refusal of every
//! command that starts a Session carries it, and so does `doctor`'s runtime check.
use orchbee_local_runtime::{BlockerRole, RecoveryBlocker};
use serde_json::{Value, json};

/// `role` is the closed token, `program` the file name the operating system shows for the process,
/// never a path, and `processId` its process ID.
pub(crate) fn member(blocker: RecoveryBlocker) -> Value {
    json!({
        "role": token(blocker.role),
        "program": blocker.role.program().file_name(),
        "processId": blocker.process_id,
    })
}

/// The process a `blockingProcess` member names, for the text that words it.
pub(crate) fn read(member: &Value) -> Option<RecoveryBlocker> {
    let role = match member["role"].as_str()? {
        "server" => BlockerRole::Server,
        "worker" => BlockerRole::Worker,
        "terminal-host" => BlockerRole::TerminalHost,
        _ => return None,
    };
    let process_id = u32::try_from(member["processId"].as_u64()?)
        .ok()
        .filter(|id| *id != 0)?;
    Some(RecoveryBlocker { role, process_id })
}

const fn token(role: BlockerRole) -> &'static str {
    match role {
        BlockerRole::Server => "server",
        BlockerRole::Worker => "worker",
        BlockerRole::TerminalHost => "terminal-host",
    }
}
