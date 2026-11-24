//! A Workspace's move as `workspace set-machine`, a child of this program on a background thread:
//! the run, the command it starts, and what its JSON document says. The TUI's Change machine and
//! the desktop window's both drive it, so a person, a Client and an Agent get one result under one
//! set of rules. `apps/orchbee/docs/machines.md#change-machine` owns the rules.

use crate::background_thread::BackgroundThread;
use crate::invocation::grammar::schemas::{
    WORKSPACE_SET_MACHINE_FAILURE_ID as FAILURE, WORKSPACE_SET_MACHINE_RESULT_ID as RESULT,
};
use crate::{
    machine_change::{Name, Stops},
    machines::LOCAL,
    workbench_client::{ClientContextLocation, ClientContextTarget, UiWakeup},
};
use orchbee_local_runtime::{CommandFailure, CommandOutput};
use orchbee_workbench_protocol::{SessionId, WorkbenchTerminalContextId, WorkbenchWorkspaceId};
use serde_json::Value;
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

/// The bound on reading what the move would stop, a login it waits for included; it changes
/// nothing.
pub(crate) const CHECK: Duration = Duration::from_secs(180);
/// The bound on the move, past each of its steps' own.
pub(crate) const MOVE: Duration = Duration::from_secs(600);

/// One run of the child.
pub(crate) struct Run {
    task: BackgroundThread,
    /// Its output; `None` when the thread ended without one.
    result: mpsc::Receiver<CommandOutput>,
    /// Asks the child to end. Only a check is asked: a move, once started, is let finish.
    stop: Arc<AtomicBool>,
}

impl Run {
    /// Starts `arguments` within `bound`, waking `wakeup` when the child ends.
    pub(crate) fn spawn(
        arguments: Vec<String>,
        bound: Duration,
        wakeup: UiWakeup,
    ) -> io::Result<Self> {
        let command = crate::self_command::command(arguments)?;
        let deadline = Instant::now() + bound;
        let stop = Arc::new(AtomicBool::new(false));
        let asked = Arc::clone(&stop);
        let (sender, result) = mpsc::sync_channel(1);
        let task = BackgroundThread::spawn("orchbee-machine-move", move || {
            let output = crate::self_command::exchange(command, deadline, || {
                Ok(asked.load(Ordering::Relaxed))
            });
            // The owner may have let go of the receiver at exit.
            let _delivered = sender.send(output);
            wakeup.signal();
        })?;
        Ok(Self { task, result, stop })
    }

    /// The child's output once the thread delivered it; `Some(None)` when it ended without one.
    pub(crate) fn settled(&self) -> Option<Option<CommandOutput>> {
        match self.result.try_recv() {
            Ok(output) => Some(Some(output)),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(None),
        }
    }

    /// Asks the child to end; only a check is ever asked.
    pub(crate) fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    pub(crate) fn join_before(&mut self, deadline: Instant) -> io::Result<()> {
        self.task.join_before(deadline)
    }
}

/// The command a run starts: `from` names the machine that holds the Workspace and `to` the one it
/// goes to, `None` for this computer.
pub(crate) fn arguments(
    workspace: &str,
    from: Option<&str>,
    to: Option<&str>,
    confirm: bool,
) -> Vec<String> {
    let mut words = Vec::new();
    if let Some(from) = from {
        words.extend(["--machine".to_owned(), from.to_owned()]);
    }
    words.extend(
        [
            "workspace",
            "set-machine",
            workspace,
            "--to",
            to.unwrap_or(LOCAL),
            if confirm { "--confirm" } else { "--dry-run" },
            "--format",
            "json",
        ]
        .map(str::to_owned),
    );
    words
}

/// What a check read: the folder the move asks for first, and what the move would stop, `None`
/// when nothing runs. `Err` when the check did not answer with its document.
pub(crate) fn checked(
    output: Option<&CommandOutput>,
) -> Result<(Option<String>, Option<Stops>), ()> {
    let Some((0, document)) = document(output).filter(|(_, document)| document["schema"] == RESULT)
    else {
        return Err(());
    };
    let result = &document["result"];
    let folder = result["folder"].as_str().map(str::to_owned);
    if result["idle"].as_bool() == Some(true) {
        return Ok((folder, None));
    }
    let busy: Vec<&Value> = result["terminals"]
        .as_array()
        .map(|terminals| {
            terminals
                .iter()
                .filter(|terminal| terminal["activity"] != "idle")
                .collect()
        })
        .unwrap_or_default();
    let stops = if result["evidence"] == "read" && !busy.is_empty() {
        Stops::Busy {
            count: busy.len(),
            names: busy.iter().filter_map(|terminal| name(terminal)).collect(),
        }
    } else {
        Stops::Unread
    };
    Ok((folder, Some(stops)))
}

/// Where a move that succeeded put the Workspace.
pub(crate) struct Landing {
    pub(crate) target: ClientContextTarget,
    /// The folder it started in, relative to that machine's home.
    pub(crate) folder: String,
    /// Its path there, as that machine spells it.
    pub(crate) path: String,
}

/// What a move's run says: `Err` when it did not succeed, otherwise where the Workspace landed,
/// `None` when the document moved it without saying where in a readable form.
pub(crate) fn landed(output: Option<&CommandOutput>) -> Result<Option<Landing>, ()> {
    let Some((0, document)) = document(output).filter(|(_, document)| document["schema"] == RESULT)
    else {
        return Err(());
    };
    let result = &document["result"];
    let created = &result["created"];
    let landing = || {
        let session_id = SessionId::parse(created["sessionId"].as_str()?).ok()?;
        let workspace_id = WorkbenchWorkspaceId::parse(created["workspaceId"].as_str()?).ok()?;
        let terminal_context_id =
            WorkbenchTerminalContextId::parse(created["terminalContextId"].as_str()?).ok()?;
        Some(Landing {
            target: ClientContextTarget {
                session_id,
                location: ClientContextLocation::Workspace {
                    workspace_id,
                    terminal_context_id,
                },
            },
            folder: result["folder"].as_str()?.to_owned(),
            path: created["path"].as_str()?.to_owned(),
        })
    };
    Ok(landing())
}

/// The folder a landing names as missing: the one the check asked for first, which is not on the
/// other machine, so the shell started in home, spelled as that machine spells its paths.
pub(crate) fn missing_folder(asked: Option<&str>, landing: &Landing) -> Option<String> {
    asked
        .filter(|asked| *asked != "~" && landing.folder == "~")
        .map(|asked| crate::invocation::user_path::spelled(asked, &landing.path))
}

/// Why a run did not succeed, as its failure document says.
pub(crate) enum Refusal {
    /// The new Workspace exists, and the old one could not be closed.
    CloseFailed,
    /// The machine this saved identity names is not connected; `None` when the document named
    /// none.
    NotConnected(Option<String>),
    /// A Workspace in a linked worktree stays with its repository.
    Worktree,
    /// Nothing changed.
    Failed,
    /// No answer said how it ended.
    Unknown,
}

/// What a run that did not succeed leaves the person with. A check changes nothing; a move that
/// ended without saying how says so.
pub(crate) fn refusal(output: Option<&CommandOutput>, moved: bool) -> Refusal {
    let unknown = if moved {
        Refusal::Unknown
    } else {
        Refusal::Failed
    };
    let Some((_, document)) =
        document(output).filter(|(_, document)| document["schema"] == FAILURE)
    else {
        return unknown;
    };
    let failure = &document["failure"];
    let details = &failure["details"];
    if details["created"].is_object() {
        return Refusal::CloseFailed;
    }
    match details["reason"].as_str() {
        Some("not-connected") => {
            Refusal::NotConnected(details["machine"].as_str().map(str::to_owned))
        }
        Some("worktree") => Refusal::Worktree,
        _ if matches!(
            failure["kind"].as_str(),
            Some("outcome-unknown" | "timeout" | "cancelled")
        ) =>
        {
            unknown
        }
        _ => Refusal::Failed,
    }
}

/// The child's JSON document, with its exit status; `None` when it ended without one.
fn document(output: Option<&CommandOutput>) -> Option<(i32, Value)> {
    let output = output?;
    if output.stdout_truncated
        || !matches!(output.failure, None | Some(CommandFailure::ExitFailure))
    {
        return None;
    }
    Some((
        output.exit_code?,
        serde_json::from_slice(&output.stdout).ok()?,
    ))
}

/// A terminal's name as the command names it: the Agent it runs, else its title.
fn name(terminal: &Value) -> Option<Name> {
    let observation = &terminal["observation"];
    observation["agent"]
        .as_str()
        .map(|agent| Name::Agent(agent.to_owned()))
        .or_else(|| {
            observation["title"]
                .as_str()
                .map(|title| Name::Title(title.to_owned()))
        })
}
