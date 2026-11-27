//! The reads answer as VS Code's Git extension reads: only a positive pattern ignores a path,
//! references carry their upstream until it is gone and tags their peeled commit, remotes take
//! their first URLs, and reference pages stay within their bounds. A show reads each version
//! through the repository's textconv filter, and a reference that moved reads its new content.

use super::{
    super::{
        command,
        discovery::Found,
        reads::{RefUpstream, Remote, check_ignore, refs, user_config},
        show::{self, Reference, Shows},
    },
    fixture::Fixture,
};
use orchbee_files_protocol::{ErrorCode, RefKind};
use std::{collections::HashSet, fs, path::Path, sync::Mutex};

fn found(fixture: &Fixture, relative: &str) -> Found {
    let toplevel = fixture.path(relative);
    Found {
        git_dir: Path::new(&toplevel).join(".git"),
        toplevel,
        safe_directory: None,
    }
}

fn version(fixture: &Fixture) -> (u32, u32, u32) {
    command::version(fixture.git("", &["--version"]).as_bytes()).unwrap()
}

#[test]
fn check_ignore_answers_only_what_a_positive_pattern_ignores() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    let repository = found(&fixture, "repo");
    fixture.write("repo/.gitignore", "*.log\n!keep.log\nbuild/\n");
    fixture.write("repo/build/out.txt", "");
    let paths = ["x.log", "keep.log", "build/out.txt", "src.rs"]
        .map(|name| fixture.path(&format!("repo/{name}")));
    let mut asked = paths.to_vec();
    asked.push(fixture.path("other.log"));

    let ignored = check_ignore(&repository, &asked, &fixture.limits(), &fixture.output).unwrap();
    assert_eq!(ignored, [paths[0].clone(), paths[2].clone()]);
    let none = check_ignore(
        &repository,
        &paths[1..2],
        &fixture.limits(),
        &fixture.output,
    );
    assert_eq!(none.unwrap(), Vec::<String>::new());
}

#[test]
fn refs_carry_upstreams_until_gone_peeled_tags_and_first_remote_urls() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    let repository = found(&fixture, "repo");
    fs::create_dir(fixture.path("remote.git")).unwrap();
    fixture.git("remote.git", &["init", "-q", "--bare"]);
    fixture.write("repo/file", "one");
    fixture.git("repo", &["add", "file"]);
    fixture.git("repo", &["commit", "-q", "-m", "seed"]);
    let head = fixture
        .git("repo", &["rev-parse", "HEAD"])
        .trim()
        .to_owned();
    let remote = fixture.path("remote.git");
    fixture.git("repo", &["remote", "add", "origin", &remote]);
    fixture.git("repo", &["push", "-q", "-u", "origin", "main"]);
    fixture.git("repo", &["branch", "topic"]);
    fixture.git("repo", &["push", "-q", "-u", "origin", "topic"]);
    fixture.git("repo", &["push", "-q", "origin", "--delete", "topic"]);
    fixture.git("repo", &["tag", "-a", "-m", "release", "v1"]);
    fixture.git(
        "repo",
        &["config", "--add", "remote.origin.pushurl", "first"],
    );
    fixture.git(
        "repo",
        &["config", "--add", "remote.origin.pushurl", "second"],
    );

    let page = refs(
        &repository,
        version(&fixture),
        None,
        &fixture.limits(),
        &fixture.output,
    )
    .unwrap();
    let listed = page
        .refs
        .iter()
        .map(|listed| (listed.name.as_str(), listed.kind, listed.upstream.clone()))
        .collect::<Vec<_>>();
    let tracking = RefUpstream {
        remote: "origin".to_owned(),
        name: "main".to_owned(),
    };
    assert_eq!(
        listed,
        [
            ("main", RefKind::Head, Some(tracking)),
            ("topic", RefKind::Head, None),
            ("origin/main", RefKind::RemoteHead, None),
            ("v1", RefKind::Tag, None),
        ]
    );
    assert!(page.refs.iter().all(|listed| listed.commit == head));
    assert_eq!(
        page.remotes,
        [Remote {
            name: "origin".to_owned(),
            fetch_url: Some(remote),
            push_url: Some("first".to_owned()),
        }]
    );
    assert!(page.cursor.is_none());

    let identity = user_config(&repository, &fixture.limits(), &fixture.output).unwrap();
    assert_eq!(identity.name.as_deref(), Some("Orchbee Fixture"));
}

#[test]
fn reference_pages_cover_every_reference_once_within_their_bounds() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    let repository = found(&fixture, "repo");
    fixture.write("repo/file", "one");
    fixture.git("repo", &["add", "file"]);
    fixture.git("repo", &["commit", "-q", "-m", "seed"]);
    let head = fixture
        .git("repo", &["rev-parse", "HEAD"])
        .trim()
        .to_owned();
    let packed = (0..2500)
        .map(|index| {
            format!(
                "{head} refs/tags/t{index:05}
"
            )
        })
        .collect::<String>();
    fs::write(repository.git_dir.join("packed-refs"), packed).unwrap();

    let version = version(&fixture);
    let mut seen = HashSet::new();
    let mut cursor = None;
    let mut pages = 0;
    loop {
        let page = refs(
            &repository,
            version,
            cursor,
            &fixture.limits(),
            &fixture.output,
        )
        .unwrap();
        assert!(page.refs.len() <= 2000);
        assert!(page.remotes.is_empty());
        pages += 1;
        for listed in page.refs {
            assert!(seen.insert(listed.name));
        }
        match page.cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(seen.len(), 2501);
    assert!(pages > 1);
}

#[test]
fn a_show_reads_each_version_through_textconv_and_follows_its_reference() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    let repository = found(&fixture, "repo");
    fixture.git("repo", &["config", "diff.upper.textconv", "tr a-z A-Z <"]);
    fixture.write("repo/.gitattributes", "*.txt diff=upper\n");
    fixture.write("repo/file.txt", "committed\n");
    fixture.git("repo", &["add", "."]);
    fixture.git("repo", &["commit", "-q", "-m", "first"]);
    let first = fixture
        .git("repo", &["rev-parse", "HEAD"])
        .trim()
        .to_owned();
    fixture.write("repo/file.txt", "staged\n");
    fixture.git("repo", &["add", "file.txt"]);

    let shows = Mutex::new(Shows::new());
    let show = |reference: Reference, name: &str, limit: u64| {
        let path = fixture.path(&format!("repo/{name}"));
        let limits = fixture.limits();
        show::content(
            &repository,
            &reference,
            &path,
            &shows,
            limit,
            &limits,
            &fixture.output,
        )
        .map(|content| (*content).as_ref().to_vec())
        .map_err(|error| error.code)
    };
    assert_eq!(
        show(Reference::Head, "file.txt", 64).unwrap(),
        b"COMMITTED\n"
    );
    assert_eq!(
        show(Reference::Index, "file.txt", 6),
        Err(ErrorCode::TooLarge)
    );
    assert_eq!(show(Reference::Index, "file.txt", 64).unwrap(), b"STAGED\n");
    let at_first = Reference::Commit(first);
    assert_eq!(
        show(at_first.clone(), "file.txt", 64).unwrap(),
        b"COMMITTED\n"
    );
    assert_eq!(
        show(Reference::Head, "missing.txt", 64),
        Err(ErrorCode::NotFound)
    );

    fixture.git("repo", &["commit", "-q", "-m", "second"]);
    assert_eq!(show(Reference::Head, "file.txt", 64).unwrap(), b"STAGED\n");
    assert_eq!(show(at_first, "file.txt", 64).unwrap(), b"COMMITTED\n");
}
