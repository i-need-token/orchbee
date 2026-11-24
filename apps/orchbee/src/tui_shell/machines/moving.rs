//! Change machine, the machine manager's side: hands the action menu the facts its list shows,
//! runs the move the person chose as `workspace set-machine`, a child of this program on a
//! background thread, and lands the view on the moved Workspace. The child first only reads what
//! the move would stop; the picker asks when something may be running, and the move runs once the
//! person confirms, or at once when nothing runs. One move runs at a time, and nothing it waits for
//! holds the shell's tick. `apps/orchbee/docs/machines.md#change-machine` owns the rules.

use super::{Manager, SurfaceSlot};
use crate::machine_move::{self, CHECK, MOVE, Refusal, Run};
use crate::{
    client_view::ClientView,
    machine_change::{Facts, Notice, Question, Request, Stops},
    machine_picker::Choice,
    machines::ProfileId,
    workbench_client::ClientStartupTarget,
};
use orchbee_local_runtime::CommandOutput;
use std::{
    io,
    time::{Duration, Instant},
};

/// How often the list's facts refresh while no menu offers Change machine.
const FACTS: Duration = Duration::from_secs(1);

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
            run.stop();
        }
    }

    /// Process exit: observes the run against the shell's deadline.
    pub(super) fn stop_before(&mut self, deadline: Instant) -> io::Result<()> {
        match &mut self.under_way {
            Some(Move {
                stage: Stage::Checking(run) | Stage::Moving(run),
                ..
            }) => run.join_before(deadline),
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
                Some(output) => match machine_move::checked(output.as_ref()) {
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
        let Ok(landing) = machine_move::landed(output) else {
            return Some(self.refusal(output, moving, true));
        };
        // It moved; a landing that cannot be read leaves the view where it is.
        let landing = landing?;
        let _selected = view.machines.select_moved(
            moving.to.clone(),
            ClientStartupTarget::Context(landing.target.clone()),
        );
        machine_move::missing_folder(moving.folder.as_deref(), &landing).map(|folder| {
            Notice::FolderMissing {
                machine: moving.request.to_label.clone(),
                folder,
            }
        })
    }

    /// What a run that did not succeed leaves the person with, worded with the saved machines'
    /// names.
    fn refusal(&self, output: Option<&CommandOutput>, moving: &Move, moved: bool) -> Notice {
        match machine_move::refusal(output, moved) {
            Refusal::CloseFailed => Notice::CloseFailed {
                machine: moving.request.to_label.clone(),
            },
            Refusal::NotConnected(id) => Notice::NotConnected {
                machine: id
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
            Refusal::Worktree => Notice::Worktree,
            Refusal::Failed => Notice::Failed,
            Refusal::Unknown => Notice::Unknown,
        }
    }

    fn spawn(&self, arguments: Vec<String>, bound: Duration) -> io::Result<Run> {
        Run::spawn(arguments, bound, self.wakeup.clone())
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
    machine_move::arguments(
        &request.workspace,
        from.map(ProfileId::as_str),
        request.to.as_deref(),
        confirm,
    )
}
