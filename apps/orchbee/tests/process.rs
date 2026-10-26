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

#[cfg(windows)]
#[test]
fn default_config_precedes_configuration_discovery() -> Result<(), Box<dyn Error>> {
    let invoke = |arguments: &[&str], locale: &str| {
        Command::new(env!("CARGO_BIN_EXE_orchbee"))
            .args(arguments)
            .env("LC_ALL", locale)
            .env("APPDATA", "relative-appdata")
            .env("LOCALAPPDATA", "relative-localappdata")
            .env("ORCHBEE_CONFIG_FILE", "relative-config.toml")
            .env("ORCHBEE_RUNTIME_BASE", "relative-runtime")
            .env("ORCHBEE_RUNTIME_SCOPE", "not-a-scope")
            .env("ORCHBEE_SESSION", "..")
            .output()
    };

    let english = invoke(&["--default-config"], "en_US.UTF-8")?;
    let chinese = invoke(&["--default-config"], "zh_CN.UTF-8")?;
    assert!(english.status.success());
    assert!(chinese.status.success());
    assert!(english.stderr.is_empty());
    assert!(chinese.stderr.is_empty());
    assert_eq!(english.stdout, chinese.stdout);
    let text = String::from_utf8(english.stdout)?;
    assert!(text.ends_with('\n'));
    let parsed: toml::Value = toml::from_str(&text)?;
    assert_eq!(parsed["keys"]["prefix"].as_str(), Some("ctrl+b"));
    for action in orchbee_local_runtime::KeySettings::registered_actions() {
        let prefix = format!("# {action} = ");
        assert!(
            text.lines().any(|line| line.starts_with(prefix.as_str())),
            "default action {action} is missing"
        );
    }
    assert!(text.contains("# resize_pane_left = []"));
    assert!(text.contains("# focus_agent = []"));

    for arguments in [
        &["--default-config", "--format=json"][..],
        &["--default-config", "extra"][..],
        &["--session", "work", "--default-config"][..],
    ] {
        let output = invoke(arguments, "en_US.UTF-8")?;
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
    }
    Ok(())
}

#[test]
fn help_is_localized_plain_text_and_precedes_runtime_discovery() -> Result<(), Box<dyn Error>> {
    for (locale, headings) in [
        (
            "en_US.UTF-8",
            [
                "Usage:",
                "Quick start:",
                "Terminals and layout:",
                "Global options:",
            ],
        ),
        (
            "zh_CN.UTF-8",
            ["用法:", "快速开始:", "终端与布局:", "全局选项:"],
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_orchbee"))
            .arg("--help")
            .env("LC_ALL", locale)
            .env("COLUMNS", "not-a-number")
            .env("ORCHBEE_CONFIG_FILE", "relative-config.toml")
            .env("ORCHBEE_RUNTIME_BASE", "relative-runtime")
            .env("ORCHBEE_SESSION", "..")
            .output()?;
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout)?;
        assert!(text.starts_with("Orchbee\n"));
        assert!(!text.contains('\x1b'));
        assert!(!text.contains('\t'));
        assert!(text.ends_with('\n'));
        let positions = headings.map(|heading| text.find(heading).unwrap());
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
        for entry in [
            "orchbee workspace create",
            "orchbee agent list",
            "--help",
            "--skill",
            "--version",
            "orchbee <command> -h",
        ] {
            assert!(text.contains(entry), "{locale}: {entry}");
        }
    }
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

#[cfg(target_os = "macos")]
#[test]
fn macos_agent_list_reaches_session_admission_instead_of_platform_stub()
-> Result<(), Box<dyn Error>> {
    // The unadorned Cargo binary has no cohort sidecar, so the read must stop
    // at that real admission check rather than report unsupported-platform.
    let output = Command::new(env!("CARGO_BIN_EXE_orchbee"))
        .args(["agent", "list", "--format=json"])
        .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
        .output()?;
    assert_eq!(output.status.code(), Some(1));
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(document["failure"]["kind"], "check-failed");
    assert_eq!(document["failure"]["details"]["stage"], "cohort");
    assert!(!output.stderr.is_empty());
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
    let text = String::from_utf8(output.stdout)?;
    assert!(text.starts_with("orchbee tui\n"));
    for entry in [
        "Usage:",
        "Options:",
        if cfg!(windows) {
            "Examples (PowerShell):"
        } else {
            "Examples (POSIX shell):"
        },
        "--mouse <on|off>",
        "default: on",
        "orchbee tui --mouse on",
    ] {
        assert!(text.contains(entry), "{entry}");
    }
    assert!(output.stderr.is_empty());
    Ok(())
}

#[test]
fn help_flag_aliases_match_the_help_command_at_every_recognized_path() -> Result<(), Box<dyn Error>>
{
    // `<path> --help` and `<path> -h` emit the identical document `help <path>`
    // emits, at the root and at every family and complete command path — all
    // without touching a running Session.
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
        &["server"],
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
        &["server", "start"],
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
fn help_overrides_business_operands_and_options_before_runtime_discovery()
-> Result<(), Box<dyn Error>> {
    for (arguments, topic) in [
        (&["--help", "status"][..], &["status"][..]),
        (&["-h", "status"][..], &["status"][..]),
        (
            &["project", "get", "x", "--help"][..],
            &["project", "get"][..],
        ),
        (&["status", "extra", "--help"][..], &["status"][..]),
        (
            &["status", "--help", "--format", "text"][..],
            &["status"][..],
        ),
        (
            &["workspace", "get", "work", "--format", "json", "-h"][..],
            &["workspace", "get"][..],
        ),
    ] {
        let canonical = run(topic.iter().copied().chain(["--help"]), "en_US.UTF-8")?;
        assert_eq!(canonical.status.code(), Some(0));
        let output = Command::new(env!("CARGO_BIN_EXE_orchbee"))
            .args(arguments)
            .env("LC_ALL", "en_US.UTF-8")
            .env("ORCHBEE_CONFIG_FILE", "relative-config.toml")
            .env("ORCHBEE_RUNTIME_BASE", "relative-runtime")
            .env("ORCHBEE_SESSION", "..")
            .output()?;
        assert_eq!(output.status.code(), Some(0), "{arguments:?}");
        assert_eq!(output.stdout, canonical.stdout, "{arguments:?}");
        assert!(output.stderr.is_empty(), "{arguments:?}");
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
            "Invalid value \"--help\" for '--cols' (expected a decimal integer from 0 to 65535). Run 'orchbee tab create --help'.\n",
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
            "'--version' is not valid in this position. Run 'orchbee --help'.\n",
        ),
        (
            &["--help", "--help"],
            "'--help' is not valid in this position. Run 'orchbee --help'.\n",
        ),
        (
            &["-h", "-h"],
            "'-h' is not valid in this position. Run 'orchbee --help'.\n",
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
            &["config", "unsupported-import"],
            "Unknown command \"unsupported-import\" for 'orchbee config'. Run 'orchbee config --help'.\n",
        ),
        (
            &["--help", "--version"],
            "'--version' is not valid in this position. Run 'orchbee --help'.\n",
        ),
        (
            &["status", "--bogus"],
            "Unknown option \"--bogus\". Run 'orchbee status --help'.\n",
        ),
    ];
    for (arguments, stderr) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_orchbee"))
            .args(*arguments)
            .env("LC_ALL", "en_US.UTF-8")
            .env("ORCHBEE_CONFIG_FILE", "relative-config.toml")
            .env("ORCHBEE_RUNTIME_BASE", "relative-runtime")
            .output()?;
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
check --default-config 1 orchbee --d
check --format 0 orchbee --default-config --f
check --default-config 0 orchbee --session work --d
check --default-config 0 orchbee --machine work --d
check events 1 orchbee e
check events 1 orchbee --session audit e
check events 1 orchbee --format json e
check attach 1 orchbee --mouse off session a
check --mouse=off 1 orchbee session attach audit --mouse=
check off 1 orchbee session attach audit --mouse ''
check --help 1 orchbee session attach audit --mouse off --h
check --format 0 orchbee session attach audit --f
check attach 0 orchbee --format json session a
check --format 0 orchbee session attach audit --format json --f
check enable 0 orchbee agent integration --provider codex --provider gemini e
check --format 0 orchbee agent integration enable --provider codex --provider gemini --f
check doctor 1 orchbee --provider codex --provider gemini agent d
check --format 1 orchbee --session work workspace create --label example --f
check --help 1 orchbee workspace create --session work --label example --h
check --session 1 orchbee workspace create --s
check --session 0 orchbee --session work workspace create --s
check --format 0 orchbee --session work --session other workspace list --f
check --format 0 orchbee --session .. workspace list --f
check --format 0 orchbee --session work session attach other --f
check --format 1 orchbee update --f
check client 0 orchbee update c
check server 0 orchbee update s
check --spec 1 orchbee panel graphics set panel --s
check --client 1 orchbee panel graphics info panel --c
check --format 1 orchbee terminal title set title --f
check --format 1 orchbee agent view set --f
check --format 1 orchbee plugin action list --f
check --format 1 orchbee plugin log list --f
check --format 1 orchbee plugin pane open --f
check list 1 orchbee --machine work workspace l
check list 1 orchbee --machine=work workspace l
check --session 0 orchbee --machine work workspace list --s
check --machine 0 orchbee workspace --m
check attach 0 orchbee --machine work terminal a
check client 0 orchbee --machine work status c
check server 1 orchbee --machine work status s
check tui 0 orchbee --machine work tu
check --format 0 orchbee --machine work --session local workspace list --f
check --help 1 orchbee workspace get target --h
check report-metadata 1 orchbee workspace report-m
check workspace 1 orchbee --source=hook:agent work
check terminal 0 orchbee --source=hook:agent ter
check --token 1 orchbee workspace report-metadata target --source hook:agent --t
check --token 1 orchbee workspace report-metadata target --token k=v --t
check --token 0 orchbee workspace list --t
check visible 0 orchbee workspace report-metadata target --source ''
check attach 1 orchbee terminal a
check attach 1 orchbee agent a
check terminal 1 orchbee --mouse off ''
check agent 1 orchbee --mouse off ''
check status 0 orchbee --mouse off ''
check attach 1 orchbee --mouse off terminal ''
check session 0 orchbee --mouse off terminal ''
check attach 1 orchbee --mouse off agent ''
check integration 0 orchbee --mouse off agent ''
check attach 1 orchbee --mouse off --format=json terminal a
check --mouse=off 1 orchbee --takeover --mouse=
check --mouse 1 orchbee terminal attach target --m
check --mouse=off 1 orchbee terminal attach target --mouse=
check --mouse 0 orchbee terminal attach target --mouse off --m
check --format 0 orchbee terminal attach target --mouse invalid --f
check --format 0 orchbee terminal read target --mouse off --f
check --mouse 1 orchbee agent attach --name alpha --m
check --agent-session 0 orchbee agent attach --name alpha --s
check --name 0 orchbee agent attach alpha --n
check --format 0 orchbee agent attach alpha --name beta --f
check --format 0 orchbee agent attach --name alpha extra --f
check control 1 orchbee terminal session c
check --format 0 orchbee --takeover terminal session observe target --f
check --format 0 orchbee terminal session observe target --takeover --f
check --takeover 1 orchbee terminal session control target --ta
check --takeover 0 orchbee terminal session observe target --ta
check control 1 orchbee --takeover terminal session ''
check observe 0 orchbee --takeover terminal session ''
check --takeover 0 orchbee terminal session control target --takeover --ta
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
