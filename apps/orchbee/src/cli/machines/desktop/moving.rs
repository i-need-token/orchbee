//! Change machine for the desktop window: runs the move its page asks for as `workspace
//! set-machine`, a child of this program on a background thread, as the TUI shell's mover does,
//! and reports it in the view: the word while a move runs, the question it asks before it stops a
//! program, how the last move ended, and where the last one landed. One move runs at a time.
//! `apps/orchbee/docs/machines.md#change-machine` owns the rules.

use crate::{
    desktop_machines::{DesktopMachines, move_notice, move_pending, move_question},
    localization::Locale,
    machine_change::{Notice, Stops},
    machine_move::{self, CHECK, MOVE, Refusal, Run},
    workbench_client::{ClientContextLocation, UiWakeup},
};
use serde_json::{Value, json};
use std::{io, time::Instant};

/// The move the page asked for: `workspace`, named `label`, from the machine whose Session holds
/// it to `to`; `None` names this computer.
pub(super) struct Request {
    pub(super) workspace: String,
    pub(super) label: String,
    pub(super) from: Option<String>,
    pub(super) to: Option<String>,
}

enum Stage {
    /// The child reads what the move would stop.
    Checking(Run),
    /// The person answers question `question` about what the move would stop.
    Asking { question: u64, stops: Stops },
    /// The child moves the Workspace.
    Moving(Run),
}

struct Move {
    request: Request,
    /// The name of the machine it goes to; empty for this computer.
    to_label: String,
    /// The folder relative to home the check named, which the move asks for first.
    folder: Option<String>,
    stage: Stage,
}

/// Where the last move put its Workspace, under the number the page says it once by.
struct Moved {
    number: u64,
    machine: Option<String>,
    workspace: String,
}

pub(super) struct DesktopMover {
    wakeup: UiWakeup,
    under_way: Option<Move>,
    /// The last number given to a question, a notice or a landing.
    number: u64,
    notice: Option<(u64, Notice)>,
    moved: Option<Moved>,
}

impl DesktopMover {
    pub(super) const fn new(wakeup: UiWakeup) -> Self {
        Self {
            wakeup,
            under_way: None,
            number: 0,
            notice: None,
            moved: None,
        }
    }

    /// Starts `request` with its check; a request while a move is under way takes none.
    pub(super) fn start(&mut self, request: Request, machines: &DesktopMachines) {
        if self.under_way.is_some() || request.from == request.to {
            return;
        }
        let to_label = request
            .to
            .as_deref()
            .map_or_else(String::new, |id| machines.machine_label(id));
        let arguments = machine_move::arguments(
            &request.workspace,
            request.from.as_deref(),
            request.to.as_deref(),
            false,
        );
        match Run::spawn(arguments, CHECK, self.wakeup.clone()) {
            Ok(run) => {
                self.under_way = Some(Move {
                    request,
                    to_label,
                    folder: None,
                    stage: Stage::Checking(run),
                });
            }
            Err(_) => self.note(Notice::Failed),
        }
    }

    /// The person's answer to question `question`; an answer to any other changes nothing.
    pub(super) fn answer(&mut self, question: u64, confirm: bool) {
        let asked = matches!(
            &self.under_way,
            Some(Move { stage: Stage::Asking { question: asked, .. }, .. }) if *asked == question
        );
        if !asked {
            return;
        }
        if confirm {
            self.run_move();
        } else {
            self.under_way = None;
        }
    }

    /// One pass over the move under way.
    pub(super) fn tick(&mut self, machines: &DesktopMachines) {
        let Some(moving) = &self.under_way else {
            return;
        };
        match &moving.stage {
            Stage::Asking { .. } => {}
            Stage::Checking(run) => {
                let Some(output) = run.settled() else {
                    return;
                };
                match machine_move::checked(output.as_ref()) {
                    Err(()) => {
                        let notice = self.refusal(output.as_ref(), false, machines);
                        self.under_way = None;
                        self.note(notice);
                    }
                    Ok((folder, stops)) => {
                        if let Some(moving) = &mut self.under_way {
                            moving.folder = folder;
                        }
                        match stops {
                            None => self.run_move(),
                            Some(stops) => {
                                self.number += 1;
                                if let Some(moving) = &mut self.under_way {
                                    moving.stage = Stage::Asking {
                                        question: self.number,
                                        stops,
                                    };
                                }
                            }
                        }
                    }
                }
            }
            Stage::Moving(run) => {
                let Some(output) = run.settled() else {
                    return;
                };
                let landed = machine_move::landed(output.as_ref());
                let notice = match &landed {
                    Err(()) => Some(self.refusal(output.as_ref(), true, machines)),
                    Ok(None) => None,
                    Ok(Some(landing)) => {
                        machine_move::missing_folder(moving.folder.as_deref(), landing).map(
                            |folder| Notice::FolderMissing {
                                machine: moving.to_label.clone(),
                                folder,
                            },
                        )
                    }
                };
                if let Ok(Some(landing)) = landed
                    && let ClientContextLocation::Workspace { workspace_id, .. } =
                        &landing.target.location
                {
                    self.number += 1;
                    self.moved = Some(Moved {
                        number: self.number,
                        machine: moving.request.to.clone(),
                        workspace: workspace_id.as_str().to_owned(),
                    });
                }
                self.under_way = None;
                if let Some(notice) = notice {
                    self.note(notice);
                }
            }
        }
    }

    /// The view's `move`.
    pub(super) fn project(&self, locale: Locale) -> Value {
        let (pending, question) = match &self.under_way {
            None => (None, Value::Null),
            Some(Move {
                stage: Stage::Asking { question, stops },
                request,
                to_label,
                ..
            }) => (
                None,
                move_question(locale, *question, &request.label, to_label, stops),
            ),
            Some(_) => (Some(move_pending(locale)), Value::Null),
        };
        json!({
            "pending": pending,
            "question": question,
            "notice": self.notice.as_ref().map(|(number, notice)| json!({
                "number": number,
                "text": move_notice(locale, notice),
            })),
            "moved": self.moved.as_ref().map(|moved| json!({
                "number": moved.number,
                "machine": moved.machine,
                "workspace": moved.workspace,
            })),
        })
    }

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

    pub(super) fn stop_before(&mut self, deadline: Instant) -> io::Result<()> {
        match &mut self.under_way {
            Some(Move {
                stage: Stage::Checking(run) | Stage::Moving(run),
                ..
            }) => run.join_before(deadline),
            _ => Ok(()),
        }
    }

    /// Runs the move itself, now that nothing it would stop needs an answer.
    fn run_move(&mut self) {
        let Some(mut moving) = self.under_way.take() else {
            return;
        };
        let arguments = machine_move::arguments(
            &moving.request.workspace,
            moving.request.from.as_deref(),
            moving.request.to.as_deref(),
            true,
        );
        match Run::spawn(arguments, MOVE, self.wakeup.clone()) {
            Ok(run) => {
                moving.stage = Stage::Moving(run);
                self.under_way = Some(moving);
            }
            Err(_) => self.note(Notice::Failed),
        }
    }

    fn note(&mut self, notice: Notice) {
        self.number += 1;
        self.notice = Some((self.number, notice));
    }

    /// What a run that did not succeed leaves the person with, worded with the saved machines'
    /// names.
    fn refusal(
        &self,
        output: Option<&orchbee_local_runtime::CommandOutput>,
        moved: bool,
        machines: &DesktopMachines,
    ) -> Notice {
        let to_label = || {
            self.under_way
                .as_ref()
                .map(|moving| moving.to_label.clone())
                .unwrap_or_default()
        };
        match machine_move::refusal(output, moved) {
            Refusal::CloseFailed => Notice::CloseFailed {
                machine: to_label(),
            },
            Refusal::NotConnected(id) => Notice::NotConnected {
                machine: id.map_or_else(to_label, |id| machines.machine_label(&id)),
            },
            Refusal::Worktree => Notice::Worktree,
            Refusal::Failed => Notice::Failed,
            Refusal::Unknown => Notice::Unknown,
        }
    }
}
