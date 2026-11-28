//! The mutation check against this machine's filesystem: a link inside the root that leads outside
//! it, a junction on Windows, refuses changes through it while the link itself stays changeable; a
//! link that names nothing refuses what passes through it; and spellings that differ in case or,
//! on Windows, use 8.3 names reach the same real location.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use orchbee_files_protocol::{
    ConnectionId, ErrorCode, FilePath, GrantSource, Operation, ScopeUpdate, WorkspaceId,
    WorkspaceScope,
};

use super::super::{Change, Scopes, View, path};

/// A private folder for one test at its real location, removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(purpose: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = fs::canonicalize(std::env::temp_dir()).unwrap();
        // Windows answers in the verbatim form, which a Client never names.
        let base = PathBuf::from(base.to_str().unwrap().trim_start_matches(r"\\?\"));
        let path = base.join(format!(
            "orchbee-files-scope-{purpose}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// Creates the folder `relative`, its segments separated by `/`, and answers its native path.
    fn folder(&self, relative: &str) -> PathBuf {
        let path = relative
            .split('/')
            .fold(self.0.clone(), |path, segment| path.join(segment));
        fs::create_dir_all(&path).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Best effort: removing a junction removes the link, never what it names.
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn workspace() -> WorkspaceId {
    WorkspaceId::from_bytes([1; 16]).unwrap()
}

fn connection() -> ConnectionId {
    ConnectionId::new(1).unwrap()
}

fn file(path: &Path) -> FilePath {
    FilePath::new(path.to_str().unwrap()).unwrap()
}

/// Scopes whose Workspace has `root`, spelled as given, for its root.
fn rooted(root: &Path) -> Scopes {
    let scopes = Scopes::new();
    scopes.update(ScopeUpdate {
        workspace: workspace(),
        scope: WorkspaceScope::Root(root.to_str().unwrap().to_owned()),
    });
    scopes
}

fn view(scopes: &Scopes) -> View {
    scopes
        .view(workspace(), connection(), Operation::GitClean)
        .unwrap()
}

fn code(view: &View, path: &Path, change: Change) -> Option<ErrorCode> {
    view.mutable(&file(path), change)
        .err()
        .map(|error| error.code)
}

/// Links `link` to the folder `target` with a junction, which needs no privilege.
#[cfg(windows)]
fn junction(target: &Path, link: &Path) {
    let status = std::process::Command::new("cmd")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
}

/// Links `link` to the folder `target` with a symbolic link, which Windows creates only with
/// Developer Mode or elevation.
fn symbolic_link(target: &Path, link: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    return std::os::windows::fs::symlink_dir(target, link);
    #[cfg(unix)]
    return std::os::unix::fs::symlink(target, link);
}

#[test]
fn a_link_leading_outside_refuses_changes_through_it_but_not_of_itself() {
    let scratch = Scratch::new("links");
    let root = scratch.folder("project");
    let source = scratch.folder("project/src");
    fs::write(source.join("a.txt"), b"inside").unwrap();
    let outside = scratch.folder("outside");
    fs::write(outside.join("b.txt"), b"outside").unwrap();
    let mut links = Vec::new();
    #[cfg(windows)]
    {
        junction(&outside, &root.join("junction"));
        junction(&source, &root.join("junction-inside"));
        links.push(("junction", "junction-inside"));
    }
    match symbolic_link(&outside, &root.join("link"))
        .and_then(|()| symbolic_link(&source, &root.join("link-inside")))
    {
        Ok(()) => links.push(("link", "link-inside")),
        Err(error) => {
            eprintln!("symbolic links not created ({error}); the junctions stand in for them");
        }
    }
    let scopes = rooted(&root);

    let view = view(&scopes);
    assert_eq!(code(&view, &source.join("new.txt"), Change::Entry), None);
    assert_eq!(code(&view, &source.join("a.txt"), Change::Content), None);
    for (out, inside) in &links {
        let out = root.join(out);
        let inside = root.join(inside);
        for change in [Change::Content, Change::Entry] {
            assert_eq!(
                code(&view, &out.join("b.txt"), change),
                Some(ErrorCode::LinkTargetOutside),
                "{change:?} through {out:?}"
            );
            assert_eq!(
                code(&view, &out.join("new").join("deeper.txt"), change),
                Some(ErrorCode::LinkTargetOutside),
                "{change:?} below {out:?}"
            );
            assert_eq!(code(&view, &inside.join("a.txt"), change), None);
        }
        // The link itself is an entry of the root; what it names is not.
        assert_eq!(code(&view, &out, Change::Entry), None);
        assert_eq!(
            code(&view, &out, Change::Content),
            Some(ErrorCode::LinkTargetOutside)
        );
    }
    // Missing folders lead where their nearest existing one leads.
    let deeper = root.join("missing").join("deeper").join("new.txt");
    assert_eq!(code(&view, &deeper, Change::Entry), None);
    assert_eq!(code(&view, &deeper, Change::Content), None);
    // The root itself is no entry of the scope, and outside it is outside.
    assert_eq!(
        code(&view, &root, Change::Entry),
        Some(ErrorCode::OutsideScope)
    );
    assert_eq!(code(&view, &root, Change::Content), None);
    let opened = outside.join("b.txt");
    assert_eq!(
        code(&view, &opened, Change::Content),
        Some(ErrorCode::OutsideScope)
    );

    // A file the person opened is written wherever it lies, but its entry stays out of scope.
    let admitted = view
        .grantable(&file(&opened), GrantSource::TerminalLink)
        .unwrap();
    scopes.grant(&view, admitted, || false).unwrap().unwrap();
    let view = self::view(&scopes);
    assert_eq!(code(&view, &opened, Change::Content), None);
    assert_eq!(
        code(&view, &opened, Change::Entry),
        Some(ErrorCode::OutsideScope)
    );

    // Once what the links name is gone, nothing passes through them, and they stay removable.
    fs::remove_dir_all(&outside).unwrap();
    for (out, _) in &links {
        let out = root.join(out);
        assert_eq!(
            code(&view, &out.join("b.txt"), Change::Entry),
            Some(ErrorCode::LinkTargetOutside)
        );
        assert_eq!(
            code(&view, &out, Change::Content),
            Some(ErrorCode::LinkTargetOutside)
        );
        assert_eq!(code(&view, &out, Change::Entry), None);
    }
}

#[test]
fn spellings_that_differ_in_case_reach_the_same_real_location() {
    let scratch = Scratch::new("case");
    let root = scratch.folder("Project");
    scratch.folder("Project/Source");
    let other_case =
        |path: &Path| PathBuf::from(path.to_str().unwrap().replace("Project", "PROJECT"));
    // Ignoring case it is the same folder; on Linux it is another one, outside the root.
    let refused = path::CASE_SENSITIVE.then_some(ErrorCode::OutsideScope);
    let view = view(&rooted(&root));
    let entry = other_case(&root.join("Source").join("new.txt"));
    assert_eq!(code(&view, &entry, Change::Entry), refused);
    // On Linux a folder spelled in another case is one more missing folder of the root.
    let entry = root.join("SOURCE").join("new.txt");
    assert_eq!(code(&view, &entry, Change::Entry), None);

    // A root reported in another case than the folder's own admits the folder's spelling too.
    let view = self::view(&rooted(&other_case(&root)));
    let entry = root.join("source").join("new.txt");
    assert_eq!(code(&view, &entry, Change::Entry), refused);
}

/// Windows names a long name by its 8.3 alias too, where the volume makes aliases.
#[cfg(windows)]
#[test]
fn an_8_3_name_reaches_the_real_location_of_its_long_name() {
    use std::os::windows::process::CommandExt;
    let scratch = Scratch::new("short-names");
    let root = scratch.folder("project-root-folder");
    let source = scratch.folder("project-root-folder/source-folder");
    let outside = scratch.folder("outside-folder");
    junction(&outside, &root.join("junction-folder"));
    let output = std::process::Command::new("cmd")
        .args(["/d", "/c"])
        .raw_arg(format!(
            "for %I in (\"{}\") do @echo %~sI",
            source.display()
        ))
        .output()
        .unwrap();
    assert!(output.status.success());
    let short_source = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim());
    if short_source == source {
        eprintln!("this volume makes no 8.3 names; their resolution is unverified here");
        return;
    }
    let short_root = short_source.parent().unwrap().to_owned();

    // A root reported by its 8.3 name admits that spelling, which reaches the long name.
    let view = view(&rooted(&short_root));
    assert_eq!(
        code(&view, &short_source.join("new.txt"), Change::Entry),
        None
    );
    assert_eq!(
        code(
            &view,
            &short_root.join("junction-folder").join("x"),
            Change::Entry
        ),
        Some(ErrorCode::LinkTargetOutside)
    );
    // The long spelling is another lexical path, as it is for reads.
    assert_eq!(
        code(&view, &source.join("new.txt"), Change::Entry),
        Some(ErrorCode::OutsideScope)
    );

    // An 8.3 name inside the root reaches the folder it names.
    let view = self::view(&rooted(&root));
    let short_name = short_source.file_name().unwrap();
    assert_eq!(
        code(&view, &root.join(short_name).join("new.txt"), Change::Entry),
        None
    );
}
