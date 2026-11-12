use super::*;
fn request(args: &[&str]) -> Result<Request, ParseFailure> {
    crate::invocation::parse_request(args.iter().map(std::ffi::OsString::from))
}
#[test]
fn popup_sizes_and_close_options_are_typed_and_scoped() {
    let Request::Plugin {
        command: super::super::PluginCommand::PaneOpen(open),
        ..
    } = request(&[
        "plugin",
        "panel",
        "open",
        "--plugin",
        "tools",
        "--entrypoint",
        "dashboard",
        "--placement",
        "popup",
        "--width",
        "75%",
        "--height",
        "0",
    ])
    .unwrap()
    else {
        panic!()
    };
    assert_eq!(
        open.width,
        Some(orchbee_workbench_protocol::PluginPopupSize::Percent(75))
    );
    assert_eq!(
        open.height,
        Some(orchbee_workbench_protocol::PluginPopupSize::Cells(0))
    );
    assert_eq!(open.placement, Some(PluginPanePlacement::Popup));
    for size in ["0%", "101%", "65536", "-1", "1.5%", "x"] {
        assert!(
            request(&[
                "plugin",
                "panel",
                "open",
                "--plugin",
                "tools",
                "--entrypoint",
                "dashboard",
                "--width",
                size
            ])
            .is_err()
        );
    }
    assert!(matches!(
        request(&["popup", "close", "--timeout", "100"]),
        Ok(Request::Plugin {
            command: super::super::PluginCommand::PopupClose {
                timeout_ms: 100,
                ..
            },
            ..
        })
    ));
    for args in [
        &["popup", "close", "--width", "20"][..],
        &["plugin", "list", "--height", "4"][..],
        &["popup", "close", "unexpected"][..],
    ] {
        assert!(request(args).is_err(), "{args:?}");
    }
}
#[test]
fn pane_open_defaults_and_scoped_launch_overrides() {
    let Request::Plugin {
        command: super::super::PluginCommand::PaneOpen(open),
        ..
    } = request(&[
        "plugin",
        "panel",
        "open",
        "--plugin",
        "tools",
        "--entrypoint",
        "dashboard",
        "--placement",
        "split",
        "--direction",
        "down",
        "--no-focus",
        "--env",
        "USER_INPUT=hello world",
    ])
    .unwrap()
    else {
        panic!()
    };
    assert!(!open.creation.focus);
    assert_eq!(
        open.creation.options.environment.entries(),
        &[("USER_INPUT".into(), "hello world".into())]
    );
    assert!(matches!(
        open.creation.kind,
        CreationKind::Split {
            direction: WorkbenchPanelSplitDirection::Below,
            ..
        }
    ));
    let base = [
        "plugin",
        "panel",
        "open",
        "--plugin",
        "tools",
        "--entrypoint",
        "dashboard",
    ];
    for invalid in [
        vec!["--exec", "C:/fake.exe"],
        vec!["--env", "ORCHBEE_PLUGIN_ID=wrong"],
        vec!["--focus", "--no-focus"],
        vec!["--direction", "above"],
        vec!["--placement", "floating"],
        vec!["unexpected"],
    ] {
        let mut args = base.to_vec();
        args.extend(invalid);
        assert!(request(&args).is_err(), "{args:?}");
    }
    assert!(request(&["plugin", "panel", "open", "--plugin", "tools"]).is_err());
    assert!(
        request(&[
            "panel",
            "split",
            "--target-panel",
            "00000000000000000000000000000001"
        ])
        .is_err()
    );
}
