//! The link rule over a scratch home: the step owns only a link to an Orchbee version's Client,
//! never replaces anything else at its name, and never leaves the name empty.
use super::*;

fn scratch(tag: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "obcli-path-link-{}-{tag}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir(&directory).expect("create scratch home");
    directory
}

#[test]
fn the_link_names_the_installed_version_and_leaves_anything_else() {
    let home = scratch("rule");
    let root = home.join("store");
    let link = link_path(Some(home.as_os_str())).expect("absolute home");
    let first = client(&root, "1.0.0");
    let second = client(&root, "1.1.0");

    // A missing folder and link are created.
    assert_eq!(place(&link, &first, &root).unwrap(), Planned::Add);
    assert_eq!(fs::read_link(&link).unwrap(), first);
    assert_eq!(occupant(&link, &root).unwrap(), Occupant::Store);
    // The same version is kept; another version of the store is followed.
    assert_eq!(place(&link, &first, &root).unwrap(), Planned::Keep);
    assert_eq!(place(&link, &second, &root).unwrap(), Planned::Keep);
    assert_eq!(fs::read_link(&link).unwrap(), second);
    // A link to another store's version is Orchbee's and is replaced.
    let other = home.join("elsewhere");
    assert_eq!(occupant(&link, &other).unwrap(), Occupant::OtherStore);
    assert_eq!(
        place(&link, &client(&other, "1.1.0"), &other).unwrap(),
        Planned::Replace
    );
    // Nothing but the link is left in the folder.
    let names: Vec<_> = fs::read_dir(link.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, [link.file_name().unwrap().to_owned()]);

    // A foreign link and a file stay as they were, and the step fails.
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink("/usr/bin/true", &link).unwrap();
    assert_eq!(occupant(&link, &root).unwrap(), Occupant::Foreign);
    assert!(place(&link, &first, &root).is_err());
    assert_eq!(fs::read_link(&link).unwrap(), Path::new("/usr/bin/true"));
    fs::remove_file(&link).unwrap();
    fs::write(&link, b"#!/bin/sh\n").unwrap();
    assert!(place(&link, &first, &root).is_err());
    assert_eq!(fs::read(&link).unwrap(), b"#!/bin/sh\n");
    assert!(!store_link(&link, &root).unwrap());

    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn a_relative_or_missing_home_names_no_link() {
    assert_eq!(link_path(None), None);
    assert_eq!(link_path(Some(OsStr::new("relative/home"))), None);
    assert_eq!(
        link_path(Some(OsStr::new("/home/a"))),
        Some(Path::new("/home/a/.local/bin").join(environment_stem()))
    );
}

#[test]
fn path_names_the_folder_only_as_a_whole_entry() {
    let folder = Path::new("/home/a/.local/bin");
    for path in [
        "/home/a/.local/bin",
        "/usr/bin:/home/a/.local/bin/",
        "/home/a/.local/bin//:/usr/bin",
    ] {
        assert!(names_folder(Some(OsStr::new(path)), folder), "{path}");
    }
    for path in [
        "",
        "/usr/bin",
        "/home/a/.local/bin/x:/usr/bin",
        "~/.local/bin",
        "/home/a/.local",
    ] {
        assert!(!names_folder(Some(OsStr::new(path)), folder), "{path}");
    }
    assert!(!names_folder(None, folder));
}
