use super::*;
fn parse(words: &[&str]) -> Result<Request, ParseFailure> {
    super::super::parse_request(words.iter().map(std::ffi::OsString::from))
}
#[test]
fn graphics_grammar_is_closed_before_any_source_or_site_effect() {
    for verb in ["set", "clear", "info", "stream"] {
        let mut args = vec!["panel", "graphics", verb, "@panel:1", "--format=json"];
        if verb == "set" {
            args.push("--spec=@C:/image.png");
        }
        assert!(matches!(
            parse(&args).unwrap(),
            Request::PaneGraphics { .. }
        ));
        args.push("--exec=C:/other.exe");
        assert!(parse(&args).is_err());
    }
    for args in [
        vec!["panel", "graphics", "set", "@panel:1"],
        vec![
            "panel",
            "graphics",
            "clear",
            "@panel:1",
            "--spec={\"zIndex\":0}",
        ],
        vec!["panel", "graphics", "info", "@panel:1", "--spec={}"],
        vec![
            "panel",
            "graphics",
            "stream",
            "@panel:1",
            "--spec={\"layerId\":\"../foreign\"}",
        ],
        vec![
            "panel",
            "graphics",
            "stream",
            "@panel:1",
            "--spec={\"zIndex\":0,\"zIndex\":1}",
        ],
        vec!["panel", "graphics", "set", "@panel:1", "--spec=@"],
        vec!["panel", "graphics", "clear", "@panel:1", "--timeout=0"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
}
