use super::*;

fn resolve(value: &str, home: Option<&str>, cwd: Option<&str>) -> Result<String, UserPathError> {
    normalize_posix_user_path(
        value,
        |name| (name == "HOME").then(|| home.map(OsString::from)).flatten(),
        || {
            cwd.map(PathBuf::from)
                .ok_or(std::io::ErrorKind::NotFound.into())
        },
    )
}

#[test]
fn posix_operands_resolve_lexically_from_the_supplied_context_only() {
    let home = Some("/Users/demo");
    let cwd = Some("/private/tmp/base");
    for (value, expected) in [
        ("/tmp//x/../y/", "/tmp//x/../y/"),
        ("~", "/Users/demo"),
        ("~/", "/Users/demo"),
        ("~//code/./a/../b", "/Users/demo/code/b"),
        ("child", "/private/tmp/base/child"),
        ("./a/b/..", "/private/tmp/base/a"),
        ("../../../..", "/"),
        ("~other", "/private/tmp/base/~other"),
        ("a\\b", "/private/tmp/base/a\\b"),
        ("项目 space", "/private/tmp/base/项目 space"),
    ] {
        assert_eq!(
            resolve(value, home, cwd).as_deref(),
            Ok(expected),
            "{value:?}"
        );
    }
    for value in ["", "C:/x", "c:", "\\x", "bad\0path"] {
        assert_eq!(
            resolve(value, home, cwd),
            Err(UserPathError::Input),
            "{value:?}"
        );
    }
    assert_eq!(
        resolve(&format!("/{}", "x".repeat(USER_PATH_LIMIT)), home, cwd),
        Err(UserPathError::Input)
    );
}

#[test]
fn missing_or_relative_context_names_the_value_that_could_not_resolve() {
    assert_eq!(resolve("/abs", None, None).as_deref(), Ok("/abs"));
    for home in [None, Some(""), Some("relative")] {
        assert_eq!(
            resolve("~/x", home, Some("/cwd")),
            Err(UserPathError::Context("HOME"))
        );
    }
    for cwd in [None, Some("relative")] {
        assert_eq!(
            resolve("x", Some("/home"), cwd),
            Err(UserPathError::Context("current-directory"))
        );
    }
}
