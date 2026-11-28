//! What a delete removes, keeps and restores.

use super::*;
use crate::{fs::tests::Scratch, git::ports::NoTrash};

/// A trash that refuses every item.
struct RefusingTrash;

impl Trash for RefusingTrash {
    fn trash(&self, _: &Path) -> Result<(), TrashError> {
        Err(TrashError::Failed)
    }
}

fn file_path(path: &Path) -> FilePath {
    FilePath::new(path.to_str().unwrap()).unwrap()
}

fn admit(path: &FilePath) -> Result<NativePath, FilesError> {
    NativePath::parse(path.as_str())
}

fn workspace() -> WorkspaceId {
    WorkspaceId::from_bytes([7; 16]).unwrap()
}

/// Deletes `paths` as `mode` says through `trash`, and answers each item's error.
fn delete(
    trash: &dyn Trash,
    mode: DeleteMode,
    paths: &[FilePath],
) -> (Vec<Option<ErrorCode>>, Vec<Kept>) {
    let deleting = Deleting {
        helpers: &Helpers::new(),
        trash,
        mode,
        capture: true,
        cancelled: &|| false,
    };
    let (items, kept) = deleting.run(paths, &admit);
    let errors = items
        .into_iter()
        .map(|item| item.error.map(|error| error.code))
        .collect();
    (errors, kept)
}

#[test]
fn an_item_the_trash_does_not_take_stays() {
    let scratch = Scratch::new("edit-no-trash");
    let (file, folder) = (
        scratch.path().join("kept.txt"),
        scratch.path().join("folder"),
    );
    fs::write(&file, "kept").unwrap();
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("inner.txt"), "inner").unwrap();
    let paths = [file_path(&file), file_path(&folder)];
    let refusals: [(&dyn Trash, ErrorCode); 2] = [
        (&NoTrash, ErrorCode::TrashUnavailable),
        (&RefusingTrash, ErrorCode::TrashFailed),
    ];
    for (trash, code) in refusals {
        let (errors, kept) = delete(trash, DeleteMode::Trash, &paths);
        assert_eq!(errors, [Some(code), Some(code)]);
        assert!(kept.is_empty(), "nothing deleted, so nothing to restore");
        assert_eq!(fs::read_to_string(&file).unwrap(), "kept");
        assert_eq!(
            fs::read_to_string(folder.join("inner.txt")).unwrap(),
            "inner"
        );
    }
    let missing = file_path(&scratch.path().join("missing.txt"));
    let (errors, _) = delete(&NoTrash, DeleteMode::Trash, &[missing]);
    assert_eq!(errors, [Some(ErrorCode::NotFound)]);
}

#[test]
fn a_restore_recreates_small_files_and_empty_folders_once() {
    let scratch = Scratch::new("edit-restore");
    let small = scratch.path().join("small.txt");
    let large = scratch.path().join("large.bin");
    let folder = scratch.path().join("folder");
    fs::write(&small, "small").unwrap();
    // One byte more than VS Code's undo keeps.
    fs::write(&large, vec![b'x'; 5_000_001]).unwrap();
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("inner.txt"), "inner").unwrap();
    let paths = [file_path(&small), file_path(&large), file_path(&folder)];
    let (errors, kept) = delete(&NoTrash, DeleteMode::Permanent, &paths);
    assert_eq!(errors, [None, None, None]);
    assert!(!small.exists() && !large.exists() && !folder.exists());

    let stashes = Stashes::new();
    let stash = stashes.keep(workspace(), kept).unwrap();
    let other = WorkspaceId::from_bytes([8; 16]).unwrap();
    assert!(
        stashes.take(other, stash).is_none(),
        "another Workspace never takes it"
    );
    let restored = restore(
        &Helpers::new(),
        stashes.take(workspace(), stash).unwrap(),
        &admit,
    );
    let restored: Vec<_> = restored
        .into_iter()
        .map(|item| (item.path, item.error))
        .collect();
    assert_eq!(
        restored,
        [(paths[0].clone(), None), (paths[2].clone(), None)]
    );
    assert_eq!(fs::read_to_string(&small).unwrap(), "small");
    assert!(!large.exists());
    assert_eq!(
        fs::read_dir(&folder).unwrap().count(),
        0,
        "a folder comes back empty"
    );
    assert!(
        stashes.take(workspace(), stash).is_none(),
        "a stash restores once"
    );

    // A file that took the name meanwhile is never replaced.
    let (_, kept) = delete(&NoTrash, DeleteMode::Permanent, &paths[..1]);
    fs::write(&small, "newer").unwrap();
    let restored = restore(&Helpers::new(), kept, &admit);
    assert_eq!(
        restored[0].error.as_ref().map(|error| error.code),
        Some(ErrorCode::Exists)
    );
    assert_eq!(fs::read_to_string(&small).unwrap(), "newer");
}

#[test]
fn stashes_keep_the_newest_within_their_budget() {
    let path = FilePath::new(if cfg!(windows) { r"C:\a" } else { "/a" }).unwrap();
    let kept = |content: usize| {
        vec![Kept {
            path: path.clone(),
            content: Some(vec![0; content]),
        }]
    };
    // Room for two stashes of four bytes of content, their paths counted.
    let budget = 2 * (path.as_str().len() + 4);
    let stashes = Stashes::with_budget(budget);
    let first = stashes.keep(workspace(), kept(4)).unwrap();
    let second = stashes.keep(workspace(), kept(4)).unwrap();
    let third = stashes.keep(workspace(), kept(4)).unwrap();
    assert!(
        stashes.take(workspace(), first).is_none(),
        "the oldest went first"
    );
    assert!(
        stashes.keep(workspace(), kept(budget)).is_none(),
        "more than the budget"
    );
    assert!(
        stashes.keep(workspace(), Vec::new()).is_none(),
        "nothing to keep"
    );
    assert!(stashes.take(workspace(), second).is_some());
    assert!(stashes.take(workspace(), third).is_some());
}
