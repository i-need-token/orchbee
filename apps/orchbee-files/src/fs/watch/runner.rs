//! The service thread: it reads each Workspace's subscribers, loaded folders and opened files from
//! the process's shared state, hands them, the other parts' inputs and the clock to the service,
//! and carries out what the service decides. Root facts, existence checks and the checks of
//! missing paths run on the process's helpers and answer as inputs, so the thread waits on a disk
//! only once, for the home directory's real location when it starts.

use std::{
    collections::HashMap,
    io,
    sync::{
        Arc,
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    thread,
    time::Instant,
};

use orchbee_files_protocol::{ConnectionId, FilesNotice, FsUpdate, Operation, WorkspaceId};
use orchbee_local_runtime::DataRoot;

use super::{
    Input, Loads, States,
    native::Adapter,
    service::{Core, Effects, Native, Notice, Repository, Timing},
    stream::{Holder, Interest},
};
use crate::{
    blocking::Helpers,
    fs::background,
    scope::{
        Scopes,
        path::{self, NativePath},
    },
    subscriptions::Subscriptions,
};

/// The shared state the service reads and the parts that carry out its effects.
pub(super) struct Context {
    pub(super) helpers: Helpers,
    pub(super) scopes: Arc<Scopes>,
    pub(super) subscriptions: Arc<Subscriptions>,
    pub(super) loads: Loads,
    pub(super) states: States,
}

/// Starts the service thread; `None` when no thread can start, which leaves every Workspace
/// unwatched.
pub(super) fn start(context: Arc<Context>, timing: Timing) -> Option<Sender<Input>> {
    let (inputs, received) = mpsc::channel();
    let own = inputs.clone();
    thread::Builder::new()
        .name("orchbee-files-watch".into())
        .spawn(move || run(&context, timing, &received, &own))
        .ok()?;
    Some(inputs)
}

fn run(context: &Context, timing: Timing, received: &Receiver<Input>, inputs: &Sender<Input>) {
    let mut core = Core::new(timing, homes());
    let mut adapter = None;
    loop {
        let input = match core.due() {
            Some(due) => {
                match received.recv_timeout(due.saturating_duration_since(Instant::now())) {
                    Ok(input) => Some(input),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            None => match received.recv() {
                Ok(input) => Some(input),
                Err(_) => return,
            },
        };
        let now = Instant::now();
        match input {
            None => {}
            // Dropping the adapter stops the native watcher.
            Some(Input::Stop) => return,
            Some(Input::Changed(workspace)) => {
                let workspaces = workspace.map_or_else(|| core.workspaces(), |one| vec![one]);
                for workspace in workspaces {
                    let generation = context.scopes.generation(workspace);
                    core.interest(workspace, interest(context, workspace), generation);
                }
            }
            Some(Input::Native(native)) => core.native(native, now),
            Some(Input::Root { workspace, fact }) => core.root(workspace, fact, now),
            Some(Input::Resolved { workspace, files }) => core.resolved(workspace, files, now),
            Some(Input::Polled {
                path,
                exists,
                files,
            }) => core.polled(&path, exists, files, now),
            Some(Input::WatchRepository {
                workspace,
                tree,
                git_dir,
                report,
            }) => core.watch_repository(
                workspace,
                Repository {
                    tree,
                    git_dir,
                    report,
                },
            ),
            Some(Input::UnwatchRepository { workspace, tree }) => {
                core.unwatch_repository(workspace, &tree);
            }
        }
        core.tick(now);
        carry_out(context, inputs, &mut adapter, core.effects());
    }
}

/// What `workspace`'s subscribers loaded and opened.
fn interest(context: &Context, workspace: WorkspaceId) -> Interest {
    let connections = context.subscriptions.connections_in(workspace);
    let mut holders: HashMap<ConnectionId, Holder> = connections
        .iter()
        .map(|connection| (*connection, Holder::default()))
        .collect();
    for (connection, loaded) in context.loads.of(workspace, &connections) {
        if let Some(holder) = holders.get_mut(&connection) {
            holder.loaded = loaded.folders;
            holder.everything = loaded.everything;
        }
    }
    let mut opened = HashMap::new();
    for (connection, file) in context.scopes.opened_in(workspace) {
        if let Some(holder) = holders.get_mut(&connection) {
            holder.opened.insert(file.key());
            opened.insert(file.key(), file);
        }
    }
    Interest {
        root: None,
        holders,
        opened,
    }
}

fn carry_out(
    context: &Context,
    inputs: &Sender<Input>,
    adapter: &mut Option<Adapter>,
    effects: Effects,
) {
    for order in effects.orders {
        if adapter.is_none() && !order.watch.is_empty() {
            *adapter = Adapter::start(inputs.clone());
            if adapter.is_none() {
                let _ = inputs.send(Input::Native(Native::Failed(None)));
            }
        }
        if let Some(adapter) = adapter {
            adapter.apply(order);
        }
    }
    for workspace in effects.roots {
        let scopes = Arc::clone(&context.scopes);
        let answer = inputs.clone();
        background::run(
            &context.helpers,
            Operation::FsSubscribe,
            move || scopes.root(workspace, |root| Ok(path::real(&root))),
            move |fact| {
                let _ = answer.send(Input::Root {
                    workspace,
                    fact: fact.ok(),
                });
            },
        );
    }
    let mut checks: HashMap<WorkspaceId, Vec<NativePath>> = HashMap::new();
    for (workspace, file) in effects.checks {
        checks.entry(workspace).or_default().push(file);
    }
    for (workspace, files) in checks {
        let answer = inputs.clone();
        let unchecked = files.clone();
        background::run(
            &context.helpers,
            Operation::FsSubscribe,
            move || {
                files
                    .into_iter()
                    .map(|file| {
                        let exists = existence(&file);
                        (file, exists)
                    })
                    .collect::<Vec<_>>()
            },
            move |checked| {
                // A file whose check failed is reported changed, which its Client reads again.
                let files = checked.unwrap_or_else(|_| {
                    unchecked
                        .into_iter()
                        .map(|file| (file, Some(false)))
                        .collect()
                });
                let _ = answer.send(Input::Resolved { workspace, files });
            },
        );
    }
    for (path, files) in effects.polls {
        let answer = inputs.clone();
        let unchecked = path.clone();
        background::run(
            &context.helpers,
            Operation::FsSubscribe,
            move || {
                let exists = path.as_path().is_dir();
                let files = files
                    .into_iter()
                    .filter(|file| exists && existence(file).is_some())
                    .collect();
                Input::Polled {
                    path,
                    exists,
                    files,
                }
            },
            move |polled| {
                let _ = answer.send(polled.unwrap_or(Input::Polled {
                    path: unchecked,
                    exists: false,
                    files: Vec::new(),
                }));
            },
        );
    }
    for notice in effects.notices {
        deliver(context, notice);
    }
    for (report, change) in effects.reports {
        report.changed(change);
    }
}

fn deliver(context: &Context, notice: Notice) {
    match notice {
        Notice::To {
            workspace,
            connection,
            notice,
        } => context
            .subscriptions
            .deliver_to(workspace, connection, &notice),
        Notice::Root {
            workspace,
            generation,
            notice,
        } => context.subscriptions.deliver(workspace, &notice, || {
            context.scopes.generation(workspace) == generation
        }),
        Notice::Watch { workspace, state } => {
            let notice = FilesNotice::FsUpdate(FsUpdate::Watch { watch: state });
            context.subscriptions.deliver(workspace, &notice, || {
                context.states.set(workspace, state);
                true
            });
        }
    }
}

/// Whether `file` exists, as a directory or not; one that exists but cannot be read counts as a
/// file.
fn existence(file: &NativePath) -> Option<bool> {
    match std::fs::symlink_metadata(file.as_path()) {
        Ok(metadata) => Some(metadata.is_dir()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            None
        }
        Err(_) => Some(false),
    }
}

/// The home directory as the platform's home variable spells it and as its real location.
fn homes() -> Vec<NativePath> {
    let home = std::env::var_os(DataRoot::home_variable())
        .and_then(|home| home.into_string().ok())
        .and_then(|home| NativePath::parse(&home).ok());
    let Some(home) = home else {
        return Vec::new();
    };
    let real = path::real(&home).ok();
    std::iter::once(home).chain(real).collect()
}
