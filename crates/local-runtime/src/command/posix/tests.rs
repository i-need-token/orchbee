use super::*;

#[test]
fn a_typed_line_refuses_control_characters() {
    assert!(posix_shell_word("a\nb").is_none());
    assert!(posix_command_line("printf", &["ok".into(), "a\tb".into()]).is_none());
    assert!(posix_command_line("x\u{1b}", &[]).is_none());
}

/// Each available shell runs the line and gets back exactly the words it was given: words that
/// some shell reads specially (blanks, quotes, expansions, globs, history, comments, braces, a
/// leading `=` or `~`), an empty word and non-ASCII text.
#[cfg(unix)]
#[test]
fn zsh_bash_and_sh_read_every_word_back_literally() {
    const AWKWARD: [&str; 22] = [
        "plain",
        "--model=gpt-5",
        "two words",
        "",
        "it's",
        "'",
        "\"double\"",
        "$HOME",
        "`id`",
        "$(id)",
        "back\\slash",
        "*.rs",
        "[ab]",
        "!!",
        "#comment",
        "{a,b}",
        "^x",
        "=ls",
        "~",
        "a;b|c&d<e>f",
        "(x)",
        "中文 ✓",
    ];
    let args: Vec<String> = std::iter::once("%s\\n".to_owned())
        .chain(AWKWARD.iter().map(|word| (*word).to_owned()))
        .collect();
    let line = posix_command_line("printf", &args).unwrap();
    let mut ran = 0;
    for (shell, options) in [
        ("/bin/zsh", &["-f", "-c"][..]),
        ("/bin/bash", &["--norc", "--noprofile", "-c"][..]),
        ("/bin/sh", &["-c"][..]),
    ] {
        if !std::path::Path::new(shell).exists() {
            continue;
        }
        let output = std::process::Command::new(shell)
            .args(options)
            .arg(&line)
            .output()
            .unwrap();
        assert!(output.status.success(), "{shell}: {line}");
        let printed = String::from_utf8(output.stdout).unwrap();
        let words: Vec<&str> = printed.split_terminator('\n').collect();
        assert_eq!(words, AWKWARD, "{shell}: {line}");
        ran += 1;
    }
    assert!(ran > 0);
}
