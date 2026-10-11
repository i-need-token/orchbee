use super::*;
use std::ffi::OsString;
fn parse(words: &[&str]) -> Result<Request, ParseFailure> {
    super::super::parse_request(words.iter().map(OsString::from))
}
#[test]
fn commands_parse_before_runtime_and_selection_is_all_or_none() {
    assert!(matches!(
        parse(&["command", "list", "--format=json"]).unwrap(),
        Request::ConfiguredCommand {
            command: Command::List,
            ..
        }
    ));
    let Request::ConfiguredCommand {
        command: Command::Invoke(invocation),
        ..
    } = parse(&[
        "command",
        "invoke",
        "cmd_profile_0",
        "--anchor=0:1",
        "--cursor=1:3",
        "--content-revision=18446744073709551615",
    ])
    .unwrap()
    else {
        panic!()
    };
    assert_eq!(invocation.selection.unwrap().revision.value(), u64::MAX);
    for args in [
        vec!["command", "invoke", "echo text"],
        vec!["command", "invoke", "cmd_profile_0", "--anchor=0:0"],
        vec![
            "command",
            "invoke",
            "cmd_profile_0",
            "--anchor=0:0",
            "--cursor=4096:0",
            "--content-revision=0",
        ],
        vec![
            "command",
            "list",
            "--client=11111111111111111111111111111111",
        ],
        vec!["command", "invoke", "cmd_profile_0", "--yes"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
}
