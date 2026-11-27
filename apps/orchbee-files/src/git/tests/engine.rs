//! The engine on a temporary repository: a subscription follows discovery, status and a write, and
//! outlives its Workspace closing; a repeated request identity answers its first outcome without
//! running again; and fetches queued behind a write share one run when they would add nothing to
//! each other.

use super::{
    super::{
        engine::{Call, Done, Ended, Git, Mutation, Reply, ScmSummary, Sink},
        errors::GitError,
        ports::{Members, NoTrash, NoWatches},
        remote::{Fetch, FetchRemotes},
    },
    fixture::Fixture,
};
use crate::requests::Outcome;
use orchbee_files_protocol::{
    ConnectionId, RepositoryKind, RequestId, WorkspaceId, WorkspaceScope,
};
use std::{
    fs,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::AtomicBool,
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

const WAIT: Duration = Duration::from_secs(60);

/// Members go nowhere: these tests read no path through a scope.
struct Unscoped;

impl Members for Unscoped {
    fn state(&mut self, _: WorkspaceId, _: &[String]) {}
}

/// A subscription that keeps every summary it was sent.
struct Recorded(Arc<Mutex<Vec<ScmSummary>>>);

impl Sink<ScmSummary> for Recorded {
    fn send(&mut self, frame: &ScmSummary) {
        self.0.lock().unwrap().push(frame.clone());
    }

    fn ended(&self) -> bool {
        false
    }
}

fn reply<T: Send + 'static>() -> (Reply<T>, Receiver<Result<T, GitError>>) {
    let (send, received) = mpsc::channel();
    (
        Box::new(move |answer| {
            let _ = send.send(answer);
        }),
        received,
    )
}

/// Waits until the latest summary satisfies `condition`, and returns it.
fn summary(
    summaries: &Mutex<Vec<ScmSummary>>,
    condition: impl Fn(&ScmSummary) -> bool,
) -> ScmSummary {
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(last) = summaries
            .lock()
            .unwrap()
            .last()
            .filter(|last| condition(last))
        {
            return last.clone();
        }
        assert!(Instant::now() < deadline, "no summary as expected");
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn writes_queue_per_repository_answer_once_per_identity_and_share_fetches() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.path("remote.git")).unwrap();
    fixture.git("remote.git", &["init", "-q", "--bare", "-b", "main"]);
    fixture.repository("repo");
    fixture.write("repo/seed", "seed");
    fixture.git("repo", &["add", "seed"]);
    fixture.git("repo", &["commit", "-q", "-m", "seed"]);
    fixture.git(
        "repo",
        &["remote", "add", "origin", &fixture.path("remote.git")],
    );
    fixture.git("repo", &["push", "-q", "-u", "origin", "main"]);

    let git = Git::start(
        None,
        Arc::new(NoTrash),
        Box::new(NoWatches),
        Box::new(Unscoped),
    )
    .unwrap();
    let workspace = WorkspaceId::from_bytes([1; 16]).unwrap();
    let connection = ConnectionId::new(7).unwrap();
    git.scope(workspace, WorkspaceScope::Root(fixture.path("repo")));
    let summaries = Arc::new(Mutex::new(Vec::new()));
    git.subscribe(
        workspace,
        connection,
        Box::new(Recorded(Arc::clone(&summaries))),
    );
    let first = summary(&summaries, |last| {
        last.summary
            .repositories
            .first()
            .is_some_and(|shown| shown.revision >= 1)
    });
    let shown = &first.summary.repositories[0];
    assert_eq!((shown.kind, shown.warm), (RepositoryKind::Root, true));
    let repository = shown.repository.clone();

    let call = |request: u128| Call {
        workspace,
        connection,
        request: RequestId::from_bytes(request.to_be_bytes()).unwrap(),
        cancel: Arc::new(AtomicBool::new(false)),
    };
    let mutate = |request: u128, mutation: Mutation| {
        let (answer, answered) = reply();
        git.mutate(call(request), repository.clone(), mutation, answer);
        answered
    };
    let added = fixture.path("repo/added");
    fixture.write("repo/added", "added");
    let stage = || Mutation::Stage(vec![added.clone()]);
    assert_eq!(
        mutate(1, stage()).recv_timeout(WAIT).unwrap(),
        Ok(Done::Empty)
    );
    summary(&summaries, |last| {
        last.summary.repositories[0].counts.index == 1
            && last.summary.repositories[0].busy.is_none()
    });

    // The repeat answers what the first run did, without staging again.
    fixture.git("repo", &["reset", "-q"]);
    assert_eq!(
        mutate(1, stage()).recv_timeout(WAIT).unwrap(),
        Ok(Done::Empty)
    );
    assert_eq!(
        fixture.git("repo", &["diff", "--cached", "--name-only"]),
        ""
    );
    let (answer, answered) = reply::<Outcome<Ended>>();
    git.outcome(
        workspace,
        RequestId::from_bytes(1u128.to_be_bytes()).unwrap(),
        answer,
    );
    assert_eq!(
        answered.recv_timeout(WAIT).unwrap(),
        Ok(Outcome::Completed(Ok(Done::Empty)))
    );

    // Behind a write that waits out another process's lock, a background fetch joins the
    // foreground fetch queued before it.
    let lock = Path::new(&repository).join(".git").join("index.lock");
    fs::write(&lock, "").unwrap();
    let fetch = |background| Mutation::Fetch {
        fetch: Fetch {
            remotes: FetchRemotes::Default,
            prune: false,
        },
        background,
    };
    let staged = mutate(2, stage());
    let foreground = mutate(3, fetch(false));
    let background = mutate(4, fetch(true));
    thread::sleep(Duration::from_millis(300));
    fs::remove_file(&lock).unwrap();
    for answered in [staged, foreground, background] {
        assert_eq!(answered.recv_timeout(WAIT).unwrap(), Ok(Done::Empty));
    }
    let (answer, answered) = reply();
    git.output(workspace, None, answer);
    let output = answered.recv_timeout(WAIT).unwrap().unwrap();
    assert_eq!(output.text.matches(" fetch --progress").count(), 1);

    // A closed Workspace's subscription stays open showing no repository, and shows it again
    // once the Workspace opens again.
    git.scope(workspace, WorkspaceScope::Closed);
    summary(&summaries, |last| last.summary.repositories.is_empty());
    git.scope(workspace, WorkspaceScope::Root(fixture.path("repo")));
    summary(&summaries, |last| {
        last.summary
            .repositories
            .first()
            .is_some_and(|shown| shown.revision >= 1 && shown.counts.index == 1)
    });
    drop(git);
}
