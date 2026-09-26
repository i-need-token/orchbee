use super::*;

#[test]
fn configuration_is_strict_bounded_and_does_not_echo_unknown_input() {
    let file = Path::new(r"C:\config\config.toml");
    for (text, reason) in [
        ("secret = 'DO-NOT-ECHO'", "unknown-key"),
        ("[terminal]\nunknown = 'DO-NOT-ECHO'", "unknown-key"),
        ("terminal = 2", "wrong-type"),
        ("[terminal]\nnew_cwd = 5", "wrong-type"),
        (
            "[terminal]\nnew_cwd = 'home'\nnew_cwd = 'follow'",
            "invalid-toml",
        ),
        ("[worktrees]\ndirectory = ''", "invalid-path"),
        ("[worktrees]\ndirectory = 'C:relative'", "invalid-path"),
    ] {
        let error = parse(text, file, &|_| None).unwrap_err();
        assert_eq!(error.reason, reason);
        assert!(!format!("{error:?}").contains("DO-NOT-ECHO"));
    }
}

#[test]
fn configuration_paths_use_file_parent_and_explicit_creation_wins() {
    let settings = parse(
        "[terminal]\nnew_cwd = '../项目'\n[worktrees]\ndirectory = '~/worktrees'",
        Path::new(r"C:\config\config.toml"),
        &|name| (name == "USERPROFILE").then(|| r"C:\home".into()),
    )
    .unwrap();
    assert_eq!(
        Path::new(settings.cwd.as_ref().unwrap().1.as_ref().unwrap().as_str()),
        Path::new(r"C:\config\..\项目")
    );
    assert_eq!(
        Path::new(settings.worktree_root.as_ref().unwrap().as_str()),
        Path::new(r"C:\home\worktrees")
    );
    let mut policy = CreationDirectory::Default;
    let mut options = LaunchOptions::default();
    settings.apply_directory(&mut policy, &mut options);
    assert_eq!(policy, CreationDirectory::Explicit);
    assert_eq!(options.cwd, settings.cwd.as_ref().unwrap().1);
    policy = CreationDirectory::Current;
    options.cwd = None;
    settings.apply_directory(&mut policy, &mut options);
    assert_eq!(policy, CreationDirectory::Current);
    assert_eq!(options.cwd, None);
}

#[test]
fn missing_location_keeps_defaults_but_invalid_explicit_location_does_not() {
    assert!(read(|_| None).unwrap().path.is_none());
    for value in ["", "relative", "C:relative", r"\\server\share\config.toml"] {
        assert_eq!(
            location(&|name| (name == "ORCHBEE_CONFIG_FILE").then(|| value.into()))
                .unwrap_err()
                .field,
            "ORCHBEE_CONFIG_FILE"
        );
    }
    let settings = parse(
        "[terminal]\nnew_cwd = 'home'",
        Path::new(r"C:\config.toml"),
        &|_| None,
    )
    .unwrap();
    assert_eq!(settings.cwd, Some((CreationDirectory::Home, None)));
}
