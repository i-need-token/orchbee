//! The conflict rules and what a copy keeps.

use super::*;
use crate::fs::tests::Scratch;

fn native(path: &Path) -> NativePath {
    NativePath::parse(path.to_str().unwrap()).unwrap()
}

fn write(path: &Path, content: &str) {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

/// Transfers each of `sources` into `directory` and answers each item's final name or error.
fn transfer(
    transfer: Transfer,
    sources: &[&Path],
    directory: &Path,
    conflict: Conflict,
) -> Vec<Result<String, ErrorCode>> {
    let directory = native(directory);
    let sources: Vec<FilePath> = sources
        .iter()
        .map(|source| FilePath::new(source.to_str().unwrap()).unwrap())
        .collect();
    let admit = |source: &FilePath| {
        let source = NativePath::parse(source.as_str())?;
        let target = names::child(&directory, source.name())?;
        Ok((source, target))
    };
    run(
        &Helpers::new(),
        transfer,
        &sources,
        &admit,
        conflict,
        &|| false,
    )
    .into_iter()
    .map(
        |transferred| match (transferred.item.path, transferred.item.error) {
            (Some(path), None) => Ok(native(Path::new(path.as_str())).name().to_owned()),
            (_, Some(error)) => Err(error.code),
            (None, None) => unreachable!("an item answers a path or an error"),
        },
    )
    .collect()
}

#[test]
fn a_taken_name_is_settled_as_the_conflict_rule_says() {
    let scratch = Scratch::new("edit-conflict");
    let (from, to) = (scratch.path().join("from"), scratch.path().join("to"));
    write(&from.join("a.txt"), "new");
    write(&from.join("f").join("x.txt"), "x new");
    write(&from.join("f").join("sub").join("y.txt"), "y");
    write(&to.join("a.txt"), "old");
    write(&to.join("f").join("x.txt"), "x old");
    write(&to.join("f").join("keep.txt"), "keep");
    let (file, folder) = (from.join("a.txt"), from.join("f"));

    let failing = transfer(Transfer::Copy, &[&file], &to, Conflict::Fail);
    assert_eq!(failing, [Err(ErrorCode::Exists)]);
    assert_eq!(read(&to.join("a.txt")), "old");

    // Each copy takes VS Code's next free name, a folder's with no extension.
    for expected in ["a copy.txt", "a copy 2.txt"] {
        let incremented = transfer(Transfer::Copy, &[&file], &to, Conflict::Increment);
        assert_eq!(incremented, [Ok(expected.to_owned())]);
        assert_eq!(read(&to.join(expected)), "new");
    }
    let beside = transfer(Transfer::Copy, &[&folder], &from, Conflict::Increment);
    assert_eq!(beside, [Ok("f copy".to_owned())]);
    assert_eq!(read(&from.join("f copy").join("sub").join("y.txt")), "y");

    // A merge combines the folders and replaces the files both hold.
    let merged = transfer(Transfer::Copy, &[&folder], &to, Conflict::Merge);
    assert_eq!(merged, [Ok("f".to_owned())]);
    assert_eq!(read(&to.join("f").join("x.txt")), "x new");
    assert_eq!(read(&to.join("f").join("keep.txt")), "keep");
    assert_eq!(read(&to.join("f").join("sub").join("y.txt")), "y");

    let replaced = transfer(Transfer::Move, &[&file], &to, Conflict::Replace);
    assert_eq!(replaced, [Ok("a.txt".to_owned())]);
    assert_eq!(read(&to.join("a.txt")), "new");
    assert!(!file.exists());

    // An item never goes into itself, and replacing what holds it would remove it.
    let into_itself = transfer(
        Transfer::Copy,
        &[&folder],
        &folder.join("sub"),
        Conflict::Fail,
    );
    assert_eq!(into_itself, [Err(ErrorCode::InvalidRequest)]);
    let nested = from.join("f").join("sub").join("f");
    write(&nested.join("z.txt"), "z");
    let holding = transfer(Transfer::Move, &[&nested], &from, Conflict::Replace);
    assert_eq!(holding, [Err(ErrorCode::InvalidRequest)]);
    assert_eq!(read(&nested.join("z.txt")), "z");

    // A moved folder merges child by child, and goes once it is empty.
    let moved = transfer(Transfer::Move, &[&folder], &to, Conflict::Merge);
    assert_eq!(moved, [Ok("f".to_owned())]);
    assert!(!folder.exists());
    assert_eq!(read(&to.join("f").join("sub").join("f").join("z.txt")), "z");
}

#[cfg(target_os = "linux")]
#[test]
fn a_copy_keeps_names_that_are_not_utf8() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

    let scratch = Scratch::new("edit-bytes");
    // Latin-1 é: a name Linux holds as bytes, which no Client can name.
    let name = OsStr::from_bytes(b"caf\xe9");
    let source = scratch.path().join("source");
    write(&source.join(name).join("inner.txt"), "inner");
    let target = scratch.path().join("target");
    fs::create_dir(&target).unwrap();
    let copied = transfer(Transfer::Copy, &[&source], &target, Conflict::Fail);
    assert_eq!(copied, [Ok("source".to_owned())]);
    assert_eq!(
        read(&target.join("source").join(name).join("inner.txt")),
        "inner"
    );
}
