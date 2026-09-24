use std::{error::Error, ffi::OsStr, process::Command};

#[test]
fn skill_is_offline_localized_and_describes_the_native_surface() -> Result<(), Box<dyn Error>> {
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        let output = Command::new(env!("CARGO_BIN_EXE_orchbee"))
            .arg("--skill")
            .env("LC_ALL", locale)
            .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
            .output()?;
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let document = String::from_utf8(output.stdout)?;
        assert!(document.contains("orchbee completion powershell"));
        assert!(document.contains("--after-revision"));
        assert!(document.contains("--paste"));
        assert!(document.contains("1 MiB"));
        assert!(document.contains("Windows"));
    }
    Ok(())
}

const EN_US_HELP: &str = "Orchbee\n\nUsage:\n  orchbee\n  orchbee tui [--mouse on|off]\n  orchbee status [--format text|json]\n  orchbee <family> <command> [operands] [--format text|json]\n  orchbee api snapshot|schema\n  orchbee site start [--format text|json]\n  orchbee site diagnose [--format text|json]\n  orchbee site stop --confirm [--detach] [--format text|json]\n  orchbee site stop-status <request-id> [--format text|json]\n  orchbee config check [--format text|json]\n  orchbee help [<family>|<command>]\n  orchbee completion bash|zsh|fish|powershell|elvish\n  orchbee -h|--help\n  orchbee --version\n  orchbee --skill\n\nModes and queries:\n  orchbee              Open the local terminal (Windows/Linux development cohort)\n  orchbee tui          Open the same local terminal explicitly\n  orchbee status       Read the running local Site's Workbench snapshot\n  orchbee project      list | get\n  orchbee workspace    list | get | close-status\n  orchbee tab          list | get\n  orchbee panel        list | get | current | process-info\n                       layout | neighbor | edges\n  orchbee client       list | get | control-status\n  orchbee terminal     list | get | read | wait-output | send\n  orchbee worktree     list | status\n  orchbee launch-profile list | get\n  orchbee agent        list | get | explain | doctor | wait | names | resolve\n                       read | send-keys | prompt | rename | start\n  orchbee agent integration\n                       list | explain | enable | disable — local provider\n                       onboarding; never contacts a Site\n  orchbee api          Machine snapshot and embedded schema documents\n\nSite lifecycle:\n  orchbee site start   Ensure the local Site is running (no TTY needed)\n  orchbee site diagnose\n                       Inspect the local Site runtime — read-only facts\n  orchbee site stop --confirm\n                       Stop the managed Windows Site and its terminals\n  orchbee site stop-status <id>\n                       Inspect a stop attempt or completed receipt\n\nMutations (one command per invocation, against the running Site):\n  orchbee project      create | rename\n  orchbee workspace    create | rename | accept-directory\n                       close | close-resume | focus\n  orchbee tab          create | rename | move | close | focus\n  orchbee panel        split | rename | move | close | focus | zoom | resize\n  orchbee layout       set-split-ratio (Windows TUI)\n  orchbee terminal     send\n  orchbee launch-profile create | delete\n  orchbee worktree     create | open | remove | resume (Windows)\n\nInspection and help:\n  orchbee config check Inspect local runtime/cohort/locale/socket inputs\n  orchbee help         Show help for a family or complete command path\n  orchbee completion   Emit a static shell completion script\n  orchbee -h|--help    Show this help; after a family or command path, show\n                       that path's help\n  orchbee --version    Show the version\n  orchbee --skill      Print the embedded Agent guide (offline)\n\nRun 'orchbee help <family>' for filters and target rules, or\n'orchbee help <family> <command>' for one mutation's full syntax.\n";
const ZH_CN_HELP: &str = "Orchbee\n\n用法：\n  orchbee\n  orchbee tui [--mouse on|off]\n  orchbee status [--format text|json]\n  orchbee <family> <command> [操作数] [--format text|json]\n  orchbee api snapshot|schema\n  orchbee site start [--format text|json]\n  orchbee site diagnose [--format text|json]\n  orchbee site stop --confirm [--detach] [--format text|json]\n  orchbee site stop-status <request-id> [--format text|json]\n  orchbee config check [--format text|json]\n  orchbee help [<family>|<command>]\n  orchbee completion bash|zsh|fish|powershell|elvish\n  orchbee -h|--help\n  orchbee --version\n  orchbee --skill\n\n模式与查询：\n  orchbee              打开本地终端（Windows/Linux 开发 cohort）\n  orchbee tui          显式打开同一本地终端\n  orchbee status       读取正在运行的本地 Site 的 Workbench 快照\n  orchbee project      list | get\n  orchbee workspace    list | get | close-status\n  orchbee tab          list | get\n  orchbee panel        list | get | current | process-info\n                       layout | neighbor | edges\n  orchbee client       list | get | control-status\n  orchbee terminal     list | get | read | wait-output | send\n  orchbee worktree     list | status\n  orchbee launch-profile list | get\n  orchbee agent        list | get | explain | doctor | wait | names | resolve\n                       read | send-keys | prompt | rename | start\n  orchbee agent integration\n                       list | explain | enable | disable——本地 provider\n                       接入；绝不接触 Site\n  orchbee api          机器快照与内嵌 schema 文档\n\nSite 生命周期：\n  orchbee site start   确保本地 Site 正在运行（无需 TTY）\n  orchbee site diagnose\n                       检查本地 Site 运行时——只读事实\n  orchbee site stop --confirm\n                       停止托管 Windows Site 及其终端\n  orchbee site stop-status <id>\n                       查询停止尝试或完成回执\n\n修改命令（每次调用执行一条命令，作用于运行中的 Site）：\n  orchbee project      create | rename\n  orchbee workspace    create | rename | accept-directory\n                       close | close-resume | focus\n  orchbee tab          create | rename | move | close | focus\n  orchbee panel        split | rename | move | close | focus | zoom | resize\n  orchbee layout       set-split-ratio (Windows TUI)\n  orchbee terminal     send\n  orchbee launch-profile create | delete\n  orchbee worktree     create | open | remove | resume (Windows)\n\n检查与帮助：\n  orchbee config check 检查本地运行时/cohort/locale/套接字输入\n  orchbee help         显示某个 family 或完整命令路径的帮助\n  orchbee completion   输出静态 shell 补全脚本\n  orchbee -h|--help    显示此帮助；跟在 family 或命令路径后时显示该路径的帮助\n  orchbee --version    显示版本\n  orchbee --skill      离线输出内嵌 Agent 指引\n\n运行 'orchbee help <family>' 查看过滤参数与 target 规则，或\n'orchbee help <family> <command>' 查看单条修改命令的完整语法。\n";
#[test]
fn emits_exact_help_in_both_locales() -> Result<(), Box<dyn Error>> {
    let english = run(["--help"], "en_US.UTF-8")?;
    assert_eq!(english.status.code(), Some(0));
    assert_eq!(String::from_utf8(english.stdout)?, EN_US_HELP);
    assert!(english.stderr.is_empty());

    let chinese = run(["--help"], "zh_CN.UTF-8")?;
    assert_eq!(chinese.status.code(), Some(0));
    assert_eq!(String::from_utf8(chinese.stdout)?, ZH_CN_HELP);
    assert!(chinese.stderr.is_empty());
    Ok(())
}

#[test]
fn emits_one_locale_neutral_version_line() -> Result<(), Box<dyn Error>> {
    let output = run(["--version"], "zh_CN.UTF-8")?;
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout)?,
        concat!("orchbee ", env!("CARGO_PKG_VERSION"), "\n")
    );
    assert!(output.stderr.is_empty());
    Ok(())
}

#[test]
fn bare_non_tty_invocation_fails_before_startup() -> Result<(), Box<dyn Error>> {
    for arguments in [Vec::<&str>::new(), vec!["tui"]] {
        let english = run(arguments.iter().copied(), "en_US.UTF-8")?;
        assert_eq!(english.status.code(), Some(1));
        assert!(english.stdout.is_empty());
        assert_eq!(
            String::from_utf8(english.stderr)?,
            if cfg!(any(target_os = "linux", windows)) {
                "Orchbee requires the same interactive terminal on standard input and output, with valid dimensions.\n"
            } else {
                "The Orchbee TUI is unavailable in this development slice.\n"
            }
        );

        let chinese = run(arguments.iter().copied(), "zh_CN.UTF-8")?;
        assert_eq!(chinese.status.code(), Some(1));
        assert!(chinese.stdout.is_empty());
        assert_eq!(
            String::from_utf8(chinese.stderr)?,
            if cfg!(any(target_os = "linux", windows)) {
                "Orchbee 的标准输入和输出需要连接同一交互终端，并提供有效尺寸。\n"
            } else {
                "Orchbee TUI 在本开发切片中尚不可用。\n"
            }
        );
    }
    Ok(())
}

#[test]
fn suffix_help_on_a_complete_path_succeeds() -> Result<(), Box<dyn Error>> {
    let output = run(["tui", "--help"], "en_US.UTF-8")?;
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout)?,
        "orchbee tui [--mouse on|off]\n\nOpen the local terminal interface (Windows/Linux development cohort).\nA bare 'orchbee' invocation does the same.\n\n  --mouse on|off   Host pointer capture for pane hit-testing; on by\n                   default. Only the explicit 'tui' route accepts it.\n"
    );
    assert!(output.stderr.is_empty());
    Ok(())
}

#[test]
fn help_flag_aliases_match_the_help_command_at_every_recognized_path() -> Result<(), Box<dyn Error>>
{
    // `<path> --help` and `<path> -h` emit the identical document `help <path>`
    // emits, at the root and at every family and complete command path — all
    // without touching a running Site.
    let root = run(["--help"], "en_US.UTF-8")?;
    assert_eq!(root.status.code(), Some(0));
    for arguments in [["-h"], ["help"]] {
        let output = run(arguments, "en_US.UTF-8")?;
        assert_eq!(output.status.code(), Some(0), "{arguments:?}");
        assert_eq!(output.stdout, root.stdout, "{arguments:?}");
        assert!(output.stderr.is_empty());
    }

    let paths: &[&[&str]] = &[
        &["tui"],
        &["status"],
        &["project"],
        &["workspace"],
        &["tab"],
        &["panel"],
        &["terminal"],
        &["worktree"],
        &["agent"],
        &["site"],
        &["api"],
        &["config"],
        &["completion"],
        &["project", "list"],
        &["project", "get"],
        &["project", "create"],
        &["project", "rename"],
        &["workspace", "list"],
        &["workspace", "get"],
        &["workspace", "create"],
        &["workspace", "rename"],
        &["workspace", "accept-directory"],
        &["tab", "list"],
        &["tab", "get"],
        &["tab", "create"],
        &["tab", "rename"],
        &["tab", "move"],
        &["tab", "close"],
        &["panel", "list"],
        &["panel", "get"],
        &["panel", "split"],
        &["panel", "close"],
        &["panel", "move"],
        &["terminal", "list"],
        &["terminal", "get"],
        &["terminal", "read"],
        &["terminal", "wait-output"],
        &["terminal", "send"],
        &["worktree", "list"],
        &["agent", "list"],
        &["agent", "get"],
        &["agent", "explain"],
        &["site", "start"],
        &["api", "snapshot"],
        &["api", "schema"],
        &["config", "check"],
    ];
    for path in paths {
        let mut help = vec!["help"];
        help.extend_from_slice(path);
        let canonical = run(help.iter().copied(), "en_US.UTF-8")?;
        assert_eq!(canonical.status.code(), Some(0), "{help:?}");
        for flag in ["--help", "-h"] {
            let mut aliased = path.to_vec();
            aliased.push(flag);
            let output = run(aliased.iter().copied(), "en_US.UTF-8")?;
            assert_eq!(output.status.code(), Some(0), "{aliased:?}");
            assert_eq!(output.stdout, canonical.stdout, "{aliased:?}");
            assert!(output.stderr.is_empty(), "{aliased:?}");
        }
    }
    Ok(())
}

#[test]
fn help_flag_ownership_terminator_and_conflicts_stay_usage() -> Result<(), Box<dyn Error>> {
    let cases: &[(&[&str], &str)] = &[
        // A flag value owns the next word: --format consumes --help, leaving
        // an invalid format selection rather than a help request.
        (
            &["--format", "--help"],
            "Invalid value \"--help\" for '--format' (expected 'text' or 'json'). Run 'orchbee --help'.\n",
        ),
        (
            &["tab", "create", "--cols", "--help"],
            "'orchbee tab create' requires '--workspace' or '--standalone'. Run 'orchbee tab create --help'.\n",
        ),
        // After `--` every word is literal; "--help" is then an unknown path.
        (
            &["--", "--help"],
            "Unknown command \"--help\". Run 'orchbee --help'.\n",
        ),
        (
            &["tab", "--", "--help"],
            "Unknown command \"--help\" for 'orchbee tab'. Run 'orchbee tab --help'.\n",
        ),
        // Version and help flags conflict; a second help flag is never legal.
        (
            &["--version", "--help"],
            "'--version' is not valid in this position. Run 'orchbee --help'.\n",
        ),
        (
            &["--help", "--version"],
            "'--help' is not valid in this position. Run 'orchbee --help'.\n",
        ),
        (
            &["--help", "--help"],
            "'--help' is not valid in this position. Run 'orchbee --help'.\n",
        ),
        (
            &["-h", "-h"],
            "'-h' is not valid in this position. Run 'orchbee --help'.\n",
        ),
        // A help flag mid-path is not a suffix.
        (
            &["--help", "status"],
            "'--help' is not valid in this position. Run 'orchbee status --help'.\n",
        ),
        (
            &["-h", "status"],
            "'-h' is not valid in this position. Run 'orchbee status --help'.\n",
        ),
        // Business arguments suppress suffix help.
        (
            &["project", "get", "x", "--help"],
            "No help topic matches \"project get x\". Run 'orchbee --help'.\n",
        ),
        (
            &["status", "extra", "--help"],
            "No help topic matches \"status extra\". Run 'orchbee --help'.\n",
        ),
        // Format on a help request is a syntax failure.
        (
            &["status", "--help", "--format", "text"],
            "'--format' is not a valid option for 'orchbee status'. Run 'orchbee status --help'.\n",
        ),
    ];
    for (arguments, stderr) in cases {
        let output = run(arguments.iter().copied(), "en_US.UTF-8")?;
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert_eq!(String::from_utf8(output.stderr)?, *stderr, "{arguments:?}");
    }
    Ok(())
}

#[test]
fn rejects_unknown_short_and_additional_arguments_before_output() -> Result<(), Box<dyn Error>> {
    let cases: &[(&[&str], &str)] = &[
        (&["-x"], "Unknown option \"-x\". Run 'orchbee --help'.\n"),
        (
            &["workbench"],
            "Unknown command \"workbench\". Run 'orchbee --help'.\n",
        ),
        (
            &["--help", "--version"],
            "'--help' is not valid in this position. Run 'orchbee --help'.\n",
        ),
        (
            &["status", "--bogus"],
            "Unknown option \"--bogus\". Run 'orchbee --help'.\n",
        ),
    ];
    for (arguments, stderr) in cases {
        let output = run(arguments.iter().copied(), "en_US.UTF-8")?;
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert_eq!(String::from_utf8(output.stderr)?, *stderr, "{arguments:?}");
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn rejects_non_unicode_arguments_at_the_process_boundary() -> Result<(), Box<dyn Error>> {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let output = run([OsString::from_vec(vec![0xff])], "zh_CN.UTF-8")?;
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr)?,
        "调用方式无效：参数必须为 Unicode，且总大小不得超过 64 KiB。请运行 ‘orchbee --help’。\n"
    );
    Ok(())
}

fn run(
    arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
    locale: &str,
) -> Result<std::process::Output, std::io::Error> {
    Command::new(env!("CARGO_BIN_EXE_orchbee"))
        .args(arguments)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("LANG")
        .env("LANG", locale)
        .output()
}

#[cfg(windows)]
#[test]
fn git_bash_completion_rejects_conflicts_before_and_after_the_verb() -> Result<(), Box<dyn Error>> {
    use std::{io::Write, path::Path, process::Stdio};
    let git = Command::new("git").arg("--exec-path").output()?;
    assert!(git.status.success());
    let exec_path = String::from_utf8(git.stdout)?;
    let bash = Path::new(exec_path.trim())
        .ancestors()
        .map(|path| path.join("bin/bash.exe"))
        .find(|path| path.is_file())
        .ok_or("Windows completion witness requires Git for Windows Bash")?;
    let completion = run(["completion", "bash"], "en_US.UTF-8")?;
    assert!(completion.status.success());
    assert!(completion.stderr.is_empty());
    let mut script = String::from_utf8(completion.stdout)?;
    script.push_str(r#"
check() {
 local expected="$1" present="$2"; shift 2
 COMP_WORDS=("$@"); COMP_CWORD=$((${#COMP_WORDS[@]}-1)); COMPREPLY=()
 _orchbee
 local found=0 candidate
 for candidate in "${COMPREPLY[@]}"; do [[ "$candidate" == "$expected" ]] && found=1; done
 if [[ "$found" != "$present" ]]; then printf '%s\n' "completion failed: $* -> ${COMPREPLY[*]} expected $expected = $present"; exit 1; fi
}
check --output 1 orchbee api schema --o
check --output 0 orchbee api snapshot --o
check schema 1 orchbee --output file.json api ''
check snapshot 0 orchbee --output file.json api ''
check --output 0 orchbee api schema --output file.json --o
check layout 1 orchbee panel ''
check --direction 1 orchbee panel focus --d
check up 1 orchbee panel focus --direction ''
check above 0 orchbee panel focus --direction ''
check focus 1 orchbee --direction down panel ''
check --direction 0 orchbee panel focus --direction down --d
check --direction 0 orchbee tab focus main --d
check up 1 orchbee panel neighbor --direction ''
check above 0 orchbee panel neighbor --direction ''
check --direction 0 orchbee panel layout --d
check --panel 0 orchbee panel edges --current --p
check set-split-ratio 1 orchbee layout ""
check down 1 orchbee panel resize --direction ""
check below 0 orchbee panel resize --direction ""
check --panel 0 orchbee panel resize source --p
check --panel 0 orchbee layout set-split-ratio --tab shell --p
check --tab 0 orchbee layout set-split-ratio --current --t
check --ratio 1 orchbee layout set-split-ratio --tab shell --r
check --amount 1 orchbee panel resize --a
check --format 0 orchbee panel neighbor --current abc ''
check neighbor 1 orchbee --direction up panel ''
check layout 0 orchbee --direction up panel ''
check list 1 orchbee client ''
check get 1 orchbee help client ''
check --client 1 orchbee tab focus shell --c
check --toggle 1 orchbee panel zoom --t
check --off 0 orchbee panel zoom --on --o
check --format 0 orchbee panel zoom --on --toggle ''
check --format 0 orchbee panel --on zoom --toggle ''
check --format 0 orchbee --on panel --toggle zoom ''
check panel 0 orchbee --on --off ''
check zoom 0 orchbee panel --on --toggle ''
check zoom 1 orchbee panel --on ''
check --panel 0 orchbee panel focus 00000000000000000000000000000001 --p
check --client 1 orchbee panel focus --current --c
check --format 0 orchbee panel focus --current 00000000000000000000000000000001 ''
printf '%s\n' completion-ok
"#);
    let mut child = Command::new(bash)
        .args(["--noprofile", "--norc", "-s"])
        .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("completion stdin")?
        .write_all(script.as_bytes())?;
    let result = child.wait_with_output()?;
    assert!(
        result.status.success()
            && result.stderr.is_empty()
            && String::from_utf8_lossy(&result.stdout).contains("completion-ok"),
        "{} {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(())
}
