use super::*;
use std::fs;

/// The `PATH` a login zsh leaves after its profile and interactive startup file add folders comes
/// back although both print first, and a shell that prints no answer gives none.
#[test]
fn the_login_shell_path_comes_back_past_startup_output() {
    let home = std::env::temp_dir().join(format!(
        "orchbee-login-path-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join(".zprofile"),
        "export PATH=\"$HOME/profile:$PATH\"\necho noise\n",
    )
    .unwrap();
    fs::write(
        home.join(".zshrc"),
        "export PATH=\"$HOME/rc:$PATH\"\necho noise\n",
    )
    .unwrap();
    let mut shell = Command::new("/bin/zsh");
    shell.env("HOME", &home).env("ZDOTDIR", &home);
    let path = lookup(shell, Instant::now() + LIMIT, || false).expect("the login shell's PATH");
    let folders: Vec<_> = std::env::split_paths(&path).take(2).collect();
    assert_eq!(folders, [home.join("rc"), home.join("profile")]);
    assert_eq!(
        lookup(
            Command::new("/usr/bin/true"),
            Instant::now() + LIMIT,
            || false
        ),
        None
    );
    fs::remove_dir_all(&home).unwrap();
}
