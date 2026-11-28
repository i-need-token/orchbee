//! Names taken and changed by each machine's own comparison.

use super::*;
use crate::{fs::tests::Scratch, scope::path::CASE_SENSITIVE};

fn native(path: &Path) -> NativePath {
    NativePath::parse(path.to_str().unwrap()).unwrap()
}

fn listed(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|item| item.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

fn file_names(names: &[&str]) -> Vec<FileName> {
    names
        .iter()
        .map(|name| FileName::new(*name).unwrap())
        .collect()
}

#[test]
fn a_name_in_another_case_is_taken_where_the_machine_ignores_case() {
    let (scratch, helpers) = (Scratch::new("edit-case"), Helpers::new());
    let folder = native(scratch.path());
    let lower = names::child(&folder, "a.txt").unwrap();
    let upper = names::child(&folder, "A.txt").unwrap();
    create(&helpers, &lower, CreateKind::File).unwrap();
    let created = create(&helpers, &upper, CreateKind::File).map(|_| ());
    let existing = exists(&helpers, &folder, file_names(&["A.TXT", "b.txt"])).unwrap();
    if CASE_SENSITIVE {
        assert_eq!(created, Ok(()));
        assert!(existing.is_empty());
        assert_eq!(listed(scratch.path()), ["A.txt", "a.txt"]);
        return;
    }
    assert_eq!(created.unwrap_err().code, ErrorCode::Exists);
    assert_eq!(existing, file_names(&["A.TXT"]));
    // A rename to the same item in another case changes its spelling, files and folders alike.
    let entry = rename(&helpers, &lower, &upper).unwrap();
    assert_eq!(entry.name.as_str(), "A.txt");
    let (folder_lower, folder_upper) = (
        names::child(&folder, "docs").unwrap(),
        names::child(&folder, "Docs").unwrap(),
    );
    create(&helpers, &folder_lower, CreateKind::Directory).unwrap();
    rename(&helpers, &folder_lower, &folder_upper).unwrap();
    assert_eq!(listed(scratch.path()), ["A.txt", "Docs"]);
    // Another item under that name is never replaced.
    let other = names::child(&folder, "other.txt").unwrap();
    create(&helpers, &other, CreateKind::File).unwrap();
    let taken = names::child(&folder, "a.TXT").unwrap();
    assert_eq!(
        rename(&helpers, &other, &taken).unwrap_err().code,
        ErrorCode::Exists
    );
    assert_eq!(listed(scratch.path()), ["A.txt", "Docs", "other.txt"]);
}

#[cfg(target_os = "macos")]
#[test]
fn macos_takes_a_name_in_either_unicode_form_and_keeps_the_one_it_was_given() {
    let (scratch, helpers) = (Scratch::new("edit-unicode"), Helpers::new());
    let folder = native(scratch.path());
    let (decomposed, composed) = ("cafe\u{301}", "caf\u{e9}");
    create(
        &helpers,
        &names::child(&folder, decomposed).unwrap(),
        CreateKind::File,
    )
    .unwrap();
    let again = create(
        &helpers,
        &names::child(&folder, composed).unwrap(),
        CreateKind::File,
    );
    assert_eq!(again.unwrap_err().code, ErrorCode::Exists);
    let existing = exists(&helpers, &folder, file_names(&[composed])).unwrap();
    assert_eq!(existing, file_names(&[composed]));
    assert_eq!(listed(scratch.path()), [decomposed]);
}
