use std::{error::Error, ffi::OsStr, process::Command};

const EN_US_HELP: &str = "Orchbee\n\nUsage:\n  orchbee\n  orchbee tui\n  orchbee status [--format text|json]\n  orchbee <family> <command> [operands] [--format text|json]\n  orchbee api snapshot|schema\n  orchbee config check [--format text|json]\n  orchbee help [<family>|<command>]\n  orchbee completion bash|zsh|fish\n  orchbee -h|--help\n  orchbee --version\n\nModes and queries:\n  orchbee              Open the local terminal (Linux development cohort)\n  orchbee tui          Open the same local terminal explicitly\n  orchbee status       Read the running local Site's Workbench snapshot\n  orchbee project      list | get\n  orchbee workspace    list | get\n  orchbee tab          list | get\n  orchbee panel        list | get\n  orchbee terminal     list | get | read\n  orchbee api          Machine snapshot and embedded schema documents\n\nMutations (one command per invocation, against the running Site):\n  orchbee project      create\n  orchbee workspace    create | rename\n  orchbee tab          create | rename | move | close\n  orchbee panel        split | move | close\n\nInspection and help:\n  orchbee config check Inspect local runtime/cohort/locale/socket inputs\n  orchbee help         Show help for a family or complete command path\n  orchbee completion   Emit a static shell completion script\n  orchbee -h|--help    Show this help; after a family or command path, show\n                       that path's help\n  orchbee --version    Show the version\n\nRun 'orchbee help <family>' for filters and target rules, or\n'orchbee help <family> <command>' for one mutation's full syntax.\n";
const ZH_CN_HELP: &str = "Orchbee\n\n用法：\n  orchbee\n  orchbee tui\n  orchbee status [--format text|json]\n  orchbee <family> <command> [操作数] [--format text|json]\n  orchbee api snapshot|schema\n  orchbee config check [--format text|json]\n  orchbee help [<family>|<command>]\n  orchbee completion bash|zsh|fish\n  orchbee -h|--help\n  orchbee --version\n\n模式与查询：\n  orchbee              打开本地终端（Linux 开发 cohort）\n  orchbee tui          显式打开同一本地终端\n  orchbee status       读取正在运行的本地 Site 的 Workbench 快照\n  orchbee project      list | get\n  orchbee workspace    list | get\n  orchbee tab          list | get\n  orchbee panel        list | get\n  orchbee terminal     list | get | read\n  orchbee api          机器快照与内嵌 schema 文档\n\n修改命令（每次调用执行一条命令，作用于运行中的 Site）：\n  orchbee project      create\n  orchbee workspace    create | rename\n  orchbee tab          create | rename | move | close\n  orchbee panel        split | move | close\n\n检查与帮助：\n  orchbee config check 检查本地运行时/cohort/locale/套接字输入\n  orchbee help         显示某个 family 或完整命令路径的帮助\n  orchbee completion   输出静态 shell 补全脚本\n  orchbee -h|--help    显示此帮助；跟在 family 或命令路径后时显示该路径的帮助\n  orchbee --version    显示版本\n\n运行 'orchbee help <family>' 查看过滤参数与 target 规则，或\n'orchbee help <family> <command>' 查看单条修改命令的完整语法。\n";

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
            if cfg!(target_os = "linux") {
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
            if cfg!(target_os = "linux") {
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
        "orchbee tui\n\nOpen the local terminal interface (Linux development cohort).\nA bare 'orchbee' invocation does the same.\n"
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
        &["api"],
        &["config"],
        &["completion"],
        &["project", "list"],
        &["project", "get"],
        &["project", "create"],
        &["workspace", "list"],
        &["workspace", "get"],
        &["workspace", "create"],
        &["workspace", "rename"],
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
