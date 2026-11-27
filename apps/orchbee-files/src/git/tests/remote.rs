//! The remote operations against a local bare repository: publishing sets the upstream, a plain
//! push and pull follow it, a sync rebases and pushes what stays ahead, and a sync without an
//! upstream asks for publishing instead.

use super::{
    super::{
        command,
        errors::GitErrorCode,
        mutations::Context,
        remote::{Fetch, FetchRemotes, Push, fetch, pull, push, sync},
        runner::Limits,
    },
    fixture::Fixture,
};
use orchbee_files_protocol::{ErrorCode, PushForce};
use std::fs;

fn context<'a>(fixture: &'a Fixture, limits: &'a Limits) -> Context<'a> {
    Context {
        limits,
        output: &fixture.output,
        relay: None,
    }
}

fn commit(fixture: &Fixture, relative: &str, name: &str) {
    fixture.write(&format!("{relative}/{name}"), name);
    fixture.git(relative, &["add", name]);
    fixture.git(relative, &["commit", "-q", "-m", name]);
}

fn head(fixture: &Fixture, relative: &str, reference: &str) -> String {
    fixture
        .git(relative, &["rev-parse", reference])
        .trim()
        .to_owned()
}

#[test]
fn publishing_pushing_pulling_and_syncing_follow_the_upstream() {
    let fixture = Fixture::new();
    let version = command::version(fixture.git("", &["--version"]).as_bytes()).unwrap();
    let limits = fixture.limits();
    let context = context(&fixture, &limits);
    fs::create_dir(fixture.path("remote.git")).unwrap();
    fixture.git("remote.git", &["init", "-q", "--bare", "-b", "main"]);
    let remote = fixture.path("remote.git");
    fixture.repository("one");
    let one = fixture.found("one");
    fixture.git("one", &["remote", "add", "origin", &remote]);
    commit(&fixture, "one", "first");

    let plain = Push {
        remote: None,
        branch: None,
        set_upstream: false,
        force: PushForce::None,
        tags: false,
    };
    let unpublished = sync(&one, false, version, &context).unwrap_err();
    assert_eq!(
        unpublished.git_error_code,
        Some(GitErrorCode::NoUpstreamBranch)
    );
    let publish = Push {
        remote: Some("origin".to_owned()),
        branch: Some("main".to_owned()),
        set_upstream: true,
        ..plain.clone()
    };
    push(&one, &publish, version, &context).unwrap();
    assert_eq!(
        head(&fixture, "remote.git", "main"),
        head(&fixture, "one", "HEAD")
    );
    assert_eq!(
        head(&fixture, "one", "@{upstream}"),
        head(&fixture, "one", "HEAD")
    );

    fixture.git("", &["clone", "-q", &remote, "two"]);
    fixture.configure("two");
    let two = fixture.found("two");
    commit(&fixture, "two", "second");
    push(&two, &plain, version, &context).unwrap();
    let fetched = Fetch {
        remotes: FetchRemotes::Remote("origin".to_owned()),
        prune: true,
    };
    fetch(&one, &fetched, &context).unwrap();
    assert_eq!(
        head(&fixture, "one", "origin/main"),
        head(&fixture, "two", "HEAD")
    );
    pull(&one, false, version, &context).unwrap();
    assert_eq!(head(&fixture, "one", "HEAD"), head(&fixture, "two", "HEAD"));

    commit(&fixture, "one", "local");
    commit(&fixture, "two", "remote");
    push(&two, &plain, version, &context).unwrap();
    sync(&one, true, version, &context).unwrap();
    let pushed = head(&fixture, "remote.git", "main");
    assert_eq!(pushed, head(&fixture, "one", "HEAD"));
    assert_eq!(
        fixture.git("one", &["log", "--format=%s", "-2"]),
        "local\nremote\n"
    );
    sync(&one, false, version, &context).unwrap();
    assert_eq!(head(&fixture, "remote.git", "main"), pushed);

    let named = Push {
        remote: Some("--upload-pack=x".to_owned()),
        ..plain
    };
    assert_eq!(
        push(&one, &named, version, &context).unwrap_err().code,
        ErrorCode::InvalidRequest
    );
}
