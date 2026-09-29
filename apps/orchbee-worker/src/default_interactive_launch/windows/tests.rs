use super::*;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "orchbee-shell-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn file(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, b"resolver fixture, never executed").unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for entry in fs::read_dir(&self.0).unwrap() {
            fs::remove_file(entry.unwrap().path()).unwrap();
        }
        fs::remove_dir(&self.0).unwrap();
    }
}

#[test]
fn configured_shell_resolves_absolute_or_path_names_without_fallback() {
    let fixture = Fixture::new();
    let path = fixture.file("pwsh.exe");
    let search = std::env::join_paths([Path::new("relative"), fixture.0.as_path()]).unwrap();
    for name in [path.to_str().unwrap(), "pwsh.exe", "pwsh"] {
        let shell = WindowsShellSettings::new(name, "auto").unwrap();
        let launch = resolve(&shell, |name| (name == "PATH").then(|| search.clone())).unwrap();
        assert_eq!(Path::new(&launch.executable), path);
        assert!(launch.arguments.is_empty());
    }
    let missing = WindowsShellSettings::new("missing.exe", "auto").unwrap();
    assert_eq!(
        resolve(&missing, |name| match name {
            "PATH" => Some(search.clone()),
            "COMSPEC" => Some(path.clone().into_os_string()),
            _ => None,
        }),
        Err(DefaultInteractiveLaunchError::NoUsableShell)
    );
    assert!(resolve_configured("pwsh", Some("relative".into())).is_none());
    let original = fixture.file("pwsh");
    assert_eq!(
        Path::new(&resolve_configured("pwsh", Some(fixture.0.clone().into_os_string())).unwrap()),
        original
    );
}

#[test]
fn windows_login_modes_follow_the_pinned_herdr_shell_name_partition() {
    let fixture = Fixture::new();
    for name in [
        "sh",
        "bash",
        "zsh",
        "ksh",
        "dash",
        "ash",
        "mksh",
        "fish",
        "csh",
        "tcsh",
        "pwsh",
        "powershell",
        "cmd",
        "nu",
        "notbash",
    ] {
        let path = fixture.file(&format!("{}.EXE", name.to_ascii_uppercase()));
        for mode in ["auto", "login", "non_login"] {
            let shell = WindowsShellSettings::new(path.to_str().unwrap(), mode).unwrap();
            let launch = resolve(&shell, |_| None).unwrap();
            let login =
                mode == "login" && !["pwsh", "powershell", "cmd", "nu", "notbash"].contains(&name);
            assert_eq!(
                launch.arguments,
                if login { vec!["-l"] } else { vec![] },
                "{name}/{mode}"
            );
        }
    }
    let fallback = fixture.file("fallback.exe");
    let launch = resolve(&WindowsShellSettings::default(), |name| {
        (name == "COMSPEC").then(|| fallback.clone().into_os_string())
    })
    .unwrap();
    assert_eq!(Path::new(&launch.executable), fallback);
}
