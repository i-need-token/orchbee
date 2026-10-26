use super::*;

fn resolve(value: &str, variables: &[(&str, &str)]) -> Result<WorkbenchDirectoryPath, PathFailure> {
    normalize(
        value,
        |key| {
            variables
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| OsString::from(value))
        },
        || Ok(PathBuf::from(r"C:\caller\repo")),
    )
}

#[test]
fn local_operands_preserve_caller_and_home_ownership() {
    // Relative operands belong to the caller; tilde expansion belongs to its user home:
    // common supported Windows spellings; malformed context refusal is an explicit stricter boundary.
    for (input, expected) in [
        (r"C:\literal $HOME %VAR%", r"C:\literal $HOME %VAR%"),
        ("C:/literal/../path", "C:/literal/../path"),
        (r"\\?\C:\literal", r"\\?\C:\literal"),
        ("relative", r"C:\caller\repo\relative"),
        (r"..\trees\new", r"C:\caller\trees\new"),
        ("~", r"D:\User space"),
        (r"~\trees\新", r"D:\User space\trees\新"),
        ("~//trees///new", r"D:\User space\trees\new"),
        ("~/../new", r"D:\new"),
        ("$HOME", r"C:\caller\repo\$HOME"),
        ("%USERPROFILE%", r"C:\caller\repo\%USERPROFILE%"),
    ] {
        assert_eq!(
            resolve(input, &[("USERPROFILE", r"D:\User space")])
                .unwrap()
                .as_str(),
            expected
        );
    }
    assert_eq!(
        resolve("~", &[("HOMEDRIVE", "E:"), ("HOMEPATH", r"\user")])
            .unwrap()
            .as_str(),
        r"E:\user"
    );
    assert_eq!(
        resolve("~/new", &[("USERPROFILE", ""), ("HOME", r"F:\home")])
            .unwrap()
            .as_str(),
        r"F:\home\new"
    );
}

#[test]
fn absolute_and_relative_operands_never_consult_home_and_invalid_home_never_falls_back() {
    normalize(
        r"C:\does-not-need-to-exist",
        |_| panic!("unused home"),
        || panic!("unused cwd"),
    )
    .unwrap();
    normalize(
        "new",
        |_| panic!("unused home"),
        || Ok(PathBuf::from(r"C:\base")),
    )
    .unwrap();
    for invalid in ["relative", "~", r"\\server\share", "C:relative"] {
        assert_eq!(
            resolve("~", &[("USERPROFILE", invalid), ("HOME", r"C:\valid")]),
            Err(PathFailure::Context("USERPROFILE"))
        );
    }
    assert_eq!(
        resolve("~", &[("HOMEDRIVE", "C:")]),
        Err(PathFailure::Context("HOMEPATH"))
    );
    assert_eq!(
        resolve("~", &[("HOMEDRIVE", "C:"), ("HOMEPATH", "relative")]),
        Err(PathFailure::Context("HOMEPATH"))
    );
    assert_eq!(
        resolve("~", &[]),
        Err(PathFailure::Context("USERPROFILE/HOMEDRIVE/HOMEPATH/HOME"))
    );
}

#[test]
fn malformed_home_operands_refuse_without_acquiring_process_context() {
    for input in ["~/bad:part", r"~\bad:part", "~//C:/escape"] {
        assert_eq!(
            normalize(
                input,
                |_| panic!("invalid input must not read the environment"),
                || panic!("invalid input must not read the current directory"),
            ),
            Err(PathFailure::Input),
            "{input:?}"
        );
    }
}

#[test]
fn bounds_and_ambiguous_roots_refuse_before_runtime_or_path_probing() {
    for input in [
        "",
        "C:relative",
        r"\rooted",
        r"\\server\share",
        r"\\.\device",
        "~/C:/escape",
        "bad\0path",
    ] {
        assert_eq!(
            resolve(input, &[("USERPROFILE", r"C:\home")]),
            Err(PathFailure::Input),
            "{input:?}"
        );
    }
    let max = format!("C:/{}", "x".repeat(MAXIMUM_DIRECTORY_PATH_BYTES - 3));
    assert_eq!(resolve(&max, &[]).unwrap().as_str(), max);
    assert_eq!(resolve(&(max + "x"), &[]), Err(PathFailure::Input));
    assert_eq!(
        resolve(&"x".repeat(MAXIMUM_DIRECTORY_PATH_BYTES), &[]),
        Err(PathFailure::Input)
    );
    assert_eq!(
        normalize("new", |_| None, || Err(std::io::ErrorKind::NotFound.into())),
        Err(PathFailure::Context("current-directory"))
    );
    use std::os::windows::ffi::OsStringExt;
    assert_eq!(
        normalize(
            "~",
            |_| Some(OsString::from_wide(&[0xd800])),
            || panic!("unused cwd")
        ),
        Err(PathFailure::Context("USERPROFILE"))
    );
}
