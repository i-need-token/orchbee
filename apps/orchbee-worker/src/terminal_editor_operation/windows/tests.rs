use super::*;
use std::{
    fs,
    os::windows::ffi::OsStringExt,
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "orchbee-editor-解析-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, b"lookup fixture, never launched").unwrap();
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
fn editor_command_preferences_preserve_native_argv_literals_without_shell_or_interactive_injection()
{
    let fixture = Fixture::new();
    let program = fixture.file("PowerShell 编辑器.exe");
    let command = format!(
        "\"{}\" --wait \"space 字\" \"\" \"&% !$()\"",
        program.display()
    );
    let (source, selection) = resolve_with(
        |name| match name {
            "VISUAL" => Some(command.clone().into()),
            "EDITOR" => panic!("VISUAL did not win"),
            _ => None,
        },
        || panic!("default was consulted"),
    )
    .unwrap();
    assert_eq!(source, Source::Visual);
    assert_eq!(Path::new(selection.executable()), program);
    assert_eq!(selection.arguments(), ["--wait", "space 字", "", "&% !$()"]);
    assert!(!selection.arguments().iter().any(|arg| arg == "-NoExit"));
    let command = format!("\"{}\" --from-editor", program.display());
    let (source, selection) = resolve_with(
        |name| match name {
            "VISUAL" => Some(" \t\r\n ".into()),
            "EDITOR" => Some(command.clone().into()),
            _ => None,
        },
        || panic!("default was consulted"),
    )
    .unwrap();
    assert_eq!(source, Source::Editor);
    assert_eq!(selection.arguments(), ["--from-editor"]);
}

#[test]
fn editor_command_default_uses_system_directory_without_environment_path_lookup() {
    let fixture = Fixture::new();
    let program = fixture.file("notepad.exe");
    let (source, selection) = resolve_with(
        |name| match name {
            "VISUAL" | "EDITOR" => Some(" \t ".into()),
            _ => panic!("fallback consulted inherited {name}"),
        },
        || Ok(fixture.0.clone()),
    )
    .unwrap();
    assert_eq!(source, Source::Default);
    assert_eq!(Path::new(selection.executable()), program);
    assert!(selection.arguments().is_empty());
    let native = winsafe::GetSystemDirectory().unwrap();
    let (_, selection) = resolve_with(|_| None, || Ok(PathBuf::from(&native))).unwrap();
    assert_eq!(
        Path::new(selection.executable()),
        Path::new(&native).join("notepad.exe")
    );
}

#[test]
fn editor_command_path_lookup_uses_absolute_directories_and_never_current_directory_or_pathext() {
    let fixture = Fixture::new();
    let exe = fixture.file("editor.exe");
    let path = std::env::join_paths([Path::new("relative"), fixture.0.as_path()]).unwrap();
    for name in ["editor", "editor.exe"] {
        let (_, selection) = resolve_with(
            |key| match key {
                "VISUAL" => Some(name.into()),
                "PATH" => Some(path.clone()),
                _ => panic!("unexpected {key}"),
            },
            || panic!("default"),
        )
        .unwrap();
        assert_eq!(Path::new(selection.executable()), exe);
    }
    let exact = fixture.file("editor");
    assert_eq!(
        Path::new(&resolve_program("editor", || Some(path.clone())).unwrap()),
        exact
    );
    for command in [
        r".\editor.exe",
        r"relative/editor.exe",
        r"C:editor.exe",
        r"\editor.exe",
        ".",
        "..",
    ] {
        assert_eq!(
            resolve_program(command, || panic!("relative program consulted PATH")),
            Err(Refusal::InvalidCommand)
        );
    }
    assert_eq!(
        resolve_program("editor.exe", || Some("relative".into())),
        Err(Refusal::ProgramUnavailable)
    );
    assert_eq!(
        resolve_program("missing", || Some(path.clone())),
        Err(Refusal::ProgramUnavailable)
    );
}

#[test]
fn editor_command_invalid_preference_refuses_whole_without_falling_through() {
    for value in [
        OsString::from_wide(&[0xd800]),
        OsString::from("bad\0.exe"),
        OsString::from(" ".to_owned() + "editor.exe"),
        OsString::from("x".repeat(MAXIMUM_LAUNCH_SELECTION_BYTES + 1)),
    ] {
        assert_eq!(
            resolve_with(
                |name| if name == "VISUAL" {
                    Some(value.clone())
                } else {
                    panic!("invalid VISUAL fell through to {name}")
                },
                || panic!("default")
            ),
            Err(Refusal::InvalidCommand)
        );
    }
    assert_eq!(
        resolve_with(
            |name| if name == "VISUAL" {
                Some("missing.exe".into())
            } else if name == "PATH" {
                None
            } else {
                panic!("unavailable VISUAL fell through")
            },
            || panic!("default")
        ),
        Err(Refusal::ProgramUnavailable)
    );
}

#[test]
fn editor_command_enforces_argument_and_escaped_byte_limits_before_returning_a_selection() {
    let fixture = Fixture::new();
    let program = fixture.file("editor.exe");
    let command = |count| format!("\"{}\"{}", program.display(), " \"\"".repeat(count));
    let (_, selection) = resolve_with(
        |key| (key == "VISUAL").then(|| command(128).into()),
        || panic!("default"),
    )
    .unwrap();
    assert_eq!(selection.arguments().len(), 128);
    assert_eq!(
        resolve_with(
            |key| (key == "VISUAL").then(|| command(129).into()),
            || panic!("default")
        ),
        Err(Refusal::InvalidCommand)
    );
    let escaped = format!("\"{}\" \"{}\"", program.display(), "\n".repeat(6000));
    assert_eq!(
        resolve_with(
            |key| (key == "VISUAL").then(|| escaped.clone().into()),
            || panic!("default")
        ),
        Err(Refusal::InvalidCommand)
    );
}
