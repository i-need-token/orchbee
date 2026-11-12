//! The removal plan's fence: a run is refused whole when a root is unsafe, the roots overlap, or a
//! target is outside the roots, a parent of a root, or reached through a link or junction. Nothing
//! here reads the disk; presence is a table.
use super::plan::{Presence, Refusal, Roots, Target, fence, plan};
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

/// Presence by table: the listed paths hold what they say, every other path is a folder.
fn presence(table: &'static [(&'static str, Presence)]) -> impl Fn(&Path) -> Presence {
    move |path| {
        table
            .iter()
            .find(|(listed, _)| Path::new(listed) == path)
            .map_or(Presence::Directory, |(_, presence)| *presence)
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
    let cases: [Case; 19] = [
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
        (STORE, DATA, true, &[(DATA, Presence::Link)], Ok(())),
        (STORE, DATA, false, &[(STORE, Presence::Link)], Ok(())),
        // A link inside a target is not on the way to it: removal removes the link alone.
        (
            STORE,
            DATA,
            true,
            &[(
                r"C:\Users\Ada\.ob-test-0123456789abcdef\internal",
                Presence::Link,
            )],
            Ok(()),
        ),
    ];
    for (store, data, delete_data, table, expected) in cases {
        let result = plan(&roots(store, data), delete_data, presence(table)).map(drop);
        assert_eq!(result, expected, "{store} | {data} | {delete_data}");
    }
}

#[test]
fn a_target_outside_the_roots_or_above_one_is_refused() {
    let roots = roots(STORE, DATA);
    let target = |path: &str| Target {
        path: PathBuf::from(path),
        presence: Presence::Directory,
    };
    let cases = [
        // Inside a root, in any letter case.
        (&format!(r"{STORE}\versions\0.1.0"), Ok(())),
        (&format!(r"{}\LOGS", DATA.to_uppercase()), Ok(())),
        // Beside, above or outside the roots.
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
