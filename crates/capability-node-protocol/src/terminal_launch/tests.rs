use super::*;

#[test]
fn windows_wire_paths_preserve_spelling_and_reject_ambiguous_roots() {
    for value in [
        r"C:\",
        r"c:/work/../工作",
        r"\\?\C:\work with spaces",
        "/work",
    ] {
        assert_eq!(AbsoluteCwd::new(value).unwrap().as_str(), value);
    }
    for value in [
        "",
        "relative",
        "C:",
        r"C:work",
        r"\work",
        r"\\server\share",
        r"\\.\pipe\x",
        r"\\?\UNC\server\share",
    ] {
        assert!(AbsoluteCwd::new(value).is_err(), "{value}");
    }
}
