//! The startup-file rule over a scratch home: the two lines are added once, removed exactly, and
//! every other line stays.
use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch() -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "obcli-path-profile-{}-{}",
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
fn the_lines_are_added_once_and_removed_exactly() {
    let home = scratch();
    let zsh = Some(OsStr::new("/bin/zsh"));
    let zshenv = home.join(".zshenv");

    // A missing file is made with the two lines, which a second add does not repeat.
    assert_eq!(add(&home, zsh), Some(zshenv.clone()));
    assert_eq!(add(&home, zsh), Some(zshenv.clone()));
    assert_eq!(
        fs::read_to_string(&zshenv).unwrap(),
        format!("{}\n{LINE}\n", marker())
    );
    // Removing them leaves no file the add made.
    remove(&home).unwrap();
    assert!(!zshenv.exists());

    // A person's lines around them stay, a last line without its newline too.
    fs::write(&zshenv, "export A=1").unwrap();
    add(&home, zsh).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&zshenv)
        .unwrap()
        .write_all(b"export B=2\n")
        .unwrap();
    assert_eq!(holders(&home), std::slice::from_ref(&zshenv));
    remove(&home).unwrap();
    assert_eq!(
        fs::read_to_string(&zshenv).unwrap(),
        "export A=1\nexport B=2\n"
    );
    assert!(holders(&home).is_empty());

    // bash gets its first login file; another shell gets none.
    let bash = Some(OsStr::new("/bin/bash"));
    assert_eq!(target(&home, bash), Some(home.join(".profile")));
    fs::write(home.join(".bash_profile"), "").unwrap();
    assert_eq!(target(&home, bash), Some(home.join(".bash_profile")));
    assert_eq!(
        target(&home, Some(OsStr::new("/opt/homebrew/bin/fish"))),
        None
    );
    assert_eq!(target(&home, None), Some(zshenv));

    fs::remove_dir_all(&home).unwrap();
}
