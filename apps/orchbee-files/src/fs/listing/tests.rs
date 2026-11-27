//! Page bounds, cursor identity and lifetime, the single-child chain, and what an entry reports
//! for links, names and write protection, against real directories.

use std::{collections::BTreeSet, ffi::OsStr, fs::File};

use super::*;
use crate::fs::tests::Scratch;

const WAIT: Duration = Duration::from_secs(10);

/// A measure that charges an entry its name and a fixed overhead, standing in for the wire's.
fn measured(entry: &Entry) -> usize {
    entry.name.as_str().len() + 64
}

fn descendant_overhead(path: &Path) -> Option<usize> {
    Some(path.as_os_str().len() + 32)
}

fn budget(bytes: usize) -> PageBudget<'static> {
    PageBudget {
        bytes,
        entry: &measured,
        descendant: &descendant_overhead,
    }
}

fn listings() -> Listings {
    Listings::new(Arc::new(Timer::new()))
}

fn connection(value: u64) -> ConnectionId {
    ConnectionId::new(value).unwrap()
}

fn files(directory: &Path, count: usize) {
    for index in 0..count {
        // Names of varying length, so the byte bound cuts pages at different counts.
        File::create(directory.join(format!("{index:0width$}", width = 1 + index % 40))).unwrap();
    }
}

/// Every page of `path`, following cursors to the end.
fn all_pages(listings: &Listings, path: &Path, bytes: usize) -> Vec<Page> {
    let never = AtomicBool::new(false);
    let mut pages = vec![
        listings
            .page(connection(1), path, None, false, &budget(bytes), &never)
            .unwrap(),
    ];
    while let Some(cursor) = pages.last().unwrap().cursor {
        pages.push(
            listings
                .page(
                    connection(1),
                    path,
                    Some(cursor),
                    false,
                    &budget(bytes),
                    &never,
                )
                .unwrap(),
        );
    }
    pages
}

fn names(pages: &[Page]) -> Vec<String> {
    pages
        .iter()
        .flat_map(|page| {
            page.entries
                .iter()
                .map(|entry| entry.name.as_str().to_owned())
        })
        .collect()
}

#[test]
fn pages_list_each_entry_once_within_the_entry_and_byte_bounds() {
    let scratch = Scratch::new("pages");
    files(scratch.path(), 2_001);
    let listings = listings();
    let expected: BTreeSet<String> = fs::read_dir(scratch.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();

    for bytes in [usize::MAX, 6_000, 64] {
        let pages = all_pages(&listings, scratch.path(), bytes);
        let listed = names(&pages);
        assert_eq!(listed.len(), expected.len(), "budget {bytes}");
        assert_eq!(listed.iter().cloned().collect::<BTreeSet<_>>(), expected);
        for page in &pages {
            assert!(page.entries.len() <= MAXIMUM_PAGE_ENTRIES);
            assert!(!page.entries.is_empty());
            let used: usize = page.entries.iter().map(measured).sum();
            assert!(used <= bytes || page.entries.len() == 1, "budget {bytes}");
        }
    }
    assert_eq!(all_pages(&listings, scratch.path(), usize::MAX).len(), 2);

    // A directory of exactly one full page has no continuation.
    fs::remove_file(scratch.path().join(expected.iter().next().unwrap())).unwrap();
    let pages = all_pages(&listings, scratch.path(), usize::MAX);
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].entries.len(), MAXIMUM_PAGE_ENTRIES);
}

#[test]
fn a_cursor_continues_only_its_own_connection_and_path_once() {
    let scratch = Scratch::new("cursor");
    files(scratch.path(), 3);
    let other = scratch.path().join("other");
    fs::create_dir(&other).unwrap();
    let listings = listings();
    let never = AtomicBool::new(false);
    let one = budget(1);
    let first = listings
        .page(connection(1), scratch.path(), None, false, &one, &never)
        .unwrap();
    let cursor = first.cursor.unwrap();
    let invalid = Err(FilesError::new(ErrorCode::InvalidRequest));

    assert_eq!(
        listings.page(
            connection(2),
            scratch.path(),
            Some(cursor),
            false,
            &one,
            &never
        ),
        invalid
    );
    assert_eq!(
        listings.page(connection(1), &other, Some(cursor), false, &one, &never),
        invalid
    );
    let second = listings
        .page(
            connection(1),
            scratch.path(),
            Some(cursor),
            false,
            &one,
            &never,
        )
        .unwrap();
    assert_ne!(second.entries, first.entries);
    assert_eq!(
        listings.page(
            connection(1),
            scratch.path(),
            Some(cursor),
            false,
            &one,
            &never
        ),
        invalid
    );

    listings.connection_lost(connection(1));
    assert_eq!(
        listings.page(
            connection(1),
            scratch.path(),
            second.cursor,
            false,
            &one,
            &never
        ),
        invalid
    );
}

#[test]
fn a_seventeenth_cursor_closes_the_least_recently_used() {
    let scratch = Scratch::new("cursors");
    files(scratch.path(), 3);
    let listings = listings();
    let never = AtomicBool::new(false);
    let one = budget(1);
    let mut cursors: Vec<CursorId> = (0..=MAXIMUM_CURSORS)
        .map(|_| {
            listings
                .page(connection(1), scratch.path(), None, false, &one, &never)
                .unwrap()
                .cursor
                .unwrap()
        })
        .collect();
    let oldest = cursors.remove(0);
    assert_eq!(
        listings.page(
            connection(1),
            scratch.path(),
            Some(oldest),
            false,
            &one,
            &never
        ),
        Err(FilesError::new(ErrorCode::InvalidRequest))
    );
    for cursor in cursors {
        assert!(
            listings
                .page(
                    connection(1),
                    scratch.path(),
                    Some(cursor),
                    false,
                    &one,
                    &never
                )
                .is_ok()
        );
    }
}

#[test]
fn an_idle_cursor_closes_its_directory() {
    let scratch = Scratch::new("idle");
    files(scratch.path(), 3);
    let listings = Listings::with_idle(Arc::new(Timer::new()), Duration::from_millis(50));
    let never = AtomicBool::new(false);
    let page = listings
        .page(
            connection(1),
            scratch.path(),
            None,
            false,
            &budget(1),
            &never,
        )
        .unwrap();
    assert!(page.cursor.is_some());
    let deadline = Instant::now() + WAIT;
    while listings.shared.lock().open.contains_key(&connection(1)) {
        assert!(Instant::now() < deadline, "the idle cursor never closed");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_cancelled_call_stops_reading() {
    let scratch = Scratch::new("cancel");
    files(scratch.path(), 3);
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        listings().page(
            connection(1),
            scratch.path(),
            None,
            false,
            &budget(usize::MAX),
            &cancelled
        ),
        Err(FilesError::new(ErrorCode::Cancelled))
    );
}

/// Creates `levels` nested single-child directories below `root` and returns the deepest.
fn chain(root: &Path, levels: usize) -> PathBuf {
    let mut path = root.to_path_buf();
    for level in 0..levels {
        path = path.join(format!("d{level}"));
    }
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn the_single_child_chain_lists_up_to_64_complete_levels() {
    let scratch = Scratch::new("chain");
    let deepest = chain(scratch.path(), MAXIMUM_DESCENDANTS + 3);
    files(&deepest, 2);
    let listings = listings();
    let never = AtomicBool::new(false);
    let page = listings
        .page(
            connection(1),
            scratch.path(),
            None,
            true,
            &budget(usize::MAX),
            &never,
        )
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.descendants.len(), MAXIMUM_DESCENDANTS);
    let mut parent = scratch.path().to_path_buf();
    for (level, descendant) in page.descendants.iter().enumerate() {
        assert_eq!(descendant.path, parent.join(format!("d{level}")));
        assert_eq!(descendant.entries.len(), 1);
        parent = descendant.path.clone();
    }

    // A short chain ends with the first directory that has more than one entry, listed whole.
    let short = Scratch::new("short-chain");
    let deepest = chain(short.path(), 2);
    files(&deepest, 3);
    let page = listings
        .page(
            connection(1),
            short.path(),
            None,
            true,
            &budget(usize::MAX),
            &never,
        )
        .unwrap();
    assert_eq!(page.descendants.len(), 2);
    assert_eq!(page.descendants[1].path, deepest);
    assert_eq!(page.descendants[1].entries.len(), 3);
}

#[test]
fn the_single_child_chain_stops_before_a_listing_that_does_not_fit() {
    let scratch = Scratch::new("chain-budget");
    let deepest = chain(scratch.path(), 2);
    files(&deepest, 50);
    let never = AtomicBool::new(false);
    // Room for the two single-entry levels, not for fifty more entries.
    let bytes = 3 * 70 + 2 * descendant_overhead(&deepest).unwrap();
    let page = listings()
        .page(
            connection(1),
            scratch.path(),
            None,
            true,
            &budget(bytes),
            &never,
        )
        .unwrap();
    assert_eq!(page.descendants.len(), 1);
    assert_eq!(page.descendants[0].path, scratch.path().join("d0"));
    let used: usize = page.entries.iter().map(measured).sum::<usize>()
        + page
            .descendants
            .iter()
            .map(|descendant| {
                descendant_overhead(&descendant.path).unwrap()
                    + descendant.entries.iter().map(measured).sum::<usize>()
            })
            .sum::<usize>();
    assert!(used <= bytes);

    // Without the request, or on a page that is not the whole directory, no chain is listed.
    let plain = listings()
        .page(
            connection(1),
            scratch.path(),
            None,
            false,
            &budget(usize::MAX),
            &never,
        )
        .unwrap();
    assert!(plain.descendants.is_empty());
}

#[test]
fn listing_what_is_not_a_directory_says_so() {
    let scratch = Scratch::new("not-directory");
    let file = scratch.path().join("file");
    File::create(&file).unwrap();
    let never = AtomicBool::new(false);
    let listings = listings();
    let list = |path: &Path| {
        listings
            .page(
                connection(1),
                path,
                None,
                false,
                &budget(usize::MAX),
                &never,
            )
            .map_err(|error| error.code)
    };
    assert_eq!(list(&file), Err(ErrorCode::NotDirectory));
    assert_eq!(
        list(&scratch.path().join("missing")),
        Err(ErrorCode::NotFound)
    );
}

fn entry(directory: &Path, name: &str) -> Entry {
    let never = AtomicBool::new(false);
    listings()
        .page(
            connection(1),
            directory,
            None,
            false,
            &budget(usize::MAX),
            &never,
        )
        .unwrap()
        .entries
        .into_iter()
        .find(|entry| entry.name.as_str() == name)
        .unwrap()
}

#[test]
fn a_link_reports_its_target_or_unknown_when_the_target_is_gone() {
    let scratch = Scratch::new("links");
    let target_file = scratch.path().join("target.txt");
    fs::write(&target_file, b"twelve bytes").unwrap();
    let target_directory = scratch.path().join("target");
    fs::create_dir(&target_directory).unwrap();
    if let Err(error) = links(scratch.path(), &target_file, &target_directory) {
        // Creating symbolic links needs Developer Mode or elevation on Windows.
        eprintln!("symbolic links not created ({error}); the link rules are unverified here");
        return;
    }
    let file_link = entry(scratch.path(), "file-link");
    assert_eq!(
        (file_link.kind, file_link.symlink, file_link.size.get()),
        (EntryKind::File, true, 12)
    );
    let directory_link = entry(scratch.path(), "directory-link");
    assert_eq!(
        (directory_link.kind, directory_link.symlink),
        (EntryKind::Directory, true)
    );
    let dangling = entry(scratch.path(), "dangling");
    assert_eq!(
        (dangling.kind, dangling.symlink),
        (EntryKind::Unknown, true)
    );
    let read = entry::read(&scratch.path().join("dangling"), OsStr::new("dangling")).unwrap();
    assert_eq!(read, dangling);
    let plain = entry(scratch.path(), "target.txt");
    assert_eq!(
        (plain.kind, plain.symlink, plain.size.get()),
        (EntryKind::File, false, 12)
    );
}

#[cfg(unix)]
fn links(directory: &Path, file: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::symlink;
    symlink(file, directory.join("file-link"))?;
    symlink(target, directory.join("directory-link"))?;
    symlink(directory.join("gone"), directory.join("dangling"))
}

#[cfg(windows)]
fn links(directory: &Path, file: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::{symlink_dir, symlink_file};
    symlink_file(file, directory.join("file-link"))?;
    symlink_dir(target, directory.join("directory-link"))?;
    symlink_file(directory.join("gone"), directory.join("dangling"))
}

/// Junctions need no privilege, and Windows Explorer and VS Code treat them as links.
#[cfg(windows)]
#[test]
fn a_junction_is_a_link_to_a_directory() {
    let scratch = Scratch::new("junction");
    let target = scratch.path().join("target");
    fs::create_dir(&target).unwrap();
    let status = std::process::Command::new("cmd")
        .arg("/c")
        .arg("mklink")
        .arg("/J")
        .arg(scratch.path().join("junction"))
        .arg(&target)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
    let junction = entry(scratch.path(), "junction");
    assert_eq!(
        (junction.kind, junction.symlink),
        (EntryKind::Directory, true)
    );
    fs::remove_dir(&target).unwrap();
    let dangling = entry(scratch.path(), "junction");
    assert_eq!(
        (dangling.kind, dangling.symlink),
        (EntryKind::Unknown, true)
    );
}

/// macOS refuses names that are not valid UTF-8, so only Linux and Windows can hold one.
#[cfg(any(target_os = "linux", windows))]
#[test]
fn a_name_that_is_not_unicode_is_listed_as_unrepresentable() {
    let scratch = Scratch::new("unrepresentable");
    let name = invalid_name();
    File::create(scratch.path().join(&name)).unwrap();
    let never = AtomicBool::new(false);
    let page = listings()
        .page(
            connection(1),
            scratch.path(),
            None,
            false,
            &budget(usize::MAX),
            &never,
        )
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    assert!(page.entries[0].unrepresentable);
    assert_eq!(page.entries[0].name.as_str(), name.to_string_lossy());
    assert_eq!(page.entries[0].kind, EntryKind::File);
}

#[cfg(target_os = "linux")]
fn invalid_name() -> std::ffi::OsString {
    use std::os::unix::ffi::OsStringExt;
    std::ffi::OsString::from_vec(vec![b'f', 0xff])
}

#[cfg(windows)]
fn invalid_name() -> std::ffi::OsString {
    use std::os::windows::ffi::OsStringExt;
    std::ffi::OsString::from_wide(&[u16::from(b'f'), 0xd800])
}

#[test]
fn write_protection_follows_vs_code() {
    let scratch = Scratch::new("readonly");
    let path = scratch.path().join("file");
    File::create(&path).unwrap();
    assert!(!entry(scratch.path(), "file").readonly);
    protect(&path);
    assert!(entry(scratch.path(), "file").readonly);
}

/// VS Code reads only the owner's write bit, so a file others may write still shows as
/// protected.
#[cfg(unix)]
fn protect(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o466)).unwrap();
}

#[cfg(windows)]
fn protect(path: &Path) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(path, permissions).unwrap();
}
