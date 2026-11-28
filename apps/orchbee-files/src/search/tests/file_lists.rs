//! Quick Open's file lists against a scripted rg: what a listing keeps of rg's output, nested
//! folders listed on their own, the run again without following links, and the lists' cache:
//! made once, replaced by another request, dropped with its cache, scope or connection, at most
//! two per Workspace, and forgotten when it fails.

use std::{
    fs,
    sync::{Arc, atomic::AtomicBool},
    thread,
};

use orchbee_files_protocol::{CacheId, ConnectionId, ErrorCode, WorkspaceId};

use super::{
    super::{
        file_list::FileList,
        js,
        listing::{self, Request},
        lists::{Key, Lists},
        rg::Ended,
        slots::Slots,
    },
    script::{Run, Script, folder, listed},
};
use crate::{fs::tests::Scratch, scope::path::NativePath};

fn request(folders: Vec<NativePath>) -> Request {
    Request {
        folders,
        includes: Vec::new(),
        excludes: Vec::new(),
        use_ignore_files: true,
        use_parent_ignore_files: false,
        follow_symlinks: false,
    }
}

/// Each listed file: the index of its folder among `folders` and its path, segments separated by
/// `/`.
fn files(list: &FileList, folders: &[NativePath]) -> Vec<(usize, String)> {
    (0..list.len())
        .map(|index| {
            let file = list.file(index);
            let folder = folders
                .iter()
                .position(|folder| *folder == file.folder.path);
            (folder.unwrap(), js::text(file.path).replace('\\', "/"))
        })
        .collect()
}

fn list(request: &Request, script: &Script) -> FileList {
    listing::list(request, script, &Slots::new(), &AtomicBool::new(false)).unwrap()
}

#[test]
fn a_listing_rechecks_includes_and_adds_what_exceptions_reinclude() {
    let scratch = Scratch::new("search-listing");
    let mut request = request(vec![folder(scratch.path())]);
    request.includes = vec!["**/*.ts".to_owned()];
    request.excludes = vec!["**/gen/**".to_owned(), "!**/gen/keep/**".to_owned()];
    let script = Script::new([
        Run::new().stdout(listed(&["a.ts", "b.js"])),
        Run::new().stdout(listed(&[
            "a.ts",
            "gen/x.ts",
            "gen/keep/k.ts",
            "gen/keep/k.js",
        ])),
    ]);
    let list = list(&request, &script);
    assert_eq!(
        files(&list, &request.folders),
        [(0, "a.ts".to_owned()), (0, "gen/keep/k.ts".to_owned())]
    );
    let seen = script.seen();
    assert!(!seen[0].names("gen/keep") && seen[1].names("gen/keep"));
}

#[test]
fn a_folder_nested_in_another_is_listed_on_its_own() {
    let scratch = Scratch::new("search-nested");
    fs::create_dir(scratch.path().join("sub")).unwrap();
    let request = request(vec![
        folder(scratch.path()),
        folder(&scratch.path().join("sub")),
    ]);
    let script = Script::new([
        Run::new().stdout(listed(&["a.ts"])),
        Run::new().stdout(listed(&["b.ts"])),
    ]);
    let list = list(&request, &script);
    assert_eq!(
        files(&list, &request.folders),
        [(0, "a.ts".to_owned()), (1, "b.ts".to_owned())]
    );
    let seen = script.seen();
    // The outer run leaves the nested folder out, anchored at its top as VS Code anchors it.
    assert!(seen[0].arguments.iter().any(|argument| argument == "!/sub"));
    assert_eq!(seen[1].folder, scratch.path().join("sub"));
}

#[test]
fn too_many_link_loops_list_the_folder_again_without_following_links() {
    let scratch = Scratch::new("search-listing-loops");
    let mut request = request(vec![folder(scratch.path())]);
    request.follow_symlinks = true;
    let loops = "rg: File system loop found: a/b points to an ancestor a\n".repeat(1001);
    let script = Script::new([
        Run::new().stdout(listed(&["a.ts"])).stderr(loops).hold(),
        Run::new().stdout(listed(&["a.ts", "b.ts"])),
    ]);
    let list = list(&request, &script);
    assert_eq!(
        files(&list, &request.folders),
        [(0, "a.ts".to_owned()), (0, "b.ts".to_owned())]
    );
    let seen = script.seen();
    assert_eq!(seen[0].ended, Ended::Stopped);
    assert!(seen[0].names("--follow") && !seen[1].names("--follow"));
}

fn key(cache: u64) -> Key {
    Key {
        connection: ConnectionId::new(1).unwrap(),
        workspace: WorkspaceId::from_bytes([1; 16]).unwrap(),
        cache: CacheId::new(cache).unwrap(),
    }
}

fn lists(script: &Arc<Script>) -> Lists {
    Lists::new(Arc::clone(script) as _, Arc::new(Slots::new()))
}

#[test]
fn a_cache_lists_once_until_a_query_asks_for_another_list() {
    let scratch = Scratch::new("search-cache");
    let script = Arc::new(Script::standing(Run::new().stdout(listed(&["a.ts"]))));
    let lists = lists(&script);
    let request = request(vec![folder(scratch.path())]);
    let first = lists.get(key(1), &request, || false).unwrap();
    let again = lists.get(key(1), &request, || false).unwrap();
    assert!(Arc::ptr_eq(&first, &again));
    assert_eq!(script.seen().len(), 1);
    let mut other = request.clone();
    other.includes = vec!["**/*.rs".to_owned()];
    lists.get(key(1), &other, || false).unwrap();
    assert_eq!(script.seen().len(), 2);
}

#[test]
fn a_list_that_goes_stops_its_listing_and_cancels_its_queries() {
    let scratch = Scratch::new("search-cache-gone");
    let request = Arc::new(request(vec![folder(scratch.path())]));
    let ends: [fn(&Lists); 3] = [
        |lists| lists.close(key(1)),
        |lists| lists.scope_changed(key(1).workspace),
        |lists| lists.connection_lost(key(1).connection),
    ];
    for end in ends {
        let script = Arc::new(Script::standing(Run::new().hold()));
        let lists = Arc::new(lists(&script));
        let query = {
            let (lists, request) = (Arc::clone(&lists), Arc::clone(&request));
            thread::spawn(move || lists.get(key(1), &request, || false).map(|_| ()))
        };
        script.wait_for(|script| script.running().len() == 1);
        end(&lists);
        let error = query.join().unwrap().unwrap_err();
        assert_eq!(error.code, ErrorCode::Cancelled);
        script.wait_for(|script| script.seen().len() == 1);
        assert_eq!(script.seen()[0].ended, Ended::Stopped);
    }
}

#[test]
fn a_workspace_keeps_its_two_most_recent_lists() {
    let scratch = Scratch::new("search-cache-bound");
    let script = Arc::new(Script::standing(Run::new().stdout(listed(&["a.ts"]))));
    let lists = lists(&script);
    let request = request(vec![folder(scratch.path())]);
    for cache in [1, 2, 3, 2] {
        lists.get(key(cache), &request, || false).unwrap();
    }
    assert_eq!(script.seen().len(), 3);
    // The first list went for the third; listing it again drops the least recent, the third.
    lists.get(key(1), &request, || false).unwrap();
    lists.get(key(2), &request, || false).unwrap();
    assert_eq!(script.seen().len(), 4);
}

#[test]
fn a_list_that_failed_is_listed_again() {
    let scratch = Scratch::new("search-cache-failed");
    let script = Arc::new(Script::with(
        [Run::new().ending(Ended::Unavailable)],
        Run::new().stdout(listed(&["a.ts"])),
    ));
    let lists = lists(&script);
    let request = request(vec![folder(scratch.path())]);
    let error = lists.get(key(1), &request, || false).err().unwrap();
    assert_eq!(error.code, ErrorCode::Unavailable);
    let list = lists.get(key(1), &request, || false).unwrap();
    assert_eq!(files(&list, &request.folders), [(0, "a.ts".to_owned())]);
}
