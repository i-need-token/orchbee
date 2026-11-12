use super::{ClipboardRoute, Exit, Facts, Notifier, Overrun, Program, locate, run};
use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

struct Scratch(PathBuf);
impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "orchbee-host-program-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn script(&self, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = self.0.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn linux(variables: &[(&str, &str)], programs: &[&str]) -> Facts {
    let variables: HashMap<_, _> = variables.iter().copied().collect();
    Facts::detect(
        |name| variables.get(name).map(OsString::from),
        false,
        |name| {
            programs
                .contains(&name)
                .then(|| Path::new("/opt/bin").join(name))
        },
    )
}

#[test]
fn local_desktops_prefer_their_clipboard_program_and_remote_logins_prefer_the_terminal() {
    let wayland = linux(
        &[("WAYLAND_DISPLAY", "wayland-0"), ("DISPLAY", ":0")],
        &["wl-copy", "xclip", "xdg-open", "notify-send"],
    );
    assert_eq!(
        wayland.clipboard,
        Some(ClipboardRoute::Program(Program {
            path: "/opt/bin/wl-copy".into(),
            arguments: &["--type", "text/plain;charset=utf-8"],
        }))
    );
    assert_eq!(
        wayland.browser.map(|program| program.path),
        Some("/opt/bin/xdg-open".into())
    );
    assert_eq!(
        wayland.notifier,
        Some(Notifier::NotifySend("/opt/bin/notify-send".into()))
    );

    let x11 = linux(&[("DISPLAY", ":0"), ("TERM", "xterm-ghostty")], &["xsel"]);
    assert_eq!(
        x11.clipboard,
        Some(ClipboardRoute::Program(Program {
            path: "/opt/bin/xsel".into(),
            arguments: &["--clipboard", "--input"],
        }))
    );
    assert_eq!(x11.browser, None);
    assert!(x11.terminal_notifications);

    let remote = linux(
        &[
            ("SSH_CONNECTION", "a"),
            ("TERM", "xterm-kitty"),
            ("DISPLAY", "localhost:10.0"),
        ],
        &["xclip", "xdg-open", "notify-send"],
    );
    assert_eq!(remote.clipboard, Some(ClipboardRoute::Terminal));
    assert_eq!((remote.browser, remote.notifier), (None, None));
}

#[test]
fn sessions_without_a_route_refuse_instead_of_assuming_one() {
    let bare = linux(
        &[("SSH_TTY", "/dev/pts/1"), ("TERM", "xterm-256color")],
        &[],
    );
    assert_eq!(bare.clipboard, None);
    assert_eq!((bare.browser, bare.notifier), (None, None));
    assert!(!bare.terminal_notifications);
    assert!(bare.window_title);

    let multiplexed = linux(&[("TMUX", "/tmp/tmux"), ("TERM_PROGRAM", "ghostty")], &[]);
    assert_eq!(multiplexed.clipboard, None);

    let console = linux(&[("TERM", "linux")], &["xclip"]);
    assert_eq!(console.clipboard, None);
    assert!(!console.window_title);
}

#[test]
fn programs_resolve_only_from_absolute_search_entries() {
    let scratch = Scratch::new("locate");
    scratch.script("xdg-open", "exit 0");
    let search = std::env::join_paths([Path::new("relative"), &scratch.0]).unwrap();
    assert_eq!(
        locate(Some(&search), "xdg-open"),
        Some(scratch.0.join("xdg-open"))
    );
    let relative = std::env::join_paths([Path::new(".")]).unwrap();
    assert_eq!(locate(Some(&relative), "xdg-open"), None);
}

#[test]
fn programs_receive_exact_arguments_and_input_without_a_shell() {
    let scratch = Scratch::new("argv");
    let record = scratch.0.join("record");
    let program = scratch.script(
        "fake-copy",
        &format!(
            "for argument in \"$@\"; do printf '%s\\0' \"$argument\"; done > '{0}.argv'\ncat > '{0}.input'",
            record.display()
        ),
    );
    let hostile = "$(touch pwned); `id` | ; & 'quoted' \"double\"";
    let input = "line one\n$HOME é\u{1F41D}";
    let exit = run(
        &program,
        &[OsStr::new("--type"), OsStr::new(hostile)],
        Some(input.as_bytes()),
        Duration::from_secs(10),
        Overrun::Kill,
    );
    assert_eq!(exit, Exit::Succeeded);
    let argv = std::fs::read(record.with_extension("argv")).unwrap();
    assert_eq!(argv, format!("--type\0{hostile}\0").into_bytes());
    let received = std::fs::read(record.with_extension("input")).unwrap();
    assert_eq!(received, input.as_bytes());
}

#[test]
fn failing_and_overrunning_programs_report_without_holding_the_caller() {
    let scratch = Scratch::new("deadline");
    let failing = scratch.script("fail", "cat > /dev/null; exit 3");
    assert_eq!(
        run(
            &failing,
            &[],
            Some(b"text"),
            Duration::from_secs(10),
            Overrun::Kill
        ),
        Exit::Failed
    );
    let missing = scratch.0.join("missing");
    assert_eq!(
        run(&missing, &[], None, Duration::from_secs(10), Overrun::Kill),
        Exit::Failed
    );
    // A program that never reads its input cannot block the write past the deadline.
    let stuck = scratch.script("stuck", "exec sleep 30");
    let started = Instant::now();
    let large = vec![b'x'; 4 * 1024 * 1024];
    assert_eq!(
        run(
            &stuck,
            &[],
            Some(&large),
            Duration::from_millis(300),
            Overrun::Kill
        ),
        Exit::Overran
    );
    assert!(started.elapsed() < Duration::from_secs(10));
}
