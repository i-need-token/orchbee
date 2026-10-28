use super::*;
#[test]
fn windows_title_stripping_matches_one_elevation_and_one_bounded_activity_prefix() {
    for raw in [
        "⠋ task",
        "✳ task",
        "  ⠙   task  ",
        "✢ task",
        "✻ task",
        "◐ task",
        "◓ task",
        "◑ task",
        "◒ task",
        "Administrator:   ⠋ task",
    ] {
        assert_eq!(stripped(raw), Some("task"));
    }
    for (raw, expected) in [
        ("⠋ ⠙ task", "⠙ task"),
        ("Administrator: Administrator: task", "Administrator: task"),
        ("★task", "★task"),
        ("✨ task", "✨ task"),
        ("@ task", "@ task"),
        ("task ⠋ detail", "task ⠋ detail"),
        ("⠋task", "⠋task"),
        (" ⠋ 修复🙂标题 ", "修复🙂标题"),
    ] {
        assert_eq!(stripped(raw), Some(expected));
    }
    for raw in ["", "  ", "⠋   ", "Administrator: "] {
        assert_eq!(stripped(raw), None);
    }
}

#[test]
fn chrome_shows_only_the_file_name_of_a_default_console_executable_title() {
    for (raw, expected) in [
        (r"C:\Program Files\PowerShell\7\pwsh.exe", "pwsh.exe"),
        (r"Administrator: C:\WINDOWS\system32\cmd.exe", "cmd.exe"),
        ("c:/tools/build.CMD", "build.CMD"),
        (r"\\server\share\tool.exe", "tool.exe"),
        (r"⠋ C:\Windows\System32\wsl.exe", "wsl.exe"),
    ] {
        assert_eq!(display(raw), Some(expected), "{raw}");
    }
    for raw in [
        r"C:\Users\luna\project",
        r"C:\Users\luna\notes.txt",
        "vim pwsh.exe",
        r"PS C:\work\app.exe",
        "make test",
    ] {
        assert_eq!(display(raw), Some(raw), "{raw}");
    }
    assert_eq!(display("  "), None);
}
