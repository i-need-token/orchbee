use std::{error::Error, ffi::OsStr, process::Command};

const EN_US_HELP: &str = "Orchbee\n\nUsage:\n  orchbee\n  orchbee --help\n  orchbee --version\n\nModes:\n  orchbee            Open the local terminal (Linux development cohort)\n  orchbee --help     Show this help\n  orchbee --version  Show the version\n";
const ZH_CN_HELP: &str = "Orchbee\n\n用法：\n  orchbee\n  orchbee --help\n  orchbee --version\n\n模式：\n  orchbee            打开本地终端（Linux 开发 cohort）\n  orchbee --help     显示此帮助\n  orchbee --version  显示版本\n";

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
    let english = run(std::iter::empty::<&str>(), "en_US.UTF-8")?;
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

    let chinese = run(std::iter::empty::<&str>(), "zh_CN.UTF-8")?;
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
    Ok(())
}

#[test]
fn rejects_unknown_short_and_additional_arguments_before_output() -> Result<(), Box<dyn Error>> {
    for arguments in [vec!["-h"], vec!["workbench"], vec!["--help", "--version"]] {
        let output = run(arguments, "en_US.UTF-8")?;
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr)?,
            "Invalid invocation. Run 'orchbee --help'.\n"
        );
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
        "调用方式无效。请运行 ‘orchbee --help’。\n"
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
