use super::*;
use crate::{
    SessionCatalog as Catalog, SessionName,
    durable_sessions::{Directory, Record},
};
use std::ffi::OsStr;

// A listing names the folders of `sessions/`, then reads each folder the names gave. A Session that
// a deletion removes between those two steps is no Session, so the listing skips it.
#[test]
fn a_session_removed_after_the_listing_named_it_is_skipped() {
    let base = Fixture::new();
    let local = Fixture::new();
    let root = local.root();
    let directory = Directory::create(&root).unwrap();
    let name = SessionName::parse("victim").unwrap();
    let runtime = base.0.canonicalize().unwrap().join(name.as_str());
    let record = Record::registered(&runtime, &name, [1; 16]).unwrap();
    directory.publish(name.as_str(), &record).unwrap();

    // Step one: the listing names the Session's folder.
    let listing = directory.listing().unwrap();
    assert!(listing.iter().any(|leaf| leaf == name.as_str()));
    // The deletion lands between the two steps.
    directory.remove(name.as_str()).unwrap();
    // Step two: the listing reads the folder it named, which is gone.
    assert!(
        directory
            .listed(OsStr::new(name.as_str()))
            .unwrap()
            .is_none()
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    let entries = Catalog::list(&base.0, &root, deadline).unwrap();
    assert!(entries.iter().all(|entry| entry.name != name));
}

// A writer that holds a Session folder for one step, such as the step that removes it, refuses the
// listing's pin of that folder for the step. The pin outlasts the hold: the writer lets go between
// the pin's first attempt and its second.
#[cfg(windows)]
#[test]
fn a_session_folder_pin_outlasts_a_writers_momentary_hold() {
    use std::os::windows::fs::OpenOptionsExt;
    const DELETE: u32 = 0x0001_0000;
    const BACKUP_SEMANTICS: u32 = 0x0200_0000;
    let local = Fixture::new();
    let folder = local.0.join("held");
    fs::create_dir(&folder).unwrap();
    let mut hold = Some(
        fs::OpenOptions::new()
            .access_mode(DELETE)
            .share_mode(7)
            .custom_flags(BACKUP_SEMANTICS)
            .open(&folder)
            .unwrap(),
    );
    let mut attempts = 0;
    let pins = crate::private_filesystem::outlast_open(|| {
        attempts += 1;
        if attempts == 2 {
            hold.take();
        }
        crate::private_filesystem::open_tree(&folder)
    })
    .unwrap();
    assert_eq!(attempts, 2);
    drop(pins);
}
