use super::with_folder;
use std::{ffi::OsStr, path::Path};

fn appended(inherited: Option<&str>, folder: &str) -> Option<String> {
    with_folder(inherited.map(OsStr::new), Path::new(folder))
        .map(|value| value.into_string().unwrap())
}

#[test]
fn the_cohort_folder_goes_last_once() {
    let folder = "/home/a/.local/share/orchbee/versions/1.2.0";
    assert_eq!(
        appended(Some("/usr/bin:/bin"), folder).as_deref(),
        Some("/usr/bin:/bin:/home/a/.local/share/orchbee/versions/1.2.0")
    );
    // A PATH that already names the folder, with or without a trailing separator, stays.
    assert_eq!(appended(Some(&format!("/usr/bin:{folder}")), folder), None);
    assert_eq!(appended(Some(&format!("{folder}/:/usr/bin")), folder), None);
}

#[test]
fn a_path_the_system_default_covers_or_no_entry_can_name_stays() {
    assert_eq!(appended(None, "/opt/orchbee"), None);
    assert_eq!(appended(Some(""), "/opt/orchbee"), None);
    assert_eq!(appended(Some("/usr/bin"), "relative/orchbee"), None);
    assert_eq!(appended(Some("/usr/bin"), "/opt/a:b"), None);
}
