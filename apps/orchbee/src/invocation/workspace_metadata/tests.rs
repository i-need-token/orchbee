use super::*;
fn parse_args(args: &[&str]) -> Result<Request, ParseFailure> {
    crate::invocation::parse_request(args.iter().map(std::ffi::OsString::from))
}
#[test]
fn metadata_normalization_order_and_counter_limits_precede_admission() {
    let Request::WorkspaceMetadata { command, format } = parse_args(&[
        "workspace",
        "report-metadata",
        "main",
        "--source= hook:agent ",
        "--token",
        "state=old",
        "--clear-token",
        "state",
        "--token=state= \n 新\t值 ",
        "--token=empty=   ",
        "--seq=18446744073709551615",
        "--ttl-ms=86400000",
        "--format=json",
    ])
    .unwrap() else {
        panic!("metadata command")
    };
    assert_eq!(format, OutputFormat::Json);
    assert_eq!(command.source.as_str(), "hook:agent");
    assert_eq!(
        command.tokens.as_map()[&MetadataTokenKey::parse("state").unwrap()]
            .as_ref()
            .unwrap()
            .as_str(),
        "新值"
    );
    assert!(command.tokens.as_map()[&MetadataTokenKey::parse("empty").unwrap()].is_none());
    assert_eq!(command.sequence, Some(WorkbenchRevision::new(u64::MAX)));
    for tail in [
        vec!["--seq=18446744073709551616"],
        vec!["--seq=-1"],
        vec!["--seq=1", "--seq=2"],
        vec!["--ttl-ms=0"],
        vec!["--ttl-ms=86400001"],
        vec!["--token=bad.key=value"],
        vec!["--token=missing-equals"],
        vec!["--clear-token=bad.key"],
        vec!["--focus"],
        vec!["--timeout=1"],
    ] {
        let args = [
            vec![
                "workspace",
                "report-metadata",
                "main",
                "--source=hook",
                "--token=a=b",
            ],
            tail,
        ]
        .concat();
        assert!(parse_args(&args).is_err(), "{args:?}");
    }
    for args in [
        vec!["workspace", "report-metadata", "main", "--token=a=b"],
        vec!["workspace", "report-metadata", "main", "--source=hook"],
        vec!["workspace", "list", "--token=a=b"],
        vec!["--seq=1", "--help"],
    ] {
        assert!(parse_args(&args).is_err(), "{args:?}");
    }
    let mut args = vec![
        "workspace".to_owned(),
        "report-metadata".into(),
        "main".into(),
        "--source=hook".into(),
    ];
    args.extend((0..17).map(|n| format!("--token=k{n}=v")));
    assert!(parse_args(&args.iter().map(String::as_str).collect::<Vec<_>>()).is_err());
}
