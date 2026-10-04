use super::*;
use crate::localization::Locale;
use crate::terminal_session::state::workspace_setup::quick::{QuickProject, QuickWorkspaceFields};
use orchbee_workbench_protocol::{WorkbenchDirectoryPath, WorkbenchProjectId};

#[test]
fn quick_workspace_project_ownership_uses_source_ids_and_requires_ambiguous_selection() {
    let cwd = WorkbenchDirectoryPath::parse("C:/项目 space").unwrap();
    let empty = projected(1, 1);
    let fields = QuickWorkspaceFields::new(&empty, cwd.clone(), Locale::EnUs);
    assert_eq!(fields.project(), &QuickProject::CreateDefault);
    assert_eq!(fields.name(), "项目 space");
    assert_eq!(fields.directory(), "C:/项目 space");
    let mut snapshot = workspace_setup::workspace_snapshot(1, 0);
    let project = snapshot.navigation.projects[0].project_id.clone();
    let standalone = project_native_standalone_tabs(&snapshot).unwrap();
    assert_eq!(
        QuickWorkspaceFields::new(&standalone, cwd.clone(), Locale::EnUs).project(),
        &QuickProject::Existing(project.clone())
    );
    let mut second = snapshot.navigation.projects[0].clone();
    second.project_id = WorkbenchProjectId::parse(&id(79)).unwrap();
    second.workspaces.clear();
    snapshot.navigation.projects.push(second);
    let standalone = project_native_standalone_tabs(&snapshot).unwrap();
    let mut fields = QuickWorkspaceFields::new(&standalone, cwd.clone(), Locale::EnUs);
    assert_eq!(fields.project(), &QuickProject::Choose);
    assert!(fields.plan().is_none());
    assert!(fields.select_project(&standalone.projects()[1].project_id, &standalone));
    assert_eq!(
        fields.plan().unwrap().project,
        QuickProject::Existing(standalone.projects()[1].project_id.clone())
    );
    let source = workspace_setup::workspace_state();
    assert_eq!(
        QuickWorkspaceFields::new(source.cache(), cwd, Locale::EnUs).project(),
        &QuickProject::Existing(project)
    );
}

#[test]
fn quick_workspace_fields_normalize_relative_and_home_paths_without_mutating_raw_text() {
    let mut fields = QuickWorkspaceFields::new(
        &projected(1, 1),
        WorkbenchDirectoryPath::parse("C:/项目 space").unwrap(),
        Locale::EnUs,
    );
    fields.directory_editor().clear();
    fields.directory_editor().insert("child").unwrap();
    assert_eq!(fields.directory(), "child");
    assert_eq!(fields.plan().unwrap().cwd.as_str(), "C:/项目 space\\child");
    fields.directory_editor().clear();
    fields.directory_editor().insert("~").unwrap();
    assert!(
        fields
            .plan()
            .unwrap()
            .cwd
            .as_str()
            .starts_with(std::env::var("USERPROFILE").unwrap().as_str())
    );
    assert_eq!(fields.directory(), "~");
    fields.directory_editor().clear();
    fields.directory_editor().insert("C:relative").unwrap();
    assert!(fields.plan().is_none());
    fields.directory_editor().clear();
    assert!(fields.plan().is_none());
}

#[test]
fn quick_workspace_name_and_directory_have_independent_atomic_limits() {
    let mut fields = QuickWorkspaceFields::new(
        &projected(1, 1),
        WorkbenchDirectoryPath::parse("C:/").unwrap(),
        Locale::ZhCn,
    );
    assert_eq!(fields.name(), "工作区");
    fields.name_editor().clear();
    fields.name_editor().insert(&"猫".repeat(512)).unwrap();
    assert!(fields.name_editor().insert("猫").is_err());
    assert_eq!(fields.name().chars().count(), 512);
    fields.directory_editor().clear();
    let path = format!("C:/{}x", "猫".repeat(1364));
    assert_eq!(path.len(), 4096);
    fields.directory_editor().insert(&path).unwrap();
    assert!(fields.directory_editor().insert("x").is_err());
    assert_eq!(fields.directory(), path);
    assert!(fields.plan().is_some());
    fields.name_editor().clear();
    assert!(fields.plan().is_none());
}
