use super::*;

#[test]
fn explicit_creation_wins_over_shared_file_policy() {
    let settings = project(WindowsUserSettings {
        cwd: Some((WindowsDirectoryPolicy::Explicit, Some(r"C:\项目".into()))),
        worktree_root: Some(r"C:\worktrees".into()),
        resume_agents_on_restore: false,
        resume_agents_on_restore_source: "file",
        ..WindowsUserSettings::default()
    })
    .unwrap();
    let mut directory = CreationDirectory::Default;
    let mut options = LaunchOptions::default();
    settings.apply_directory(&mut directory, &mut options);
    assert_eq!(directory, CreationDirectory::Explicit);
    assert_eq!(options.cwd.as_ref().unwrap().as_str(), r"C:\项目");
    directory = CreationDirectory::Current;
    options.cwd = None;
    settings.apply_directory(&mut directory, &mut options);
    assert_eq!(directory, CreationDirectory::Current);
    assert!(options.cwd.is_none());
    let details = settings.check_details();
    assert_eq!(details["resumeAgentsOnRestore"], false);
    assert_eq!(details["resumeAgentsOnRestoreSource"], "file");
    assert_eq!(details["worktreeDirectory"], r"C:\worktrees");
}

#[test]
fn shared_path_budget_matches_wire_and_cli_failure_preserves_field() {
    assert_eq!(
        orchbee_local_site_runtime::WINDOWS_USER_PATH_LIMIT,
        orchbee_workbench_protocol::MAXIMUM_DIRECTORY_PATH_BYTES
    );
    let error = Error::from(WindowsSettingsError::new(
        "session.resume_agents_on_restore",
        "wrong-type",
    ));
    assert_eq!(error.field, "session.resume_agents_on_restore");
    assert_eq!(error.reason, "wrong-type");
}
