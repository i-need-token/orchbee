//! The removal plan: a run is refused whole when a root is unsafe, the roots overlap, a root is a
//! link, a target is outside the roots, a parent of a root, or reached through a link or junction,
//! or a root holds what the run would remove without its layout's mark; otherwise it removes only
//! the entries the layouts name, keeps and counts everything else, and leaves each mark for last.
//! Nothing here reads the disk; presence is a table.
use super::plan::{Contents, Presence, Refusal, Roots, Target, fence, plan};
use std::path::{Path, PathBuf};

const STORE: &str = r"C:\Users\Ada\AppData\Local\Programs\ob-test-0123456789abcdef";
const DATA: &str = r"C:\Users\Ada\.ob-test-0123456789abcdef";

fn roots(store: &str, data: &str) -> Roots {
    Roots {
        store: PathBuf::from(store),
        data: PathBuf::from(data),
        protected: [
            r"C:\Users\Ada",
            r"C:\Users\Ada\AppData\Local",
            r"C:\Users\Ada\AppData\Local\Programs",
            r"C:\Users\Ada\AppData\Roaming",
            r"C:\Windows",
            r"C:\Users\Ada\AppData\Local\Temp",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect(),
    }
}

/// A root that holds its whole layout and its mark.
fn marked(names: &[&str]) -> Contents {
    Contents {
        names: Some(names.iter().map(|name| (*name).to_owned()).collect()),
        marked: true,
    }
}

const STORE_LAYOUT: &[&str] = &["bin", "desktop", "versions", "install.lock", "active.json"];
const DATA_LAYOUT: &[&str] = &["config.toml", "sessions", "logs", "internal"];

/// Presence by table: the listed paths hold what they say, every other path is a folder except
/// the two marks, which are files.
fn presence(table: &'static [(&'static str, Presence)]) -> impl Fn(&Path) -> Presence {
    move |path| {
        table
            .iter()
            .find(|(listed, _)| Path::new(listed) == path)
            .map_or_else(
                || {
                    if path.ends_with("active.json") || path.ends_with(r"internal\version.json") {
                        Presence::File
                    } else {
                        Presence::Directory
                    }
                },
                |(_, presence)| *presence,
            )
    }
}

fn refusal(reason: fn(PathBuf) -> Refusal, path: &str) -> Result<(), Refusal> {
    Err(reason(PathBuf::from(path)))
}

#[test]
fn the_plan_is_refused_whole_unless_every_target_is_safely_inside_a_root() {
    type Case = (
        &'static str,
        &'static str,
        bool,
        &'static [(&'static str, Presence)],
        Result<(), Refusal>,
    );
    let cases: [Case; 21] = [
        // The ordinary layout, both modes.
        (STORE, DATA, false, &[], Ok(())),
        (STORE, DATA, true, &[], Ok(())),
        // A root that is relative, a volume, one folder below a volume, spelled with `..`, equal to a
        // protected folder or holding one.
        (
            "Programs\\ob-test",
            DATA,
            false,
            &[],
            refusal(Refusal::UnsafeRoot, "Programs\\ob-test"),
        ),
        (
            STORE,
            r"C:\",
            false,
            &[],
            refusal(Refusal::UnsafeRoot, r"C:\"),
        ),
        (
            r"D:\Orchbee",
            DATA,
            false,
            &[],
            refusal(Refusal::UnsafeRoot, r"D:\Orchbee"),
        ),
        (
            r"C:\Users\Ada\..\Ada\Programs",
            DATA,
            false,
            &[],
            refusal(Refusal::UnsafeRoot, r"C:\Users\Ada\..\Ada\Programs"),
        ),
        (
            STORE,
            r"C:\users\ADA",
            true,
            &[],
            refusal(Refusal::UnsafeRoot, r"C:\users\ADA"),
        ),
        (
            r"C:\Users\Ada\AppData\Local\Programs",
            DATA,
            false,
            &[],
            refusal(Refusal::UnsafeRoot, r"C:\Users\Ada\AppData\Local\Programs"),
        ),
        (
            STORE,
            r"C:\Users",
            false,
            &[],
            refusal(Refusal::UnsafeRoot, r"C:\Users"),
        ),
        // Roots that overlap, in either direction or by letter case alone.
        (
            STORE,
            joined(STORE, r"\data"),
            false,
            &[],
            Err(Refusal::RootsOverlap),
        ),
        (
            joined(DATA, r"\store"),
            DATA,
            false,
            &[],
            Err(Refusal::RootsOverlap),
        ),
        (
            DATA,
            r"C:\USERS\ada\.OB-TEST-0123456789ABCDEF",
            false,
            &[],
            Err(Refusal::RootsOverlap),
        ),
        // A link or an unreadable folder on the way to a target refuses the run.
        (
            STORE,
            DATA,
            false,
            &[(
                r"C:\Users\Ada\.ob-test-0123456789abcdef\internal",
                Presence::Link,
            )],
            refusal(
                Refusal::ThroughLink,
                r"C:\Users\Ada\.ob-test-0123456789abcdef\internal",
            ),
        ),
        (
            STORE,
            DATA,
            false,
            &[(
                r"C:\Users\Ada\.ob-test-0123456789abcdef\internal",
                Presence::Unreadable,
            )],
            refusal(
                Refusal::ThroughLink,
                r"C:\Users\Ada\.ob-test-0123456789abcdef\internal",
            ),
        ),
        (
            STORE,
            DATA,
            false,
            &[(DATA, Presence::Link)],
            refusal(Refusal::ThroughLink, DATA),
        ),
        // A root that is itself a link is refused in every mode; its entries lie through it.
        (
            STORE,
            DATA,
            true,
            &[(DATA, Presence::Link)],
            refusal(Refusal::ThroughLink, DATA),
        ),
        (
            STORE,
            DATA,
            false,
            &[(STORE, Presence::Link)],
            refusal(Refusal::ThroughLink, STORE),
        ),
        // A root that is a file holds nothing of the layout and carries no mark.
        (
            STORE,
            DATA,
            false,
            &[(STORE, Presence::File)],
            refusal(Refusal::UnmarkedStore, STORE),
        ),
        // A target that is itself a link is removed as a link, never followed: admitted.
        (
            STORE,
            DATA,
            false,
            &[(
                r"C:\Users\Ada\.ob-test-0123456789abcdef\logs",
                Presence::Link,
            )],
            Ok(()),
        ),
        (
            STORE,
            DATA,
            true,
            &[(
                r"C:\Users\Ada\.ob-test-0123456789abcdef\sessions",
                Presence::Link,
            )],
            Ok(()),
        ),
        // A link inside a target is not on the way to it: removal removes the link alone.
        (
            STORE,
            DATA,
            true,
            &[(
                r"C:\Users\Ada\.ob-test-0123456789abcdef\sessions\default",
                Presence::Link,
            )],
            Ok(()),
        ),
    ];
    for (store, data, delete_data, table, expected) in cases {
        let result = plan(
            &roots(store, data),
            delete_data,
            &marked(STORE_LAYOUT),
            &marked(DATA_LAYOUT),
            presence(table),
        )
        .map(drop);
        assert_eq!(result, expected, "{store} | {data} | {delete_data}");
    }
}

/// Presence that follows what each root holds: `None` is a missing root; `Some` lists the paths
/// below it that exist. A name with a dot after its first character is a file.
fn holding(
    store: Option<&'static [&'static str]>,
    data: Option<&'static [&'static str]>,
) -> impl Fn(&Path) -> Presence {
    move |path| {
        for (root, held) in [(STORE, store), (DATA, data)] {
            let Ok(rest) = path.strip_prefix(root) else {
                continue;
            };
            let Some(held) = held else {
                return Presence::Missing;
            };
            if rest.as_os_str().is_empty() {
                return Presence::Directory;
            }
            let rest = rest.to_string_lossy();
            return match held
                .iter()
                .find(|entry| **entry == rest || entry.starts_with(&format!(r"{rest}\")))
            {
                Some(entry) if *entry == rest && rest.rfind('.').is_some_and(|dot| dot > 0) => {
                    Presence::File
                }
                Some(_) => Presence::Directory,
                None => Presence::Missing,
            };
        }
        Presence::Directory
    }
}

/// What a root lists: the first part of every path it holds, once each.
fn contents(held: Option<&[&str]>, marked: bool) -> Contents {
    let mut names: Vec<String> = held
        .unwrap_or_default()
        .iter()
        .map(|entry| entry.split('\\').next().unwrap_or_default().to_owned())
        .collect();
    names.dedup();
    Contents {
        names: Some(names),
        marked,
    }
}

/// Each root's kept count and whether the root itself goes once empty, or the refusal.
type Summary = Result<((usize, bool), (usize, bool)), Refusal>;

#[test]
fn uninstall_removes_only_what_the_layouts_name_and_only_from_a_marked_root() {
    const STORE_FULL: &[&str] = &[
        r"bin\ob-test.exe",
        r"desktop\Orchbee.exe",
        r"versions\0.1.0\ob-test.exe",
        "install.lock",
        "active.json",
    ];
    const DATA_FULL: &[&str] = &[
        "config.toml",
        r"sessions\default",
        r"logs\2026-11-12.jsonl",
        r"internal\version.json",
        r"internal\cache",
    ];
    type Side = (Option<&'static [&'static str]>, bool);
    let cases: [(Side, Side, bool, Summary); 15] = [
        // Both layouts complete and marked: everything of them goes, the store root too.
        (
            (Some(STORE_FULL), true),
            (Some(DATA_FULL), true),
            false,
            Ok(((0, true), (0, false))),
        ),
        (
            (Some(STORE_FULL), true),
            (Some(DATA_FULL), true),
            true,
            Ok(((0, true), (0, true))),
        ),
        // Other items in a marked root stay and are counted; a staging folder is the store's.
        (
            (
                Some(&[
                    r"bin\ob-test.exe",
                    "active.json",
                    "unins000.exe",
                    r".stage-12-0\x",
                    ".stage-x",
                ]),
                true,
            ),
            (Some(DATA_FULL), true),
            false,
            Ok(((2, true), (0, false))),
        ),
        (
            (Some(STORE_FULL), true),
            (
                Some(&[
                    "config.toml",
                    r"internal\version.json",
                    "notes.txt",
                    r"work\plan.md",
                ]),
                true,
            ),
            true,
            Ok(((0, true), (2, true))),
        ),
        // A folder that holds what the run removes but not the mark refuses, in both modes.
        (
            (Some(STORE_FULL), true),
            (Some(&[r"logs\a.log", "notes.txt"]), false),
            false,
            refused(Refusal::UnmarkedData, DATA),
        ),
        (
            (Some(STORE_FULL), true),
            (Some(&[r"logs\a.log", "notes.txt"]), false),
            true,
            refused(Refusal::UnmarkedData, DATA),
        ),
        // A record that reads but is not where the layout keeps it is no mark.
        (
            (Some(STORE_FULL), true),
            (Some(&[r"logs\a.log"]), true),
            false,
            refused(Refusal::UnmarkedData, DATA),
        ),
        (
            (Some(&["bin", r"versions\0.1.0\ob-test.exe"]), false),
            (Some(DATA_FULL), true),
            false,
            refused(Refusal::UnmarkedStore, STORE),
        ),
        // A pointer that does not read as one is no mark either.
        (
            (Some(&["active.json"]), false),
            (Some(DATA_FULL), true),
            false,
            refused(Refusal::UnmarkedStore, STORE),
        ),
        // A run interrupted before the marks went: the next run still recognizes both roots.
        (
            (
                Some(&[r"versions\0.1.0\ob-test.exe", "active.json", "unins000.exe"]),
                true,
            ),
            (Some(&[r"internal\version.json", "notes.txt"]), true),
            true,
            Ok(((1, true), (1, true))),
        ),
        // After a run that kept other items, only those are left: nothing to remove, no refusal.
        (
            (Some(&["unins000.exe"]), false),
            (Some(&["notes.txt"]), false),
            true,
            Ok(((1, false), (1, false))),
        ),
        // Default mode reads only what it removes: settings alone need no mark.
        (
            (Some(STORE_FULL), true),
            (Some(&["config.toml"]), false),
            false,
            Ok(((0, true), (0, false))),
        ),
        // Empty and missing roots are already absent; an empty root someone made stays, while a
        // data root the run itself creates to record the request goes again in either mode.
        (
            (Some(&[]), false),
            (Some(&[]), false),
            true,
            Ok(((0, false), (0, false))),
        ),
        (
            (None, false),
            (None, false),
            true,
            Ok(((0, false), (0, true))),
        ),
        (
            (None, false),
            (None, false),
            false,
            Ok(((0, false), (0, true))),
        ),
    ];
    for (index, ((store, store_mark), (data, data_mark), delete_data, expected)) in
        cases.into_iter().enumerate()
    {
        let result = plan(
            &roots(STORE, DATA),
            delete_data,
            &contents(store, store_mark),
            &contents(data, data_mark),
            holding(store, data),
        );
        let summary = result
            .as_ref()
            .map(|plan| {
                (
                    (plan.store.kept, plan.store.remove_root),
                    (plan.data.kept, plan.data.remove_root),
                )
            })
            .map_err(Clone::clone);
        assert_eq!(summary, expected, "case {index}");
        let Ok(plan) = result else {
            continue;
        };
        // Each mark goes after everything else of its layout.
        assert!(
            plan.store
                .mark
                .iter()
                .all(|mark| mark.path.ends_with("active.json"))
        );
        assert!(
            plan.store
                .targets
                .iter()
                .all(|target| !target.path.ends_with("active.json"))
        );
        if delete_data {
            let last = plan.data.targets.last().unwrap();
            assert!(last.path.ends_with("internal") && last.last == Some("version.json"));
        } else {
            assert!(plan.data.targets.iter().all(|target| target.last.is_none()));
        }
    }
}

fn refused(reason: fn(PathBuf) -> Refusal, path: &str) -> Summary {
    Err(reason(PathBuf::from(path)))
}

#[test]
fn a_target_outside_the_roots_or_above_one_is_refused() {
    let roots = roots(STORE, DATA);
    let target = |path: &str| Target {
        path: PathBuf::from(path),
        presence: Presence::Directory,
        last: None,
    };
    let cases = [
        // Inside a root, in any letter case.
        (&format!(r"{STORE}\versions\0.1.0"), Ok(())),
        (&format!(r"{}\LOGS", DATA.to_uppercase()), Ok(())),
        // A root itself, or beside, above or outside the roots.
        (&STORE.to_owned(), refusal(Refusal::RootParent, STORE)),
        (
            &format!(r"{STORE}-old"),
            refusal(Refusal::OutsideRoots, &format!(r"{STORE}-old")),
        ),
        (
            &r"C:\Users\Ada\AppData\Local\Programs".to_owned(),
            refusal(Refusal::RootParent, r"C:\Users\Ada\AppData\Local\Programs"),
        ),
        (
            &r"C:\Users\Ada".to_owned(),
            refusal(Refusal::RootParent, r"C:\Users\Ada"),
        ),
        (
            &r"D:\Users\Ada\.ob-test-0123456789abcdef".to_owned(),
            refusal(
                Refusal::OutsideRoots,
                r"D:\Users\Ada\.ob-test-0123456789abcdef",
            ),
        ),
        (
            &format!(r"{DATA}\..\.claude"),
            refusal(Refusal::OutsideRoots, &format!(r"{DATA}\..\.claude")),
        ),
        (&"logs".to_owned(), refusal(Refusal::OutsideRoots, "logs")),
    ];
    for (path, expected) in cases {
        let target = target(path);
        assert_eq!(
            fence(&roots, [&target], &|_: &Path| Presence::Directory),
            expected,
            "{path}"
        );
    }
}

/// A root spelled from another; leaked so that a case row stays `'static`.
fn joined(base: &str, rest: &str) -> &'static str {
    Box::leak(format!("{base}{rest}").into_boxed_str())
}
