//! Change machine, the machine manager's side: hands the action menu the facts its list shows,
//! runs the move the person chose as `workspace set-machine`, a child of this program on a
//! background thread, and lands the view on the moved Workspace. The child first only reads what
//! the move would stop; the picker asks when something may be running, and the move runs once the
//! person confirms, or at once when nothing runs. One move runs at a time, and nothing it waits for
//! holds the shell's tick. `apps/orchbee/docs/machines.md#change-machine` owns the rules.

use super::{Manager, SurfaceSlot};
use crate::background_thread::BackgroundThread;
use crate::invocation::grammar::schemas::{
    WORKSPACE_SET_MACHINE_FAILURE_ID as FAILURE, WORKSPACE_SET_MACHINE_RESULT_ID as RESULT,
};
use crate::{
    client_view::ClientView,
    machine_change::{Facts, Name, Notice, Question, Request, Stops},
    machine_picker::Choice,
    machines::{LOCAL, ProfileId},
    workbench_client::{ClientContextLocation, ClientContextTarget, ClientStartupTarget},
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
const CHECK: Duration = Duration::from_secs(180);
/// The bound on the move, past each of its steps' own.
const MOVE: Duration = Duration::from_secs(600);
/// How often the list's facts refresh while no menu offers Change machine.
const FACTS: Duration = Duration::from_secs(1);

/// One run of the child.
struct Run {
    task: BackgroundThread,
    /// Its output; `None` when the thread ended without one.
    result: mpsc::Receiver<CommandOutput>,
    /// Asks the child to end. Only a check is asked: a move, once started, is let finish.
    stop: Arc<AtomicBool>,
}

impl Run {
    /// The child's output once the thread delivered it; `Some(None)` when it ended without one.
    fn settled(&self) -> Option<Option<CommandOutput>> {
        match self.result.try_recv() {
            Ok(output) => Some(Some(output)),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(None),
        }
    }
}

enum Stage {
    /// The child reads what the move would stop.
    Checking(Run),
    /// The person answers the question with this token.
    Asking(u64),
    /// The child moves the Workspace.
    Moving(Run),
}

struct Move {
    request: Request,
    /// The machine that holds the Workspace; `None` for this computer.
    from: Option<ProfileId>,
    to: Choice,
    /// The folder relative to home the check named, which the move asks for first.
    folder: Option<String>,
    stage: Stage,
}

/// What a tick does with the move under way.
enum Next {
    Wait,
    Ask(Stops),
    Move,
    /// It ended, leaving this notice.
    End(Option<Notice>),
}

#[derive(Default)]
pub(super) struct Mover {
    under_way: Option<Move>,
    /// When the list's facts next refresh while no menu wants them.
    due: Option<Instant>,
    /// Whether the facts handed over last said a move was under way.
    handed_moving: bool,
}

impl Mover {
    /// Process exit: a check ends at once; a move is let finish, so the Workspace is not left on
    /// both machines.
    pub(super) fn request_stop(&self) {
        if let Some(Move {
            stage: Stage::Checking(run),
            ..
        }) = &self.under_way
        {
            run.stop.store(true, Ordering::Relaxed);
        }
    }

    /// Process exit: observes the run against the shell's deadline.
    pub(super) fn stop_before(&mut self, deadline: Instant) -> io::Result<()> {
        match &mut self.under_way {
            Some(Move {
                stage: Stage::Checking(run) | Stage::Moving(run),
                ..
            }) => run.task.join_before(deadline),
            _ => Ok(()),
        }
    }
}

impl Manager {
    /// One tick of Change machine, before the picker's selection, which lands the view on a moved
    /// Workspace in the same tick: the facts an open list shows, the move the person chose, and
    /// the one under way.
    pub(super) fn exchange_moves(
        &mut self,
        surface: &mut SurfaceSlot<'_>,
        view: &mut ClientView,
        now: Instant,
    ) -> bool {
        let mut redraw = false;
        let wanted = surface.active().machine_change_wanted();
        let moving = self.moves.under_way.is_some();
        // Only the open list shows free capacity; sampling it starts the hub.
        if surface.active().machine_list_open() {
            self.discovery.sample_capacity(now);
        }
        if wanted
            || moving != self.moves.handed_moving
            || self.moves.due.is_none_or(|due| now >= due)
        {
            self.moves.due = Some(now + FACTS);
            self.moves.handed_moving = moving;
            let facts = Facts {
                here: self
                    .current
                    .as_ref()
                    .map(|profile| profile.id.as_str().to_owned()),
                local: self.discovery.capacity(None),
                machines: self.settings.machines(
                    &mut self.discovery,
                    &self.profiles,
                    |_| None,
                    now,
                ),
                moving,
            };
            redraw |= surface.active().observe_machine_change(facts);
        }
        // A list offers no row while a move is under way; one chosen just before waits for none.
        if let Some(request) = surface.active().take_machine_change_request()
            && self.moves.under_way.is_none()
        {
            self.start_move(request, view);
            redraw = true;
        }
        redraw |= self.drive_move(view);
        redraw |= view.machines.tick_move(now);
        redraw
    }

    /// Starts the move `request` asks for with its check.
    fn start_move(&mut self, request: Request, view: &mut ClientView) {
        let to = match &request.to {
            None => Some(Choice::Local),
            Some(id) => self
                .profiles
                .iter()
                .find(|profile| profile.id.as_str() == id)
                .map(|profile| Choice::Remote(profile.id.clone())),
        };
        // A machine removed since the list opened takes nothing.
        let Some(to) = to else {
            view.machines.move_notice(Notice::Failed);
            return;
        };
        let from = self.current.as_ref().map(|profile| profile.id.clone());
        match self.spawn(arguments(&request, from.as_ref(), false), CHECK) {
            Ok(run) => {
                self.moves.under_way = Some(Move {
                    request,
                    from,
                    to,
                    folder: None,
                    stage: Stage::Checking(run),
                });
                view.machines.set_moving(true);
            }
            Err(_) => view.machines.move_notice(Notice::Failed),
        }
    }

    fn drive_move(&mut self, view: &mut ClientView) -> bool {
        let Some(mut moving) = self.moves.under_way.take() else {
            return false;
        };
        let next = match &moving.stage {
            Stage::Checking(run) => match run.settled() {
                None => Next::Wait,
                Some(output) => match checked(output.as_ref()) {
                    Err(()) => Next::End(Some(self.refusal(output.as_ref(), &moving, false))),
                    Ok((folder, stops)) => {
                        moving.folder = folder;
                        stops.map_or(Next::Move, Next::Ask)
                    }
                },
            },
            Stage::Asking(token) => match view.machines.take_move_answer() {
                Some((answered, confirm)) if answered == *token => {
                    if confirm {
                        Next::Move
                    } else {
                        Next::End(None)
                    }
                }
                // The question went without an answer, as when its view was let go.
                _ if !view.machines.move_shown() => Next::End(None),
                _ => Next::Wait,
            },
            Stage::Moving(run) => match run.settled() {
                None => Next::Wait,
                Some(output) => Next::End(self.land(&moving, output.as_ref(), view)),
            },
        };
        match next {
            Next::Wait => {
                self.moves.under_way = Some(moving);
                return false;
            }
            Next::Ask(stops) => {
                let question = Question {
                    workspace: moving.request.label.clone(),
                    to: moving.request.to_label.clone(),
                    stops,
                };
                if let Some(token) = view.machines.ask_move(question) {
                    moving.stage = Stage::Asking(token);
                    self.moves.under_way = Some(moving);
                    view.machines.set_moving(false);
                } else {
                    finish(view, Some(Notice::Failed));
                }
            }
            Next::Move => {
                let confirmed = arguments(&moving.request, moving.from.as_ref(), true);
                match self.spawn(confirmed, MOVE) {
                    Ok(run) => {
                        moving.stage = Stage::Moving(run);
                        self.moves.under_way = Some(moving);
                        view.machines.set_moving(true);
                    }
                    Err(_) => finish(view, Some(Notice::Failed)),
                }
            }
            Next::End(notice) => finish(view, notice),
        }
        true
    }

    /// Lands the view on the Workspace the move created and returns the notice it leaves, if any:
    /// the folder it asked for first is not on that machine, so the shell started in home.
    fn land(
        &self,
        moving: &Move,
        output: Option<&CommandOutput>,
        view: &mut ClientView,
    ) -> Option<Notice> {
        let Some((0, document)) =
            document(output).filter(|(_, document)| document["schema"] == RESULT)
        else {
            return Some(self.refusal(output, moving, true));
        };
        let result = &document["result"];
        // It moved; a landing that cannot be read leaves the view where it is.
        let (target, folder, path) = landing(result)?;
        let _selected = view
            .machines
            .select_moved(moving.to.clone(), ClientStartupTarget::Context(target));
        // The folder reads as that machine spells its paths.
        moving
            .folder
            .as_deref()
            .filter(|asked| *asked != "~" && folder == "~")
            .map(|asked| Notice::FolderMissing {
                machine: moving.request.to_label.clone(),
                folder: crate::invocation::user_path::spelled(asked, &path),
            })
    }

    /// What a run that did not succeed leaves the person with. A check changes nothing; a move that
    /// ended without saying how, or with the new Workspace created and the old one still open,
    /// says so.
    fn refusal(&self, output: Option<&CommandOutput>, moving: &Move, moved: bool) -> Notice {
        let unknown = if moved {
            Notice::Unknown
        } else {
            Notice::Failed
        };
        let Some((_, document)) =
            document(output).filter(|(_, document)| document["schema"] == FAILURE)
        else {
            return unknown;
        };
        let failure = &document["failure"];
        let details = &failure["details"];
        if details["created"].is_object() {
            return Notice::CloseFailed {
                machine: moving.request.to_label.clone(),
            };
        }
        match details["reason"].as_str() {
            Some("not-connected") => Notice::NotConnected {
                machine: details["machine"]
                    .as_str()
                    .and_then(|id| {
                        self.profiles
                            .iter()
                            .find(|profile| profile.id.as_str() == id)
                    })
                    .map_or_else(
                        || moving.request.to_label.clone(),
                        |profile| profile.draft.label.clone(),
                    ),
            },
            Some("worktree") => Notice::Worktree,
            _ if matches!(
                failure["kind"].as_str(),
                Some("outcome-unknown" | "timeout" | "cancelled")
            ) =>
            {
                unknown
            }
            _ => Notice::Failed,
        }
    }

    fn spawn(&self, arguments: Vec<String>, bound: Duration) -> io::Result<Run> {
        let command = crate::self_command::command(arguments)?;
        let deadline = Instant::now() + bound;
        let stop = Arc::new(AtomicBool::new(false));
        let asked = Arc::clone(&stop);
        let (sender, result) = mpsc::sync_channel(1);
        let wakeup = self.wakeup.clone();
        let task = BackgroundThread::spawn("orchbee-machine-move", move || {
            let output = crate::self_command::exchange(command, deadline, || {
                Ok(asked.load(Ordering::Relaxed))
            });
            // The owner may have let go of the receiver at exit.
            let _delivered = sender.send(output);
            wakeup.signal();
        })?;
        Ok(Run { task, result, stop })
    }
}

/// The move ended: the status line stops saying it runs and shows what it left.
fn finish(view: &mut ClientView, notice: Option<Notice>) {
    view.machines.set_moving(false);
    if let Some(notice) = notice {
        view.machines.move_notice(notice);
    }
}

/// The command a run starts: `from` names the machine that holds the Workspace.
fn arguments(request: &Request, from: Option<&ProfileId>, confirm: bool) -> Vec<String> {
    let mut words = Vec::new();
    if let Some(from) = from {
        words.extend(["--machine".to_owned(), from.as_str().to_owned()]);
    }
    words.extend(
        [
            "workspace",
            "set-machine",
            request.workspace.as_str(),
            "--to",
            request.to.as_deref().unwrap_or(LOCAL),
            if confirm { "--confirm" } else { "--dry-run" },
            "--format",
            "json",
        ]
        .map(str::to_owned),
    );
    words
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

/// What a check read: the folder the move asks for first, and what the move would stop, `None`
/// when nothing runs. `Err` when the check did not answer with its document.
fn checked(output: Option<&CommandOutput>) -> Result<(Option<String>, Option<Stops>), ()> {
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

/// Where the moved Workspace is, the folder it started in, and its path there.
fn landing(result: &Value) -> Option<(ClientContextTarget, String, String)> {
    let created = &result["created"];
    let session_id = SessionId::parse(created["sessionId"].as_str()?).ok()?;
    let workspace_id = WorkbenchWorkspaceId::parse(created["workspaceId"].as_str()?).ok()?;
    let terminal_context_id =
        WorkbenchTerminalContextId::parse(created["terminalContextId"].as_str()?).ok()?;
    let target = ClientContextTarget {
        session_id,
        location: ClientContextLocation::Workspace {
            workspace_id,
            terminal_context_id,
        },
    };
    Some((
        target,
        result["folder"].as_str()?.to_owned(),
        created["path"].as_str()?.to_owned(),
    ))
}
