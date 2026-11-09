use super::*;
use crate::localization::Locale;
#[test]
fn console_default_titles_name_their_program_and_program_set_titles_stay_as_written() {
    for (raw, en_us, zh_cn) in [
        (
            r"C:\Program Files\PowerShell\7\pwsh.exe",
            "PowerShell",
            "PowerShell",
        ),
        (
            r"C:\WINDOWS\System32\WindowsPowerShell\v1.0\powershell.exe",
            "Windows PowerShell",
            "Windows PowerShell",
        ),
        (
            r"Administrator: C:\WINDOWS\system32\cmd.exe",
            "Command Prompt",
            "命令提示符",
        ),
        (
            r"管理员: C:\WINDOWS\system32\cmd.exe",
            "Command Prompt",
            "命令提示符",
        ),
        (
            r"C:\WINDOWS\system32\cmd.exe - ping  localhost",
            "Command Prompt - ping  localhost",
            "命令提示符 - ping  localhost",
        ),
        (r"C:\My - Tools\app.exe - run", "app - run", "app - run"),
        (r"C:\Program Files\Git\bin\BASH.EXE", "Bash", "Bash"),
        ("c:/tools/build.CMD", "build", "build"),
        (r"\\server\share\tool.exe", "tool", "tool"),
        (r"⠋ C:\Windows\System32\wsl.exe", "wsl", "wsl"),
    ] {
        let name = ChromeName::of_title(raw).unwrap();
        assert!(matches!(name, ChromeName::Program { .. }), "{raw}");
        assert_eq!(Locale::EnUs.chrome_name(name), en_us, "{raw}");
        assert_eq!(Locale::ZhCn.chrome_name(name), zh_cn, "{raw}");
    }
    for raw in [
        r"C:\Users\luna\project",
        r"C:\Users\luna\notes.txt",
        "vim pwsh.exe",
        r"PS C:\work\app.exe",
        "pwsh",
        "Command Prompt - ping",
        "make test",
    ] {
        assert_eq!(
            ChromeName::of_title(raw),
            Some(ChromeName::Text(raw)),
            "{raw}"
        );
    }
    assert_eq!(ChromeName::of_title("  "), None);
}

#[test]
fn launch_paths_name_their_program_by_one_rule_on_every_platform() {
    for (path, en_us, zh_cn) in [
        ("/bin/zsh", "Zsh", "Zsh"),
        ("/usr/bin/bash", "Bash", "Bash"),
        ("/opt/homebrew/bin/fish", "fish", "fish"),
        ("/usr/local/bin/node", "node", "node"),
        ("pwsh", "PowerShell", "PowerShell"),
        (
            r"C:\Program Files\PowerShell\7\PWSH.EXE",
            "PowerShell",
            "PowerShell",
        ),
        ("cmd.exe", "Command Prompt", "命令提示符"),
        (r"C:\tools\node.exe", "node", "node"),
    ] {
        let name = ChromeName::Program {
            program: Program::from_path(path).unwrap(),
            command: None,
        };
        assert_eq!(Locale::EnUs.chrome_name(name), en_us, "{path}");
        assert_eq!(Locale::ZhCn.chrome_name(name), zh_cn, "{path}");
    }
    for path in ["", "  ", r"C:\tools\", "/usr/bin/"] {
        assert_eq!(Program::from_path(path), None, "{path}");
    }
}
