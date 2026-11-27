//! Discovery finds the root's repository, the repositories one level below it, and the repository
//! above a root inside one, which opens only with consent; only a repository holding the root is
//! trusted for its owner.

use super::{super::discovery::discover, fixture::Fixture};
use std::sync::{Arc, atomic::AtomicBool};

#[test]
fn discovery_finds_the_root_nested_and_parent_repositories() {
    let fixture = Fixture::new();
    let root = fixture.repository("root");
    let nested = fixture.repository("root/nested");
    fixture.repository("root/node_modules");
    fixture.repository("root/deeper/below");
    fixture.write("root/plain/file", "");
    let cancel = Arc::new(AtomicBool::new(false));

    let discovered = discover(&root, &cancel, &fixture.output);
    let found = discovered.root.unwrap();
    assert_eq!(found.toplevel, root);
    assert!(found.safe_directory.is_some());
    assert_eq!(
        discovered
            .nested
            .iter()
            .map(|found| (found.toplevel.as_str(), found.safe_directory.is_some()))
            .collect::<Vec<_>>(),
        [(nested.as_str(), false)]
    );
    assert!(discovered.parent.is_none() && discovered.failure.is_none());

    let inside = fixture.path("root/plain");
    let discovered = discover(&inside, &cancel, &fixture.output);
    assert!(discovered.root.is_none() && discovered.nested.is_empty());
    let parent = discovered.parent.unwrap();
    assert_eq!(parent.toplevel, root);
    assert!(parent.safe_directory.is_some());

    let outside = fixture.path("");
    let discovered = discover(&outside, &cancel, &fixture.output);
    assert!(discovered.root.is_none() && discovered.parent.is_none());
    assert_eq!(discovered.nested.len(), 1);

    let missing = fixture.path("missing");
    let discovered = discover(&missing, &cancel, &fixture.output);
    assert!(discovered.root.is_none() && discovered.nested.is_empty());
}
