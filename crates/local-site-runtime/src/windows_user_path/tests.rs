use super::*;

#[test]
fn syntax_admission_retains_raw_operands_without_execution_context() {
    for raw in [
        r"C:\literal $HOME %VAR%",
        r"\\?\C:\literal",
        "~",
        r"~\remote",
        "~//remote",
        r"..\remote",
        "中文",
    ] {
        assert_eq!(WindowsUserPathOperand::parse(raw).unwrap().as_str(), raw);
    }
    for raw in [
        "",
        "C:relative",
        r"\rooted",
        r"\\server\share",
        r"\\.\device",
        "~/C:/escape",
        "bad\0path",
    ] {
        assert_eq!(
            WindowsUserPathOperand::parse(raw),
            Err(WindowsUserPathError::Input),
            "{raw:?}"
        );
    }
    assert_eq!(
        WindowsUserPathOperand::parse(&"x".repeat(WINDOWS_USER_PATH_LIMIT + 1)),
        Err(WindowsUserPathError::Input)
    );
}

#[test]
fn resolution_acquires_only_the_context_declared_by_the_operand() {
    let absolute = WindowsUserPathOperand::parse(r"C:\literal\..\%USERPROFILE%").unwrap();
    assert_eq!(
        absolute
            .resolve(
                |_| panic!("absolute path needs no environment"),
                || panic!("absolute path needs no cwd")
            )
            .unwrap(),
        absolute.as_str()
    );

    let home = WindowsUserPathOperand::parse("~//remote").unwrap();
    let mut requested = Vec::new();
    let resolved = home
        .resolve(
            |name| {
                requested.push(name.to_owned());
                Some(OsString::from(r"D:\home"))
            },
            || panic!("home path needs no cwd"),
        )
        .unwrap();
    assert_eq!(requested, ["USERPROFILE"]);
    assert_eq!(resolved, r"D:\home\remote");

    let relative = WindowsUserPathOperand::parse(r"..\remote").unwrap();
    let mut cwd_reads = 0;
    let resolved = relative
        .resolve(
            |_| panic!("relative path needs no environment"),
            || {
                cwd_reads += 1;
                Ok(PathBuf::from(r"E:\caller"))
            },
        )
        .unwrap();
    assert_eq!(cwd_reads, 1);
    assert_eq!(resolved, r"E:\remote");
}

#[test]
fn resolved_relative_and_home_operands_are_normalized_lexically() {
    let cwd = || Ok(PathBuf::from(r"C:\work\proj-b"));
    let relative = |raw: &str| {
        WindowsUserPathOperand::parse(raw)
            .unwrap()
            .resolve(|_| panic!("no environment"), cwd)
            .unwrap()
    };
    assert_eq!(relative("."), r"C:\work\proj-b");
    assert_eq!(relative(r"proj-a\"), r"C:\work\proj-b\proj-a");
    assert_eq!(relative(r"..\proj-a\."), r"C:\work\proj-a");
    assert_eq!(relative("a/./b//"), r"C:\work\proj-b\a\b");
    assert_eq!(relative(r"..\..\..\.."), r"C:\");
    let home = WindowsUserPathOperand::parse(r"~\code\..\notes\")
        .unwrap()
        .resolve(|_| Some(OsString::from(r"D:\home")), || panic!("no cwd"))
        .unwrap();
    assert_eq!(home, r"D:\home\notes");
    // Absolute literals are never rewritten.
    let literal = WindowsUserPathOperand::parse(r"C:\work\.\x\").unwrap();
    assert_eq!(
        literal
            .resolve(|_| panic!("no environment"), || panic!("no cwd"))
            .unwrap(),
        r"C:\work\.\x\"
    );
}

#[test]
fn admitted_operands_bind_to_the_final_context_and_never_retain_an_earlier_home() {
    let home = WindowsUserPathOperand::parse(r"~\remote").unwrap();
    let relative = WindowsUserPathOperand::parse("remote").unwrap();
    for root in [r"C:\sender", r"D:\executor"] {
        let expected = format!(r"{root}\remote");
        assert_eq!(
            home.resolve(|_| Some(OsString::from(root)), || panic!("no cwd"))
                .unwrap(),
            expected
        );
        assert_eq!(
            relative
                .resolve(|_| panic!("no environment"), || Ok(PathBuf::from(root)))
                .unwrap(),
            expected
        );
    }
    let mut requested = Vec::new();
    assert_eq!(
        home.resolve(
            |name| {
                requested.push(name.to_owned());
                Some(OsString::from(if name == "USERPROFILE" {
                    "invalid-relative-home"
                } else {
                    r"C:\fallback"
                }))
            },
            || panic!("no cwd")
        ),
        Err(WindowsUserPathError::Context("USERPROFILE"))
    );
    assert_eq!(requested, ["USERPROFILE"]);
}

#[test]
fn syntactically_valid_operands_can_fail_only_when_execution_context_is_required() {
    let relative = WindowsUserPathOperand::parse("remote").unwrap();
    assert_eq!(
        relative.resolve(
            |_| panic!("no environment"),
            || Err(std::io::ErrorKind::NotFound.into())
        ),
        Err(WindowsUserPathError::Context("current-directory"))
    );
    let bounded = WindowsUserPathOperand::parse(&"x".repeat(WINDOWS_USER_PATH_LIMIT)).unwrap();
    assert_eq!(
        bounded.resolve(
            |_| panic!("no environment"),
            || Ok(PathBuf::from(r"C:\base"))
        ),
        Err(WindowsUserPathError::Input)
    );
}
